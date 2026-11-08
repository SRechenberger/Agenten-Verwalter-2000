//! Tausch der laufenden exe, Neustart und Start des Installers; dazu die Art der Installation.
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use super::model::InstallKind;

/// Die alte exe nach dem Tausch; der nächste Start löscht sie.
pub const OLD_SUFFIX: &str = ".alt";
/// Die geladene exe vor dem Tausch.
pub const NEW_SUFFIX: &str = ".neu";
const UNINSTALLER: &str = "uninstall.exe";

/// `Portable` ohne, `Installer` mit `uninstall.exe` neben der exe; außerhalb von Windows `Manual`.
pub fn install_kind(exe: &Path) -> InstallKind {
    if !cfg!(windows) {
        return InstallKind::Manual;
    }
    let has_uninstaller = exe
        .parent()
        .is_some_and(|dir: &Path| dir.join(UNINSTALLER).is_file());
    if has_uninstaller {
        InstallKind::Installer
    } else {
        InstallKind::Portable
    }
}

/// `<path><suffix>` im selben Ordner, z.B. `verwalter.exe.alt`.
pub fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name: OsString = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

/// Legt `target` als `<target>.alt` beiseite und setzt `new` an seine Stelle. Windows erlaubt das
/// Umbenennen einer laufenden exe, nicht das Überschreiben. Scheitert das Einsetzen, steht die alte
/// Datei wieder am Pfad.
pub fn swap_files(target: &Path, new: &Path) -> Result<(), String> {
    let old = sibling(target, OLD_SUFFIX);
    // Kann noch gesperrt sein, wenn ein Agent von einem früheren Tausch weiterläuft; dann scheitert
    // das Umbenennen unten mit einer verständlichen Meldung.
    let _ = fs::remove_file(&old);
    fs::rename(target, &old)
        .map_err(|error| format!("Laufende Datei lässt sich nicht beiseitelegen: {error}"))?;
    let Err(error) = fs::rename(new, target) else {
        return Ok(());
    };
    match fs::rename(&old, target) {
        Ok(()) => Err(format!("Neue Datei lässt sich nicht einsetzen: {error}")),
        Err(restore_error) => Err(format!(
            "Neue Datei lässt sich nicht einsetzen: {error}. Die alte liegt unter {} und lässt sich nicht zurückbenennen: {restore_error}",
            old.display()
        )),
    }
}

/// Löscht Reste eines früheren Tauschs; eine gesperrte Datei bleibt bis zum nächsten Start liegen.
pub fn cleanup(exe: &Path) {
    let _ = fs::remove_file(sibling(exe, OLD_SUFFIX));
    let _ = fs::remove_file(sibling(exe, NEW_SUFFIX));
}

/// Startet die neue exe nach zwei Sekunden, damit der laufende Prozess vorher endet und die
/// Datenbank freigibt.
pub fn relaunch(exe: &Path) -> Result<(), String> {
    launch_delayed(exe)
}

/// Startet den Installer verzögert wie `relaunch`; das Starten über die Shell fragt nach
/// Administratorrechten, falls der Installer sie verlangt.
pub fn start_installer(installer: &Path) -> Result<(), String> {
    launch_delayed(installer)
}

#[cfg(windows)]
fn launch_delayed(path: &Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    use std::process::Stdio;

    use crate::processes;

    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let text = path.to_str().ok_or_else(|| {
        format!(
            "Pfad {} lässt sich nicht an die Shell geben.",
            path.display()
        )
    })?;
    // In Anführungszeichen ist für cmd nur `%` (Variable) gefährlich; `"` kommt in Windows-Pfaden nicht vor.
    if text.contains(['"', '%']) {
        return Err(format!("Pfad {text} lässt sich nicht an die Shell geben."));
    }
    let shell = std::env::var_os("ComSpec").unwrap_or_else(|| OsString::from("cmd.exe"));
    let mut command = processes::command(shell);
    // `raw_arg`, weil Rust sonst die Anführungszeichen für `start` maskierte. Kein DETACHED_PROCESS:
    // ohne eigene (verborgene) Konsole öffnete `ping` ein sichtbares Fenster. Der Prozess überlebt
    // das Ende des Verwalters auch so; die App hängt ihre Kinder an kein Job-Objekt.
    command
        .raw_arg(format!(
            "/d /s /c \"ping -n 3 127.0.0.1 >nul & start \"\" \"{text}\"\""
        ))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP);
    command
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Neustart lässt sich nicht anstoßen: {error}"))
}

#[cfg(not(windows))]
fn launch_delayed(_path: &Path) -> Result<(), String> {
    Err(super::releases::MANUAL_TEXT.to_owned())
}
