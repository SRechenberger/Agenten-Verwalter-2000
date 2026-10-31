//! Nummerierte Schema-Änderungen. `PRAGMA user_version` zählt, wie viele davon angewendet sind.
use rusqlite::Connection;

use crate::error::CommandError;

/// Eine neue Migration hängt als neue Datei hinten an und verlängert diese Liste;
/// eine bestehende wird nie geändert, weil sie auf fremden Rechnern schon gelaufen ist.
const MIGRATIONS: [&str; 16] = [
    include_str!("migrations/001_sessions_and_chat.sql"),
    include_str!("migrations/002_repositories_and_worktrees.sql"),
    include_str!("migrations/003_background.sql"),
    include_str!("migrations/004_main_checkout.sql"),
    include_str!("migrations/005_projects.sql"),
    include_str!("migrations/006_settings.sql"),
    include_str!("migrations/007_session_changes.sql"),
    include_str!("migrations/008_session_activity.sql"),
    include_str!("migrations/009_session_handoff.sql"),
    include_str!("migrations/010_session_chrome.sql"),
    include_str!("migrations/011_session_codex_model.sql"),
    include_str!("migrations/012_session_prompt_cache.sql"),
    include_str!("migrations/013_name_auto.sql"),
    include_str!("migrations/014_project_auto_handoff.sql"),
    include_str!("migrations/015_tool_details.sql"),
    include_str!("migrations/016_session_parent.sql"),
];

pub fn run(connection: &mut Connection) -> Result<(), CommandError> {
    let stored_version: u32 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let applied: usize = usize::try_from(stored_version).unwrap_or(usize::MAX);
    if applied > MIGRATIONS.len() {
        return Err(CommandError::Database(
            "Die Datenbank stammt von einer neueren Version der App.".to_owned(),
        ));
    }
    for (index, script) in MIGRATIONS.iter().enumerate().skip(applied) {
        let transaction = connection.transaction()?;
        transaction.execute_batch(script)?;
        transaction.execute_batch(&format!("PRAGMA user_version = {}", index + 1))?;
        transaction.commit()?;
    }
    Ok(())
}
