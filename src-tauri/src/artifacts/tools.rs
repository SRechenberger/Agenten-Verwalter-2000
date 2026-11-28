//! Werkzeuge der Artefakte im MCP-Zugang (ADR 046): der Agent listet die Artefakte seines
//! Vorhabens und wählt eines im Reiter „Artefakte“ aus.
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use tauri::{Emitter, Manager};

use super::model::{Artifact, ArtifactShowEvent};
use crate::mcp_access::ProcessKind;
use crate::mcp_access::tools::{ToolCall, ToolResult, ToolSet, ToolSpec};
use crate::sessions::registry::SessionRegistry;

const SHOW_EVENT: &str = "artifacts://show";
const LIST: &str = "artifact_list";
const SHOW: &str = "artifact_show";
const NO_REGISTRY: &str = "Der Verwalter startet noch";
const SIDE_FORBIDDEN: &str = "Eine Nebenfrage darf Artefakte nur auflisten.";

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
                              mit Titel, Typ, letzter Änderung und der Session, die sie anlegte; \
                              gezeigte Dateien aus eingetragenen Ordnern als „Extern“ mit ihrem \
                              absoluten Pfad.",
                input_schema: json!({
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false,
                }),
            },
            ToolSpec {
                name: SHOW,
                description: "Wählt ein Artefakt im Reiter „Artefakte“ des Verwalters aus; sieht \
                              der Benutzer diese Session, springt die Ansicht dorthin. Auch eine \
                              Datei aus einem in den Einstellungen eingetragenen Ordner (etwa dem \
                              Vault) geht, mit absolutem Pfad; sie erscheint nur lesend in der \
                              Gruppe „Extern“. Ein Pfad außerhalb dieser Ordner wird abgelehnt.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "file": {
                            "type": "string",
                            "description": "Name einer Datei oder eines Ordners direkt in .artefakte, \
                                            oder der absolute Pfad (auch ~/…) einer Datei in einem \
                                            eingetragenen Ordner",
                        },
                    },
                    "required": ["file"],
                    "additionalProperties": false,
                }),
            },
        ]
    }

    fn call(&self, call: &ToolCall) -> ToolResult {
        // Vor der Registry: eine Nebenfrage darf nur lesen, `artifact_show` wechselt den Reiter der
        // Oberfläche. Ihr Prozess hat dieselben Argumente wie der Hauptprozess (ADR 044).
        if call.kind == ProcessKind::Side && !side_may_call(call.name) {
            return ToolResult::Err(SIDE_FORBIDDEN.to_owned());
        }
        let Some(registry) = call.app.try_state::<SessionRegistry>() else {
            return ToolResult::Err(NO_REGISTRY.to_owned());
        };
        let (dir, items) = match artifacts_of(&registry, call.session_id) {
            Ok(found) => found,
            Err(error) => return ToolResult::Err(error),
        };
        match call.name {
            LIST => list(call.session_id, &dir, &items),
            SHOW => show(call, &registry, &dir, &items),
            other => ToolResult::Err(format!("Unbekanntes Werkzeug: {other}")),
        }
    }
}

fn now_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |elapsed| elapsed.as_secs_f64() * 1000.0)
}

/// Was eine Nebenfrage benutzen darf: nur lesen.
pub(crate) fn side_may_call(name: &str) -> bool {
    name == LIST
}

fn artifacts_of(
    registry: &SessionRegistry,
    session_id: &str,
) -> Result<(String, Vec<Artifact>), String> {
    let (workspace, owners) = registry
        .artifact_scope(session_id)
        .map_err(|error| error.to_string())?;
    let dir = super::dir(&workspace);
    let mut items = super::list(&dir, &owners);
    super::extern_files::append_to(&dir, &registry.artifact_folders(), &mut items);
    Ok((dir.to_string_lossy().into_owned(), items))
}

fn list(session_id: &str, dir: &str, items: &[Artifact]) -> ToolResult {
    let lines: Vec<String> = items
        .iter()
        .map(|item: &Artifact| {
            // Eine Datei aus einem eingetragenen Ordner unter ihrem echten Pfad: `.extern/…` gibt es
            // auf der Platte nicht (ADR 026 X3).
            let (file, owner) = match (&item.external_path, item.session_number, &item.session_name)
            {
                (Some(path), _, _) => (path.as_str(), "Extern".to_owned()),
                (None, Some(number), Some(name)) => {
                    (item.file.as_str(), format!("#{number} {name}"))
                }
                _ => (item.file.as_str(), "ohne Zuordnung".to_owned()),
            };
            format!("„{}“ ({file}, {}, {owner})", item.title, item.kind.as_str())
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
                "externalPath": item.external_path,
            })
        })
        .collect();
    ToolResult::Ok {
        text,
        structured: Some(json!({ "dir": dir, "items": structured })),
    }
}

/// Nur ein Name aus der Liste oder ein Pfad, den `extern_files::show` prüft: so erreicht kein Pfad aus
/// dem Agenten die Oberfläche ungeprüft.
fn show(call: &ToolCall, registry: &SessionRegistry, dir: &str, items: &[Artifact]) -> ToolResult {
    let Some(file) = call.arguments.get("file").and_then(Value::as_str) else {
        return ToolResult::Err("Parameter file fehlt".to_owned());
    };
    let shown: Artifact;
    let item = if super::extern_files::is_path(file) {
        match super::extern_files::show(
            Path::new(dir),
            file,
            &registry.artifact_folders(),
            call.session_id,
            now_ms(),
        ) {
            Ok(artifact) => {
                shown = artifact;
                &shown
            }
            Err(error) => return ToolResult::Err(error),
        }
    } else {
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
        item
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_side_question_may_list_artifacts_but_not_show_one() {
        assert!(side_may_call(LIST));
        assert!(!side_may_call(SHOW));
        assert!(!side_may_call("artifact_unknown"));
    }

    #[test]
    fn artifact_list_names_an_external_file_by_its_real_path_as_extern() {
        let own = super::super::file_item(
            Path::new("/w/.artefakte/bericht.md"),
            "bericht.md".to_owned(),
            None,
        );
        let mut external =
            super::super::file_item(Path::new("/vault/notiz.md"), "notiz.md".to_owned(), None);
        external.file = ".extern/abc/notiz.md".to_owned();
        external.external_path = Some("/vault/notiz.md".to_owned());
        let ToolResult::Ok { text, structured } = list("s1", "/w/.artefakte", &[own, external])
        else {
            panic!("Liste");
        };
        assert_eq!(
            text,
            "„bericht“ (bericht.md, markdown, ohne Zuordnung)\n„notiz“ (/vault/notiz.md, markdown, Extern)"
        );
        let items = &structured.expect("structured")["items"];
        assert_eq!(items[0]["externalPath"], Value::Null);
        assert_eq!(items[1]["file"], ".extern/abc/notiz.md");
        assert_eq!(items[1]["externalPath"], "/vault/notiz.md");
    }
}
