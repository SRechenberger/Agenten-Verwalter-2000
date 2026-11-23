//! Lauf einer Befehls-Station (ADR 073): Start durch ▶ oder die Automatik, Schritte im eigenen Thread,
//! Ende mit `complete_station_with_handoff` (ohne Meldung, ohne Wecken) oder Fehler (weckt den Master).
//! Ohne Session, ohne Modell und ohne Platz am Tor: `Countdowns` kennt diese Stationen nicht.
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};

use super::command::{self, Ended, PROGRESS_EVERY, REASON_ABORTED, TIME_LIMIT};
use super::master::Decider;
use super::model::{ImplementationView, PhaseView, StationKind, StationState, StationView};
use super::start::{Trigger, is_allowed, is_held};
use super::wake::{WakeEvent, WakeFacts};
use super::{announce, complete_station_with_handoff, find_station, now_ms, spawn_wake};
use crate::db::implementations::StationRow;
use crate::error::CommandError;
use crate::sessions::registry::SessionRegistry;

const NOT_A_COMMAND: &str = "Das ist keine Befehls-Station.";
const NOT_RUNNING: &str = "Die Station läuft nicht.";
const ALREADY_RUNNING: &str = "Die Station läuft schon oder ist fertig.";

// ponytail: ein Verzeichnis je Prozess der App statt eines Feldes der Registry; ein Neustart der App
// leert es und `interrupt_commands` räumt die Zeilen. Ändert sich das, wandert es in `Countdowns`.
fn running() -> &'static Mutex<HashMap<String, Arc<AtomicBool>>> {
    static RUNNING: OnceLock<Mutex<HashMap<String, Arc<AtomicBool>>>> = OnceLock::new();
    RUNNING.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lock_running() -> std::sync::MutexGuard<'static, HashMap<String, Arc<AtomicBool>>> {
    running()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Gesetzt, sobald die App endet: Ein Lauf, der danach endet, schreibt nichts mehr, die Station bleibt
/// „läuft“ und steht nach dem Neustart auf „unterbrochen“.
static SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);

/// Beendet jeden laufenden Schritt, wenn die App endet (neben den Terminals, ADR 073), und wartet bis zu
/// `wait`, bis die Läufe ihre Prozesse beendet haben. Ohne Lauf kehrt sie sofort zurück.
pub fn stop_all(wait: Duration) {
    SHUTTING_DOWN.store(true, Ordering::SeqCst);
    for stop in lock_running().values() {
        stop.store(true, Ordering::SeqCst);
    }
    let deadline = Instant::now() + wait;
    while !lock_running().is_empty() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(20));
    }
}

/// Trägt den Lauf ein, es sei denn, die App endet oder die Station läuft schon. Prüfen und Eintragen
/// geschehen unter einer Sperre: `stop_all` setzt das Zeichen vor der Sperre, also sieht ein späterer
/// Start es, und jeder frühere Lauf steht schon im Verzeichnis, wenn `stop_all` es liest.
fn register_run(station_id: &str) -> Result<Arc<AtomicBool>, CommandError> {
    let mut runs = lock_running();
    if SHUTTING_DOWN.load(Ordering::SeqCst) {
        return Err(CommandError::Implementation(
            "Die App wird beendet.".to_owned(),
        ));
    }
    if runs.contains_key(station_id) {
        return Err(CommandError::Implementation(ALREADY_RUNNING.to_owned()));
    }
    let stop = Arc::new(AtomicBool::new(false));
    runs.insert(station_id.to_owned(), Arc::clone(&stop));
    Ok(stop)
}

/// Ob die Station dieser Umsetzung eine Befehls-Station ist.
pub fn is_command_station(
    registry: &SessionRegistry,
    implementation_id: &str,
    station_id: &str,
) -> bool {
    registry
        .implementation_view(implementation_id)
        .is_ok_and(|view: ImplementationView| {
            find_station(&view, station_id).is_some_and(
                |(_, station): (&PhaseView, &StationView)| station.kind == StationKind::Command,
            )
        })
}

/// Beendet den laufenden Schritt (`implementation_stop_command`); die Station endet im Fehler
/// „abgebrochen“.
pub fn stop_command(
    app: &AppHandle,
    implementation_id: &str,
    station_id: &str,
) -> Result<(), CommandError> {
    let registry = app.state::<SessionRegistry>();
    if !is_command_station(&registry, implementation_id, station_id) {
        return Err(CommandError::Implementation(NOT_A_COMMAND.to_owned()));
    }
    match lock_running().get(station_id) {
        Some(stop) => {
            stop.store(true, Ordering::SeqCst);
            Ok(())
        }
        None => Err(CommandError::Implementation(NOT_RUNNING.to_owned())),
    }
}

/// Befehls-Stationen, die der Automatik jetzt zustehen: bereit, mit wirksamem „Automatisch starten“, ohne
/// Startfehler und an der Reihe. Ein Countdown gibt es nicht: Sie belegen keinen Platz am Tor.
pub fn due_commands(phases: &[PhaseView]) -> Vec<&StationView> {
    phases
        .iter()
        .flat_map(|phase: &PhaseView| phase.stations.iter())
        .filter(|station: &&StationView| {
            station.kind == StationKind::Command
                && station.state == StationState::Ready
                && station.starts_automatically
                && super::schedule::may_start(phases, &station.id)
        })
        .collect()
}

/// Startet die Befehls-Stationen, die der Umsetzung nach ihrem Stand zustehen (ein Durchgang der
/// Warteschlange ruft das nach jedem Ende). Fehler beim Start stehen als Startfehler an der Station.
pub fn start_due(app: &AppHandle, view: &ImplementationView) {
    if is_held(view).is_some() || view.stop_after_current || view.finished_at.is_some() {
        return;
    }
    for station in due_commands(&view.phases) {
        if let Err(error) = start_command(app, &view.id, &station.id, Trigger::Countdown) {
            eprintln!("Befehls-Station nicht gestartet: {error}");
        }
    }
}

/// Startet die Station: prüft die Liste, die Werte und den Ordner erneut (ein Fehler ist ein Startfehler
/// an der Station), beginnt den Lauf in der Datenbank und führt die Schritte im eigenen Thread aus.
pub(super) fn start_command(
    app: &AppHandle,
    implementation_id: &str,
    station_id: &str,
    trigger: Trigger,
) -> Result<(), CommandError> {
    let registry = app.state::<SessionRegistry>();
    let view = registry.implementation_view(implementation_id)?;
    if let Some(reason) = is_held(&view) {
        return Err(CommandError::Implementation(reason.to_owned()));
    }
    let (_, station) = super::station_in(&view, station_id)?;
    if station.kind != StationKind::Command {
        return Err(CommandError::Implementation(NOT_A_COMMAND.to_owned()));
    }
    if matches!(station.state, StationState::Running | StationState::Done) {
        return Err(CommandError::Implementation(ALREADY_RUNNING.to_owned()));
    }
    if !is_allowed(trigger, &view.phases, station_id) {
        return Err(CommandError::Implementation(
            "Die Vorgänger dieser Station sind noch nicht fertig.".to_owned(),
        ));
    }
    let rows = registry.station_rows(implementation_id)?;
    let (folder, repositories) = registry.command_roots(&view.project_id);
    let prepared = rows
        .iter()
        .find(|row: &&StationRow| row.id == station_id)
        .ok_or_else(|| "Die Station gibt es nicht.".to_owned())
        .and_then(|row: &StationRow| {
            prepare(
                row,
                &rows,
                &registry.command_programs(),
                folder.as_deref(),
                &repositories,
            )
        });
    let (steps, cwd) = match prepared {
        Ok(prepared) => prepared,
        Err(reason) => {
            registry.store_start_error(station_id, Some(&reason))?;
            announce(app, &registry.implementation_view(implementation_id)?);
            return Err(CommandError::Implementation(reason));
        }
    };
    let stop = register_run(station_id)?;
    match registry.begin_command(station_id) {
        Ok(true) => {}
        Ok(false) => {
            lock_running().remove(station_id);
            return Err(CommandError::Implementation(ALREADY_RUNNING.to_owned()));
        }
        Err(error) => {
            lock_running().remove(station_id);
            return Err(error);
        }
    }
    let thread_app = app.clone();
    let thread_implementation = implementation_id.to_owned();
    let thread_station = station_id.to_owned();
    let thread_stop = Arc::clone(&stop);
    let spawned = thread::Builder::new()
        .name("command-station".to_owned())
        .spawn(move || {
            drive(
                &thread_app,
                &thread_implementation,
                &thread_station,
                &steps,
                &cwd,
                TIME_LIMIT,
                &thread_stop,
            );
            lock_running().remove(&thread_station);
        });
    if let Err(error) = spawned {
        lock_running().remove(station_id);
        registry.fail_command(station_id, "Lauf nicht gestartet", "")?;
        return Err(CommandError::Implementation(format!(
            "Lauf nicht gestartet: {error}"
        )));
    }
    announce(app, &registry.implementation_view(implementation_id)?);
    Ok(())
}

/// Der Start einer Befehls-Station: Freigabeliste, Werte (auch `{{env:…}}`) und Ordner. Der Fehler ist der
/// Text für den Startfehler; er nennt Namen, nie Werte der Umgebung.
pub fn prepare(
    row: &StationRow,
    rows: &[StationRow],
    allowed: &[String],
    folder: Option<&Path>,
    repositories: &[std::path::PathBuf],
) -> Result<(Vec<Vec<String>>, std::path::PathBuf), String> {
    let default_cwd = folder.ok_or_else(|| "Der Ordner des Vorhabens ist unbekannt.".to_owned())?;
    let env = |name: &str| std::env::var(name).ok();
    let (steps, cwd) = command::prepare(row, rows, allowed, default_cwd, &env)?;
    if !command::cwd_allowed(&cwd.to_string_lossy(), Some(default_cwd), repositories) {
        return Err(command::CWD_REFUSED.to_owned());
    }
    if !cwd.is_dir() {
        return Err(format!("Der Ordner „{}“ gibt es nicht.", cwd.display()));
    }
    Ok((steps, cwd))
}

/// Der Lauf im Thread: Schritte, Fortschritt (höchstens alle 2 s an die Oberfläche), „Bei Fehler“ von
/// vorn, Ende. Nie unter einer Sperre.
fn drive(
    app: &AppHandle,
    implementation_id: &str,
    station_id: &str,
    steps: &[Vec<String>],
    cwd: &Path,
    limit: Duration,
    stop: &AtomicBool,
) {
    let registry = app.state::<SessionRegistry>();
    let restarts = registry
        .station_rows(implementation_id)
        .ok()
        .and_then(|rows: Vec<StationRow>| {
            rows.into_iter()
                .find(|row: &StationRow| row.id == station_id)
        })
        .map_or(0, |row: StationRow| row.restarts);
    let max_restarts = registry
        .implementation_view(implementation_id)
        .ok()
        .and_then(|view: ImplementationView| {
            find_station(&view, station_id)
                .map(|(_, station): (&PhaseView, &StationView)| station.on_error.max_restarts())
        })
        .unwrap_or(0);
    let mut last_output = String::new();
    let mut announced = Instant::now();
    let ended = command::run_restarting(
        steps,
        cwd,
        limit,
        stop,
        restarts,
        max_restarts,
        &mut |seen: u32| {
            // Ein Halt, das Ende oder eine Anfrage zum Abbrechen verbietet den Neustart.
            let halted = registry.implementation_view(implementation_id).is_ok_and(
                |view: ImplementationView| {
                    view.stop_after_current
                        || view.finished_at.is_some()
                        || view.pending_cancel.is_some()
                },
            );
            !halted && registry.restart_command(station_id, seen).unwrap_or(false)
        },
        &mut |step: usize, tail: &[String]| {
            last_output = tail.join("\n");
            let _ = registry.store_command_progress(
                station_id,
                u32::try_from(step).unwrap_or(u32::MAX),
                &last_output,
            );
            if registry
                .is_implementation_cancelled(implementation_id)
                .unwrap_or(false)
            {
                stop.store(true, Ordering::SeqCst);
            }
            if announced.elapsed() >= PROGRESS_EVERY {
                announced = Instant::now();
                if let Ok(view) = registry.implementation_view(implementation_id) {
                    announce(app, &view);
                }
            }
        },
    );
    if SHUTTING_DOWN.load(Ordering::SeqCst) {
        return;
    }
    match ended {
        Ended::Success { result } => {
            finish_success(app, implementation_id, station_id, &result);
        }
        Ended::Failed { reason } => {
            finish_failure(app, implementation_id, station_id, &reason, &last_output);
        }
    }
}

fn finish_success(app: &AppHandle, implementation_id: &str, station_id: &str, result: &str) {
    let registry = app.state::<SessionRegistry>();
    // Wie eine Annahme, ohne Meldung und ohne Wecken: Der Master hat nichts zu entscheiden.
    let completed = complete_station_with_handoff(
        app,
        implementation_id,
        station_id,
        result,
        false,
        &super::model::Handoff::default(),
        Decider::Master,
    );
    if let Err(error) = completed {
        finish_failure(app, implementation_id, station_id, &error.to_string(), "");
        return;
    }
    if let Ok(view) = registry.implementation_view(implementation_id) {
        announce(app, &view);
    }
}

/// Fehler: `error` mit Grund, die Oberfläche folgt, und der Master wird mit dem vorhandenen Text geweckt
/// („Fehler“, nach „Abbrechen“ „gestoppt“).
fn finish_failure(
    app: &AppHandle,
    implementation_id: &str,
    station_id: &str,
    reason: &str,
    output: &str,
) {
    let registry = app.state::<SessionRegistry>();
    let view = registry.implementation_view(implementation_id).ok();
    if !registry
        .fail_command(station_id, reason, output)
        .unwrap_or(false)
    {
        return;
    }
    if let Ok(fresh) = registry.implementation_view(implementation_id) {
        announce(app, &fresh);
    }
    let Some(view) = view else { return };
    let Some((_, station)) = find_station(&view, station_id) else {
        return;
    };
    let event = failure_event(station_id, station.returns, station.started_at, reason);
    let facts = WakeFacts {
        implementation: view.name.clone(),
        lane: station.plan_id.clone(),
        stage: station.stage.clone(),
        text: String::new(),
        note: String::new(),
    };
    spawn_wake(app, implementation_id, vec![event], facts);
}

/// Das Ereignis, das den Master weckt: der vorhandene Text „Fehler“, nach „Abbrechen“ „gestoppt“. Jeder
/// Lauf ist ein eigener Zug: sein Beginn in ms ist die Marke für „genau einmal“.
pub fn failure_event(
    station_id: &str,
    returns: u32,
    started_at: Option<f64>,
    reason: &str,
) -> WakeEvent {
    WakeEvent::Failed {
        station_id: station_id.to_owned(),
        returns,
        turn: started_at.unwrap_or_else(now_ms).to_bits(),
        stopped: reason == REASON_ABORTED,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Der Prozess des Tests selbst als langer Schritt (läuft unter jedem Betriebssystem): Nur mit
    /// `--ignored` schläft er; `cargo test` ohne das Flag überspringt ihn.
    #[test]
    #[ignore = "langer Schritt für stop_all, wird nur als Kindprozess gestartet"]
    fn a_long_step_for_the_test() {
        thread::sleep(Duration::from_secs(120));
    }

    /// Die Tests setzen `SHUTTING_DOWN` im selben Prozess; sie laufen hintereinander unter dieser Sperre.
    static SERIAL: Mutex<()> = Mutex::new(());

    #[test]
    fn no_run_is_registered_while_the_app_shuts_down() {
        let _serial = SERIAL
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        SHUTTING_DOWN.store(true, Ordering::SeqCst);
        let refused = register_run("register-refused-test");
        SHUTTING_DOWN.store(false, Ordering::SeqCst);
        assert!(refused.is_err());
        assert!(!lock_running().contains_key("register-refused-test"));
    }

    #[test]
    fn stop_all_finds_a_registered_run_and_stops_it() {
        let _serial = SERIAL
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        SHUTTING_DOWN.store(false, Ordering::SeqCst);
        let stop = register_run("register-found-test").expect("Eintrag");
        assert!(register_run("register-found-test").is_err());
        let remover = thread::spawn(|| {
            thread::sleep(Duration::from_millis(100));
            lock_running().remove("register-found-test");
        });
        stop_all(Duration::from_secs(10));
        SHUTTING_DOWN.store(false, Ordering::SeqCst);
        remover.join().expect("Thread");
        assert!(stop.load(Ordering::SeqCst), "stop_all fand den Lauf nicht");
    }

    #[test]
    fn stop_all_ends_a_running_step_before_the_app_ends() {
        let _serial = SERIAL
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let step = vec![
            std::env::current_exe()
                .expect("Pfad der Test-Exe")
                .to_string_lossy()
                .into_owned(),
            "--ignored".to_owned(),
            "--exact".to_owned(),
            "implementation::command_run::tests::a_long_step_for_the_test".to_owned(),
        ];
        let stop = Arc::new(AtomicBool::new(false));
        lock_running().insert("stop-all-test".to_owned(), Arc::clone(&stop));
        let worker_stop = Arc::clone(&stop);
        let started = Instant::now();
        let worker = thread::spawn(move || {
            let ended = command::run(
                &[step],
                &std::env::temp_dir(),
                TIME_LIMIT,
                &worker_stop,
                &mut |_: usize, _: &[String]| {},
            );
            lock_running().remove("stop-all-test");
            ended
        });
        thread::sleep(Duration::from_millis(500));
        stop_all(Duration::from_secs(10));
        SHUTTING_DOWN.store(false, Ordering::SeqCst);
        let emptied = lock_running().is_empty();
        // Räumt den Kindprozess auch dann ab, wenn `stop_all` versagt hat.
        stop.store(true, Ordering::SeqCst);
        let ended = worker.join().expect("Thread");
        assert!(emptied, "stop_all ließ den Lauf eingetragen");
        assert_eq!(
            ended,
            Ended::Failed {
                reason: REASON_ABORTED.to_owned()
            }
        );
        assert!(started.elapsed() < Duration::from_secs(30));
    }
}
