//! Werkzeuge der Umsetzung im MCP-Zugang (ADR 048): der Agent legt eine Umsetzung an, liest ihren
//! Lagebericht und beschreibt Phasen. Jeder Aufruf wirkt im Vorhaben der aufrufenden Session.
use serde::Deserialize;
use serde_json::{Value, json};
use tauri::{Emitter, Manager};

use super::model::{
    ImplementationChangedEvent, ImplementationView, MAX_DESCRIPTION_CHARS, MAX_NAME_CHARS,
    MAX_SOURCE_CHARS, MAX_TITLE_CHARS, PhaseInput, check_length, validate,
};
use super::{IMPLEMENTATION_CHANGED_EVENT, changes};
use crate::mcp_access::ProcessKind;
use crate::mcp_access::tools::{ToolCall, ToolResult, ToolSet, ToolSpec};
use crate::sessions::registry::SessionRegistry;

const CREATE: &str = "implementation_create";
const GET: &str = "implementation_get";
const SET_PHASES: &str = "implementation_set_phases";
const DESCRIBE_PHASE: &str = "implementation_describe_phase";
const COMPLETE_STATION: &str = "implementation_complete_station";
const NO_REGISTRY: &str = "Der Verwalter startet noch";
const NOT_A_STATION: &str = "Diese Session ist keine Station.";
const SIDE_FORBIDDEN: &str = "Eine Nebenfrage darf die Umsetzung nur lesen.";
const MAX_RESULT_CHARS: usize = 200;

pub struct ImplementationTools;

impl ToolSet for ImplementationTools {
    fn prefix(&self) -> &'static str {
        "implementation_"
    }

    fn tools(&self) -> Vec<ToolSpec> {
        vec![
            ToolSpec {
                name: CREATE,
                description: "Legt nach der Freigabe eines Plans die Umsetzung in diesem Vorhaben \
                              an: Phasen in Plan-Reihenfolge, je Phase Stationen (z. B. Test, \
                              Umsetzung, Review), die der Verwalter nacheinander als frische \
                              Sessions startet. Liefert den Lagebericht.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "minLength": 1, "maxLength": MAX_NAME_CHARS },
                        "source": {
                            "type": "string",
                            "maxLength": MAX_SOURCE_CHARS,
                            "description": "Woher der Plan stammt, z. B. Pfad des Plan-Ordners",
                        },
                        "phases": phases_schema(),
                    },
                    "required": ["name", "phases"],
                    "additionalProperties": false,
                }),
            },
            ToolSpec {
                name: GET,
                description: "Liefert den Lagebericht der Umsetzung, zu der diese Session gehört \
                              (sonst der neuesten des Vorhabens): Stand, was läuft, was fertig \
                              ist, was offen ist.",
                input_schema: json!({
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false,
                }),
            },
            ToolSpec {
                name: SET_PHASES,
                description: "Schlägt die vollständige Liste der kommenden Phasen vor (alle \
                              Phasen ohne gestartete Station); der Benutzer übernimmt oder \
                              verwirft sie.",
                input_schema: json!({
                    "type": "object",
                    "properties": { "phases": phases_schema() },
                    "required": ["phases"],
                    "additionalProperties": false,
                }),
            },
            ToolSpec {
                name: DESCRIBE_PHASE,
                description: "Ändert Titel und/oder Beschreibung einer Phase der Umsetzung, auch \
                              einer fertigen.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "key": { "type": "string", "minLength": 1 },
                        "title": { "type": "string", "minLength": 1, "maxLength": MAX_TITLE_CHARS },
                        "description": { "type": "string", "maxLength": MAX_DESCRIPTION_CHARS },
                    },
                    "required": ["key"],
                    "additionalProperties": false,
                }),
            },
            ToolSpec {
                name: COMPLETE_STATION,
                description: "Meldet die Station dieser Session fertig, mit einer Zeile Ergebnis; \
                              `fix_needed` bei einem Review, das eine Fix-Runde braucht.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "result": { "type": "string", "minLength": 1, "maxLength": MAX_RESULT_CHARS },
                        "fix_needed": { "type": "boolean" },
                    },
                    "required": ["result"],
                    "additionalProperties": false,
                }),
            },
        ]
    }

    fn call(&self, call: &ToolCall) -> ToolResult {
        // Vor der Registry: eine Nebenfrage darf nur lesen. Ihr Prozess hat dieselben Argumente wie der
        // Hauptprozess (ADR 044) und damit auch die Freigabe, nur der Schlüssel unterscheidet sie.
        if call.kind == ProcessKind::Side && call.name != GET {
            return ToolResult::Err(SIDE_FORBIDDEN.to_owned());
        }
        let Some(registry) = call.app.try_state::<SessionRegistry>() else {
            return ToolResult::Err(NO_REGISTRY.to_owned());
        };
        let outcome = match call.name {
            CREATE => create(call, &registry),
            GET => get(call, &registry),
            DESCRIBE_PHASE => describe_phase(call, &registry),
            COMPLETE_STATION => complete_station(call, &registry),
            SET_PHASES => set_phases(call),
            other => Err(format!("Unbekanntes Werkzeug: {other}")),
        };
        outcome.unwrap_or_else(ToolResult::Err)
    }
}

fn phases_schema() -> Value {
    json!({
        "type": "array",
        "minItems": 1,
        "items": {
            "type": "object",
            "properties": {
                "key": { "type": "string", "minLength": 1, "maxLength": 12, "description": "Nummer der Phase, eindeutig, z. B. \"3\" oder \"3a\"" },
                "title": { "type": "string", "minLength": 1, "maxLength": 80 },
                "description": { "type": "string", "maxLength": 500 },
                "wave": { "type": "integer", "minimum": 1, "description": "Aufeinanderfolgende Phasen derselben Welle laufen parallel" },
                "stations": {
                    "type": "array",
                    "minItems": 1,
                    "items": {
                        "type": "object",
                        "properties": {
                            "stage": { "type": "string", "minLength": 1, "maxLength": 40, "description": "z. B. Test, Umsetzung, Review" },
                            "model": { "type": "string", "enum": ["fable", "opusplan", "opus", "sonnet", "haiku"] },
                            "by": { "type": "string", "enum": ["agent", "you"], "description": "\"you\": der Benutzer erledigt die Station selbst" },
                            "skill": { "type": "string", "minLength": 1, "maxLength": 40, "description": "Skill ohne \"/\", mit dem die Session beginnt" },
                            "brief": { "type": "string", "maxLength": 1000, "description": "Auftrag an die Session" },
                        },
                        "required": ["stage"],
                        "additionalProperties": false,
                    },
                },
            },
            "required": ["key", "title", "stations"],
            "additionalProperties": false,
        },
    })
}

#[derive(Deserialize)]
struct CreateArguments {
    #[serde(default)]
    name: String,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    phases: Vec<PhaseInput>,
}

#[derive(Deserialize)]
struct SetPhasesArguments {
    #[serde(default)]
    phases: Vec<PhaseInput>,
}

fn set_phases(call: &ToolCall) -> Result<ToolResult, String> {
    let arguments: SetPhasesArguments = parse(call.arguments)?;
    let text = changes::propose(call.app, call.session_id, &arguments.phases)?;
    let structured = call
        .app
        .try_state::<SessionRegistry>()
        .and_then(|registry: tauri::State<'_, SessionRegistry>| {
            registry
                .require_implementation_of_session(call.session_id)
                .ok()
        })
        .and_then(|view: ImplementationView| serde_json::to_value(view).ok());
    Ok(ToolResult::Ok { text, structured })
}

#[derive(Deserialize)]
struct DescribeArguments {
    #[serde(default)]
    key: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    description: Option<String>,
}

fn create(call: &ToolCall, registry: &SessionRegistry) -> Result<ToolResult, String> {
    let arguments: CreateArguments = parse(call.arguments)?;
    check_length("name", arguments.name.trim(), 1, MAX_NAME_CHARS)?;
    let source = arguments.source.unwrap_or_default();
    check_length("source", source.trim(), 0, MAX_SOURCE_CHARS)?;
    validate(&arguments.phases)?;
    let view = registry
        .create_implementation(call.session_id, &arguments.name, &source, &arguments.phases)
        .map_err(|error| error.to_string())?;
    announce(call, &view);
    // Die erste Station steht bereit (▶): wann die Umsetzung beginnt, entscheidet der Benutzer. Die
    // Automatik, die danach die Folgestationen startet, ist an.
    Ok(answer(registry, &view))
}

fn get(call: &ToolCall, registry: &SessionRegistry) -> Result<ToolResult, String> {
    let view = registry
        .require_implementation_of_session(call.session_id)
        .map_err(|error| error.to_string())?;
    Ok(answer(registry, &view))
}

fn describe_phase(call: &ToolCall, registry: &SessionRegistry) -> Result<ToolResult, String> {
    let arguments: DescribeArguments = parse(call.arguments)?;
    let key = arguments.key.trim();
    check_length("key", key, 1, super::model::MAX_KEY_CHARS)?;
    if let Some(title) = &arguments.title {
        check_length("title", title.trim(), 1, MAX_TITLE_CHARS)?;
    }
    if let Some(description) = &arguments.description {
        check_length("description", description, 0, MAX_DESCRIPTION_CHARS)?;
    }
    let view = registry
        .describe_implementation_phase(
            call.session_id,
            key,
            arguments.title.as_deref(),
            arguments.description.as_deref(),
        )
        .map_err(|error| error.to_string())?;
    announce(call, &view);
    Ok(ToolResult::Ok {
        text: format!("Phase {key} beschrieben."),
        structured: None,
    })
}

#[derive(Deserialize)]
struct CompleteArguments {
    #[serde(default)]
    result: String,
    #[serde(default)]
    fix_needed: bool,
}

/// Die Station wird über die aufrufende Session gefunden, nie über eine Angabe des Agenten.
fn complete_station(call: &ToolCall, registry: &SessionRegistry) -> Result<ToolResult, String> {
    let arguments: CompleteArguments = parse(call.arguments)?;
    check_length("result", arguments.result.trim(), 1, MAX_RESULT_CHARS)?;
    let Some((implementation_id, station_id)) = registry
        .station_of_session(call.session_id)
        .map_err(|error| error.to_string())?
    else {
        return Err(NOT_A_STATION.to_owned());
    };
    let outcome = super::complete_station(
        call.app,
        &implementation_id,
        &station_id,
        &arguments.result,
        arguments.fix_needed,
    )
    .map_err(|error| error.to_string())?;
    Ok(ToolResult::Ok {
        text: outcome.text(),
        structured: None,
    })
}

fn parse<T: for<'de> Deserialize<'de>>(arguments: &Value) -> Result<T, String> {
    serde_json::from_value(arguments.clone())
        .map_err(|error| format!("Eingabe nicht lesbar: {error}"))
}

fn answer(registry: &SessionRegistry, view: &ImplementationView) -> ToolResult {
    ToolResult::Ok {
        text: registry.report_text(view, false, super::now_ms()),
        structured: serde_json::to_value(view).ok(),
    }
}

/// Die Oberfläche lädt die Umsetzung neu; ohne Fenster geht nichts verloren.
fn announce(call: &ToolCall, view: &ImplementationView) {
    let _ = call.app.emit(
        IMPLEMENTATION_CHANGED_EVENT,
        ImplementationChangedEvent {
            implementation: view.clone(),
        },
    );
}
