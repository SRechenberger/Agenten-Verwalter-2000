//! Welcher Befehl wem erlaubt ist (ADR 059): die Klasse je Command, die IDs eines Aufrufs und die
//! eine Prüfung, die für die Oberfläche des Hosts und für Gäste dieselbe ist. Kein Tauri.
use serde_json::Value;

use super::control::{Control, Holder};
use super::model::DeviceRef;
use crate::error::CommandError;

/// Die Klassen A bis G der ADR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// A: Gast nur als Inhaber, Host-Oberfläche immer.
    HostRead,
    /// B: je Oberfläche; die ID ist freiwillig.
    PerSurface,
    /// C: Gast nur als Inhaber, Host-Oberfläche gesperrt, solange ein Gast steuert.
    HostWrite,
    /// D: nie vom Gast, Host-Oberfläche gesperrt, solange ein Gast steuert.
    NeverGuestLocked,
    /// E: nie vom Gast, Host-Oberfläche immer.
    NeverGuestOpen,
    /// F: ohne ID, über die Zusammenführung oder eigene Wege erreichbar.
    Reachable,
    /// G: ohne ID, nur lokal.
    LocalOnly,
}

/// Woran ein Command sein Vorhaben erkennt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdKind {
    Session,
    Project,
    Implementation,
    Terminal,
}

/// Die Erlaubnisliste: Ein Command, der hier fehlt, ist für Gäste nicht erreichbar, und der Test
/// über `lib.rs` wird rot, bis ihn jemand einträgt.
const TABLE: &[(Class, Option<IdKind>, &[&str])] = &[
    (
        Class::HostRead,
        Some(IdKind::Session),
        &[
            "artifacts_list",
            "background_load",
            "background_output",
            "scratchpad_list",
            "scratchpad_read",
            "changes_load",
            "changes_file_diff",
            "changes_file_lines",
            "chat_history",
            "context_load",
            "context_refresh",
            "git_status",
            "git_branches",
            "git_stash_list",
            "git_log",
            "mcp_load",
            "mcp_refresh",
            "session_log",
            "side_load",
            "skill_list_for_session",
            "tldr_session_load",
            "tool_detail",
        ],
    ),
    (
        Class::HostRead,
        Some(IdKind::Project),
        &["terminal_list", "tldr_project_load"],
    ),
    (
        Class::HostRead,
        Some(IdKind::Implementation),
        &["implementation_report"],
    ),
    (
        Class::HostRead,
        Some(IdKind::Terminal),
        &["terminal_attach", "terminal_detach"],
    ),
    (
        Class::PerSurface,
        Some(IdKind::Session),
        &["session_set_viewed"],
    ),
    (
        Class::HostWrite,
        Some(IdKind::Session),
        &[
            "background_stop",
            "chat_send",
            "chat_answer",
            "git_commit",
            "git_push",
            "git_pull",
            "git_fetch",
            "git_switch",
            "git_create_branch",
            "git_abort_operation",
            "git_discard",
            "git_stash_push",
            "git_stash_pop",
            "git_pull_rebase",
            "git_merge",
            "git_delete_branch",
            "git_create_ticket_worktree",
            "git_suggest_message",
            "mcp_reconnect",
            "mcp_toggle",
            "session_pause",
            "session_resume",
            "session_cancel",
            "session_rename",
            "session_restart",
            "session_set_model",
            "session_set_auto_resume",
            "session_set_mode",
            "session_set_effort",
            "session_set_codex_model",
            "side_ask",
            "side_dismiss",
            "tldr_session_create",
            "tldr_handoff_brief",
            "tldr_set_carry",
        ],
    ),
    (
        Class::HostWrite,
        Some(IdKind::Project),
        &[
            "project_rename",
            "project_set_self_release",
            "retro_run",
            "session_create_in_project",
            "terminal_open",
            "tldr_project_create",
        ],
    ),
    (
        Class::HostWrite,
        Some(IdKind::Implementation),
        &[
            "implementation_new_inquiry",
            "implementation_rename",
            "implementation_set_auto_start",
            "implementation_set_stop_after_current",
            "implementation_start_station",
            "implementation_cancel_countdown",
            "implementation_apply_change",
            "implementation_complete_station",
        ],
    ),
    (
        Class::HostWrite,
        Some(IdKind::Terminal),
        &["terminal_write", "terminal_resize", "terminal_close"],
    ),
    (
        Class::NeverGuestLocked,
        Some(IdKind::Session),
        &["mcp_authenticate", "session_set_chrome", "session_delete"],
    ),
    (
        Class::NeverGuestLocked,
        Some(IdKind::Project),
        &[
            "project_add_repository",
            "project_archive",
            "project_restore",
        ],
    ),
    (
        Class::NeverGuestOpen,
        Some(IdKind::Session),
        &[
            "artifact_open_default",
            "artifact_reveal",
            "file_link_open",
            "git_open",
        ],
    ),
    (
        Class::NeverGuestOpen,
        Some(IdKind::Project),
        &["project_set_group", "remote_share_set"],
    ),
    (
        Class::Reachable,
        None,
        &[
            "attachment_add_bytes",
            "usage_load",
            "usage_refresh",
            "codex_models",
            "settings_local_models",
            "skill_uses",
            "project_list",
            "session_list",
            "implementation_list",
            "session_overlaps",
        ],
    ),
    (
        Class::LocalOnly,
        None,
        &[
            "app_info",
            "account_load",
            "account_login",
            "app_update_load",
            "app_update_install",
            "cli_update_load",
            "cli_update_run",
            "codex_account_load",
            "codex_account_login",
            "environment_status",
            "environment_mirror",
            "repository_list",
            "repository_add",
            "repository_remove",
            "settings_load",
            "settings_update",
            "archive_search",
            "project_create",
            "project_group_list",
            "project_group_create",
            "project_group_rename",
            "project_group_delete",
            "project_group_set_collapsed",
            "skill_list_for_repositories",
            "attachment_add_files",
            "attachment_discard",
            "voice_model_status",
            "voice_model_download",
            "voice_model_cancel_download",
            "voice_start",
            "voice_stop",
            "voice_cancel",
            "remote_server_status",
            "remote_server_set",
            "remote_invitation_create",
            "remote_devices",
            "remote_device_revoke",
            "remote_connect",
            "remote_hosts",
            "remote_disconnect",
            // Am Gast lokal; am Host beantwortet der Server dieselben Namen vor der Prüfung.
            "remote_take",
            "remote_give_back",
        ],
    ),
];

fn entry_of(command: &str) -> Option<(Class, Option<IdKind>)> {
    TABLE
        .iter()
        .find(|(_, _, commands)| commands.contains(&command))
        .map(|(class, id_kind, _)| (*class, *id_kind))
}

pub fn class_of(command: &str) -> Option<Class> {
    entry_of(command).map(|(class, _)| class)
}

/// Ob der Gast einen Aufruf mit der ID eines entfernten Vorhabens an dessen Host gibt. Was dort
/// nie erlaubt ist, lehnt der Host ab; nur `project_set_group` bleibt lokal (Vertrag), was ohne
/// ID auskommt, hat eigene Wege, und `session_set_viewed` geht an beide Seiten (der Command).
pub fn routes_to_host(command: &str) -> bool {
    command != "project_set_group"
        && matches!(
            class_of(command),
            Some(
                Class::HostRead
                    | Class::HostWrite
                    | Class::NeverGuestLocked
                    | Class::NeverGuestOpen
            )
        )
}

/// Eine ID aus den Argumenten eines Aufrufs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetId {
    pub kind: IdKind,
    pub value: String,
}

/// Alle IDs, die der Aufruf nennt: `sessionId` und `projectId` immer, damit eine fremde ID nicht
/// neben einer eigenen durchrutscht, und `id` nur dort, wo sie die Umsetzung oder das Terminal ist.
/// Ein Wert, der kein Text ist (`sessionId: null`), zählt als nicht genannt.
pub fn ids_of(command: &str, payload: &Value) -> Vec<TargetId> {
    let mut ids: Vec<TargetId> = Vec::new();
    let mut read = |key: &str, kind: IdKind| {
        if let Some(value) = payload.get(key).and_then(Value::as_str) {
            ids.push(TargetId {
                kind,
                value: value.to_owned(),
            });
        }
    };
    read("sessionId", IdKind::Session);
    read("projectId", IdKind::Project);
    if let Some((_, Some(kind @ (IdKind::Implementation | IdKind::Terminal)))) = entry_of(command) {
        read("id", kind);
    }
    ids
}

/// Eine ID samt dem Vorhaben, zu dem sie führt; `None`, wenn es sie hier nicht gibt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedId {
    pub target: TargetId,
    pub project: Option<String>,
}

pub fn resolve(
    ids: Vec<TargetId>,
    project_of: impl Fn(&TargetId) -> Option<String>,
) -> Vec<ResolvedId> {
    ids.into_iter()
        .map(|target: TargetId| ResolvedId {
            project: project_of(&target),
            target,
        })
        .collect()
}

/// Wer den Befehl auslöst.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Actor<'a> {
    HostSurface,
    /// Die ID des gekoppelten Geräts.
    Guest(&'a str),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Denied {
    #[error("Dieser Befehl ist für Gäste nicht erlaubt.")]
    NotAllowed,
    #[error("Diesem Befehl fehlt die ID seines Vorhabens.")]
    MissingId,
    #[error("Diese ID gehört zu keinem Vorhaben, das dieses Gerät steuert: {0}")]
    ForeignId(String),
    #[error("Gesteuert von {}", .0.name)]
    ControlledBy(DeviceRef),
}

impl From<Denied> for CommandError {
    fn from(denied: Denied) -> Self {
        match denied {
            Denied::ControlledBy(device) => CommandError::ControlledBy(device.name),
            other => CommandError::Internal(other.to_string()),
        }
    }
}

/// Die Prüfung vor jedem Command. Für die Oberfläche des Hosts sperrt sie nur, wenn ein Gast
/// Inhaber ist; was es nicht gibt, meldet danach der Command selbst wie bisher.
pub fn check(
    command: &str,
    ids: &[ResolvedId],
    actor: Actor<'_>,
    control: &Control,
) -> Result<(), Denied> {
    match actor {
        Actor::HostSurface => check_host_surface(command, ids, control),
        Actor::Guest(device_id) => check_guest(command, ids, device_id, control),
    }
}

fn check_host_surface(command: &str, ids: &[ResolvedId], control: &Control) -> Result<(), Denied> {
    if !matches!(
        class_of(command),
        Some(Class::HostWrite | Class::NeverGuestLocked)
    ) {
        return Ok(());
    }
    let holder: Option<&Holder> = ids
        .iter()
        .filter_map(|id: &ResolvedId| id.project.as_deref())
        .find_map(|project_id: &str| control.holder(project_id));
    match holder {
        Some(holder) => Err(Denied::ControlledBy(holder.device.clone())),
        None => Ok(()),
    }
}

fn check_guest(
    command: &str,
    ids: &[ResolvedId],
    device_id: &str,
    control: &Control,
) -> Result<(), Denied> {
    let (class, required) = entry_of(command).ok_or(Denied::NotAllowed)?;
    if matches!(
        class,
        Class::NeverGuestLocked | Class::NeverGuestOpen | Class::LocalOnly
    ) {
        return Err(Denied::NotAllowed);
    }
    if let (Class::HostRead | Class::HostWrite, Some(required)) = (class, required)
        && !ids.iter().any(|id: &ResolvedId| id.target.kind == required)
    {
        return Err(Denied::MissingId);
    }
    for id in ids {
        let held = id
            .project
            .as_deref()
            .and_then(|project_id: &str| control.holder(project_id))
            .is_some_and(|holder: &Holder| holder.device.id == device_id);
        if !held {
            return Err(Denied::ForeignId(id.target.value.clone()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
