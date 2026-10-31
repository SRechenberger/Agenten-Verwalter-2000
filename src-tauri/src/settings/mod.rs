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
use crate::skills::{self, model::SkillKind, model::SkillOrigin};
use model::{ColorScheme, OperatingMode, Settings, SettingsChange, SettingsOverview};

const KEY_COLOR_SCHEME: &str = "color_scheme";
const KEY_DEFAULT_MODEL: &str = "default_model";
const KEY_DEFAULT_EFFORT: &str = "default_effort";
const KEY_DEFAULT_MODE: &str = "default_mode";
const KEY_OPERATING_MODE: &str = "operating_mode";
const KEY_LOCAL_MODEL: &str = "local_model";
const KEY_DEFAULT_CODEX_MODEL: &str = "default_codex_model";
const KEY_MARK_UNSOURCED: &str = "mark_unsourced";

/// „Neues Vorhaben“ startet mit denselben Werten, solange die Einstellungen nicht geladen sind.
const DEFAULT_SETTINGS: Settings = Settings {
    color_scheme: ColorScheme::System,
    default_model: ModelId::Sonnet,
    default_effort: Effort::High,
    default_mode: Mode::Auto,
    operating_mode: OperatingMode::Claude,
    local_model: None,
    default_codex_model: None,
    mark_unsourced: true,
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
        mark_unsourced: stored
            .get(KEY_MARK_UNSOURCED)
            .and_then(|text: &String| text.parse::<bool>().ok())
            .unwrap_or(DEFAULT_SETTINGS.mark_unsourced),
    })
}

/// Speichert genau die geänderten Schlüssel und gibt den ganzen neuen Stand zurück.
pub fn update(database: &Database, change: SettingsChange) -> Result<Settings, CommandError> {
    if let SettingsChange::LocalModel { value } = &change
        && value.trim().is_empty()
    {
        return Err(CommandError::Internal("Modellname leer".to_owned()));
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
            SettingsChange::MarkUnsourced { value } => {
                setting_rows::write(&transaction, KEY_MARK_UNSOURCED, &value.to_string())?;
            }
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

fn stored_or<T: DeserializeOwned>(stored: &HashMap<String, String>, key: &str, default: T) -> T {
    stored
        .get(key)
        .and_then(|text: &String| enum_from_text(text).ok())
        .unwrap_or(default)
}
