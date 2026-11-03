//! Betrachter-Seiten für Artefakte, die keine HTML-Seite sind (ADR 047). Der Artefakt-Server
//! liefert sie unter `<datei>?view=1` aus; die Seite holt die Datei selbst per `fetch` aus
//! demselben Ordner und läuft im selben abgeschotteten Rahmen wie eine HTML-Seite.
use super::model::ArtifactKind;

const SHELL: &str = include_str!("shell.html");
const COMMON: &str = include_str!("common.js");
const IMAGE: &str = include_str!("image.js");
const VIDEO: &str = include_str!("video.js");
const AUDIO: &str = include_str!("audio.js");
const TEXT: &str = include_str!("text.js");
const MARKDOWN: &str = include_str!("markdown.js");
const TREE: &str = include_str!("tree.js");
const JSON: &str = include_str!("json.js");
const YAML: &str = include_str!("yaml.js");
const CONFIG: &str = include_str!("config.js");
const XML: &str = include_str!("xml.js");
const CSV: &str = include_str!("csv.js");
const PDF: &str = include_str!("pdf.js");
const MERMAID: &str = include_str!("mermaid.js");
const FOLDER: &str = include_str!("folder.js");

/// Das Skript des Betrachters für den Typ; `None` → die Oberfläche zeigt die Rest-Karte. Ein neuer
/// Betrachter ist eine `.js` daneben und eine Zeile hier.
pub fn page(kind: ArtifactKind) -> Option<&'static str> {
    match kind {
        ArtifactKind::Json | ArtifactKind::JsonLines => Some(JSON),
        ArtifactKind::Yaml => Some(YAML),
        ArtifactKind::Config => Some(CONFIG),
        ArtifactKind::Xml => Some(XML),
        ArtifactKind::Csv => Some(CSV),
        ArtifactKind::Image => Some(IMAGE),
        ArtifactKind::Video => Some(VIDEO),
        ArtifactKind::Audio => Some(AUDIO),
        ArtifactKind::Text | ArtifactKind::Code | ArtifactKind::Log | ArtifactKind::Diff => {
            Some(TEXT)
        }
        ArtifactKind::Markdown => Some(MARKDOWN),
        ArtifactKind::Pdf => Some(PDF),
        ArtifactKind::Mermaid => Some(MERMAID),
        ArtifactKind::Folder => Some(FOLDER),
        _ => None,
    }
}

/// Diese Betrachter zeichnen Bäume und bekommen `tree.js` vor ihr eigenes Skript.
fn needs_tree(kind: ArtifactKind) -> bool {
    matches!(
        kind,
        ArtifactKind::Json | ArtifactKind::JsonLines | ArtifactKind::Yaml | ArtifactKind::Xml
    )
}

/// Die fertige Betrachter-Seite für `file`, ohne Kommentar-Brücke (die hängt der Server an).
pub fn render(kind: ArtifactKind, file: &str) -> Option<String> {
    let page_script = page(kind)?;
    let script = if needs_tree(kind) {
        format!("{TREE}\n{page_script}")
    } else {
        page_script.to_owned()
    };
    // Erst die Skripte, zuletzt der Dateiname: so kann kein Name einen Platzhalter tragen,
    // der danach noch ersetzt würde.
    Some(
        SHELL
            .replace("__COMMON__", COMMON)
            .replace("__SCRIPT__", &script)
            .replace("__KIND__", &script_string(kind.as_str()))
            .replace("__FILE__", &script_string(file)),
    )
}

/// Ein JSON-String, der in einem `<script>` stehen darf: `</` beendet sonst das Element.
fn script_string(text: &str) -> String {
    serde_json::Value::String(text.to_owned())
        .to_string()
        .replace("</", "<\\/")
}
