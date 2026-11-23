//! Typen der Prompt-Vorlagen über die Tauri-Grenze (ADR 075).
use serde::{Deserialize, Serialize};
use ts_rs::TS;

// Eine Prompt-Vorlage aus den Einstellungen; die Liste kommt in der Reihenfolge der Anlage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PromptTemplate {
    // UUID.
    pub id: String,
    // Getrimmt, 1 bis 40 Zeichen, ohne Beachtung der Schreibweise eindeutig.
    pub name: String,
    // Vorlagentext, 1 bis 16 000 UTF-16-Einheiten.
    pub text: String,
    // Namensvorlage, 0 bis 200 Zeichen; leer = keine.
    pub name_template: String,
}

// Eingabe für Anlegen und Ändern: wie `PromptTemplate` ohne `id`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PromptTemplateInput {
    pub name: String,
    pub text: String,
    pub name_template: String,
}

impl From<&PromptTemplate> for PromptTemplateInput {
    fn from(prompt_template: &PromptTemplate) -> PromptTemplateInput {
        PromptTemplateInput {
            name: prompt_template.name.clone(),
            text: prompt_template.text.clone(),
            name_template: prompt_template.name_template.clone(),
        }
    }
}

// Das Feld, an dem die Oberfläche eine Meldung zeigt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum PromptTemplateField {
    Name,
    Text,
    NameTemplate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PromptTemplateError {
    pub field: PromptTemplateField,
    pub message: String,
}

// Ergebnis von Anlegen und Ändern. Ist `errors` nicht leer, ist `prompt_template` `None` und nichts
// gespeichert; Fehler der Datenbank kommen als `CommandError`, nicht hier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PromptTemplateSaved {
    pub prompt_template: Option<PromptTemplate>,
    pub errors: Vec<PromptTemplateError>,
}
