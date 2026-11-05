use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::agents::event::{Effort, Mode, ModelId};
use crate::session_tools::model::SelfRelease;

/// `System` folgt der Windows-Einstellung für hell oder dunkel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ColorScheme {
    Dark,
    Light,
    System,
}

/// Wo das Terminal-Dock sitzt (ADR 055): unter der Hauptfläche oder als Spalte rechts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum TerminalPlacement {
    Bottom,
    Right,
}

/// Eine der vier seitlichen Leisten, deren Breite die Oberfläche verschieben lässt (ADR 056).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum PanelId {
    /// Seitenleiste mit den Vorhaben.
    Sidebar,
    /// Dateibaum der Changes.
    ChangesTree,
    /// Liste der Artefakte.
    ArtifactsList,
    /// Hintergrund-Leiste.
    Background,
}

/// Gezogene Breite je Leiste in Pixeln; `None` = Standardbreite der Oberfläche.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PanelWidths {
    pub sidebar: Option<u32>,
    pub changes_tree: Option<u32>,
    pub artifacts_list: Option<u32>,
    pub background: Option<u32>,
}

/// Woher die Modellanfragen der Agenten kommen (ADR 016). `ClaudeCodeLocal` startet dieselbe
/// Claude-Kommandozeile, schickt die Anfragen aber an LM Studio. `Standalone` („Autark“, ADR 017)
/// startet statt der Claude-Kommandozeile den eigenen Agenten des Verwalters. `Codex` (ADR 031)
/// startet den Übersetzer des Verwalters vor `codex app-server`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum OperatingMode {
    Claude,
    ClaudeCodeLocal,
    Standalone,
    Codex,
}

/// Werte, die für die ganze App gelten. Modell, Modus und Denkaufwand gelten für die erste Session
/// eines neuen Vorhabens.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub color_scheme: ColorScheme,
    pub default_model: ModelId,
    pub default_effort: Effort,
    pub default_mode: Mode,
    pub operating_mode: OperatingMode,
    /// Kennung des Modells in LM Studio, z. B. `google/gemma-4-12b-qat`; `None`, solange keins gewählt ist.
    pub local_model: Option<String>,
    /// Modell für neue Vorhaben in der Betriebsart Codex; `None` = Standardmodell von Codex.
    pub default_codex_model: Option<String>,
    /// Ordner mit den Originalen von Skills, Befehlen, Ausgabestilen und `CLAUDE.md`, mit dem die
    /// Umgebung abgeglichen wird (ADR 038); `None`, solange keiner gewählt ist.
    pub master_dir: Option<String>,
    /// Ob der Verlauf Angaben ohne Fundstelle unterstreicht (ADR 036); der Core ermittelt sie immer.
    pub mark_unsourced: bool,
    /// Selbstfreigabe, die ein neues Vorhaben übernimmt; bestehende bleiben unverändert.
    pub default_self_release: SelfRelease,
    pub terminal_placement: TerminalPlacement,
    /// Gezogene Höhe des Terminal-Docks in Pixeln.
    pub terminal_height: u32,
    /// Gezogene Breite des Terminal-Docks in Pixeln.
    pub terminal_width: u32,
    /// Breite je Leiste in Pixeln; `None` = Standardbreite der Oberfläche (ADR 056).
    pub panel_widths: PanelWidths,
}

/// Was die Einstellungsseite zeigt: die Werte und die Ordner, die sie nur anzeigt.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SettingsOverview {
    pub settings: Settings,
    /// Absolut: `<Benutzerordner>\.claude\skills`.
    pub user_skills_dir: String,
    /// Skills des Benutzers ohne Befehle.
    pub user_skill_count: u32,
    /// Absolut: `<Benutzerordner>\.verwalter\workspaces`.
    pub workspaces_dir: String,
}

/// Eine Änderung aus der Einstellungsseite. Modus und Denkaufwand ändern sich gemeinsam, weil sie
/// im selben Menü stehen.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum SettingsChange {
    ColorScheme {
        value: ColorScheme,
    },
    DefaultModel {
        value: ModelId,
    },
    DefaultMode {
        mode: Mode,
        effort: Effort,
    },
    OperatingMode {
        value: OperatingMode,
    },
    LocalModel {
        value: String,
    },
    /// `None` setzt zurück auf das Standardmodell von Codex.
    DefaultCodexModel {
        value: Option<String>,
    },
    /// `None` oder ein leerer Text entfernt den Master-Ordner.
    MasterDir {
        value: Option<String>,
    },
    MarkUnsourced {
        value: bool,
    },
    DefaultSelfRelease {
        value: SelfRelease,
    },
    TerminalPlacement {
        value: TerminalPlacement,
    },
    TerminalHeight {
        value: u32,
    },
    TerminalWidth {
        value: u32,
    },
    /// `None` löscht die gemerkte Breite und stellt die Standardbreite wieder her.
    PanelWidth {
        panel: PanelId,
        value: Option<u32>,
    },
}
