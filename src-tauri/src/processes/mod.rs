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
mod tests;

use std::env;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;
#[cfg(not(windows))]
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
