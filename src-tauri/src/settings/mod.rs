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
const KEY_TERMINAL_PLACEMENT: &str = "terminal_placement";
const KEY_TERMINAL_HEIGHT: &str = "terminal_height";
const KEY_TERMINAL_WIDTH: &str = "terminal_width";
const KEY_REMOTE_ENABLED: &str = "remote_enabled";
const KEY_REMOTE_PORT: &str = "remote_port";
const KEY_REMOTE_HOST_ID: &str = "remote_host_id";

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
    }
}

fn stored_panel_width(stored: &HashMap<String, String>, panel: PanelId) -> Option<u32> {
    stored
        .get(panel_key(panel))
        .and_then(|text: &String| text.parse::<u32>().ok())
}

/// „Neues Vorhaben“ startet mit denselben Werten, solange die Einstellungen nicht geladen sind.
const DEFAULT_SETTINGS: Settings = Settings {
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
    terminal_placement: TerminalPlacement::Bottom,
    terminal_height: 260,
    terminal_width: 460,
    panel_widths: PanelWidths {
        sidebar: None,
        changes_tree: None,
        artifacts_list: None,
        background: None,
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

fn stored_or<T: DeserializeOwned>(stored: &HashMap<String, String>, key: &str, default: T) -> T {
    stored
        .get(key)
        .and_then(|text: &String| enum_from_text(text).ok())
        .unwrap_or(default)
}
