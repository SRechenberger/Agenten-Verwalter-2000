//! Einstellungen der App (ADR 012). Die Datenbank ist die Quelle; fehlt ein Wert oder ist er
//! unlesbar, gilt der Standardwert, ohne dass der gespeicherte Text überschrieben wird.
pub mod model;

use std::collections::HashMap;

use serde::de::DeserializeOwned;
use tauri::AppHandle;

use crate::agents::event::{Effort, Mode, ModelId};
use crate::db::settings as setting_rows;
use crate::db::{Database, enum_from_text, enum_to_text};
use crate::error::CommandError;
use crate::filesystem::workspace::{WORKSPACES_DIR, data_dir, home_dir};
use crate::remote::server::DEFAULT_PORT as DEFAULT_REMOTE_PORT;
use crate::session_tools::model::SelfRelease;
use crate::skills::{self, model::SkillKind, model::SkillOrigin};
use model::{
    ColorScheme, OperatingMode, PanelId, PanelWidths, Settings, SettingsChange, SettingsOverview,
    TerminalPlacement,
};

const KEY_COLOR_SCHEME: &str = "color_scheme";
const KEY_DEFAULT_MODEL: &str = "default_model";
const KEY_DEFAULT_EFFORT: &str = "default_effort";
const KEY_DEFAULT_MODE: &str = "default_mode";
const KEY_OPERATING_MODE: &str = "operating_mode";
const KEY_LOCAL_MODEL: &str = "local_model";
const KEY_DEFAULT_CODEX_MODEL: &str = "default_codex_model";
const KEY_MASTER_DIR: &str = "master_dir";
const KEY_MARK_UNSOURCED: &str = "mark_unsourced";
const KEY_DEFAULT_SELF_RELEASE: &str = "default_self_release";
const KEY_DEFAULT_AUTO_RESUME: &str = "default_auto_resume";
/// Leer = unbegrenzt; fehlt der Schlüssel, gilt der Standard (ADR 068).
const KEY_MAX_AGENTS: &str = "max_agents";
/// Leer = aus; fehlt der Schlüssel, gilt der Standard (ADR 068, Taiga-Story 130).
const KEY_QUOTA_HOLD_PERCENT: &str = "quota_hold_percent";
/// Leer = aus; fehlt der Schlüssel, gilt der Standard (ADR 070).
const KEY_MASTER_COMPACT_TOKENS: &str = "master_compact_tokens";
/// JSON-Liste; fehlt der Schlüssel, gilt `DEFAULT_COMMAND_PROGRAMS` (ADR 073).
const KEY_COMMAND_PROGRAMS: &str = "command_programs";
const KEY_TERMINAL_PLACEMENT: &str = "terminal_placement";
const KEY_TERMINAL_HEIGHT: &str = "terminal_height";
const KEY_TERMINAL_WIDTH: &str = "terminal_width";
const KEY_REMOTE_ENABLED: &str = "remote_enabled";
const KEY_REMOTE_PORT: &str = "remote_port";
const KEY_REMOTE_HOST_ID: &str = "remote_host_id";

/// Erlaubte Werte von „Stationen anhalten ab … %“.
pub const QUOTA_HOLD_RANGE: std::ops::RangeInclusive<u32> = 1..=100;

/// Erlaubte Grenzen von „Master zusammenfassen ab … Tokens“.
pub const MASTER_COMPACT_RANGE: std::ops::RangeInclusive<u32> = 50_000..=2_000_000;

/// Grenzen für die gemerkte Größe des Terminal-Docks in Pixeln.
const TERMINAL_SIZE_RANGE: std::ops::RangeInclusive<u32> = 100..=10000;

/// Grenzen für die gemerkte Breite einer Leiste in Pixeln. Die eigentlichen Mindest- und
/// Standardbreiten kennt nur die Oberfläche (ADR 056).
const PANEL_WIDTH_RANGE: std::ops::RangeInclusive<u32> = 120..=10000;

fn panel_key(panel: PanelId) -> &'static str {
    match panel {
        PanelId::Sidebar => "panel_width.sidebar",
        PanelId::ChangesTree => "panel_width.changes_tree",
        PanelId::ArtifactsList => "panel_width.artifacts_list",
        PanelId::Background => "panel_width.background",
        PanelId::SettingsNav => "panel_width.settings_nav",
        PanelId::DashboardFolders => "panel_width.dashboard_folders",
        PanelId::DashboardReader => "panel_width.dashboard_reader",
        PanelId::ImplementationTimeline => "panel_width.implementation_timeline",
    }
}

fn stored_panel_width(stored: &HashMap<String, String>, panel: PanelId) -> Option<u32> {
    stored
        .get(panel_key(panel))
        .and_then(|text: &String| text.parse::<u32>().ok())
}

/// „Neues Vorhaben“ startet mit denselben Werten, solange die Einstellungen nicht geladen sind.
pub const DEFAULT_SETTINGS: Settings = Settings {
    color_scheme: ColorScheme::System,
    default_model: ModelId::Sonnet,
    default_effort: Effort::High,
    default_mode: Mode::Auto,
    operating_mode: OperatingMode::Claude,
    local_model: None,
    default_codex_model: None,
    master_dir: None,
    mark_unsourced: true,
    default_self_release: SelfRelease::OFF,
    default_auto_resume: false,
    max_agents: Some(4),
    quota_hold_percent: Some(90),
    master_compact_tokens: Some(150_000),
    // ponytail: eine `const` hält keine Zeichenketten; `load` setzt `DEFAULT_COMMAND_PROGRAMS` ein.
    command_programs: Vec::new(),
    terminal_placement: TerminalPlacement::Bottom,
    terminal_height: 260,
    terminal_width: 460,
    panel_widths: PanelWidths {
        sidebar: None,
        changes_tree: None,
        artifacts_list: None,
        background: None,
        settings_nav: None,
        dashboard_folders: None,
        dashboard_reader: None,
        implementation_timeline: None,
    },
};

pub fn load(database: &Database) -> Result<Settings, CommandError> {
    let stored: HashMap<String, String> =
        database.with(|connection| setting_rows::read_all(connection))?;
    Ok(Settings {
        color_scheme: stored_or(&stored, KEY_COLOR_SCHEME, DEFAULT_SETTINGS.color_scheme),
        default_model: stored_or(&stored, KEY_DEFAULT_MODEL, DEFAULT_SETTINGS.default_model),
        default_effort: stored_or(&stored, KEY_DEFAULT_EFFORT, DEFAULT_SETTINGS.default_effort),
        default_mode: stored_or(&stored, KEY_DEFAULT_MODE, DEFAULT_SETTINGS.default_mode),
        operating_mode: stored_or(&stored, KEY_OPERATING_MODE, DEFAULT_SETTINGS.operating_mode),
        local_model: stored
            .get(KEY_LOCAL_MODEL)
            .map(|text: &String| text.trim())
            .filter(|text: &&str| !text.is_empty())
            .map(str::to_owned),
        default_codex_model: stored
            .get(KEY_DEFAULT_CODEX_MODEL)
            .map(|text: &String| text.trim())
            .filter(|text: &&str| !text.is_empty())
            .map(str::to_owned),
        master_dir: stored
            .get(KEY_MASTER_DIR)
            .map(|text: &String| text.trim())
            .filter(|text: &&str| !text.is_empty())
            .map(str::to_owned),
        mark_unsourced: stored
            .get(KEY_MARK_UNSOURCED)
            .and_then(|text: &String| text.parse::<bool>().ok())
            .unwrap_or(DEFAULT_SETTINGS.mark_unsourced),
        default_self_release: stored
            .get(KEY_DEFAULT_SELF_RELEASE)
            .and_then(|text: &String| serde_json::from_str(text).ok())
            .unwrap_or(DEFAULT_SETTINGS.default_self_release),
        default_auto_resume: stored
            .get(KEY_DEFAULT_AUTO_RESUME)
            .and_then(|text: &String| text.parse::<bool>().ok())
            .unwrap_or(DEFAULT_SETTINGS.default_auto_resume),
        max_agents: stored
            .get(KEY_MAX_AGENTS)
            .map_or(DEFAULT_SETTINGS.max_agents, |text: &String| {
                stored_max_agents(text)
            }),
        quota_hold_percent: stored
            .get(KEY_QUOTA_HOLD_PERCENT)
            .map_or(DEFAULT_SETTINGS.quota_hold_percent, |text: &String| {
                stored_quota_hold_percent(text)
            }),
        master_compact_tokens: stored
            .get(KEY_MASTER_COMPACT_TOKENS)
            .map_or(DEFAULT_SETTINGS.master_compact_tokens, |text: &String| {
                stored_master_compact_tokens(text)
            }),
        command_programs: stored
            .get(KEY_COMMAND_PROGRAMS)
            .and_then(|text: &String| serde_json::from_str::<Vec<String>>(text).ok())
            .unwrap_or_else(default_command_programs),
        terminal_placement: stored_or(
            &stored,
            KEY_TERMINAL_PLACEMENT,
            DEFAULT_SETTINGS.terminal_placement,
        ),
        terminal_height: stored
            .get(KEY_TERMINAL_HEIGHT)
            .and_then(|text: &String| text.parse::<u32>().ok())
            .unwrap_or(DEFAULT_SETTINGS.terminal_height),
        terminal_width: stored
            .get(KEY_TERMINAL_WIDTH)
            .and_then(|text: &String| text.parse::<u32>().ok())
            .unwrap_or(DEFAULT_SETTINGS.terminal_width),
        panel_widths: PanelWidths {
            sidebar: stored_panel_width(&stored, PanelId::Sidebar),
            changes_tree: stored_panel_width(&stored, PanelId::ChangesTree),
            artifacts_list: stored_panel_width(&stored, PanelId::ArtifactsList),
            background: stored_panel_width(&stored, PanelId::Background),
            settings_nav: stored_panel_width(&stored, PanelId::SettingsNav),
            dashboard_folders: stored_panel_width(&stored, PanelId::DashboardFolders),
            dashboard_reader: stored_panel_width(&stored, PanelId::DashboardReader),
            implementation_timeline: stored_panel_width(&stored, PanelId::ImplementationTimeline),
        },
    })
}

/// Speichert genau die geänderten Schlüssel und gibt den ganzen neuen Stand zurück.
pub fn update(database: &Database, change: SettingsChange) -> Result<Settings, CommandError> {
    if let SettingsChange::LocalModel { value } = &change
        && value.trim().is_empty()
    {
        return Err(CommandError::Internal("Modellname leer".to_owned()));
    }
    if let SettingsChange::TerminalHeight { value } | SettingsChange::TerminalWidth { value } =
        &change
        && !TERMINAL_SIZE_RANGE.contains(value)
    {
        return Err(CommandError::Internal(
            "Terminalgröße außerhalb 100–10000 px".to_owned(),
        ));
    }
    if let SettingsChange::MaxAgents { value: Some(0) } = &change {
        return Err(CommandError::Internal(
            "Gleichzeitige Agenten: mindestens 1 oder leer".to_owned(),
        ));
    }
    if let SettingsChange::QuotaHoldPercent {
        value: Some(percent),
    } = &change
        && !QUOTA_HOLD_RANGE.contains(percent)
    {
        return Err(CommandError::Internal(
            "Stationen anhalten ab: 1 bis 100 % oder leer".to_owned(),
        ));
    }
    if let SettingsChange::MasterCompactTokens {
        value: Some(tokens),
    } = &change
        && !MASTER_COMPACT_RANGE.contains(tokens)
    {
        return Err(CommandError::Internal(
            "Master zusammenfassen: 50 000 bis 2 000 000 Tokens oder leer".to_owned(),
        ));
    }
    if let SettingsChange::PanelWidth {
        value: Some(width), ..
    } = &change
        && !PANEL_WIDTH_RANGE.contains(width)
    {
        return Err(CommandError::Internal(
            "Leistenbreite außerhalb 120–10000 px".to_owned(),
        ));
    }
    database.with(|connection| {
        let transaction = connection.transaction()?;
        match change {
            SettingsChange::ColorScheme { value } => {
                setting_rows::write(&transaction, KEY_COLOR_SCHEME, &enum_to_text(&value)?)?;
            }
            SettingsChange::DefaultModel { value } => {
                setting_rows::write(&transaction, KEY_DEFAULT_MODEL, &enum_to_text(&value)?)?;
            }
            SettingsChange::DefaultMode { mode, effort } => {
                setting_rows::write(&transaction, KEY_DEFAULT_MODE, &enum_to_text(&mode)?)?;
                setting_rows::write(&transaction, KEY_DEFAULT_EFFORT, &enum_to_text(&effort)?)?;
            }
            SettingsChange::OperatingMode { value } => {
                setting_rows::write(&transaction, KEY_OPERATING_MODE, &enum_to_text(&value)?)?;
            }
            SettingsChange::LocalModel { value } => {
                setting_rows::write(&transaction, KEY_LOCAL_MODEL, value.trim())?;
            }
            SettingsChange::DefaultCodexModel { value } => {
                // Leer = Standardmodell; `load` liest leere Texte als „nicht gesetzt“.
                setting_rows::write(
                    &transaction,
                    KEY_DEFAULT_CODEX_MODEL,
                    value.as_deref().map_or("", str::trim),
                )?;
            }
            SettingsChange::MasterDir { value } => {
                // Leer = kein Master-Ordner; `load` liest leere Texte als „nicht gesetzt“.
                setting_rows::write(
                    &transaction,
                    KEY_MASTER_DIR,
                    value.as_deref().map_or("", str::trim),
                )?;
            }
            SettingsChange::MarkUnsourced { value } => {
                setting_rows::write(&transaction, KEY_MARK_UNSOURCED, &value.to_string())?;
            }
            SettingsChange::DefaultSelfRelease { value } => {
                let text = serde_json::to_string(&value)
                    .map_err(|error| CommandError::Internal(error.to_string()))?;
                setting_rows::write(&transaction, KEY_DEFAULT_SELF_RELEASE, &text)?;
            }
            SettingsChange::MaxAgents { value } => {
                let text = value
                    .map(|limit: u32| limit.to_string())
                    .unwrap_or_default();
                setting_rows::write(&transaction, KEY_MAX_AGENTS, &text)?;
            }
            SettingsChange::QuotaHoldPercent { value } => {
                let text = value
                    .map(|percent: u32| percent.to_string())
                    .unwrap_or_default();
                setting_rows::write(&transaction, KEY_QUOTA_HOLD_PERCENT, &text)?;
            }
            SettingsChange::MasterCompactTokens { value } => {
                let text = value
                    .map(|tokens: u32| tokens.to_string())
                    .unwrap_or_default();
                setting_rows::write(&transaction, KEY_MASTER_COMPACT_TOKENS, &text)?;
            }
            SettingsChange::CommandPrograms { value } => {
                let programs = checked_programs(&value)?;
                let text = serde_json::to_string(&programs)
                    .map_err(|error| CommandError::Internal(error.to_string()))?;
                setting_rows::write(&transaction, KEY_COMMAND_PROGRAMS, &text)?;
            }
            SettingsChange::DefaultAutoResume { value } => {
                setting_rows::write(&transaction, KEY_DEFAULT_AUTO_RESUME, &value.to_string())?;
            }
            SettingsChange::TerminalPlacement { value } => {
                setting_rows::write(&transaction, KEY_TERMINAL_PLACEMENT, &enum_to_text(&value)?)?;
            }
            SettingsChange::TerminalHeight { value } => {
                setting_rows::write(&transaction, KEY_TERMINAL_HEIGHT, &value.to_string())?;
            }
            SettingsChange::TerminalWidth { value } => {
                setting_rows::write(&transaction, KEY_TERMINAL_WIDTH, &value.to_string())?;
            }
            SettingsChange::PanelWidth { panel, value } => match value {
                Some(width) => {
                    setting_rows::write(&transaction, panel_key(panel), &width.to_string())?;
                }
                None => setting_rows::delete(&transaction, panel_key(panel))?,
            },
        }
        transaction.commit()?;
        Ok(())
    })?;
    load(database)
}

pub fn overview(app: &AppHandle, database: &Database) -> Result<SettingsOverview, CommandError> {
    let settings = load(database)?;
    let home = home_dir(app)?;
    let user_skill_count = skills::collect(&home, &[])
        .iter()
        .filter(|skill: &&skills::model::SkillInfo| {
            skill.kind == SkillKind::Skill && matches!(skill.origin, SkillOrigin::User)
        })
        .count();
    Ok(SettingsOverview {
        settings,
        user_skills_dir: skills::skills_dir(&home).to_string_lossy().into_owned(),
        user_skill_count: u32::try_from(user_skill_count).unwrap_or(u32::MAX),
        workspaces_dir: data_dir(app)?
            .join(WORKSPACES_DIR)
            .to_string_lossy()
            .into_owned(),
    })
}

/// Schalter „Gäste erlauben“ und Port der Fernsteuerung (ADR 059); ohne Eintrag aus und
/// `remote::server::DEFAULT_PORT`.
pub fn remote_server(database: &Database) -> Result<(bool, u16), CommandError> {
    let stored: HashMap<String, String> =
        database.with(|connection| setting_rows::read_all(connection))?;
    let enabled = stored
        .get(KEY_REMOTE_ENABLED)
        .and_then(|text: &String| text.parse::<bool>().ok())
        .unwrap_or(false);
    let port = stored
        .get(KEY_REMOTE_PORT)
        .and_then(|text: &String| text.parse::<u16>().ok())
        .filter(|port: &u16| *port != 0)
        .unwrap_or(DEFAULT_REMOTE_PORT);
    Ok((enabled, port))
}

pub fn set_remote_server(
    database: &Database,
    enabled: bool,
    port: u16,
) -> Result<(), CommandError> {
    if port == 0 {
        return Err(CommandError::Internal("Port außerhalb 1–65535".to_owned()));
    }
    database.with(|connection| {
        let transaction = connection.transaction()?;
        setting_rows::write(&transaction, KEY_REMOTE_ENABLED, &enabled.to_string())?;
        setting_rows::write(&transaction, KEY_REMOTE_PORT, &port.to_string())?;
        transaction.commit()?;
        Ok(())
    })
}

/// Die ID, unter der Gäste diesen Host kennen; beim ersten Aufruf erzeugt und dann behalten.
pub fn remote_host_id(database: &Database) -> Result<String, CommandError> {
    database.with(|connection| {
        let stored = setting_rows::read_all(connection)?;
        if let Some(id) = stored
            .get(KEY_REMOTE_HOST_ID)
            .filter(|id: &&String| !id.is_empty())
        {
            return Ok(id.clone());
        }
        let id = uuid::Uuid::new_v4().to_string();
        setting_rows::write(connection, KEY_REMOTE_HOST_ID, &id)?;
        Ok(id)
    })
}

/// Standard der Freigabeliste (ADR 073).
pub fn default_command_programs() -> Vec<String> {
    crate::implementation::command::DEFAULT_PROGRAMS
        .iter()
        .map(|name: &&str| (*name).to_owned())
        .collect()
}

/// Getrimmt, ohne Doppel; jedes Programm ein bloßer Name ohne Pfad, Leerraum und Anführungszeichen.
fn checked_programs(value: &[String]) -> Result<Vec<String>, CommandError> {
    let mut programs: Vec<String> = Vec::new();
    for name in value.iter().map(|name: &String| name.trim()) {
        if name.is_empty()
            || name.contains(['/', '\\', '"', '{', '}'])
            || name.contains(char::is_whitespace)
        {
            return Err(CommandError::Internal(format!(
                "„{name}“ ist kein Programmname: ein bloßer Name ohne Pfad."
            )));
        }
        if !programs.iter().any(|known: &String| known == name) {
            programs.push(name.to_owned());
        }
    }
    Ok(programs)
}

/// Gespeicherter Wert von „Gleichzeitige Agenten max.“: leer = unbegrenzt, ein unlesbarer Wert oder 0
/// = der Standard.
fn stored_max_agents(text: &str) -> Option<u32> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    text.parse::<u32>()
        .ok()
        .filter(|limit: &u32| *limit >= 1)
        .or(DEFAULT_SETTINGS.max_agents)
}

/// Gespeicherter Wert von „Stationen anhalten ab“: leer = aus, ein unlesbarer oder unzulässiger Wert =
/// der Standard.
fn stored_quota_hold_percent(text: &str) -> Option<u32> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    text.parse::<u32>()
        .ok()
        .filter(|percent: &u32| QUOTA_HOLD_RANGE.contains(percent))
        .or(DEFAULT_SETTINGS.quota_hold_percent)
}

/// Gespeicherter Wert von „Master zusammenfassen ab“: leer = aus, ein unlesbarer oder unzulässiger Wert
/// = der Standard.
fn stored_master_compact_tokens(text: &str) -> Option<u32> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    text.parse::<u32>()
        .ok()
        .filter(|tokens: &u32| MASTER_COMPACT_RANGE.contains(tokens))
        .or(DEFAULT_SETTINGS.master_compact_tokens)
}

fn stored_or<T: DeserializeOwned>(stored: &HashMap<String, String>, key: &str, default: T) -> T {
    stored
        .get(key)
        .and_then(|text: &String| enum_from_text(text).ok())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::load;
    use super::model::SettingsChange;
    use crate::db::Database;

    #[test]
    fn the_answer_model_setting_is_gone() {
        // Die Einordnung per Modell entfällt (ADR 077): weder Feld noch Änderung, und eine Zeile
        // `answer_model` aus einer alten Datenbank bleibt ohne Wirkung.
        let database = Database::open(Path::new(":memory:")).expect("Datenbank im Speicher");
        database
            .with(|connection| crate::db::settings::write(connection, "answer_model", "sonnet"))
            .expect("schreiben");
        let loaded = serde_json::to_value(load(&database).expect("laden")).expect("json");
        assert!(loaded.get("answerModel").is_none(), "{loaded}");
        let change: Result<SettingsChange, _> =
            serde_json::from_value(serde_json::json!({"kind": "answerModel", "value": null}));
        assert!(change.is_err(), "die Änderung gibt es nicht mehr");
    }

    #[test]
    fn a_stored_dashboard_choice_is_ignored() {
        // Die Wahl im Kopf des Dashboards entfällt (Postfach): weder Feld noch Änderung, und eine Zeile
        // `dashboard` aus einer alten Datenbank bleibt unberührt und ohne Wirkung.
        let database = Database::open(Path::new(":memory:")).expect("Datenbank im Speicher");
        database
            .with(|connection| {
                crate::db::settings::write(connection, "dashboard", r#"{"byGroup": true}"#)
            })
            .expect("schreiben");
        let loaded = serde_json::to_value(load(&database).expect("laden")).expect("json");
        assert!(loaded.get("dashboard").is_none(), "{loaded}");
        let change: Result<SettingsChange, _> =
            serde_json::from_value(serde_json::json!({"kind": "dashboard", "value": {}}));
        assert!(change.is_err(), "die Änderung gibt es nicht mehr");
        let kept = database
            .with(|connection| crate::db::settings::read_all(connection))
            .expect("lesen");
        assert_eq!(
            kept.get("dashboard").map(String::as_str),
            Some(r#"{"byGroup": true}"#)
        );
    }
}
