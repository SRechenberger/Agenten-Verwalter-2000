//! Kindprozesse der App; Plattformunterschiede beim Starten bleiben hier gekapselt.
//!
//! Jeder Kindprozess entsteht über `command` (das Pseudo-Terminal über `pty_command`), nie über
//! `Command::new` oder `CommandBuilder::new`; Clippy sperrt beide (`src-tauri/clippy.toml`).
//! Grund: Das AppImage setzt vor dem Start Variablen (`LD_LIBRARY_PATH`, `PATH`, `PYTHONHOME` …),
//! deren Einträge in das Bündel unter `$APPDIR` zeigen; den alten Wert hängt es hinten an. Erbt ein
//! Programm des Systems sie (`node`, `python3`, `curl`, `git`), lädt es zuerst die älteren
//! Bibliotheken des Bündels und bricht ab. Die Helfer nehmen darum jeden Eintrag unter `$APPDIR`
//! aus der Umgebung des Kindprozesses; ohne `APPDIR` (jede andere Plattform, jeder Lauf außerhalb
//! des AppImage) ändern sie nichts.

#[cfg(test)]
pub(crate) mod tests;

use std::env;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::process::Stdio;

use portable_pty::CommandBuilder;

/// Die Namen, die der Start des AppImage selbst setzt und die der Kern nie ändert: `APPDIR` ist die
/// Marke, an der er das AppImage erkennt; die anderen drei sind keine Suchpfade.
const LAUNCHER_MARKS: [&str; 4] = ["APPDIR", "APPIMAGE", "OWD", "ARGV0"];

/// Eine Änderung an der Umgebung eines Kindprozesses.
#[derive(Debug, PartialEq, Eq)]
pub enum EnvChange {
    /// Die Variable bekommt diesen Wert.
    Set(OsString, OsString),
    /// Die Variable entfällt.
    Remove(OsString),
}

/// Rein, ohne Zugriff auf die Umgebung: was an `vars` zu ändern ist, damit ein Kindprozess nichts
/// aus `app_dir` sieht. Ein Wert wird mit `split_paths` in Einträge zerlegt; ein Eintrag liegt im
/// Bündel, wenn sein Pfad mit `app_dir` beginnt (nach Pfadteilen: `/tmp/.mount_ab` ist nicht
/// `/tmp/.mount_abc/…`, der Ordner selbst zählt). Ohne solchen Eintrag bleibt die Variable
/// unberührt und steht nicht in der Liste. Bleibt nach dem Herausnehmen nur Leeres, entfällt sie,
/// sonst bekommt sie die übrigen Einträge in der alten Reihenfolge. `APPDIR`, `APPIMAGE`, `OWD`
/// und `ARGV0` ändert die Funktion nie, und ein leeres `app_dir` zählt als nicht gesetzt. Die
/// Reihenfolge der Änderungen ist die der Eingabe.
pub fn bundle_free_env(
    vars: impl IntoIterator<Item = (OsString, OsString)>,
    app_dir: &Path,
) -> Vec<EnvChange> {
    // Ein leerer Pfad ist Vorfahr jedes Pfads: Ohne diese Prüfung verschwänden alle Variablen.
    if app_dir.as_os_str().is_empty() {
        return Vec::new();
    }
    vars.into_iter()
        .filter(|(name, _): &(OsString, OsString)| {
            !LAUNCHER_MARKS
                .iter()
                .any(|mark: &&str| OsStr::new(mark) == name.as_os_str())
        })
        .filter_map(|(name, value): (OsString, OsString)| change_for(name, &value, app_dir))
        .collect()
}

/// Die Änderung für eine Variable, oder `None`, wenn sie nichts aus dem Bündel enthält oder sich
/// nicht neu zusammensetzen lässt (dann bleibt sie, wie sie ist).
fn change_for(name: OsString, value: &OsStr, app_dir: &Path) -> Option<EnvChange> {
    let (inside, rest): (Vec<PathBuf>, Vec<PathBuf>) =
        env::split_paths(value).partition(|entry: &PathBuf| entry.starts_with(app_dir));
    if inside.is_empty() {
        return None;
    }
    if rest
        .iter()
        .all(|entry: &PathBuf| entry.as_os_str().is_empty())
    {
        return Some(EnvChange::Remove(name));
    }
    env::join_paths(rest)
        .ok()
        .map(|joined: OsString| EnvChange::Set(name, joined))
}

/// `bundle_free_env` für ein Programm. Ohne `app_dir` (`APPDIR` fehlt) ändert sich nichts; liegt
/// das Programm selbst im Bündel (die eigene Exe in den Betriebsarten Autark und Codex, mit den
/// Bibliotheken des Bündels gebaut), bleibt die Umgebung ebenfalls, wie sie ist.
fn changes_for(
    program: &OsStr,
    app_dir: Option<&OsStr>,
    vars: impl IntoIterator<Item = (OsString, OsString)>,
) -> Vec<EnvChange> {
    match app_dir.map(Path::new) {
        Some(app_dir) if !Path::new(program).starts_with(app_dir) => bundle_free_env(vars, app_dir),
        _ => Vec::new(),
    }
}

/// Die Änderungen für `program` in der Umgebung des Verwalters.
fn current_changes(program: &OsStr) -> Vec<EnvChange> {
    changes_for(program, env::var_os("APPDIR").as_deref(), env::vars_os())
}

fn apply_to_command(command: &mut Command, changes: &[EnvChange]) {
    for change in changes {
        match change {
            EnvChange::Set(name, value) => {
                command.env(name, value);
            }
            EnvChange::Remove(name) => {
                command.env_remove(name);
            }
        }
    }
}

fn apply_to_pty(builder: &mut CommandBuilder, changes: &[EnvChange]) {
    for change in changes {
        match change {
            EnvChange::Set(name, value) => builder.env(name, value),
            EnvChange::Remove(name) => builder.env_remove(name),
        }
    }
}

/// `Command::new` mit bereinigter Umgebung. Liest `APPDIR` und die Umgebung des Verwalters und
/// wendet die Änderungen sofort an, nie mit `env_clear`: Spätere `.env(…)` des Aufrufers gewinnen,
/// und alles andere (der Schlüssel des MCP-Zugangs, die Variablen der Betriebsarten) erbt der
/// Kindprozess wie bisher. Der Helfer scheitert nie: Was sich nicht lesen oder zusammensetzen
/// lässt, bleibt stehen.
pub fn command(program: impl AsRef<OsStr>) -> Command {
    let program = program.as_ref();
    #[allow(
        clippy::disallowed_methods,
        reason = "Der Helfer ist die eine Stelle, an der ein Kindprozess entsteht"
    )]
    let mut command = Command::new(program);
    apply_to_command(&mut command, &current_changes(program));
    command
}

/// Dasselbe für das Pseudo-Terminal (`portable_pty::CommandBuilder`), mit `env` und `env_remove`.
pub fn pty_command(program: impl AsRef<OsStr>) -> CommandBuilder {
    let program = program.as_ref();
    #[allow(
        clippy::disallowed_methods,
        reason = "Der Helfer ist die eine Stelle, an der das Terminal entsteht"
    )]
    let mut builder = CommandBuilder::new(program);
    apply_to_pty(&mut builder, &current_changes(program));
    builder
}

/// Unter Unix startet der Kindprozess in einer eigenen Prozessgruppe, deren Nummer seine PID ist
/// (`process_group(0)`); was er startet, erbt sie, und `signal_group` beendet alles zusammen (ADR 074).
/// Für Agentenprozesse, nie für kurze Befehle wie `git`. Unter Windows ändert der Helfer nichts.
pub fn own_group(command: &mut Command) -> &mut Command {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    command
}

/// Das Signal an eine Prozessgruppe: `Terminate` lässt ihr Zeit zum Aufräumen, `Kill` nicht.
#[cfg(unix)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupSignal {
    Terminate,
    Kill,
}

/// Schickt der Prozessgruppe `pgid` ein Signal (`kill(-pgid, …)`). Eine Gruppe, die es nicht mehr
/// gibt (`ESRCH`), ist kein Fehler. `0`, `1` und die eigene Gruppe des Verwalters lehnt der Helfer
/// ab, ohne zu signalisieren (`group_target`).
///
/// Pflicht des Aufrufers: Der Anführer der Gruppe (der Prozess mit der PID `pgid`) ist noch nicht
/// eingesammelt (`wait`, `try_wait` mit Ergebnis). Danach kann die Nummer frei sein und zu einem
/// fremden Prozess gehören.
#[cfg(unix)]
pub fn signal_group(pgid: u32, signal: GroupSignal) -> std::io::Result<()> {
    let target = group_target(pgid)?;
    let number = match signal {
        GroupSignal::Terminate => libc::SIGTERM,
        GroupSignal::Kill => libc::SIGKILL,
    };
    // SAFETY: `kill` liest und schreibt keinen Speicher des Prozesses. `target` ist negativ und nie
    // `-1` oder die eigene Gruppe (`group_target`), trifft also genau die Gruppe `pgid`.
    if unsafe { libc::kill(target, number) } == 0 {
        return Ok(());
    }
    let error = std::io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ESRCH) {
        return Ok(());
    }
    Err(error)
}

/// Fängt SIGTERM für Übersetzer und eigenen Agenten, damit sie vor dem Ende ihre Kinder in eigenen
/// Gruppen abräumen (ADR 074): blockiert das Signal für diesen und jeden später gestarteten Thread
/// und ruft `handler` auf einem eigenen Thread, sobald es kommt (`sigwait`). Vor dem ersten
/// weiteren Thread aufrufen; ein Thread ohne Sperre nähme das Signal an, und der Prozess endet
/// ohne Aufräumen. Kindprozesse erben die Sperre nicht: `Command` setzt ihre Signalmaske zurück.
/// Nie im Testprozess aufrufen.
#[cfg(unix)]
pub fn on_terminate(handler: impl FnOnce() + Send + 'static) -> std::io::Result<()> {
    // SAFETY: `sigset_t` ist ein C-Struct ohne Zeiger; `sigemptyset` füllt es gleich danach.
    let mut set: libc::sigset_t = unsafe { std::mem::zeroed() };
    // SAFETY: `set` ist ein gültiger, beschreibbarer `sigset_t`, SIGTERM eine gültige Nummer.
    unsafe {
        libc::sigemptyset(&raw mut set);
        libc::sigaddset(&raw mut set, libc::SIGTERM);
    }
    let change_mask = |how: libc::c_int| -> std::io::Result<()> {
        // SAFETY: `set` ist gefüllt; die alte Maske wird nicht gebraucht (Nullzeiger erlaubt).
        match unsafe { libc::pthread_sigmask(how, &raw const set, std::ptr::null_mut()) } {
            0 => Ok(()),
            error => Err(std::io::Error::from_raw_os_error(error)),
        }
    };
    change_mask(libc::SIG_BLOCK)?;
    let spawned = std::thread::Builder::new()
        .name("sigterm".to_owned())
        .spawn(move || {
            let mut signal: libc::c_int = 0;
            // SAFETY: `set` und `signal` sind gültig und gehören diesem Thread; `sigwait` nimmt
            // nur SIGTERM an, das alle Threads des Prozesses gesperrt haben.
            while unsafe { libc::sigwait(&raw const set, &raw mut signal) } != 0 {}
            handler();
        });
    if let Err(error) = spawned {
        // Sonst bliebe SIGTERM gesperrt, und nur SIGKILL beendete den Prozess.
        let _ = change_mask(libc::SIG_UNBLOCK);
        return Err(error);
    }
    Ok(())
}

/// Beendet den Prozess `pid` samt Kindern. Unter Unix SIGKILL an seine Gruppe (er startete mit
/// `own_group`), unter Windows `taskkill /PID <pid> /T /F` für den ganzen Baum; Kinder, deren
/// Elternprozess schon beendet ist, erreicht `/T` nicht. Pflicht des Aufrufers wie bei
/// `signal_group`: Der Prozess ist noch nicht eingesammelt (ADR 074).
#[cfg(unix)]
pub fn kill_process_tree(pid: u32) {
    let _ = signal_group(pid, GroupSignal::Kill);
}

/// Siehe die Fassung für Unix.
#[cfg(not(unix))]
pub fn kill_process_tree(pid: u32) {
    let mut taskkill = command("taskkill");
    taskkill
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    hide_console(&mut taskkill);
    let _ = taskkill.status();
}

/// Das Ziel für `kill` zur Gruppe `pgid`, also `-pgid`. Abgelehnt: `0` (die eigene Gruppe), `1`
/// (`kill(-1, …)` träfe jeden Prozess des Benutzers), eine Nummer über `i32::MAX` und die eigene
/// Gruppe des Verwalters (`getpgrp()`).
#[cfg(unix)]
fn group_target(pgid: u32) -> std::io::Result<libc::pid_t> {
    let refuse = |reason: &str| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("Prozessgruppe {pgid} {reason}"),
        )
    };
    let Ok(pgid) = libc::pid_t::try_from(pgid) else {
        return Err(refuse("gibt es nicht"));
    };
    if pgid <= 1 {
        return Err(refuse("ist keine Gruppe eines Agenten"));
    }
    // SAFETY: `getpgrp` hat keine Vorbedingungen und kann nicht scheitern.
    if pgid == unsafe { libc::getpgrp() } {
        return Err(refuse("ist die Gruppe des Verwalters"));
    }
    Ok(-pgid)
}

/// Ob das Kind `pid` beendet ist, ohne es einzusammeln (`waitid` mit `WNOWAIT`): Solange es nicht
/// eingesammelt ist, gehört die Nummer seiner Gruppe noch ihm, und `signal_group` darf sie treffen.
/// Ein Fehler (etwa `ECHILD`: kein Kind des Verwalters) kommt zurück.
#[cfg(unix)]
pub fn has_exited(pid: u32) -> std::io::Result<bool> {
    let pid = libc::id_t::from(pid);
    // SAFETY: `siginfo_t` ist ein C-Struct ohne Zeiger; Nullen sind ein gültiger Wert.
    let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
    // SAFETY: `info` ist ein gültiger, beschreibbarer `siginfo_t`. `WNOHANG` kehrt sofort zurück,
    // `WNOWAIT` lässt das Kind uneingesammelt.
    let result = unsafe {
        libc::waitid(
            libc::P_PID,
            pid,
            &raw mut info,
            libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
        )
    };
    if result != 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: `waitid` hat `info` gefüllt oder gelassen, wie es war (genullt); `si_pid` liest nur.
    // Ohne beendetes Kind bleibt `si_pid` 0 (der portable Weg laut `waitid(2)`).
    Ok(unsafe { info.si_pid() } != 0)
}

/// Unterdrückt das Konsolenfenster, das Windows für jeden Kindprozess einer GUI-App öffnet.
pub fn hide_console(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    {
        let _ = command;
    }
}

/// Gegenstück zu `hide_console`: Unter Windows bekommt der Kindprozess ein eigenes sichtbares
/// Konsolenfenster (für Programme, die dem Nutzer etwas anzeigen oder von ihm lesen). Anderswo gibt
/// es kein Fenster, die Ein- und Ausgaben gehen ins Leere.
pub fn show_console(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
        command.creation_flags(CREATE_NEW_CONSOLE);
    }
    #[cfg(not(windows))]
    {
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
    }
}
