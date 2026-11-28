//! Nummerierte Schema-Änderungen. `PRAGMA user_version` zählt, wie viele davon angewendet sind.
use rusqlite::Connection;

use crate::error::CommandError;

/// Eine neue Migration hängt als neue Datei hinten an und verlängert diese Liste (`LATEST` folgt
/// ihr, Tests vergleichen damit); eine bestehende wird nie geändert, weil sie auf fremden Rechnern schon gelaufen ist.
const MIGRATIONS: [&str; 42] = [
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
    include_str!("migrations/017_session_files_first.sql"),
    include_str!("migrations/018_session_tools.sql"),
    include_str!("migrations/019_implementations.sql"),
    include_str!("migrations/020_project_groups.sql"),
    include_str!("migrations/021_remote.sql"),
    include_str!("migrations/022_agent_types.sql"),
    include_str!("migrations/023_station_agent.sql"),
    include_str!("migrations/024_session_agent_prompt.sql"),
    include_str!("migrations/025_station_edges.sql"),
    include_str!("migrations/026_station_options.sql"),
    include_str!("migrations/027_auto_resume.sql"),
    include_str!("migrations/028_implementation_cancel.sql"),
    include_str!("migrations/029_agent_queue.sql"),
    include_str!("migrations/030_master_session.sql"),
    include_str!("migrations/031_dashboard.sql"),
    include_str!("migrations/032_session_messages.sql"),
    include_str!("migrations/033_command_stations.sql"),
    include_str!("migrations/034_user_messages.sql"),
    include_str!("migrations/035_prompt_templates.sql"),
    include_str!("migrations/036_implementation_per_master.sql"),
    include_str!("migrations/037_group_share.sql"),
    include_str!("migrations/038_dashboard_seen.sql"),
    include_str!("migrations/039_device_create_right.sql"),
    include_str!("migrations/040_token_usage.sql"),
    include_str!("migrations/041_message_stations.sql"),
    include_str!("migrations/042_project_position.sql"),
];

/// Die Schema-Version nach `run`: die Zahl der Migrationen.
#[cfg(test)]
pub(crate) const LATEST: u32 = {
    assert!(MIGRATIONS.len() <= u32::MAX as usize);
    #[allow(clippy::cast_possible_truncation)]
    {
        MIGRATIONS.len() as u32
    }
};

/// Die ersten `count` Skripte, für Tests, die einen alten Stand nachbauen.
#[cfg(test)]
pub(crate) fn first(count: usize) -> impl Iterator<Item = &'static str> {
    MIGRATIONS.iter().copied().take(count)
}

/// Nach dieser Migration leitet Rust die Kanten der Bestandsdaten ab (ADR 061, Regel R1), in
/// derselben Transaktion.
const EDGES_MIGRATION: usize = 25;

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
        if index + 1 == EDGES_MIGRATION {
            crate::db::implementations::backfill_edges(&transaction)?;
        }
        transaction.execute_batch(&format!("PRAGMA user_version = {}", index + 1))?;
        transaction.commit()?;
    }
    Ok(())
}
