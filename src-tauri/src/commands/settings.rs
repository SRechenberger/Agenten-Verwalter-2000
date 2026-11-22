use std::sync::Arc;

use crate::db::Database;
use crate::error::CommandError;
use crate::lmstudio::{self, model::LocalModels};
use crate::sessions::registry::schedule_pass;
use crate::settings::{
    self,
    model::{OperatingMode, Settings, SettingsChange, SettingsOverview},
};
use crate::usage::UsageService;

// `async`, damit Datenbank und Skill-Ordner nicht auf dem Haupt-Thread gelesen werden.

#[tauri::command]
pub async fn settings_load(
    app: tauri::AppHandle,
    database: tauri::State<'_, Arc<Database>>,
) -> Result<SettingsOverview, CommandError> {
    settings::overview(&app, &database)
}

#[tauri::command]
pub async fn settings_update(
    app: tauri::AppHandle,
    database: tauri::State<'_, Arc<Database>>,
    usage: tauri::State<'_, UsageService>,
    change: SettingsChange,
) -> Result<Settings, CommandError> {
    let changes_operating_mode = matches!(change, SettingsChange::OperatingMode { .. });
    // Die Betriebsart zählt mit: Der Halt bei knappem Kontingent gilt nur in Claude.
    let changes_limit = matches!(
        change,
        SettingsChange::MaxAgents { .. }
            | SettingsChange::QuotaHoldPercent { .. }
            | SettingsChange::OperatingMode { .. }
    );
    let updated = settings::update(&database, change)?;
    // Eine höhere Grenze oder ein aufgehobener Halt lässt sofort nachrücken, eine niedrigere Grenze
    // beendet nichts (ADR 068).
    if changes_limit {
        schedule_pass(&app);
    }
    // Das Kontingent gehört zum Abo der Betriebsart; der Stand der alten gilt nicht weiter.
    if changes_operating_mode
        && matches!(
            updated.operating_mode,
            OperatingMode::Claude | OperatingMode::Codex
        )
    {
        usage.restart(&app);
    }
    Ok(updated)
}

/// Die Modelle aus LM Studio; ein nicht erreichbarer Server steht in `error`, kein Fehler.
#[tauri::command]
pub async fn settings_local_models() -> Result<LocalModels, CommandError> {
    Ok(lmstudio::list())
}
