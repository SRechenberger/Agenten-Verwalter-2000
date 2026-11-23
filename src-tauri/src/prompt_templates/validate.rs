//! Prüfung einer Prompt-Vorlage vor dem Speichern (ADR 075, A8, M4): rein, die Meldungen so, wie
//! die Oberfläche sie am Feld zeigt.
use super::model::{PromptTemplate, PromptTemplateError, PromptTemplateField, PromptTemplateInput};
use super::name_key;
use super::syntax::{self, Source};

pub const MAX_NAME_CHARS: usize = 40;
/// In UTF-16-Einheiten wie der Systemprompt der Agententypen: Der Text geht als erste Nachricht mit.
pub const MAX_TEXT_UNITS: usize = 16_000;
pub const MAX_NAME_TEMPLATE_CHARS: usize = 200;

/// Prüft die Eingabe gegen die gespeicherten Vorlagen und gibt sie so zurück, wie sie gespeichert
/// wird: Name und Namensvorlage getrimmt, der Text unverändert. `own_id` ist die Vorlage, die
/// geändert wird; mit sich selbst kollidiert sie nicht. Die Syntax prüft sie nur, wenn beide Längen
/// stimmen, und an der Namensvorlage wie eingegeben: So nennen Core und Editor dieselbe Zeile und
/// Spalte, auch bei Leerraum am Anfang.
pub fn validate(
    input: &PromptTemplateInput,
    existing: &[PromptTemplate],
    own_id: Option<&str>,
) -> Result<PromptTemplateInput, Vec<PromptTemplateError>> {
    let typed_name_template = input.name_template.as_str();
    let input = PromptTemplateInput {
        name: input.name.trim().to_owned(),
        text: input.text.clone(),
        name_template: input.name_template.trim().to_owned(),
    };
    let mut errors: Vec<PromptTemplateError> = Vec::new();
    let mut fail = |field: PromptTemplateField, message: String| {
        errors.push(PromptTemplateError { field, message });
    };

    let name = input.name.as_str();
    if name.is_empty() {
        fail(PromptTemplateField::Name, "Der Name fehlt.".to_owned());
    } else if name.chars().count() > MAX_NAME_CHARS {
        fail(
            PromptTemplateField::Name,
            "Der Name ist länger als 40 Zeichen.".to_owned(),
        );
    } else if existing.iter().any(|other: &PromptTemplate| {
        Some(other.id.as_str()) != own_id && name_key(&other.name) == name_key(name)
    }) {
        fail(
            PromptTemplateField::Name,
            format!("Der Name „{name}“ gehört schon einer anderen Vorlage."),
        );
    }

    let mut lengths_fit = true;
    if input.text.trim().is_empty() {
        lengths_fit = false;
        fail(
            PromptTemplateField::Text,
            "Der Vorlagentext fehlt.".to_owned(),
        );
    } else if input.text.encode_utf16().count() > MAX_TEXT_UNITS {
        lengths_fit = false;
        fail(
            PromptTemplateField::Text,
            "Der Vorlagentext ist länger als 16 000 Zeichen.".to_owned(),
        );
    }
    if input.name_template.chars().count() > MAX_NAME_TEMPLATE_CHARS {
        lengths_fit = false;
        fail(
            PromptTemplateField::NameTemplate,
            "Die Namensvorlage ist länger als 200 Zeichen.".to_owned(),
        );
    }
    if lengths_fit && let Err(error) = syntax::check(&input.text, typed_name_template) {
        let field = match error.source {
            Source::Text => PromptTemplateField::Text,
            Source::NameTemplate => PromptTemplateField::NameTemplate,
        };
        fail(
            field,
            format!(
                "Zeile {}, Spalte {}: {}",
                error.line, error.column, error.message
            ),
        );
    }

    if errors.is_empty() {
        Ok(input)
    } else {
        Err(errors)
    }
}
