use std::path::PathBuf;

use tauri::Manager;
use tauri_plugin_opener::OpenerExt;

use crate::artifacts::{self, model::ArtifactList, server::ArtifactServer};
use crate::error::CommandError;
use crate::sessions::registry::SessionRegistry;

const SERVER_MISSING: &str = "Der Server für Artefakte ist nicht gestartet";

/// Die Artefakte des Vorhabens, zu dem die Session gehört (ADR 026). `async`, damit Ordner und
/// Datenbank nicht auf dem Haupt-Thread gelesen werden.
#[tauri::command]
pub async fn artifacts_list(
    app: tauri::AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    session_id: String,
) -> Result<ArtifactList, CommandError> {
    let server = app
        .try_state::<ArtifactServer>()
        .ok_or_else(|| CommandError::Io(SERVER_MISSING.to_owned()))?;
    let (workspace, owners) = registry.artifact_scope(&session_id)?;
    let dir = artifacts::dir(&workspace);
    let mut items = artifacts::list(&dir, &owners);
    artifacts::extern_files::append_to(&dir, &registry.artifact_folders(), &mut items);
    Ok(ArtifactList {
        dir: dir.to_string_lossy().into_owned(),
        base_url: server.base_url(&session_id),
        items,
    })
}

/// Öffnet ein Artefakt mit dem Standardprogramm, einen Ordner im Explorer, eine Website über ihre
/// `index.html`. `file` kommt aus der
/// Oberfläche: nur ein Name direkt im Ordner, und `artifacts::open_target` prüft Grenze und
/// Endung, bevor etwas geöffnet wird.
#[tauri::command]
pub async fn artifact_open_default(
    app: tauri::AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    session_id: String,
    file: String,
) -> Result<(), CommandError> {
    let dir = checked_dir(&registry, &session_id, &file)?;
    let target = artifacts::open_target(&dir, &file)?;
    app.opener()
        .open_path(target.to_string_lossy(), None::<&str>)
        .map_err(|error| CommandError::Io(error.to_string()))
}

/// Zeigt ein Artefakt im Explorer, markiert in seinem Ordner.
#[tauri::command]
pub async fn artifact_reveal(
    app: tauri::AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    session_id: String,
    file: String,
) -> Result<(), CommandError> {
    let dir = checked_dir(&registry, &session_id, &file)?;
    let target = artifacts::resolve_entry(&dir, &file)?;
    app.opener()
        .reveal_item_in_dir(&target)
        .map_err(|error| CommandError::Io(error.to_string()))
}

/// Der Artefakte-Ordner der Session, wenn `file` ein Name direkt darin ist.
fn checked_dir(
    registry: &SessionRegistry,
    session_id: &str,
    file: &str,
) -> Result<PathBuf, CommandError> {
    if !artifacts::is_plain_name(file) {
        return Err(CommandError::FileNotAllowed(format!(
            "Kein Artefakt: {file}"
        )));
    }
    registry.artifacts_dir(session_id)
}

/// Speichert den Zustand eines Artefakts (ADR 081) und gibt den Änderungszeitpunkt der Datei zurück.
/// `file` ist das Artefakt, das die Oberfläche anzeigt, nie ein Wert der Seite; `data` ist JSON-Text
/// eines Objekts aus Zeichenketten, höchstens 1 MiB (`artifacts::state::save` prüft beides).
#[tauri::command]
pub async fn artifact_state_save(
    registry: tauri::State<'_, SessionRegistry>,
    session_id: String,
    file: String,
    data: String,
) -> Result<f64, CommandError> {
    let dir = checked_dir(&registry, &session_id, &file)?;
    artifacts::state::save(&dir, &file, &data)
}

/// Löscht den Zustand eines Artefakts („Zustand zurücksetzen“); die Seite lädt danach leer.
#[tauri::command]
pub async fn artifact_state_reset(
    registry: tauri::State<'_, SessionRegistry>,
    session_id: String,
    file: String,
) -> Result<(), CommandError> {
    let dir = checked_dir(&registry, &session_id, &file)?;
    artifacts::state::reset(&dir, &file)
}
