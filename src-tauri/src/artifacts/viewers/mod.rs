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

#[cfg(test)]
mod tests {
    use super::*;

    /// Die Zeilen eines `{ … }`-Blocks hinter `start`, ohne Einzug.
    fn block_after<'a>(text: &'a str, start: &str) -> Vec<&'a str> {
        let from = text.find(start).expect("Block steht in der Seite") + start.len();
        let body = &text[from..];
        body[..body.find('}').expect("Block endet")]
            .lines()
            .map(str::trim)
            .filter(|line: &&str| !line.is_empty())
            .collect()
    }

    #[test]
    fn markdown_and_mermaid_ask_the_shared_scheme_helper() {
        for kind in [ArtifactKind::Markdown, ArtifactKind::Mermaid] {
            let script = page(kind).expect("Betrachter vorhanden");
            assert!(
                script.contains("isDarkScheme()"),
                "{kind:?} fragt die Hilfe nicht"
            );
            assert!(
                !script.contains("matchMedia"),
                "{kind:?} fragt das System selbst"
            );
        }
        assert_eq!(
            COMMON.matches("matchMedia").count(),
            1,
            "die Hilfe fragt das System einmal"
        );
    }

    #[test]
    fn every_viewer_page_sets_data_theme_before_its_own_script_runs() {
        for kind in [
            ArtifactKind::Text,
            ArtifactKind::Markdown,
            ArtifactKind::Mermaid,
            ArtifactKind::Folder,
        ] {
            let html = render(kind, "x").expect("Betrachter vorhanden");
            let sets = html
                .find("dataset.theme = APP_SCHEME")
                .expect("setzt data-theme");
            let script = html
                .rfind(page(kind).expect("Betrachter vorhanden"))
                .expect("Skript steht in der Seite");
            assert!(sets < script, "{kind:?}: data-theme kommt zu spät");
        }
    }

    #[test]
    fn the_scheme_parameter_sets_only_light_or_dark() {
        assert!(COMMON.contains("scheme === 'light' || scheme === 'dark' ? scheme : null"));
    }

    #[test]
    fn the_dark_colors_of_a_chosen_scheme_and_of_the_system_are_the_same() {
        let chosen = block_after(SHELL, ":root[data-theme='dark'] {");
        let system = block_after(
            SHELL,
            "@media (prefers-color-scheme: dark) {\n        :root:not([data-theme='light']) {",
        );
        assert!(chosen.len() > 10, "zu wenige Farben unter data-theme");
        assert_eq!(chosen, system);
    }

    #[test]
    fn the_folder_viewer_hands_the_scheme_to_its_preview() {
        assert!(FOLDER.contains("scheme=${APP_SCHEME}"));
    }
}
