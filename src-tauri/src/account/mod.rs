//! Claude-Konto: gelesen über `claude auth status`, gewechselt über `claude auth login` (ADR 021).
pub mod model;

use std::io::Read;
use std::path::Path;
use std::process::Stdio;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::Duration;

use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};

use crate::agents::claude::locate::find_claude;
use crate::db::Database;
use crate::processes::{self, hide_console, show_console};
use crate::settings::{self, model::OperatingMode};
use crate::usage::UsageService;
use model::{AccountInfo, AccountStatus};

const STATUS_TIMEOUT: Duration = Duration::from_secs(15);
pub const ACCOUNT_CHANGED_EVENT: &str = "account://changed";
const CLAUDE_NOT_FOUND: &str = "Claude-Kommandozeile nicht gefunden";

/// Das Konto gehört der Kommandozeile, nicht der App: die App liest es nur und startet deren
/// Anmeldung, statt selbst ein OAuth-Verfahren nachzubauen (ADR 021).
pub struct AccountService {
    state: Mutex<AccountState>,
}

struct AccountState {
    info: Option<AccountInfo>,
    error: Option<String>,
    is_logging_in: bool,
}

impl Default for AccountService {
    fn default() -> Self {
        AccountService::new()
    }
}

impl AccountService {
    pub fn new() -> AccountService {
        AccountService {
            state: Mutex::new(AccountState {
                info: None,
                error: None,
                is_logging_in: false,
            }),
        }
    }

    /// Liest das Konto neu und blockiert dabei bis zu `STATUS_TIMEOUT`.
    pub fn load(&self) -> AccountStatus {
        self.store(read_current_status())
    }

    /// Startet die Anmeldung der Kommandozeile und kehrt sofort zurück; Ende und neues Konto meldet
    /// `account://changed`. Nie zwei Anmeldungen gleichzeitig.
    pub fn login(&self, app: &AppHandle) {
        {
            let mut state = self.lock();
            if state.is_logging_in {
                return;
            }
            state.is_logging_in = true;
        }
        emit_changed(app);
        let worker_app = app.clone();
        let spawned = thread::Builder::new()
            .name("account-login".to_owned())
            .spawn(move || {
                let outcome = run_login();
                worker_app
                    .state::<AccountService>()
                    .finish_login(&worker_app, outcome);
            });
        if let Err(error) = spawned {
            self.finish_login(app, Err(format!("Anmeldung startet nicht: {error}")));
        }
    }

    fn finish_login(&self, app: &AppHandle, outcome: Result<(), String>) {
        self.lock().is_logging_in = false;
        // Das Konto wird auch nach einem Abbruch neu gelesen: die Kommandozeile meldet das alte
        // Konto womöglich schon vor dem Ende der Anmeldung ab.
        let status = outcome.and_then(|()| read_current_status());
        self.store(status);
        emit_changed(app);
        refresh_usage(app);
    }

    fn store(&self, result: Result<AccountInfo, String>) -> AccountStatus {
        let mut state = self.lock();
        match result {
            Ok(info) => {
                state.info = Some(info);
                state.error = None;
            }
            Err(error) => state.error = Some(error),
        }
        AccountStatus {
            info: state.info.clone(),
            error: state.error.clone(),
            is_logging_in: state.is_logging_in,
        }
    }

    fn lock(&self) -> MutexGuard<'_, AccountState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn read_current_status() -> Result<AccountInfo, String> {
    let exe = find_claude().ok_or_else(|| CLAUDE_NOT_FOUND.to_owned())?;
    read_status(&exe)
}

/// Fragt `claude auth status --json`. Der Exit-Code zählt nicht: ohne Anmeldung endet die
/// Kommandozeile womöglich mit Fehlercode, nennt das aber im JSON (`loggedIn: false`).
fn read_status(exe: &Path) -> Result<AccountInfo, String> {
    let mut command = processes::command(exe);
    command
        .args(["auth", "status", "--json"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    hide_console(&mut command);
    let mut child = command
        .spawn()
        .map_err(|error| format!("Claude-Kommandozeile startet nicht: {error}"))?;
    let Some(mut stdout) = child.stdout.take() else {
        return Err("Ausgabe von Claude nicht verfügbar".to_owned());
    };
    let (sender, receiver) = mpsc::channel::<Vec<u8>>();
    let reader = thread::Builder::new()
        .name("account-status".to_owned())
        .spawn(move || {
            let mut bytes: Vec<u8> = Vec::new();
            let _ = stdout.read_to_end(&mut bytes);
            // Nach einem Zeitlimit gibt es keinen Empfänger mehr; die Ausgabe verfällt dann.
            let _ = sender.send(bytes);
        });
    let answer = match reader {
        Ok(_) => match receiver.recv_timeout(STATUS_TIMEOUT) {
            Ok(bytes) => Ok(bytes),
            Err(RecvTimeoutError::Timeout) => Err(format!(
                "Claude antwortete nicht innerhalb von {} s",
                STATUS_TIMEOUT.as_secs()
            )),
            Err(RecvTimeoutError::Disconnected) => {
                Err("Claude beendete sich ohne Antwort".to_owned())
            }
        },
        Err(error) => Err(format!("Lese-Thread startet nicht: {error}")),
    };
    // Ohne `wait` bliebe ein Prozess-Handle offen; der Lese-Thread endet mit dem Dateiende nach `kill`.
    let _ = child.kill();
    let _ = child.wait();
    parse_info(&String::from_utf8_lossy(&answer?))
}

fn parse_info(text: &str) -> Result<AccountInfo, String> {
    let value: Value =
        serde_json::from_str(text.trim()).map_err(|_| "Antwort von Claude unlesbar".to_owned())?;
    Ok(AccountInfo {
        logged_in: value
            .get("loggedIn")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        email: text_field(&value, "email"),
        org_name: text_field(&value, "orgName"),
        plan: text_field(&value, "subscriptionType"),
        auth_method: text_field(&value, "authMethod"),
    })
}

fn text_field(value: &Value, key: &str) -> Option<String> {
    value.get(key)?.as_str().map(str::to_owned)
}

/// Startet `claude auth login` und wartet auf sein Ende. Unter Windows in einem eigenen Fenster,
/// damit der Nutzer Adresse und Code-Eingabe sieht; Schließen des Fensters bricht ab.
fn run_login() -> Result<(), String> {
    let exe = find_claude().ok_or_else(|| CLAUDE_NOT_FOUND.to_owned())?;
    let mut command = processes::command(exe);
    command.args(["auth", "login"]);
    show_console(&mut command);
    let mut child = command
        .spawn()
        .map_err(|error| format!("Claude-Kommandozeile startet nicht: {error}"))?;
    child
        .wait()
        .map(|_| ())
        .map_err(|error| format!("Anmeldung nicht abgewartet: {error}"))
}

/// Das Kontingent hängt am Konto; in einer lokalen Betriebsart fragt der Verwalter Anthropic nicht
/// (ADR 016).
fn refresh_usage(app: &AppHandle) {
    let database = app.state::<Arc<Database>>();
    match settings::load(&database) {
        Ok(loaded) if loaded.operating_mode == OperatingMode::Claude => {
            app.state::<UsageService>().refresh(app, true);
        }
        Ok(_) => {}
        Err(error) => eprintln!("Kontingent nach der Anmeldung nicht abgerufen: {error}"),
    }
}

fn emit_changed(app: &AppHandle) {
    if let Err(error) = app.emit(ACCOUNT_CHANGED_EVENT, ()) {
        eprintln!("Konto-Ereignis nicht gesendet: {error}");
    }
}
