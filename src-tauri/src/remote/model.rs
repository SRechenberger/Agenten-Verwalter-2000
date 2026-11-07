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

/// Der Server am Host, wie die Einstellungen ihn zeigen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RemoteServerStatus {
    /// Der Schalter „Gäste erlauben“; ohne ihn lauscht nichts.
    pub enabled: bool,
    pub port: u16,
    pub listening: bool,
    /// Adressen im LAN oder VPN, unter denen der Host erreichbar ist; leer, solange er nicht lauscht.
    pub addresses: Vec<String>,
    /// Warum der Server nicht lauscht, obwohl er an ist, etwa ein belegter Port.
    pub error: Option<String>,
    /// SHA-256 des Zertifikats, hex; `None`, solange es keins gibt.
    pub fingerprint: Option<String>,
}

/// Ein gekoppeltes, nicht gesperrtes Gerät am Host.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RemoteDevice {
    pub id: String,
    pub name: String,
    /// Millisekunden seit 1970; `None`, solange es sich nach der Kopplung nicht angemeldet hat.
    pub last_seen_at: Option<f64>,
    pub online: bool,
}

/// Ein Host, mit dem dieser Gast gekoppelt ist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RemoteHostStatus {
    pub host_id: String,
    pub host_name: String,
    /// `<Adresse>:<Port>`.
    pub address: String,
    pub fingerprint: String,
    pub online: bool,
    /// Warum keine Verbindung besteht; `None`, solange alles läuft oder getrennt wurde.
    pub error: Option<String>,
}

/// Eine frische Einladung für einen Gast.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RemoteInvitation {
    /// `verwalter-remote://…`, zum Kopieren.
    pub text: String,
    /// Millisekunden seit 1970.
    pub expires_at: f64,
}
