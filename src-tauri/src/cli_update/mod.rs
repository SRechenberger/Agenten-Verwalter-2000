//! Version der Claude-Kommandozeile: gelesen über `claude --version`, die jüngste Version aus der
//! npm-Registry, aktualisiert über `claude update`.
pub mod model;

use std::cmp::Ordering;
use std::io::Read;
use std::process::Stdio;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};

use crate::agents::claude::locate::find_claude;
use crate::processes::{self, hide_console};
use model::{CliUpdateOutcome, CliVersionStatus};

pub const CLI_UPDATE_CHANGED_EVENT: &str = "cli-update://changed";
const CLAUDE_NOT_FOUND: &str = "Claude-Kommandozeile nicht gefunden";
const LATEST_URL: &str = "https://registry.npmjs.org/@anthropic-ai/claude-code/latest";
const VERSION_TIMEOUT: Duration = Duration::from_secs(15);
const LATEST_TIMEOUT: Duration = Duration::from_secs(10);
/// Eine Aktualisierung lädt und installiert; fünf Minuten sind reichlich für eine langsame Leitung.
const UPDATE_TIMEOUT: Duration = Duration::from_secs(300);
const POLL_INTERVAL: Duration = Duration::from_millis(100);
const OUTPUT_TAIL_LINES: usize = 20;

/// Die Kommandozeile aktualisiert sich selbst (`claude update`); die App liest nur die Versionen und
/// stößt das an, statt selbst herunterzuladen.
pub struct CliUpdateService {
    state: Mutex<UpdateState>,
}

struct UpdateState {
    is_updating: bool,
    last_update: Option<CliUpdateOutcome>,
}

impl Default for CliUpdateService {
    fn default() -> Self {
        CliUpdateService::new()
    }
}

impl CliUpdateService {
    pub fn new() -> CliUpdateService {
        CliUpdateService {
            state: Mutex::new(UpdateState {
                is_updating: false,
                last_update: None,
            }),
        }
    }

    /// Liest installierte und jüngste Version neu, beide gleichzeitig; blockiert bis zu
    /// `VERSION_TIMEOUT`.
    pub fn load(&self) -> CliVersionStatus {
        let (installed, latest) = thread::scope(|scope| {
            let latest_handle = scope.spawn(read_latest);
            let installed = read_installed();
            let latest = latest_handle
                .join()
                .unwrap_or_else(|_| Err("Abfrage der jüngsten Version abgestürzt".to_owned()));
            (installed, latest)
        });
        let has_update = match (&installed, &latest) {
            (Ok(current), Ok(newest)) => compare_versions(newest, current) == Ordering::Greater,
            _ => false,
        };
        let state = self.lock();
        CliVersionStatus {
            installed: installed.clone().ok(),
            latest: latest.clone().ok(),
            has_update,
            is_updating: state.is_updating,
            last_update: state.last_update.clone(),
            installed_error: installed.err(),
            latest_error: latest.err(),
        }
    }

    /// Startet `claude update` und kehrt sofort zurück; Anfang und Ende meldet
    /// `cli-update://changed`. Nie zwei Aktualisierungen gleichzeitig.
    pub fn update(&self, app: &AppHandle) {
        {
            let mut state = self.lock();
            if state.is_updating {
                return;
            }
            state.is_updating = true;
        }
        emit_changed(app);
        let worker_app = app.clone();
        let spawned = thread::Builder::new()
            .name("cli-update".to_owned())
            .spawn(move || {
                let outcome = run_update();
                worker_app
                    .state::<CliUpdateService>()
                    .finish_update(&worker_app, outcome);
            });
        if let Err(error) = spawned {
            self.finish_update(
                app,
                CliUpdateOutcome {
                    succeeded: false,
                    output: format!("Aktualisierung startet nicht: {error}"),
                },
            );
        }
    }

    fn finish_update(&self, app: &AppHandle, outcome: CliUpdateOutcome) {
        {
            let mut state = self.lock();
            state.is_updating = false;
            state.last_update = Some(outcome);
        }
        emit_changed(app);
    }

    fn lock(&self) -> MutexGuard<'_, UpdateState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

struct Finished {
    succeeded: bool,
    output: String,
}

fn read_installed() -> Result<String, String> {
    let finished = run_claude(&["--version"], VERSION_TIMEOUT)?;
    parse_installed(&finished.output)
        .ok_or_else(|| "Antwort von `claude --version` unlesbar".to_owned())
}

/// `2.1.284 (Claude Code)` → `2.1.284`.
fn parse_installed(text: &str) -> Option<String> {
    let token = text.split_whitespace().next()?;
    token
        .chars()
        .next()
        .filter(char::is_ascii_digit)
        .map(|_| token.to_owned())
}

fn read_latest() -> Result<String, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(LATEST_TIMEOUT))
        .build()
        .into();
    let mut response = agent
        .get(LATEST_URL)
        .call()
        .map_err(|error: ureq::Error| format!("Abfrage der jüngsten Version scheitert: {error}"))?;
    let body = response
        .body_mut()
        .read_to_string()
        .map_err(|error: ureq::Error| format!("Antwort der Registry nicht lesbar: {error}"))?;
    let value: Value =
        serde_json::from_str(&body).map_err(|_| "Antwort der Registry unlesbar".to_owned())?;
    value
        .get("version")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| "Registry nennt keine Version".to_owned())
}

/// Vergleicht numerisch je Abschnitt (`2.1.9` < `2.1.10`); ein Zusatz wie `-beta` zählt nicht.
pub(crate) fn compare_versions(left: &str, right: &str) -> Ordering {
    numbers(left).cmp(&numbers(right))
}

fn numbers(version: &str) -> Vec<u64> {
    version
        .split('.')
        .map(|part: &str| {
            part.chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
                .parse::<u64>()
                .unwrap_or(0)
        })
        .collect()
}

fn run_update() -> CliUpdateOutcome {
    match run_claude(&["update"], UPDATE_TIMEOUT) {
        Ok(finished) => CliUpdateOutcome {
            succeeded: finished.succeeded,
            output: tail(&finished.output),
        },
        Err(error) => CliUpdateOutcome {
            succeeded: false,
            output: error,
        },
    }
}

fn tail(output: &str) -> String {
    let lines: Vec<&str> = output
        .lines()
        .map(str::trim_end)
        .filter(|line: &&str| !line.is_empty())
        .collect();
    let start = lines.len().saturating_sub(OUTPUT_TAIL_LINES);
    lines[start..].join("\n")
}

/// Startet die Claude-Kommandozeile ohne Fenster und sammelt Standard- und Fehlerausgabe. Nach dem
/// Zeitlimit wird der Prozess beendet.
fn run_claude(args: &[&str], timeout: Duration) -> Result<Finished, String> {
    let exe = find_claude().ok_or_else(|| CLAUDE_NOT_FOUND.to_owned())?;
    let mut command = processes::command(exe);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    hide_console(&mut command);
    let mut child = command
        .spawn()
        .map_err(|error| format!("Claude-Kommandozeile startet nicht: {error}"))?;
    let stdout = child.stdout.take().map(read_in_thread);
    let stderr = child.stderr.take().map(read_in_thread);
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() >= deadline => {
                // Die Lese-Threads enden mit dem Dateiende nach `kill`; hier wird nicht auf sie gewartet.
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "Claude antwortete nicht innerhalb von {} s",
                    timeout.as_secs()
                ));
            }
            Ok(None) => thread::sleep(POLL_INTERVAL),
            Err(error) => return Err(format!("Claude nicht abgewartet: {error}")),
        }
    };
    let mut output = collect(stdout);
    output.push('\n');
    output.push_str(&collect(stderr));
    Ok(Finished {
        succeeded: status.success(),
        output,
    })
}

fn read_in_thread<R: Read + Send + 'static>(mut source: R) -> JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut bytes: Vec<u8> = Vec::new();
        let _ = source.read_to_end(&mut bytes);
        bytes
    })
}

fn collect(handle: Option<JoinHandle<Vec<u8>>>) -> String {
    let bytes = handle
        .map(|reader: JoinHandle<Vec<u8>>| reader.join().unwrap_or_default())
        .unwrap_or_default();
    String::from_utf8_lossy(&bytes).into_owned()
}

fn emit_changed(app: &AppHandle) {
    if let Err(error) = app.emit(CLI_UPDATE_CHANGED_EVENT, ()) {
        eprintln!("Update-Ereignis nicht gesendet: {error}");
    }
}
