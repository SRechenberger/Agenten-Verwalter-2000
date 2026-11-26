use std::sync::Arc;

use crate::db::Database;
use crate::error::CommandError;
use crate::sessions::registry::now_ms;
use crate::tempo::{self, TempoScope, TempoSnapshot};

/// Das Tempo einer Session oder aller zusammen; Änderungen meldet `tempo://changed`.
// `async`, damit die Abfrage nicht auf dem Haupt-Thread läuft.
#[tauri::command]
pub async fn tempo_load(
    database: tauri::State<'_, Arc<Database>>,
    scope: TempoScope,
) -> Result<TempoSnapshot, CommandError> {
    tempo::load(&database, &scope, now_ms())
}
