//! Fernsteuerung: Eine Instanz übernimmt über das Netz ein Vorhaben einer anderen, gearbeitet wird
//! nur am Host (ADR 059). `access` und `control` sind rein; der Zustand liegt in `RemoteControl`.
pub mod access;
pub mod control;
pub mod intercept;
pub mod model;

use std::sync::{Mutex, PoisonError};

use crate::db::{Database, remote as remote_rows};
use crate::error::CommandError;
use control::{Control, Refused};

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

#[cfg(test)]
mod tests;
