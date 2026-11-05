use std::sync::Arc;

use crate::db::Database;
use crate::environment::{self, EnvironmentService, model::EnvironmentStatus};
use crate::error::CommandError;

// `async`, damit das Lesen von `~\.claude` und des Master-Ordners nicht auf dem Haupt-Thread läuft.

/// Verknüpfungen und Abgleich neu ermitteln; schreibt nichts.
#[tauri::command]
pub async fn environment_status(
    app: tauri::AppHandle,
    database: tauri::State<'_, Arc<Database>>,
    environment: tauri::State<'_, EnvironmentService>,
) -> Result<EnvironmentStatus, CommandError> {
    environment::status(&app, &database, &environment)
}

/// Kopiert aus dem Master-Ordner, was dort neuer ist oder lokal fehlt; löscht nie.
#[tauri::command]
pub async fn environment_mirror(
    app: tauri::AppHandle,
    database: tauri::State<'_, Arc<Database>>,
    environment: tauri::State<'_, EnvironmentService>,
) -> Result<EnvironmentStatus, CommandError> {
    environment::run_mirror(&app, &database, &environment)
}
