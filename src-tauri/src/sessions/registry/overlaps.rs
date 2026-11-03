//! Sessions, die gerade im selben Ordner schreiben (ADR 037).
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Arc;

use super::{OVERLAP_HINT, ProjectState, Session, SessionRegistry};
use crate::changes::attribution::normalize_path;
use crate::db::session_files;
use crate::error::CommandError;
use crate::sessions::model::{SessionOverlap, SessionStatus};
use crate::worktrees::{RepositoryCheckout, SessionRepository, TicketRoot};

/// Was eine laufende Session zur Rechnung beiträgt, kopiert unter ihrer Sperre.
struct WriterSnapshot {
    id: String,
    number: u32,
    name: String,
    project_name: String,
    repositories: Vec<SessionRepository>,
    ticket_folders: Vec<PathBuf>,
}

impl SessionRegistry {
    /// Je laufende Session, die mit einer anderen im selben Schreib-Ordner arbeitet, ein Eintrag.
    pub fn overlaps(&self) -> Result<Vec<SessionOverlap>, CommandError> {
        self.find_overlaps(None)
    }

    /// Die Lage einer Session, die gleich sendet: sie zählt als laufend, auch wenn sie noch ruht.
    /// Ein Fehler der Abfrage gilt als „keine Überschneidung“ — der Satz an den Agenten ist kein Muss.
    pub fn overlap_of(&self, session_id: &str) -> Option<SessionOverlap> {
        self.find_overlaps(Some(session_id))
            .ok()?
            .into_iter()
            .find(|overlap: &SessionOverlap| overlap.session_id == session_id)
    }

    fn find_overlaps(
        &self,
        about_to_run: Option<&str>,
    ) -> Result<Vec<SessionOverlap>, CommandError> {
        let snapshots = self.writer_snapshots(about_to_run);
        // Erst ohne jede Session-Sperre an die Datenbank.
        let mut folders_of: Vec<BTreeMap<String, String>> = Vec::with_capacity(snapshots.len());
        for snapshot in &snapshots {
            let touched: HashMap<String, f64> = self.database.with(|connection| {
                session_files::load_for(connection, std::slice::from_ref(&snapshot.id))
            })?;
            folders_of.push(write_folders(snapshot, &touched));
        }
        Ok(pair_up(&snapshots, &folders_of))
    }

    /// Eine Session nach der anderen sperren — nie zwei zugleich und nie unter der Map-Sperre.
    fn writer_snapshots(&self, about_to_run: Option<&str>) -> Vec<WriterSnapshot> {
        let sessions: Vec<Arc<Session>> = self.lock_sessions().values().cloned().collect();
        let mut snapshots: Vec<WriterSnapshot> = Vec::new();
        for session in sessions {
            let repositories = session.repositories();
            let copied: Option<(String, Vec<PathBuf>)> = {
                let state = session.lock();
                let is_running = state.is_awaiting_subagent()
                    || matches!(
                        state.status,
                        SessionStatus::Starting | SessionStatus::Running | SessionStatus::Waiting
                    );
                if is_running || about_to_run == Some(session.id.as_str()) {
                    let ticket_folders = ticket_folder_paths(
                        &repositories,
                        &state.ticket_roots,
                        &state.ticket_worktrees,
                    );
                    Some((state.name.clone(), ticket_folders))
                } else {
                    None
                }
            };
            let Some((name, ticket_folders)) = copied else {
                continue;
            };
            let project_name: String = self
                .lock_projects()
                .get(&session.project_id)
                .map(|project: &ProjectState| project.name.clone())
                .unwrap_or_default();
            snapshots.push(WriterSnapshot {
                id: session.id.clone(),
                number: session.number,
                name,
                project_name,
                repositories,
                ticket_folders,
            });
        }
        snapshots.sort_by(|left: &WriterSnapshot, right: &WriterSnapshot| {
            (left.number, &left.id).cmp(&(right.number, &right.id))
        });
        snapshots
    }
}

/// Absolute Pfade der Ticket-Worktrees, die eine Session benutzt hat; unauflösbare entfallen.
fn ticket_folder_paths(
    repositories: &[SessionRepository],
    roots: &[TicketRoot],
    worktrees: &[(u32, String)],
) -> Vec<PathBuf> {
    worktrees
        .iter()
        .filter_map(|(position, folder): &(u32, String)| {
            ticket_folder_path(repositories, roots, *position, folder)
        })
        .collect()
}

/// Wo ein Ticket-Worktree liegt: neben dem angehängten Repository, bei einem inneren Repository
/// darin.
fn ticket_folder_path(
    repositories: &[SessionRepository],
    roots: &[TicketRoot],
    position: u32,
    folder: &str,
) -> Option<PathBuf> {
    let repository = repositories.get(usize::try_from(position).ok()?)?;
    let lowered = folder.to_ascii_lowercase();
    let root = roots.iter().find(|root: &&TicketRoot| {
        root.position == position && lowered.starts_with(&root.prefix)
    })?;
    if root.inner.is_none() {
        Some(repository.repository_path.parent()?.join(folder))
    } else {
        Some(repository.repository_path.join(folder))
    }
}

/// Die Schreib-Ordner einer Session als normalisierter Schlüssel → Anzeige. Ein Haupt-Checkout
/// zählt nur, wenn die Session dort eine Datei geschrieben hat, die in keinem ihrer
/// Ticket-Worktrees liegt (ein innerer Worktree liegt im Haupt-Checkout).
fn write_folders(
    snapshot: &WriterSnapshot,
    touched: &HashMap<String, f64>,
) -> BTreeMap<String, String> {
    let mut folders: BTreeMap<String, String> = BTreeMap::new();
    let ticket_keys: Vec<String> = snapshot
        .ticket_folders
        .iter()
        .map(|folder: &PathBuf| normalize_path(&folder.to_string_lossy()))
        .collect();
    for (folder, key) in snapshot.ticket_folders.iter().zip(&ticket_keys) {
        folders.insert(key.clone(), folder.display().to_string());
    }
    for repository in &snapshot.repositories {
        if matches!(repository.checkout, RepositoryCheckout::AppWorktree { .. }) {
            continue;
        }
        let main_key = normalize_path(&repository.repository_path.to_string_lossy());
        let has_main_write = touched.keys().any(|path: &String| {
            is_inside(path, &main_key)
                && !ticket_keys
                    .iter()
                    .any(|ticket_key: &String| is_inside(path, ticket_key))
        });
        if has_main_write {
            folders.insert(main_key, repository.repository_path.display().to_string());
        }
    }
    folders
}

fn is_inside(path_key: &str, folder_key: &str) -> bool {
    path_key
        .strip_prefix(folder_key)
        .is_some_and(|rest: &str| rest.starts_with('\\'))
}

/// Je Session den ersten Ordner (nach Schlüssel), den mindestens eine andere teilt.
fn pair_up(
    snapshots: &[WriterSnapshot],
    folders_of: &[BTreeMap<String, String>],
) -> Vec<SessionOverlap> {
    // Ordner-Schlüssel → Indizes der Sessions, aufsteigend nach Nummer (Reihenfolge von `snapshots`).
    let mut sharing: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (index, folders) in folders_of.iter().enumerate() {
        for key in folders.keys() {
            sharing.entry(key.as_str()).or_default().push(index);
        }
    }
    let mut overlaps: Vec<SessionOverlap> = Vec::new();
    for (index, snapshot) in snapshots.iter().enumerate() {
        let shared = folders_of[index]
            .iter()
            .find_map(|(key, display): (&String, &String)| {
                let group = sharing.get(key.as_str())?;
                (group.len() > 1).then_some((key, display, group))
            });
        let Some((key, display, group)) = shared else {
            continue;
        };
        let Some(&other_index) = group.iter().find(|member: &&usize| **member != index) else {
            continue;
        };
        let other = &snapshots[other_index];
        let mut ids: Vec<&str> = group
            .iter()
            .map(|member: &usize| snapshots[*member].id.as_str())
            .collect();
        ids.sort_unstable();
        overlaps.push(SessionOverlap {
            session_id: snapshot.id.clone(),
            folder: display.clone(),
            other_session_id: other.id.clone(),
            other_number: other.number,
            other_name: other.name.clone(),
            other_project_name: other.project_name.clone(),
            more: u32::try_from(group.len() - 2).unwrap_or(u32::MAX),
            key: format!("{key}|{}", ids.join(",")),
        });
    }
    overlaps
}

/// Der Satz an den Agenten für eine Überschneidung (`OVERLAP_HINT`).
pub(super) fn overlap_hint(overlap: &SessionOverlap) -> String {
    OVERLAP_HINT
        .replace("{folder}", &overlap.folder)
        .replace("{name}", &overlap.other_name)
}
