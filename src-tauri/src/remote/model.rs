//! Typen der Fernsteuerung über die Tauri-Grenze (ADR 059).
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Ein gekoppeltes Gerät, wie es der Host anzeigt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DeviceRef {
    pub id: String,
    pub name: String,
}

/// Wem ein Vorhaben aus Sicht dieser Instanz gehört.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ProjectAccess {
    #[default]
    Own,
    /// Am Host: für Gäste freigegeben; `holder` ist der Gast, der gerade steuert.
    Shared { holder: Option<DeviceRef> },
    /// Am Gast: Vorhaben eines Hosts; `mine`: dieses Gerät ist Inhaber.
    Remote {
        host_id: String,
        host_name: String,
        mine: bool,
        online: bool,
    },
}
