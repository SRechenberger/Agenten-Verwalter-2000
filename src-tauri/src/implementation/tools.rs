//! Werkzeuge der Umsetzung im MCP-Zugang (ADR 048): der Agent legt eine Umsetzung an, liest ihren
//! Lagebericht, beschreibt Lanes und schlägt Plan-Änderungen vor; der Planer liest die Agententypen
//! (ADR 060). Jeder Aufruf wirkt im Vorhaben der aufrufenden Session. Die Beschreibung von
//! `implementation_create` ist die einzige Quelle der Feldnamen (ADR 061, Werkzeuge); im Code heißen
//! Lanes weiter Phasen.
use serde::Deserialize;
use serde_json::{Value, json};
use tauri::{Emitter, Manager};

use super::model::{
    Handoff, ImplementationChangedEvent, ImplementationView, MAX_BRIEF_CHARS,
    MAX_DESCRIPTION_CHARS, MAX_NAME_CHARS, MAX_PLAN_ID_CHARS, MAX_PROMPT_CHARS, MAX_SOURCE_CHARS,
    MAX_TITLE_CHARS, PhaseInput, PlanEdit, StationAddition, StationPatch, check_length,
    check_max_parallel, refuse_user_options, stations_in, validate,
};
use super::{IMPLEMENTATION_CHANGED_EVENT, changes, edit};
use crate::agent_types::model::AgentType;
use crate::error::CommandError;
use crate::mcp_access::ProcessKind;
use crate::mcp_access::tools::{ToolCall, ToolResult, ToolSet, ToolSpec};
use crate::sessions::registry::SessionRegistry;

/// Legt die Umsetzung an; `plan::plan_prompt` nennt es beim Namen.
pub const CREATE: &str = "implementation_create";
const GET: &str = "implementation_get";
pub const SET_PHASES: &str = "implementation_set_phases";
/// Schlägt eine Plan-Änderung als Delta vor (ADR 061, Werkzeuge).
pub const EDIT: &str = "implementation_edit";
const DESCRIBE_PHASE: &str = "implementation_describe_phase";
const COMPLETE_STATION: &str = "implementation_complete_station";
/// Liest die Agententypen für den Planer (ADR 060); `session_tools_prompt` nennt es beim Namen.
pub const AGENT_TYPES: &str = "implementation_agent_types";
/// Was eine Nebenfrage aufrufen darf: nur Lesendes.
const SIDE_READABLE: [&str; 2] = [GET, AGENT_TYPES];
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
                description: CREATE_DESCRIPTION,
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "minLength": 1, "maxLength": MAX_NAME_CHARS },
                        "source": {
                            "type": "string",
                            "maxLength": MAX_SOURCE_CHARS,
                            "description": "Woher der Plan stammt, z. B. Pfad des Plan-Ordners",
                        },
                        "phases": phases_schema(false),
                        "lanes": { "description": "Gleichwertiger Name für phases; nenne nur eines von beiden" },
                        "max_parallel": { "type": "integer", "minimum": 1, "description": "Höchstens so viele Stationen laufen gleichzeitig; ohne das Feld unbegrenzt. Weitere bereite Stationen warten in einer Warteschlange und rücken selbst nach" },
                    },
                    "required": ["name"],
                    "additionalProperties": false,
                }),
            },
            ToolSpec {
                name: GET,
                description: "Liefert den Lagebericht der Umsetzung, zu der diese Session gehört \
                              (sonst der neuesten des Vorhabens): Stand, Grenze und Warteschlange, \
                              Startstationen, was läuft, was fertig ist, was offen ist, und je \
                              Station ihre Lane, Stufe, Vorgänger und Nachfolger.",
                input_schema: json!({
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false,
                }),
            },
            ToolSpec {
                name: AGENT_TYPES,
                description: "Liest die Agententypen aus den Einstellungen: Name, Beschreibung, \
                              Stationsnamen, Modell, Denkaufwand und Skills, nie den Prompt-Text. \
                              Lies sie, bevor du einen Plan mit Stationen vorlegst, und empfiehl je \
                              Phase Stationen, Reihenfolge und Typ mit kurzer Begründung. Eine \
                              Station nennt ihren Typ im Feld `agent`; ohne das Feld gilt der Typ, \
                              der ihren Namen führt. Für eine einzelne Station weichen `model`, \
                              `effort`, `prompt` (Zusatztext zum Systemprompt) und `skill` vom Typ \
                              ab, etwa `effort: max` bei einem besonders kritischen Review; nutze \
                              das sparsam und begründe es im Plan. Ohne Typen ist die Liste leer.",
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
                    "properties": { "phases": phases_schema(false) },
                    "required": ["phases"],
                    "additionalProperties": false,
                }),
            },
            ToolSpec {
                name: EDIT,
                description: "Schlägt eine Änderung am Plan dieser Umsetzung vor, als Delta statt \
                              der vollen Liste; sie wirkt erst, wenn der Benutzer sie auf der Karte \
                              „Plan ändern?“ übernimmt, und ersetzt einen offenen Vorschlag. \
                              `remove` entfernt nicht gestartete Stationen (ihre Nachfolger warten \
                              danach auf deren Vorgänger), `change` ändert nicht gestartete Stationen \
                              (nur die genannten Felder; ein neuer `brief` verwirft Ergänzungen der \
                              Vorgänger), `add` hängt Stationen an eine bestehende Lane, `lanes` \
                              legt neue Lanes an, `max_parallel` setzt die Grenze (0 = unbegrenzt). \
                              Die Felder einer Station sind die von implementation_create, aber ohne \
                              `wave`; eine Station ohne `after` ist eine Startstation und startet \
                              sofort, auch neben der Station davor in ihrer Lane. Typ, Modell, Denkaufwand \
                              und Zusatztext änderst du nur, wenn der Benutzer es will. Gestartete \
                              und fertige Stationen bleiben, wie sie sind.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "lanes": phases_schema(true),
                        "add": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "lane": { "type": "string", "minLength": 1, "description": "Schlüssel der Lane, an deren Ende die Station kommt" },
                                },
                                "required": ["lane", "stage"],
                                "description": "Eine Station wie in implementation_create, dazu `lane`",
                            },
                        },
                        "change": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "id": { "type": "string", "minLength": 1, "description": "Kennung der Station" },
                                    "stage": { "type": "string" },
                                    "brief": { "type": "string" },
                                    "after": { "type": "array", "items": { "type": "string" } },
                                    "priority": { "type": "integer" },
                                    "agent": { "type": "string" },
                                    "model": { "type": "string" },
                                    "effort": { "type": "string" },
                                    "prompt": { "type": "string", "description": "Leer: kein Zusatztext" },
                                    "skill": { "type": "string" },
                                },
                                "required": ["id"],
                                "additionalProperties": false,
                            },
                        },
                        "remove": { "type": "array", "items": { "type": "string" }, "description": "Kennungen der Stationen" },
                        "max_parallel": { "type": "integer", "minimum": 0 },
                    },
                    "additionalProperties": false,
                }),
            },
            ToolSpec {
                name: DESCRIBE_PHASE,
                description: "Ändert Titel und/oder Beschreibung einer Lane der Umsetzung, auch \
                              einer fertigen; `key` ist ihr Schlüssel.",
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
                              `fix_needed` bei einem Review, das eine Fix-Runde braucht. Was ein \
                              wartender Nachfolger zusätzlich wissen muss, gibst du als `handoff` \
                              mit: `note` ergänzt den Auftrag jedes wartenden Nachfolgers (mit \
                              `fix_needed` den der Fix-Runde); `replace` ändert den Auftrag eines \
                              Nachfolgers statt ihn zu ergänzen, nur wo die Station es erlaubt. Lehnt \
                              ein Nachfolger ab, ist nichts gemeldet und der Fehler nennt das Feld; \
                              melde dann ohne `handoff`. Die Antwort nennt, wessen Auftrag sich \
                              geändert hat. Dem Nachfolger selbst schreibst du nicht.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "result": { "type": "string", "minLength": 1, "maxLength": MAX_RESULT_CHARS },
                        "fix_needed": { "type": "boolean" },
                        "handoff": {
                            "type": "object",
                            "properties": {
                                "note": { "type": "string", "minLength": 1, "maxLength": MAX_BRIEF_CHARS, "description": "Übergabenotiz; ergänzt den Auftrag jedes wartenden Nachfolgers" },
                                "replace": {
                                    "type": "array",
                                    "items": {
                                        "type": "object",
                                        "properties": {
                                            "id": { "type": "string", "minLength": 1, "description": "Kennung eines wartenden Nachfolgers" },
                                            "text": { "type": "string", "minLength": 1, "maxLength": MAX_BRIEF_CHARS, "description": "Neuer Auftrag dieses Nachfolgers" },
                                        },
                                        "required": ["id", "text"],
                                        "additionalProperties": false,
                                    },
                                },
                            },
                            "additionalProperties": false,
                        },
                    },
                    "required": ["result"],
                    "additionalProperties": false,
                }),
            },
        ]
    }

    /// Der Planer liest die Typen im Modus Plan, bevor er den Plan vorlegt (ADR 060).
    fn is_read_only(&self, name: &str) -> bool {
        name == AGENT_TYPES
    }

    fn call(&self, call: &ToolCall) -> ToolResult {
        // Vor der Registry: eine Nebenfrage darf nur lesen. Ihr Prozess hat dieselben Argumente wie der
        // Hauptprozess (ADR 044) und damit auch die Freigabe, nur der Schlüssel unterscheidet sie.
        if call.kind == ProcessKind::Side && !side_may_call(call.name) {
            return ToolResult::Err(SIDE_FORBIDDEN.to_owned());
        }
        let Some(registry) = call.app.try_state::<SessionRegistry>() else {
            return ToolResult::Err(NO_REGISTRY.to_owned());
        };
        let outcome = match call.name {
            CREATE => create(call, &registry),
            GET => get(call, &registry),
            AGENT_TYPES => registry
                .agent_types()
                .map(|types: Vec<AgentType>| agent_types_answer(&types))
                .map_err(|error: CommandError| error.to_string()),
            DESCRIBE_PHASE => describe_phase(call, &registry),
            COMPLETE_STATION => complete_station(call, &registry),
            SET_PHASES => set_phases(call),
            EDIT => edit_plan(call),
            other => Err(format!("Unbekanntes Werkzeug: {other}")),
        };
        outcome.unwrap_or_else(ToolResult::Err)
    }
}

pub(super) fn side_may_call(name: &str) -> bool {
    SIDE_READABLE.contains(&name)
}

/// Was der Planer von den Typen liest, als `structuredContent` und gleich als Text; nie den
/// Prompt-Text und nie die Dateien (V13). `model` und `effort` sind `null` bei „Standard“.
pub(super) fn agent_types_answer(types: &[AgentType]) -> ToolResult {
    let listed: Vec<Value> = types
        .iter()
        .map(|agent_type: &AgentType| {
            json!({
                "name": agent_type.name,
                "description": agent_type.description,
                "stages": agent_type.stages,
                "model": agent_type.model,
                "effort": agent_type.effort,
                "skills": agent_type.skills,
            })
        })
        .collect();
    let structured = json!({ "agentTypes": listed });
    ToolResult::Ok {
        text: structured.to_string(),
        structured: Some(structured),
    }
}

/// Was `implementation_create` tut und wie ein Plan aussieht; die Feldnamen stehen nur hier und im Schema.
const CREATE_DESCRIPTION: &str = "Legt nach der Freigabe eines Plans die Umsetzung in diesem \
     Vorhaben an: Lanes (`phases`, gleichwertig `lanes`) in Plan-Reihenfolge, je Lane Stationen (z. B. \
     Test, Umsetzung, Review), die der Verwalter als frische Sessions startet. Die Reihenfolge kommt aus \
     Kanten: Eine Station hat eine Kennung `id` (ohne Angabe `<Lane-Schlüssel>-<Nummer>`, die Nummer \
     1-basiert in der Lane) und nennt in `after` die Kennungen der Stationen, auf die sie wartet; ohne \
     `after` ist sie eine Startstation und darf sofort und parallel zu anderen starten. Unter bereiten \
     Stationen startet höhere `priority` zuerst. `max_parallel` begrenzt, wie viele gleichzeitig \
     laufen. Nennt keine Station `after`, gilt das alte Schema: Stationen einer Lane nacheinander, \
     aufeinanderfolgende Lanes mit derselben `wave` gleichzeitig, sonst eine nach der anderen. Zyklen, \
     unbekannte Kennungen und Selbstverweise lehnt der Verwalter mit einem Satz ab, der die Stationen \
     nennt. Liefert den Lagebericht samt Startstationen.";

/// `is_delta`: die neuen Lanes von `implementation_edit`. Dort gilt nur das Kanten-Schema (ADR 061): Es gibt
/// keine `wave`, und eine Station ohne `after` startet sofort, auch neben der Station davor.
fn phases_schema(is_delta: bool) -> Value {
    let mut schema = phases_schema_with_wave();
    let lane = &mut schema["items"]["properties"];
    if is_delta {
        lane.as_object_mut().expect("Objekt").remove("wave");
        lane["stations"]["items"]["properties"]["after"]["description"] = json!(
            "Kennungen der Stationen, auf die diese wartet; ohne das Feld ist sie eine Startstation und startet sofort, auch neben der Station davor in ihrer Lane: nenne die davor, wenn sie warten soll"
        );
    }
    schema
}

fn phases_schema_with_wave() -> Value {
    json!({
        "type": "array",
        "minItems": 1,
        "items": {
            "type": "object",
            "properties": {
                "key": { "type": "string", "minLength": 1, "maxLength": 12, "description": "Schlüssel der Lane, eindeutig, z. B. \"3\" oder \"3a\"" },
                "title": { "type": "string", "minLength": 1, "maxLength": 80 },
                "description": { "type": "string", "maxLength": 500 },
                "wave": { "type": "integer", "minimum": 1, "description": "Nur ohne after: Aufeinanderfolgende Lanes derselben Welle laufen parallel" },
                "stations": {
                    "type": "array",
                    "minItems": 1,
                    "items": {
                        "type": "object",
                        "properties": {
                            "stage": { "type": "string", "minLength": 1, "maxLength": 40, "description": "z. B. Test, Umsetzung, Review" },
                            "agent": { "type": "string", "minLength": 1, "maxLength": 40, "description": "Agententyp der Station (implementation_agent_types); ohne das Feld der Typ, der den Stationsnamen führt" },
                            "model": { "type": "string", "enum": ["fable", "opusplan", "opus", "sonnet", "haiku"], "description": "Modell dieser Station, abweichend vom Typ" },
                            "effort": { "type": "string", "enum": ["low", "medium", "high", "xhigh", "max"], "description": "Denkaufwand dieser Station, abweichend vom Typ" },
                            "prompt": { "type": "string", "maxLength": MAX_PROMPT_CHARS, "description": "Zusatztext zum Systemprompt dieser Station; steht hinter dem Text des Typs" },
                            "by": { "type": "string", "enum": ["agent", "you"], "description": "\"you\": der Benutzer erledigt die Station selbst" },
                            "skill": { "type": "string", "minLength": 1, "maxLength": 40, "description": "Skill ohne \"/\", mit dem die Session beginnt; abweichend vom ersten Skill des Typs" },
                            "brief": { "type": "string", "maxLength": 1000, "description": "Auftrag an die Session" },
                            "id": { "type": "string", "minLength": 1, "maxLength": MAX_PLAN_ID_CHARS, "description": "Kennung der Station, im Plan eindeutig; ohne Angabe <Lane-Schlüssel>-<Nummer>" },
                            "after": { "type": "array", "items": { "type": "string" }, "description": "Kennungen der Stationen, auf die diese wartet; leer oder ohne das Feld: Startstation" },
                            "priority": { "type": "integer", "description": "Vorrang unter bereiten Stationen, höher zuerst; Standard 0" },
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

// `lanes` ist ein gleichwertiger Name für `phases` (ADR 061, Werkzeuge); beide zugleich scheitern.
#[derive(Deserialize)]
struct CreateArguments {
    #[serde(default)]
    name: String,
    #[serde(default)]
    source: Option<String>,
    #[serde(default, alias = "lanes")]
    phases: Vec<PhaseInput>,
    #[serde(default)]
    max_parallel: Option<u32>,
}

#[derive(Deserialize)]
struct SetPhasesArguments {
    #[serde(default, alias = "lanes")]
    phases: Vec<PhaseInput>,
}

// Die Eingabe von `implementation_edit` mit den Feldnamen der Werkzeuge; `PlanEdit` ist dieselbe
// Änderung über die Tauri-Grenze.
#[derive(Deserialize)]
struct EditArguments {
    #[serde(default)]
    lanes: Vec<PhaseInput>,
    #[serde(default)]
    add: Vec<StationAddition>,
    #[serde(default)]
    change: Vec<StationPatch>,
    #[serde(default)]
    remove: Vec<String>,
    #[serde(default)]
    max_parallel: Option<u32>,
}

fn set_phases(call: &ToolCall) -> Result<ToolResult, String> {
    let arguments: SetPhasesArguments = parse(call.arguments)?;
    refuse_user_options(&stations_in("phases", &arguments.phases))?;
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

/// `implementation_edit`: Typ, Modell, Denkaufwand und Zusatztext ändert ein Agent nur so, als
/// Vorschlag, den der Benutzer übernimmt; die drei Einstellungen nur des Benutzers gar nicht.
fn edit_plan(call: &ToolCall) -> Result<ToolResult, String> {
    let arguments: EditArguments = parse(call.arguments)?;
    let edit = PlanEdit {
        lanes: arguments.lanes,
        add: arguments.add,
        change: arguments.change,
        remove: arguments.remove,
        max_parallel: arguments.max_parallel,
    };
    refuse_user_options(&edit.new_stations())?;
    let text = edit::propose(call.app, call.session_id, &edit)?;
    Ok(ToolResult::Ok {
        text,
        structured: None,
    })
}

fn create(call: &ToolCall, registry: &SessionRegistry) -> Result<ToolResult, String> {
    let arguments: CreateArguments = parse(call.arguments)?;
    check_length("name", arguments.name.trim(), 1, MAX_NAME_CHARS)?;
    let source = arguments.source.unwrap_or_default();
    check_length("source", source.trim(), 0, MAX_SOURCE_CHARS)?;
    validate(&arguments.phases)?;
    refuse_user_options(&stations_in("phases", &arguments.phases))?;
    check_max_parallel(arguments.max_parallel)?;
    let view = registry
        .create_implementation(
            call.session_id,
            &arguments.name,
            &source,
            &arguments.phases,
            arguments.max_parallel,
        )
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
        text: format!("Lane {key} beschrieben."),
        structured: None,
    })
}

#[derive(Deserialize)]
struct CompleteArguments {
    #[serde(default)]
    result: String,
    #[serde(default)]
    fix_needed: bool,
    #[serde(default)]
    handoff: Option<HandoffArguments>,
}

#[derive(Deserialize)]
struct HandoffArguments {
    #[serde(default)]
    note: Option<String>,
    #[serde(default)]
    replace: Vec<ReplaceArguments>,
}

#[derive(Deserialize)]
struct ReplaceArguments {
    #[serde(default)]
    id: String,
    #[serde(default)]
    text: String,
}

/// Die Übergabenotiz, Längen geprüft mit dem Feld im Satz.
fn checked_handoff(arguments: Option<HandoffArguments>) -> Result<Handoff, String> {
    let Some(arguments) = arguments else {
        return Ok(Handoff::default());
    };
    if let Some(note) = &arguments.note {
        check_length("handoff.note", note.trim(), 1, MAX_BRIEF_CHARS)?;
    }
    for (index, replacement) in arguments.replace.iter().enumerate() {
        let field = format!("handoff.replace[{index}]");
        check_length(
            &format!("{field}.id"),
            replacement.id.trim(),
            1,
            MAX_PLAN_ID_CHARS,
        )?;
        check_length(
            &format!("{field}.text"),
            replacement.text.trim(),
            1,
            MAX_BRIEF_CHARS,
        )?;
    }
    Ok(Handoff {
        note: arguments.note,
        replace: arguments
            .replace
            .into_iter()
            .map(|replacement: ReplaceArguments| (replacement.id, replacement.text))
            .collect(),
    })
}

/// Die Station wird über die aufrufende Session gefunden, nie über eine Angabe des Agenten.
fn complete_station(call: &ToolCall, registry: &SessionRegistry) -> Result<ToolResult, String> {
    let arguments: CompleteArguments = parse(call.arguments)?;
    check_length("result", arguments.result.trim(), 1, MAX_RESULT_CHARS)?;
    let handoff = checked_handoff(arguments.handoff)?;
    let Some((implementation_id, station_id)) = registry
        .station_of_session(call.session_id)
        .map_err(|error| error.to_string())?
    else {
        return Err(NOT_A_STATION.to_owned());
    };
    let outcome = super::complete_station_with_handoff(
        call.app,
        &implementation_id,
        &station_id,
        &arguments.result,
        arguments.fix_needed,
        &handoff,
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
