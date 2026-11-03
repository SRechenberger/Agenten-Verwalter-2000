//! Typen der Session-Werkzeuge über die Tauri-Grenze.
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::agents::event::ModelId;

/// Selbstfreigabe je Vorhaben: welche Werkzeuge ohne Kasten laufen, und welches Modell das TL;DR schreibt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SelfRelease {
    pub messages: bool,
    pub tldr: bool,
    pub open_session: bool,
    pub tldr_model: ModelId,
}

impl SelfRelease {
    /// Alles aus, Modell Haiku: der Standard für neue Vorhaben, bis die Einstellungen etwas anderes sagen.
    pub const OFF: SelfRelease = SelfRelease {
        messages: false,
        tldr: false,
        open_session: false,
        tldr_model: ModelId::Haiku,
    };
}
