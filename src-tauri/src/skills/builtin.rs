//! Mitgelieferte Befehle der Betriebsart fürs `/`-Menü (ADR 034); Quelle ist `system/init` der
//! Claude-Kommandozeile. Autark und Codex führen keinen eigenen Befehl aus, ihr Menü bleibt bei
//! den Dateien.
use std::path::{Path, PathBuf};

use super::model::{SkillInfo, SkillKind, SkillOrigin};
use crate::db::{self, Database};
use crate::error::CommandError;
use crate::settings;
use crate::settings::model::OperatingMode;

const SETTINGS_KEY: &str = "claude_slash_commands";

/// Namen aus der Liste der Kommandozeile, die das Menü nie zeigt.
const HIDDEN: [&str; 11] = [
    // Der Verwalter hat dafür eigene Einträge.
    "model",
    "effort",
    "mcp",
    "rename",
    "chrome",
    // Setzt den Kontext des Agenten zurück, der Verlauf im Verwalter bliebe stehen („Neue Session“).
    "clear",
    // Antwortet nur „wizard has been removed“.
    "agents",
    // Bedienung des Terminals bzw. intern.
    "heapdump",
    "color",
    "focus",
    "workflow-launch-exec",
];
/// Interne Namen der Kommandozeile.
const HIDDEN_PREFIX: &str = "__";

/// Deutsche Beschreibungen der heute bekannten Namen; alle anderen bleiben ohne.
const DESCRIPTIONS: [(&str, &str); 33] = [
    (
        "compact",
        "Verlauf zusammenfassen, um Platz im Kontext zu schaffen",
    ),
    ("context", "Belegung des Kontexts als Tabelle"),
    ("usage", "Kontingent und Verbrauch des Abos"),
    (
        "config",
        "Einstellung der Kommandozeile setzen: /config schlüssel=wert",
    ),
    ("output-style", "Ausgabestil anzeigen oder wechseln"),
    ("advisor", "Berater-Modell ein- oder ausschalten"),
    ("goal", "Ziel setzen, an dem der Agent weiterarbeitet"),
    ("fast", "Schnellmodus umschalten"),
    ("autocompact", "Automatisches Zusammenfassen umschalten"),
    ("reload-skills", "Skills neu einlesen"),
    ("reload-plugins", "Plugins neu einlesen"),
    ("list-agents", "Andere laufende Claude-Sessions anzeigen"),
    ("skill-doctor", "Geladene Skills mit Verbrauch prüfen"),
    ("init", "CLAUDE.md für das Projekt anlegen"),
    ("doctor", "Installation prüfen – startet einen Agenten-Lauf"),
    ("insights", "Auswertung der eigenen Nutzung"),
    ("recap", "Kurzfassung der Session"),
    (
        "security-review",
        "Sicherheits-Review der offenen Änderungen",
    ),
    (
        "ultrareview",
        "Gründliches Review mit mehreren Agenten in der Cloud, kostenpflichtig",
    ),
    ("code-review", "Änderungen auf Fehler prüfen"),
    ("simplify", "Geänderten Code vereinfachen und aufräumen"),
    ("loop", "Befehl in Abständen wiederholen: /loop 5m /befehl"),
    (
        "schedule",
        "Geplante Agenten in der Cloud anlegen und verwalten",
    ),
    ("run", "App starten und eine Änderung live ansehen"),
    ("claude-api", "Nachschlagen zur Claude-API und den SDKs"),
    ("update-config", "Einstellungen von Claude Code ändern"),
    (
        "fewer-permission-prompts",
        "Häufige Rechte-Abfragen als Freigaben eintragen",
    ),
    ("dataviz", "Diagramme und Dashboards gestalten"),
    ("anthropic-skills:pdf", "PDF-Dateien lesen und erzeugen"),
    ("anthropic-skills:docx", "Word-Dateien lesen und erzeugen"),
    ("anthropic-skills:xlsx", "Excel-Dateien lesen und erzeugen"),
    (
        "anthropic-skills:pptx",
        "PowerPoint-Dateien lesen und erzeugen",
    ),
    (
        "anthropic-skills:deep-research",
        "Recherche über viele Quellen mit Bericht",
    ),
];

/// Merkt die Befehlsliste der Kommandozeile für das Menü; schreibt nur, wenn sie sich geändert hat.
pub fn remember_claude_commands(database: &Database, names: &[String]) -> Result<(), CommandError> {
    let json: String = serde_json::to_string(names)
        .map_err(|error: serde_json::Error| CommandError::Internal(error.to_string()))?;
    let stored = database.with(|connection| db::settings::read_all(connection))?;
    if stored.get(SETTINGS_KEY) == Some(&json) {
        return Ok(());
    }
    database.with(|connection| db::settings::write(connection, SETTINGS_KEY, &json))
}

/// Die mitgelieferten Befehle der Betriebsart; ohne gespeicherte Liste (Claude) und in Autark und
/// Codex eine leere.
fn names_for(database: &Database, mode: OperatingMode) -> Vec<String> {
    match mode {
        OperatingMode::Claude | OperatingMode::ClaudeCodeLocal => {
            let stored = database
                .with(|connection| db::settings::read_all(connection))
                .unwrap_or_default();
            let names: Vec<String> = stored
                .get(SETTINGS_KEY)
                .and_then(|json: &String| serde_json::from_str::<Vec<String>>(json).ok())
                .unwrap_or_default();
            names
                .into_iter()
                .filter(|name: &String| {
                    !HIDDEN.contains(&name.as_str()) && !name.starts_with(HIDDEN_PREFIX)
                })
                .collect()
        }
        OperatingMode::Standalone | OperatingMode::Codex => Vec::new(),
    }
}

/// Die Einträge des `/`-Menüs: erst die Dateien, dann die mitgelieferten Befehle der Betriebsart
/// aus den Einstellungen, nach Name sortiert.
pub fn menu_entries(
    database: &Database,
    home: &Path,
    roots: &[(String, PathBuf)],
) -> Result<Vec<SkillInfo>, CommandError> {
    let mode: OperatingMode = settings::load(database)?.operating_mode;
    let mut files: Vec<SkillInfo> = super::collect(home, roots);
    let mut builtins: Vec<String> = names_for(database, mode);
    // Die Kommandozeile kennt die Datei-Einträge selbst: sie behalten Beschreibung und Herkunft.
    builtins.retain(|name: &String| !files.iter().any(|file: &SkillInfo| &file.name == name));
    builtins.sort_by_key(|name: &String| name.to_lowercase());
    files.extend(builtins.into_iter().map(|name: String| SkillInfo {
        description: describe(&name),
        name,
        kind: SkillKind::Command,
        origin: SkillOrigin::Builtin,
    }));
    Ok(files)
}

fn describe(name: &str) -> String {
    DESCRIPTIONS
        .iter()
        .find(|(known, _)| *known == name)
        .map(|(_, description)| (*description).to_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Eine Datenbank im Speicher mit Betriebsart und der Befehlsliste der Kommandozeile.
    fn database_with(mode: OperatingMode, commands: &[&str]) -> Database {
        let database = Database::open(Path::new(":memory:")).expect("Datenbank im Speicher");
        let names: Vec<String> = commands
            .iter()
            .map(|name: &&str| (*name).to_owned())
            .collect();
        remember_claude_commands(&database, &names).expect("schreibbar");
        database
            .with(|connection| {
                db::settings::write(connection, "operating_mode", &db::enum_to_text(&mode)?)
            })
            .expect("schreibbar");
        database
    }

    fn builtin_names(database: &Database, home: &Path) -> Vec<String> {
        menu_entries(database, home, &[])
            .expect("Einträge")
            .into_iter()
            .filter(|skill: &SkillInfo| skill.origin == SkillOrigin::Builtin)
            .map(|skill: SkillInfo| skill.name)
            .collect()
    }

    /// Ein leerer Benutzerordner; `.claude` fehlt darin, also gibt es keine Datei-Einträge.
    fn empty_home() -> PathBuf {
        std::env::temp_dir().join("verwalter-builtin-test-leer")
    }

    #[test]
    fn standalone_and_codex_list_no_builtin_commands() {
        for mode in [OperatingMode::Standalone, OperatingMode::Codex] {
            let database = database_with(mode, &["compact", "context"]);
            assert!(builtin_names(&database, &empty_home()).is_empty());
        }
    }

    #[test]
    fn claude_modes_list_builtin_commands_sorted_by_name() {
        for mode in [OperatingMode::Claude, OperatingMode::ClaudeCodeLocal] {
            let database = database_with(mode, &["usage", "Compact", "context"]);
            assert_eq!(
                builtin_names(&database, &empty_home()),
                ["Compact", "context", "usage"]
            );
        }
    }

    #[test]
    fn hidden_names_and_internal_prefix_never_reach_the_menu() {
        let database = database_with(
            OperatingMode::Claude,
            &["model", "clear", "agents", "__internal", "context"],
        );
        assert_eq!(builtin_names(&database, &empty_home()), ["context"]);
    }

    #[test]
    fn a_name_with_its_own_file_displaces_the_builtin_command() {
        let home = std::env::temp_dir().join("verwalter-builtin-test-datei");
        let commands = home.join(".claude").join("commands");
        std::fs::create_dir_all(&commands).expect("Ordner");
        std::fs::write(commands.join("context.md"), "Eigene Fassung\n").expect("Datei");
        let database = database_with(OperatingMode::Claude, &["context", "usage"]);
        let entries = menu_entries(&database, &home, &[]).expect("Einträge");
        let _ = std::fs::remove_dir_all(&home);
        let context: Vec<&SkillInfo> = entries
            .iter()
            .filter(|skill: &&SkillInfo| skill.name == "context")
            .collect();
        assert_eq!(context.len(), 1);
        assert_ne!(context[0].origin, SkillOrigin::Builtin);
        assert!(
            entries
                .iter()
                .any(|skill: &SkillInfo| skill.name == "usage"
                    && skill.origin == SkillOrigin::Builtin)
        );
    }
}
