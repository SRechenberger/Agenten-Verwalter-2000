use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::db::Database;
use crate::db::repositories::{self as repository_rows, RepositoryRow};
use crate::db::skill_uses as skill_use_rows;
use crate::error::CommandError;
use crate::filesystem::workspace::home_dir;
use crate::sessions::registry::SessionRegistry;
use crate::skills::{
    self,
    model::{SkillInfo, SkillUse},
};

const SKILL_USE_WINDOW_MS: f64 = 30.0 * 24.0 * 3600.0 * 1000.0;

// `async`, damit das Lesen der Skill-Ordner nicht auf dem Haupt-Thread läuft.

#[tauri::command]
pub async fn skill_list_for_session(
    app: tauri::AppHandle,
    database: tauri::State<'_, Arc<Database>>,
    registry: tauri::State<'_, SessionRegistry>,
    session_id: String,
) -> Result<Vec<SkillInfo>, CommandError> {
    let roots = registry.skill_roots(&session_id)?;
    skills::builtin::menu_entries(&database, &home_dir(&app)?, &roots)
}

/// Aufrufe je Skill in den letzten 30 Tagen (ADR 039).
#[tauri::command]
pub async fn skill_uses(
    database: tauri::State<'_, Arc<Database>>,
) -> Result<Vec<SkillUse>, CommandError> {
    let since_ms: f64 = now_ms() - SKILL_USE_WINDOW_MS;
    database.with(|connection| skill_use_rows::count_since(connection, since_ms))
}

fn now_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |elapsed| elapsed.as_secs_f64() * 1000.0)
}

/// Für „Neue Session“: gesucht wird in den Haupt-Checkouts der gewählten Repositories.
#[tauri::command]
pub async fn skill_list_for_repositories(
    app: tauri::AppHandle,
    database: tauri::State<'_, Arc<Database>>,
    repository_ids: Vec<String>,
) -> Result<Vec<SkillInfo>, CommandError> {
    let rows: Vec<RepositoryRow> =
        database.with(|connection| repository_rows::get_many(connection, &repository_ids))?;
    let roots: Vec<(String, PathBuf)> = rows
        .into_iter()
        .map(|row: RepositoryRow| (row.name, PathBuf::from(row.path)))
        .collect();
    skills::builtin::menu_entries(&database, &home_dir(&app)?, &roots)
}
