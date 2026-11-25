//! Agententypen im MCP-Zugang (ADR 063): ein Agent liest einen Typ vollständig und schlägt einen
//! neuen Typ oder eine Änderung vor. Ein Vorschlag speichert nichts: Er öffnet einen Kasten im Chat
//! und wartet, bis der Benutzer über `agent_type_proposal_answer` übernimmt oder verwirft. Nur im
//! Claude-Betrieb sichtbar; Selbstfreigabe und Kettenbremse gelten hier nicht.
use serde::{Deserialize, Deserializer};
use serde_json::{Value, json};
use tauri::{AppHandle, Manager};

use super::model::{AgentType, AgentTypeInput, FieldError};
use super::{name_key, validate};
use crate::agents::event::{Effort, ModelId};
use crate::error::CommandError;
use crate::mcp_access::ProcessKind;
use crate::mcp_access::tools::{ToolCall, ToolResult, ToolSet, ToolSpec};
use crate::sessions::registry::SessionRegistry;

pub const GET: &str = "agent_type_get";
pub const PROPOSE_CREATE: &str = "agent_type_propose_create";
pub const PROPOSE_UPDATE: &str = "agent_type_propose_update";
const NO_REGISTRY: &str = "Der Verwalter startet noch.";
const SIDE_FORBIDDEN: &str = "Eine Nebenfrage darf Agententypen nur lesen.";
const MODELS: [&str; 5] = ["fable", "opus", "opusplan", "sonnet", "haiku"];
const EFFORTS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];

const GET_DESCRIPTION: &str = "Liest einen Agententyp aus den Einstellungen vollständig: Name, \
     Beschreibung, Stationsnamen, Modell, Denkaufwand, Systemprompt (`prompt`), Dateien und Skills. \
     Der Name wird ohne Beachtung der Schreibweise verglichen.";
const PROPOSE_CREATE_DESCRIPTION: &str = "Schlägt einen neuen Agententyp vor. Gespeichert wird \
     nichts: Der Benutzer sieht den Vorschlag in einem Kasten im Chat und übernimmt ihn, bearbeitet \
     ihn vorher oder verwirft ihn; das Werkzeug wartet so lange. Ergebnis: übernommen mit dem \
     gespeicherten Typ (`edited` sagt, ob der Benutzer ihn geändert hat) oder verworfen mit \
     Begründung. Ein ungültiger Vorschlag kommt sofort zurück, eine Zeile je Fehler mit Feld und \
     Meldung; dann gibt es keinen Kasten. `files` sind absolut, `~/…` oder relativ zu den \
     Repositories des Vorhabens; beim Start einer Station werden nur reguläre Dateien bis 16 000 \
     Zeichen gelesen. `model` und `effort` gelten nur zusammen.";
const PROPOSE_UPDATE_DESCRIPTION: &str = "Schlägt eine Änderung an einem bestehenden Agententyp \
     vor; `target` ist sein heutiger Name. Ein genanntes Feld ersetzt den gespeicherten Wert, ein \
     fehlendes behält ihn; `model: null` und `effort: null` setzen „Standard“. Lies den Typ vorher mit \
     agent_type_get. Gespeichert wird nichts: Der Benutzer sieht alt und neu in einem Kasten im Chat \
     und übernimmt, bearbeitet oder verwirft; das Werkzeug wartet so lange. Ergebnis: übernommen mit \
     dem gespeicherten Typ (`edited` sagt, ob der Benutzer ihn geändert hat) oder verworfen mit \
     Begründung. Ein ungültiger Vorschlag kommt sofort zurück, eine Zeile je Fehler mit Feld und \
     Meldung; dann gibt es keinen Kasten. `files` wie beim Anlegen.";

pub struct AgentTypeTools;

impl ToolSet for AgentTypeTools {
    fn prefix(&self) -> &'static str {
        "agent_type_"
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

    /// Ein Vorschlag wartet auf den Kasten; den gibt es nur im Claude-Betrieb (D5).
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
            name: GET,
            description: GET_DESCRIPTION,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "minLength": 1, "maxLength": 40 },
                },
                "required": ["name"],
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

/// `is_update`: `target` ist Pflicht, alles andere optional, und `model`/`effort` dürfen `null` sein.
fn proposal_schema(is_update: bool) -> Value {
    let choice = |values: &[&str], description: &str| {
        let mut allowed: Vec<Value> = values.iter().map(|value: &&str| json!(value)).collect();
        if is_update {
            allowed.push(Value::Null);
            json!({ "type": ["string", "null"], "enum": allowed, "description": description })
        } else {
            json!({ "type": "string", "enum": allowed, "description": description })
        }
    };
    let texts = |description: &str| json!({ "type": "array", "items": { "type": "string" }, "description": description });
    let mut properties = json!({
        "name": { "type": "string", "maxLength": 40, "description": "1 bis 40 Zeichen, eindeutig." },
        "description": {
            "type": "string",
            "maxLength": 500,
            "description": "Bis 500 Zeichen; der Planer liest sie für seine Empfehlung.",
        },
        "stages": texts("Stationsnamen, die der Typ führt (etwa „Review“); höchstens 20."),
        "model": choice(&MODELS, "Ohne Angabe „Standard“; nur zusammen mit effort."),
        "effort": choice(&EFFORTS, "Ohne Angabe „Standard“; nur zusammen mit model."),
        "prompt": {
            "type": "string",
            "maxLength": 16_000,
            "description": "Systemprompt der Stationen dieses Typs, bis 16 000 Zeichen.",
        },
        "files": texts("Höchstens 10 Dateien, deren Text an den Systemprompt angehängt wird."),
        "skills": texts("Höchstens 10 Skills ohne „/“, die die erste Nachricht aufruft."),
    });
    let required: Vec<&str> = if is_update {
        properties["target"] = json!({
            "type": "string",
            "description": "Heutiger Name des Typs, ohne Beachtung der Schreibweise.",
        });
        vec!["target"]
    } else {
        vec!["name", "description"]
    };
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false,
    })
}

pub(crate) fn is_read_only(name: &str) -> bool {
    name == GET
}

pub(crate) fn side_may_call(name: &str) -> bool {
    is_read_only(name)
}

fn handle(call: &ToolCall) -> Result<Value, String> {
    // Vor der Registry: eine Nebenfrage darf nur lesen. Ihr Prozess hat dieselben Argumente wie der
    // Hauptprozess (ADR 044), nur der Schlüssel unterscheidet sie.
    if call.kind == ProcessKind::Side && !side_may_call(call.name) {
        return Err(SIDE_FORBIDDEN.to_owned());
    }
    let Some(registry) = call.app.try_state::<SessionRegistry>() else {
        return Err(NO_REGISTRY.to_owned());
    };
    let types = registry
        .agent_types()
        .map_err(|error: CommandError| error.to_string())?;
    match call.name {
        GET => get_answer(call.arguments, &types),
        PROPOSE_CREATE => {
            let proposal = checked(&create_input(call.arguments)?, &types, None)?;
            length_checked(&registry, None, &proposal)?;
            registry.tool_propose_agent_type(call.app, call, None, proposal)
        }
        PROPOSE_UPDATE => {
            let (base, input) = update_input(call.arguments, &types)?;
            let proposal = checked(&input, &types, Some(&base.id))?;
            length_checked(&registry, Some(&base.id), &proposal)?;
            registry.tool_propose_agent_type(call.app, call, Some(base), proposal)
        }
        other => Err(format!("Unbekanntes Werkzeug: {other}")),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GetArguments {
    name: String,
}

/// Die Argumente beider Vorschläge. `model` und `effort`: äußeres `None` = fehlt, `Some(None)` = `null`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProposalArguments {
    #[serde(default)]
    target: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    stages: Option<Vec<String>>,
    #[serde(default, deserialize_with = "present")]
    model: Option<Option<ModelId>>,
    #[serde(default, deserialize_with = "present")]
    effort: Option<Option<Effort>>,
    #[serde(default)]
    prompt: Option<String>,
    #[serde(default)]
    files: Option<Vec<String>>,
    #[serde(default)]
    skills: Option<Vec<String>>,
}

/// Ein genanntes Feld, auch `null`; ein fehlendes bleibt über `default` beim äußeren `None`.
fn present<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

fn parse<T: for<'de> Deserialize<'de>>(arguments: &Value) -> Result<T, String> {
    T::deserialize(arguments)
        .map_err(|error| format!("Die Argumente passen nicht zum Schema: {error}"))
}

fn unknown(name: &str) -> String {
    format!("Kein Agententyp „{name}“.")
}

fn find<'a>(types: &'a [AgentType], name: &str) -> Option<&'a AgentType> {
    types
        .iter()
        .find(|agent_type: &&AgentType| name_key(&agent_type.name) == name_key(name))
}

/// Ergebnis von `agent_type_get`: der ganze Typ, samt Systemprompt und Dateien.
pub(crate) fn get_answer(arguments: &Value, types: &[AgentType]) -> Result<Value, String> {
    let GetArguments { name } = parse(arguments)?;
    let agent_type = find(types, &name).ok_or_else(|| unknown(&name))?;
    Ok(json!({ "agentType": agent_type }))
}

/// Die vollständige Eingabe eines neuen Typs; fehlende Felder sind leer bzw. „Standard“.
pub(crate) fn create_input(arguments: &Value) -> Result<AgentTypeInput, String> {
    let fields: ProposalArguments = parse(arguments)?;
    if fields.target.is_some() {
        return Err(format!(
            "Das Argument target gibt es nur bei {PROPOSE_UPDATE}."
        ));
    }
    let required = |value: Option<String>, field: &str| {
        value.ok_or_else(|| format!("Das Argument {field} fehlt."))
    };
    Ok(AgentTypeInput {
        name: required(fields.name, "name")?,
        description: required(fields.description, "description")?,
        stages: fields.stages.unwrap_or_default(),
        model: fields.model.flatten(),
        effort: fields.effort.flatten(),
        prompt: fields.prompt.unwrap_or_default(),
        files: fields.files.unwrap_or_default(),
        skills: fields.skills.unwrap_or_default(),
    })
}

/// Der gespeicherte Typ (`base`) und seine Eingabe mit den genannten Feldern ersetzt.
pub(crate) fn update_input(
    arguments: &Value,
    types: &[AgentType],
) -> Result<(AgentType, AgentTypeInput), String> {
    let fields: ProposalArguments = parse(arguments)?;
    let target = fields
        .target
        .ok_or_else(|| "Das Argument target fehlt.".to_owned())?;
    let base = find(types, &target)
        .ok_or_else(|| unknown(&target))?
        .clone();
    let stored = AgentTypeInput::from(&base);
    let input = AgentTypeInput {
        name: fields.name.unwrap_or(stored.name),
        description: fields.description.unwrap_or(stored.description),
        stages: fields.stages.unwrap_or(stored.stages),
        model: fields.model.unwrap_or(stored.model),
        effort: fields.effort.unwrap_or(stored.effort),
        prompt: fields.prompt.unwrap_or(stored.prompt),
        files: fields.files.unwrap_or(stored.files),
        skills: fields.skills.unwrap_or(stored.skills),
    };
    Ok((base, input))
}

/// Die Längenprüfung beim Speichern (S2, S3) schon beim Vorschlag; Meldungen wie bei `checked`.
fn length_checked(
    registry: &SessionRegistry,
    own_id: Option<&str>,
    input: &AgentTypeInput,
) -> Result<(), String> {
    let errors = registry
        .type_length_errors(own_id, input)
        .map_err(|error: CommandError| error.to_string())?;
    if errors.is_empty() {
        return Ok(());
    }
    Err(errors_text(&errors))
}

/// Eine Zeile je Fehler: `<feld>: <Meldung>`.
fn errors_text(errors: &[FieldError]) -> String {
    errors
        .iter()
        .map(|error: &FieldError| {
            let field = serde_json::to_value(error.field).unwrap_or(Value::Null);
            format!("{}: {}", field.as_str().unwrap_or_default(), error.message)
        })
        .collect::<Vec<String>>()
        .join("\n")
}

/// Dieselbe Prüfung wie Anlegen und Ändern im Dialog; Meldungen als eine Zeile je Fehler
/// `<feld>: <Meldung>`, dann gibt es keinen Kasten.
pub(crate) fn checked(
    input: &AgentTypeInput,
    types: &[AgentType],
    own_id: Option<&str>,
) -> Result<AgentTypeInput, String> {
    validate(input, types, own_id).map_err(|errors: Vec<FieldError>| errors_text(&errors))
}

/// Ergebnis eines übernommenen Vorschlags. `edited`: Der Benutzer hat vor dem Speichern geändert.
pub(crate) fn accepted_result(saved: &AgentType, proposal: &AgentTypeInput) -> Value {
    json!({
        "decision": "accepted",
        "edited": AgentTypeInput::from(saved) != *proposal,
        "agentType": saved,
    })
}

/// Ergebnis eines verworfenen Vorschlags; `reason` getrimmt, `""` ohne Begründung.
pub(crate) fn rejected_result(reason: &str) -> Value {
    json!({ "decision": "rejected", "reason": reason.trim() })
}

/// Die Antwortzeile des Kastens.
pub(crate) fn accepted_label(edited: bool) -> String {
    if edited {
        "Übernommen mit Änderungen".to_owned()
    } else {
        "Übernommen".to_owned()
    }
}

pub(crate) fn rejected_label(reason: &str) -> String {
    match reason.trim() {
        "" => "Verworfen".to_owned(),
        reason => format!("Verworfen: {reason}"),
    }
}
