//! Werkzeuge der Artefakte im MCP-Zugang (ADR 046): der Agent listet die Artefakte seines
//! Vorhabens und wählt eines im Reiter „Artefakte“ aus.
use serde_json::{Value, json};
use tauri::{Emitter, Manager};

use super::model::{Artifact, ArtifactShowEvent};
use crate::mcp_access::tools::{ToolCall, ToolResult, ToolSet, ToolSpec};
use crate::sessions::registry::SessionRegistry;

const SHOW_EVENT: &str = "artifacts://show";
const LIST: &str = "artifact_list";
const SHOW: &str = "artifact_show";
const NO_REGISTRY: &str = "Der Verwalter startet noch";

pub struct ArtifactTools;

impl ToolSet for ArtifactTools {
    fn prefix(&self) -> &'static str {
        "artifact_"
    }

    fn tools(&self) -> Vec<ToolSpec> {
        vec![
            ToolSpec {
                name: LIST,
                description: "Listet die Artefakte dieses Vorhabens (Dateien und Ordner in .artefakte) \
                              mit Titel, Typ, letzter Änderung und der Session, die sie anlegte.",
                input_schema: json!({
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false,
                }),
            },
            ToolSpec {
                name: SHOW,
                description: "Wählt ein Artefakt im Reiter „Artefakte“ des Verwalters aus; sieht \
                              der Benutzer diese Session, springt die Ansicht dorthin.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "file": {
                            "type": "string",
                            "description": "Name einer Datei oder eines Ordners direkt in .artefakte",
                        },
                    },
                    "required": ["file"],
                    "additionalProperties": false,
                }),
            },
        ]
    }

    fn call(&self, call: &ToolCall) -> ToolResult {
        let Some(registry) = call.app.try_state::<SessionRegistry>() else {
            return ToolResult::Err(NO_REGISTRY.to_owned());
        };
        let (dir, items) = match artifacts_of(&registry, call.session_id) {
            Ok(found) => found,
            Err(error) => return ToolResult::Err(error),
        };
        match call.name {
            LIST => list(call.session_id, &dir, &items),
            SHOW => show(call, &registry, &items),
            other => ToolResult::Err(format!("Unbekanntes Werkzeug: {other}")),
        }
    }
}

fn artifacts_of(
    registry: &SessionRegistry,
    session_id: &str,
) -> Result<(String, Vec<Artifact>), String> {
    let (workspace, owners) = registry
        .artifact_scope(session_id)
        .map_err(|error| error.to_string())?;
    let dir = super::dir(&workspace);
    let items = super::list(&dir, &owners);
    Ok((dir.to_string_lossy().into_owned(), items))
}

fn list(session_id: &str, dir: &str, items: &[Artifact]) -> ToolResult {
    let lines: Vec<String> = items
        .iter()
        .map(|item: &Artifact| {
            let owner = match (item.session_number, &item.session_name) {
                (Some(number), Some(name)) => format!("#{number} {name}"),
                _ => "ohne Zuordnung".to_owned(),
            };
            format!(
                "„{}“ ({}, {}, {owner})",
                item.title,
                item.file,
                item.kind.as_str()
            )
        })
        .collect();
    let text = if lines.is_empty() {
        format!("Keine Artefakte in {dir}.")
    } else {
        lines.join("\n")
    };
    let structured: Vec<Value> = items
        .iter()
        .map(|item: &Artifact| {
            json!({
                "file": item.file,
                "title": item.title,
                "kind": item.kind,
                "modifiedAt": item.modified_at,
                "sessionNumber": item.session_number,
                "sessionName": item.session_name,
                "isOwnSession": item.session_id.as_deref() == Some(session_id),
            })
        })
        .collect();
    ToolResult::Ok {
        text,
        structured: Some(json!({ "dir": dir, "items": structured })),
    }
}

/// Nur ein Name aus der Liste: so erreicht kein Pfad aus dem Agenten die Oberfläche ungeprüft.
fn show(call: &ToolCall, registry: &SessionRegistry, items: &[Artifact]) -> ToolResult {
    let Some(file) = call.arguments.get("file").and_then(Value::as_str) else {
        return ToolResult::Err("Parameter file fehlt".to_owned());
    };
    let found = items
        .iter()
        .find(|item: &&Artifact| item.file == file)
        .or_else(|| {
            items
                .iter()
                .find(|item: &&Artifact| item.file.eq_ignore_ascii_case(file))
        });
    let Some(item) = found else {
        return ToolResult::Err(format!("Kein Artefakt: {file}"));
    };
    let project_id = match registry.project_of(call.session_id) {
        Ok(project_id) => project_id,
        Err(error) => return ToolResult::Err(error.to_string()),
    };
    let event = ArtifactShowEvent {
        session_id: call.session_id.to_owned(),
        project_id,
        file: item.file.clone(),
    };
    if let Err(error) = call.app.emit(SHOW_EVENT, event) {
        return ToolResult::Err(format!("Oberfläche nicht erreichbar: {error}"));
    }
    ToolResult::Ok {
        text: format!("„{}“ im Reiter Artefakte ausgewählt.", item.title),
        structured: None,
    }
}
