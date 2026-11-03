//! Typen der Hinweise im Verlauf (ADR 033).
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::agents::event::{Mode, ModelId};

/// Ein abgelehnter Werkzeugaufruf — von einer Regel oder vom User.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Denial {
    pub tool: String,
    /// Worauf das Werkzeug zielte, eine Zeile wie bei `ChatEntry::Tool`.
    pub target: String,
}

/// Was ein Hinweis im Verlauf festhält (`ChatEntry::Notice`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Notice {
    /// Claude hat den bisherigen Verlauf durch eine Kurzfassung ersetzt. `trigger` ist `manual`
    /// oder `auto`; ohne `compact_metadata` (Agent „Autark“) fehlen alle drei Angaben.
    Compacted {
        trigger: Option<String>,
        pre_tokens: Option<u32>,
        post_tokens: Option<u32>,
        /// Text der Kurzfassung; kommt erst nach der Trennlinie.
        summary: Option<String>,
    },
    /// Der User hat das Modell gewechselt, obwohl ein Prompt-Cache bestand.
    ModelSwitched {
        from: ModelId,
        to: ModelId,
        /// Token, die die nächste Nachricht neu schreibt.
        recache_tokens: u32,
    },
    /// Claude ist nach einer Markierung durch den Sicherheitsfilter auf ein anderes Modell
    /// ausgewichen oder zum ursprünglichen zurückgekehrt. Die IDs sind `None` bei einer
    /// Kennung, die der Verwalter nicht kennt; `direction` ist `retry`, `sticky` oder `revert`.
    ModelFallback {
        original_model: String,
        original_id: Option<ModelId>,
        fallback_model: String,
        fallback_id: Option<ModelId>,
        direction: String,
        text: Option<String>,
    },
    /// Allgemeiner Hinweis der Kommandozeile.
    Info { text: String },
    /// Die Antwort endete am aufgebrauchten Kontingent (ADR 051). `text` ist die Meldung der
    /// Kommandozeile unverändert; `resets_at` in Millisekunden seit Epoche, `None` wenn unbekannt.
    QuotaExhausted {
        text: String,
        resets_at: Option<f64>,
    },
    /// Der Modus der Session hat sich ohne Zutun des Users geändert, etwa durch einen
    /// angenommenen Plan.
    ModeChanged { from: Mode, to: Mode },
    /// Ende einer Antwort mit abgelehnten Aufrufen oder Kosten.
    TurnSummary {
        denials: Vec<Denial>,
        /// Rechenwert dieser Antwort in US-Dollar, nur in der Betriebsart Claude.
        cost_usd: Option<f64>,
        /// `seq` der Text-Einträge dieser Antwort mit Angaben ohne Fundstelle, in Reihenfolge
        /// (ADR 036); Einträge von vorher haben das Feld nicht.
        #[serde(default)]
        unsourced: Vec<u32>,
    },
}
