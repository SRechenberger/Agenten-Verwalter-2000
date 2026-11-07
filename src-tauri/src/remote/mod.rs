//! Fernsteuerung: Eine Instanz übernimmt über das Netz ein Vorhaben einer anderen, gearbeitet wird
//! nur am Host (ADR 059). `access` und `control` sind rein; der Zustand liegt in `RemoteControl`.
pub mod access;
pub mod client;
pub mod control;
pub mod dispatch;
pub mod forward;
pub mod guest;
pub mod intercept;
pub mod model;
pub mod peer;
pub mod server;
pub mod tls;
pub mod wire;

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::sync::{Mutex, PoisonError};

use crate::db::{Database, remote as remote_rows};
use crate::error::CommandError;
use control::{Control, Refused};

/// Ohne Inhalt, am Gast und am Host: Server, Geräte, Hosts oder Verbindungen haben sich geändert.
pub const CHANGED_EVENT: &str = "remote://changed";
/// Ordner unter `~/.verwalter` für Zertifikat und Hosts.
pub const DIR_NAME: &str = "remote";
pub const HOST_KEY_FILE: &str = "host-key.pem";
pub const HOSTS_FILE: &str = "hosts.json";

/// Das Steuerrecht als Tauri-State. Die Sperre ist die innerste der App: Unter `with` wird keine
/// andere genommen, also darf `with` unter jeder anderen laufen.
#[derive(Debug, Default)]
pub struct RemoteControl {
    control: Mutex<Control>,
}

impl RemoteControl {
    /// Der Stand nach dem Start: Freigaben und Sperren aus der Datenbank, kein Inhaber.
    pub fn restore(database: &Database) -> Result<RemoteControl, CommandError> {
        let (shared, revoked) = database.with(|connection| {
            Ok((
                remote_rows::shared_project_ids(connection)?,
                remote_rows::revoked_device_ids(connection)?,
            ))
        })?;
        Ok(RemoteControl {
            control: Mutex::new(Control::restore(shared, revoked)),
        })
    }

    pub fn with<R>(&self, work: impl FnOnce(&mut Control) -> R) -> R {
        let mut control = self.control.lock().unwrap_or_else(PoisonError::into_inner);
        work(&mut control)
    }
}

impl From<Refused> for CommandError {
    fn from(refused: Refused) -> Self {
        match refused {
            Refused::HeldBy(device) => CommandError::ControlledBy(device.name),
            Refused::NotShared => {
                CommandError::Internal("Dieses Vorhaben ist nicht freigegeben.".to_owned())
            }
            Refused::DeviceRevoked => {
                CommandError::Internal("Dieses Gerät ist gesperrt.".to_owned())
            }
            Refused::NotHolder => {
                CommandError::Internal("Dieses Gerät steuert das Vorhaben nicht.".to_owned())
            }
        }
    }
}

/// Der Name dieses Rechners, wie ihn Gäste als Host und Hosts als Gerät sehen.
pub fn host_name() -> String {
    std::env::var("COMPUTERNAME")
        .ok()
        .or_else(|| fs::read_to_string("/etc/hostname").ok())
        .or_else(|| std::env::var("HOSTNAME").ok())
        .map(|name: String| name.trim().to_owned())
        .filter(|name: &String| !name.is_empty())
        .unwrap_or_else(|| "Host".to_owned())
}

/// Schreibt eine Datei mit Geheimnissen über eine Zwischendatei; unter Linux und macOS nur für den
/// Besitzer lesbar (0600).
pub(crate) fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("tmp");
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temporary)?;
    // Gab es die Zwischendatei schon, gilt `mode` nicht: die Rechte ausdrücklich setzen.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&temporary, path)
}

#[cfg(test)]
mod tests;
