//! Steuerrecht je Vorhaben als reiner Zustandsautomat (ADR 059): `shared` liegt dauerhaft in SQLite,
//! der Inhaber nur hier im Speicher. Kein Tauri, keine Uhr; die Zeit kommt als Argument, damit die
//! Tests jede Zeile der Tabelle aus der ADR durchspielen können.
use std::collections::{HashMap, HashSet};

use super::model::{DeviceRef, ProjectAccess};

/// Nach dieser Zeit ohne Verbindung fällt das Vorhaben an den Host zurück; die Freigabe bleibt.
pub const LOST_LIMIT_MS: f64 = 5.0 * 60.0 * 1000.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Link {
    Connected,
    /// `since`: Millisekunden seit 1970, wie `now_ms` in der Registry.
    Lost {
        since: f64,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Holder {
    pub device: DeviceRef,
    pub link: Link,
}

/// Warum ein Übergang nicht stattfand; der Zustand bleibt dann unverändert.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    NotShared,
    HeldBy(DeviceRef),
    DeviceRevoked,
    NotHolder,
}

/// Invariante: Jedes Vorhaben in `holders` steht auch in `shared`.
#[derive(Debug, Default)]
pub struct Control {
    shared: HashSet<String>,
    holders: HashMap<String, Holder>,
    revoked: HashSet<String>,
}

impl Control {
    /// Stand nach einem Neustart des Hosts: Freigaben und Sperren aus SQLite, kein Inhaber.
    pub fn restore(
        shared: impl IntoIterator<Item = String>,
        revoked: impl IntoIterator<Item = String>,
    ) -> Control {
        Control {
            shared: shared.into_iter().collect(),
            holders: HashMap::new(),
            revoked: revoked.into_iter().collect(),
        }
    }

    pub fn is_shared(&self, project_id: &str) -> bool {
        self.shared.contains(project_id)
    }

    pub fn holder(&self, project_id: &str) -> Option<&Holder> {
        self.holders.get(project_id)
    }

    pub fn has_holder(&self) -> bool {
        !self.holders.is_empty()
    }

    /// Was `ProjectSummary.access` am Host zeigt.
    pub fn access(&self, project_id: &str) -> ProjectAccess {
        if self.is_shared(project_id) {
            ProjectAccess::Shared {
                holder: self
                    .holder(project_id)
                    .map(|holder: &Holder| holder.device.clone()),
            }
        } else {
            ProjectAccess::Own
        }
    }

    /// Nur ein Angebot: Der Host arbeitet weiter, bis ein Gast übernimmt.
    pub fn share(&mut self, project_id: &str) {
        self.shared.insert(project_id.to_owned());
    }

    /// Das Herauswerfen: wirkt sofort und ohne Netz.
    pub fn unshare(&mut self, project_id: &str) {
        self.shared.remove(project_id);
        self.holders.remove(project_id);
    }

    /// Dasselbe Gerät darf erneut übernehmen, das zählt dann als neu verbunden.
    pub fn take(&mut self, project_id: &str, device: DeviceRef) -> Result<(), Refused> {
        if self.revoked.contains(&device.id) {
            return Err(Refused::DeviceRevoked);
        }
        if !self.is_shared(project_id) {
            return Err(Refused::NotShared);
        }
        if let Some(holder) = self.holder(project_id)
            && holder.device.id != device.id
        {
            return Err(Refused::HeldBy(holder.device.clone()));
        }
        self.holders.insert(
            project_id.to_owned(),
            Holder {
                device,
                link: Link::Connected,
            },
        );
        Ok(())
    }

    /// Die Freigabe bleibt.
    pub fn give_back(&mut self, project_id: &str, device_id: &str) -> Result<(), Refused> {
        match self.holder(project_id) {
            Some(holder) if holder.device.id == device_id => {
                self.holders.remove(project_id);
                Ok(())
            }
            _ => Err(Refused::NotHolder),
        }
    }

    /// Eine zweite Meldung verschiebt `since` nicht, sonst liefe die Frist nie ab. Liefert die
    /// betroffenen Vorhaben.
    pub fn connection_lost(&mut self, device_id: &str, now: f64) -> Vec<String> {
        self.update_links(device_id, |link: Link| match link {
            Link::Connected => Link::Lost { since: now },
            lost @ Link::Lost { .. } => lost,
        })
    }

    /// Das Gerät hält seine Vorhaben ohne neues `take`. Liefert die betroffenen Vorhaben.
    pub fn reconnected(&mut self, device_id: &str) -> Vec<String> {
        self.update_links(device_id, |_: Link| Link::Connected)
    }

    fn update_links(&mut self, device_id: &str, next: impl Fn(Link) -> Link) -> Vec<String> {
        let mut changed: Vec<String> = Vec::new();
        for (project_id, holder) in &mut self.holders {
            if holder.device.id == device_id {
                holder.link = next(holder.link);
                changed.push(project_id.clone());
            }
        }
        changed.sort();
        changed
    }

    /// Nimmt jedem Inhaber sein Vorhaben, dessen Verbindung seit `LOST_LIMIT_MS` weg ist. Liefert
    /// die betroffenen Vorhaben.
    pub fn expire(&mut self, now: f64) -> Vec<String> {
        self.drop_holders(|holder: &Holder| match holder.link {
            Link::Connected => false,
            Link::Lost { since } => now - since >= LOST_LIMIT_MS,
        })
    }

    /// Alle Verbindungen sind zu, alle Vorhaben fallen an den Host zurück; die Freigaben bleiben.
    pub fn server_off(&mut self) -> Vec<String> {
        self.drop_holders(|_: &Holder| true)
    }

    /// Das Gerät verliert seine Vorhaben und kann keins mehr übernehmen. Liefert die betroffenen.
    pub fn revoke_device(&mut self, device_id: &str) -> Vec<String> {
        self.revoked.insert(device_id.to_owned());
        self.drop_holders(|holder: &Holder| holder.device.id == device_id)
    }

    fn drop_holders(&mut self, condition: impl Fn(&Holder) -> bool) -> Vec<String> {
        let mut dropped: Vec<String> = self
            .holders
            .iter()
            .filter(|(_, holder)| condition(holder))
            .map(|(project_id, _)| project_id.clone())
            .collect();
        for project_id in &dropped {
            self.holders.remove(project_id);
        }
        dropped.sort();
        dropped
    }

    /// Archivieren geht nur ohne Inhaber. Prüft nur: Die Freigabe endet erst mit `unshare`, wenn die
    /// Datenbank geschrieben ist.
    pub fn may_archive(&self, project_id: &str) -> Result<(), Refused> {
        match self.holder(project_id) {
            Some(holder) => Err(Refused::HeldBy(holder.device.clone())),
            None => Ok(()),
        }
    }

    /// Das Vorhaben ist endgültig gelöscht.
    pub fn forget(&mut self, project_id: &str) {
        self.unshare(project_id);
    }
}

#[cfg(test)]
mod tests;
