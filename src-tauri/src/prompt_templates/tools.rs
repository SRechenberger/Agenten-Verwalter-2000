//! Prompt-Vorlagen im MCP-Zugang (ADR 075, „Per MCP“), nach dem Muster der Agententypen (ADR 063):
//! Ein Agent liest alle Vorlagen und schlägt eine neue oder eine Änderung vor. Ein Vorschlag
//! speichert nichts: Er öffnet einen Kasten im Chat und wartet, bis der Benutzer über
//! `prompt_template_proposal_answer` übernimmt oder verwirft. Nur im Claude-Betrieb sichtbar;
//! Selbstfreigabe und Kettenbremse gelten hier nicht.
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};
use tauri::{AppHandle, Manager};

use super::model::{PromptTemplate, PromptTemplateError, PromptTemplateInput};
use super::{name_key, validate};
use crate::db::Database;
use crate::error::CommandError;
use crate::mcp_access::ProcessKind;
use crate::mcp_access::tools::{ToolCall, ToolResult, ToolSet, ToolSpec};
use crate::sessions::registry::SessionRegistry;

// Ergebnis und Antwortzeilen sind wörtlich die der Agententypen.
pub(crate) use crate::agent_types::tools::{accepted_label, rejected_label, rejected_result};

pub const LIST: &str = "prompt_template_list";
pub const PROPOSE_CREATE: &str = "prompt_template_propose_create";
pub const PROPOSE_UPDATE: &str = "prompt_template_propose_update";
const NO_REGISTRY: &str = "Der Verwalter startet noch.";
const SIDE_FORBIDDEN: &str = "Eine Nebenfrage darf Prompt-Vorlagen nur lesen.";

/// Die Syntax (E1, A4, A5), wie beide Vorschläge sie beschreiben.
macro_rules! syntax {
    () => {
        "Syntax: Variable {{Name}} wird ein Textfeld; Abschnitt {{#Name}}…{{/Name}} steht im \
         Auftrag, wenn der Haken gesetzt ist, Umkehrung {{^Name}}…{{/Name}}, wenn nicht; Kommentar \
         {{! … }} wird Hilfetext der nächsten Variable und steht nie im Auftrag. Ein Name hat 1 bis \
         40 Zeichen ohne { und }, Leerraum im Tag zählt nicht. Abschnitte dürfen geschachtelt sein \
         und schließen in umgekehrter Reihenfolge. Derselbe Name als Variable und als Abschnitt ist \
         ein Fehler. Nicht unterstützt: Listen, {{>…}}, {{=…=}} und {{&…}}; {{{Name}}} wirkt wie \
         {{Name}}, es gibt kein Escaping. Steht ein Abschnitts-, Umkehr-, Schluss- oder \
         Kommentar-Tag allein auf seiner Zeile, entfällt die Zeile. Die Namensvorlage nutzt \
         dieselben Felder."
    };
}

const LIST_DESCRIPTION: &str = "Liest alle Prompt-Vorlagen aus den Einstellungen vollständig, in \
     der Reihenfolge der Anlage: Name, Vorlagentext (`text`) und Namensvorlage (`nameTemplate`, \
     leer = keine). Eine Vorlage füllt über ein Formular den Auftrag und den Namen eines neuen \
     Vorhabens.";
const PROPOSE_CREATE_DESCRIPTION: &str = concat!(
    "Schlägt eine neue Prompt-Vorlage vor. Gespeichert wird nichts: Der Benutzer sieht den \
     Vorschlag in einem Kasten im Chat und übernimmt ihn, bearbeitet ihn vorher oder verwirft \
     ihn; das Werkzeug wartet so lange. Ergebnis: übernommen mit der gespeicherten Vorlage \
     (`edited` sagt, ob der Benutzer sie geändert hat) oder verworfen mit Begründung. Ein \
     ungültiger Vorschlag (Name leer, zu lang oder schon vergeben, Text leer oder zu lang, \
     Syntaxfehler mit Zeile und Spalte) kommt sofort zurück, eine Zeile je Fehler mit Feld und \
     Meldung; dann gibt es keinen Kasten. ",
    syntax!()
);
const PROPOSE_UPDATE_DESCRIPTION: &str = concat!(
    "Schlägt eine Änderung an einer bestehenden Prompt-Vorlage vor; `target` ist ihr heutiger \
     Name. Ein genanntes Feld ersetzt den gespeicherten Wert, ein fehlendes behält ihn. Lies die \
     Vorlage vorher mit prompt_template_list. Gespeichert wird nichts: Der Benutzer sieht die \
     Änderung in einem Kasten im Chat und übernimmt, bearbeitet oder verwirft; das Werkzeug \
     wartet so lange. Ergebnis wie beim Anlegen. Ein ungültiger Vorschlag kommt sofort zurück, \
     eine Zeile je Fehler mit Feld und Meldung; dann gibt es keinen Kasten. ",
    syntax!()
);

pub struct PromptTemplateTools;

impl ToolSet for PromptTemplateTools {
    fn prefix(&self) -> &'static str {
        "prompt_template_"
    }

    fn tools(&self) -> Vec<ToolSpec> {
        specs()
    }

    fn call(&self, call: &ToolCall) -> ToolResult {
        match handle(call) {
            Ok(value) => ToolResult::Ok {
                text: value.to_string(),
                structured: Some(value),
            },
            Err(sentence) => ToolResult::Err(sentence),
        }
    }

    /// Ein Vorschlag wartet auf den Kasten; den gibt es nur im Claude-Betrieb.
    fn requires_claude(&self) -> bool {
        true
    }

    fn blocks(&self) -> bool {
        true
    }

    fn is_read_only(&self, name: &str) -> bool {
        is_read_only(name)
    }

    fn cancel(&self, app: &AppHandle, session_id: &str, call_id: &str) {
        if let Some(registry) = app.try_state::<SessionRegistry>() {
            registry.cancel_tool_call(app, session_id, call_id);
        }
    }
}

/// Die drei Werkzeuge, wie `tools/list` sie nennt.
pub(crate) fn specs() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: LIST,
            description: LIST_DESCRIPTION,
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: PROPOSE_CREATE,
            description: PROPOSE_CREATE_DESCRIPTION,
            input_schema: proposal_schema(false),
        },
        ToolSpec {
            name: PROPOSE_UPDATE,
            description: PROPOSE_UPDATE_DESCRIPTION,
            input_schema: proposal_schema(true),
        },
    ]
}

/// `is_update`: `target` ist Pflicht, alles andere optional.
fn proposal_schema(is_update: bool) -> Value {
    let mut properties = json!({
        "name": {
            "type": "string",
            "description": "1 bis 40 Zeichen, eindeutig ohne Beachtung der Schreibweise.",
        },
        "text": {
            "type": "string",
            "description": "Vorlagentext, 1 bis 16 000 Zeichen; wird der Auftrag des Vorhabens.",
        },
        "nameTemplate": {
            "type": "string",
            "description": "Namensvorlage für den Namen des Vorhabens, bis 200 Zeichen; leer = keine.",
        },
    });
    let required: Vec<&str> = if is_update {
        properties["target"] = json!({
            "type": "string",
            "description": "Heutiger Name der Vorlage, ohne Beachtung der Schreibweise.",
        });
        vec!["target"]
    } else {
        vec!["name", "text"]
    };
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false,
    })
}

pub(crate) fn is_read_only(name: &str) -> bool {
    name == LIST
}

pub(crate) fn side_may_call(name: &str) -> bool {
    is_read_only(name)
}

fn handle(call: &ToolCall) -> Result<Value, String> {
    // Vor allem anderen: eine Nebenfrage darf nur lesen (ADR 044).
    if call.kind == ProcessKind::Side && !side_may_call(call.name) {
        return Err(SIDE_FORBIDDEN.to_owned());
    }
    let (Some(registry), Some(database)) = (
        call.app.try_state::<SessionRegistry>(),
        call.app.try_state::<Arc<Database>>(),
    ) else {
        return Err(NO_REGISTRY.to_owned());
    };
    let templates = super::list(&database).map_err(|error: CommandError| error.to_string())?;
    match call.name {
        LIST => list_answer(call.arguments, &templates),
        PROPOSE_CREATE => {
            let proposal = checked(&create_input(call.arguments)?, &templates, None)?;
            registry.tool_propose_prompt_template(call.app, call, None, proposal)
        }
        PROPOSE_UPDATE => {
            let (base, input) = update_input(call.arguments, &templates)?;
            let proposal = checked(&input, &templates, Some(&base.id))?;
            registry.tool_propose_prompt_template(call.app, call, Some(base), proposal)
        }
        other => Err(format!("Unbekanntes Werkzeug: {other}")),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ListArguments {}

/// Die Argumente beider Vorschläge; ein fehlendes Feld ist `None`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ProposalArguments {
    #[serde(default)]
    target: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    name_template: Option<String>,
}

fn parse<T: for<'de> Deserialize<'de>>(arguments: &Value) -> Result<T, String> {
    T::deserialize(arguments)
        .map_err(|error| format!("Die Argumente passen nicht zum Schema: {error}"))
}

/// Ergebnis von `prompt_template_list`: jede Vorlage ganz, ohne ID.
pub(crate) fn list_answer(
    arguments: &Value,
    templates: &[PromptTemplate],
) -> Result<Value, String> {
    // Ohne Argumente schickt die Kommandozeile mitunter gar keins.
    if !arguments.is_null() {
        let ListArguments {} = parse(arguments)?;
    }
    let all: Vec<PromptTemplateInput> = templates.iter().map(PromptTemplateInput::from).collect();
    Ok(json!({ "promptTemplates": all }))
}

/// Die vollständige Eingabe einer neuen Vorlage; eine fehlende Namensvorlage ist leer.
pub(crate) fn create_input(arguments: &Value) -> Result<PromptTemplateInput, String> {
    let fields: ProposalArguments = parse(arguments)?;
    if fields.target.is_some() {
        return Err(format!(
            "Das Argument target gibt es nur bei {PROPOSE_UPDATE}."
        ));
    }
    let required = |value: Option<String>, field: &str| {
        value.ok_or_else(|| format!("Das Argument {field} fehlt."))
    };
    Ok(PromptTemplateInput {
        name: required(fields.name, "name")?,
        text: required(fields.text, "text")?,
        name_template: fields.name_template.unwrap_or_default(),
    })
}

/// Die gespeicherte Vorlage (`base`) und ihre Eingabe mit den genannten Feldern ersetzt.
pub(crate) fn update_input(
    arguments: &Value,
    templates: &[PromptTemplate],
) -> Result<(PromptTemplate, PromptTemplateInput), String> {
    let fields: ProposalArguments = parse(arguments)?;
    let target = fields
        .target
        .ok_or_else(|| "Das Argument target fehlt.".to_owned())?;
    let base = templates
        .iter()
        .find(|template: &&PromptTemplate| name_key(&template.name) == name_key(&target))
        .ok_or_else(|| format!("Keine Vorlage „{target}“."))?
        .clone();
    let stored = PromptTemplateInput::from(&base);
    let input = PromptTemplateInput {
        name: fields.name.unwrap_or(stored.name),
        text: fields.text.unwrap_or(stored.text),
        name_template: fields.name_template.unwrap_or(stored.name_template),
    };
    Ok((base, input))
}

/// Dieselbe Prüfung wie Anlegen und Ändern im Editor, samt Syntax; Meldungen als eine Zeile je
/// Fehler `<feld>: <Meldung>`, dann gibt es keinen Kasten.
pub(crate) fn checked(
    input: &PromptTemplateInput,
    templates: &[PromptTemplate],
    own_id: Option<&str>,
) -> Result<PromptTemplateInput, String> {
    validate(input, templates, own_id).map_err(|errors: Vec<PromptTemplateError>| {
        errors
            .iter()
            .map(|error: &PromptTemplateError| {
                let field = serde_json::to_value(error.field).unwrap_or(Value::Null);
                format!("{}: {}", field.as_str().unwrap_or_default(), error.message)
            })
            .collect::<Vec<String>>()
            .join("\n")
    })
}

/// Ergebnis eines übernommenen Vorschlags. `edited`: Der Benutzer hat vor dem Speichern geändert.
pub(crate) fn accepted_result(saved: &PromptTemplate, proposal: &PromptTemplateInput) -> Value {
    json!({
        "decision": "accepted",
        "edited": PromptTemplateInput::from(saved) != *proposal,
        "promptTemplate": saved,
    })
}
