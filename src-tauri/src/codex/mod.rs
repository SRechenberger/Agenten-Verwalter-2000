//! Der Übersetzer der Betriebsart Codex (ADR 031): spricht nach außen den Ausschnitt des
//! Zeilenprotokolls der Claude-Kommandozeile, den der Verwalter liest (Kontrakt aus ADR 017), und
//! nach innen JSON-RPC mit `codex app-server`.
pub mod approvals;
pub mod helper;
pub mod input;
pub mod instructions;
pub mod items;
pub mod models;
pub mod policy;
pub mod print;
pub mod rate_limits;
pub mod rpc;
pub mod session;
pub mod tokens;

use std::env;
use std::ffi::OsString;
use std::path::PathBuf;

use crate::standalone::{EXIT_START_FAILED, args, home_dir};

/// Erstes Argument von `verwalter.exe`, das statt der App den Übersetzer startet.
pub const SUBCOMMAND: &str = "codex";
/// Pfad zu `codex.exe`, gesetzt vom Verwalter (`agents::claude::process`).
pub const EXE_VARIABLE: &str = "VERWALTER_CODEX_EXE";

const DATA_DIR: &str = ".verwalter";
const THREAD_DIR: &str = "codex";
/// So lange bekommt Codex nach einem SIGTERM an den Übersetzer, bevor SIGKILL seine Gruppe trifft;
/// das App-Ende wartet höchstens 2 s auf alle Agenten zusammen (ADR 074).
#[cfg(unix)]
const STOP_GRACE: std::time::Duration = std::time::Duration::from_millis(500);
/// Exit-Code nach SIGTERM, wie ihn eine Shell meldet (128 + 15).
#[cfg(unix)]
const EXIT_TERMINATED: i32 = 143;

/// `<Benutzerordner>\.verwalter\codex\<id>.json` mit der Codex-Thread-ID der Session; ohne bekannten
/// Benutzerordner `None`.
pub fn thread_file(session_id: &str) -> Option<PathBuf> {
    Some(
        home_dir()?
            .join(DATA_DIR)
            .join(THREAD_DIR)
            .join(format!("{session_id}.json")),
    )
}

/// Läuft bis zum Dateiende der Standardeingabe; Rückgabe ist der Exit-Code des Prozesses.
pub fn run(raw_args: Vec<String>) -> i32 {
    // Codex läuft in eigener Gruppe, und das SIGTERM des Verwalters an die Gruppe des Übersetzers
    // erreicht ihn nicht: Der Übersetzer beendet ihn selbst, ohne `SHUTDOWN_GRACE` abzuwarten. Die
    // Liste von `stop_all` enthält hier nur die eigenen Codex-Prozesse (`RpcClient`).
    #[cfg(unix)]
    if let Err(error) = crate::processes::on_terminate(|| {
        crate::agents::claude::process::stop_all(STOP_GRACE);
        std::process::exit(EXIT_TERMINATED);
    }) {
        eprintln!("SIGTERM wird nicht abgefangen: {error}");
    }
    let parsed = match args::parse(&raw_args) {
        Ok(parsed) => parsed,
        Err(error) => {
            eprintln!("{error}");
            return EXIT_START_FAILED;
        }
    };
    let Some(exe) = env::var_os(EXE_VARIABLE)
        .filter(|value: &OsString| !value.is_empty())
        .map(PathBuf::from)
    else {
        eprintln!("{EXE_VARIABLE} fehlt");
        return EXIT_START_FAILED;
    };
    if parsed.print {
        return print::run(&parsed, &exe);
    }
    session::run(parsed, &exe)
}
