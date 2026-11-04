//! Der Verlauf einer Session als Textdatei für die Vorhaben-Retro: wie das TL;DR-Transkript,
//! dazu die Werkzeug-Aufrufe, die schiefgingen — dort stecken die Korrekturen des Nutzers. Eine
//! Hook-Sperre steht als solche im Verlauf, nicht als Fehlschlag (ADR 053).
use super::traces::{HookBlock, RuleTraces, skills_line};
use crate::agents::event::{ChatEntry, ModelId, TodoItem, ToolState};
use crate::sessions::model::SessionStatus;
use crate::skills::model::SkillRef;
use crate::tldr::transcript::{Block, entry_block, fit_with_flag, status_label, todo_block};

/// Der Skill, dessen Aufruf eine Session als Retro-Session ausweist.
const RETRO_SKILL: &str = "session-review";

/// Das Transkript und ob es gekürzt wurde. Werkzeug-Aufrufe erscheinen nur, wenn sie
/// fehlschlugen oder abgebrochen wurden.
pub fn retro_transcript(entries: &[ChatEntry], traces: &RuleTraces) -> (String, bool) {
    let mut blocks: Vec<Block> = entries
        .iter()
        .filter_map(|entry: &ChatEntry| retro_block(entry, traces))
        .collect();
    let last_todos: Option<&Vec<TodoItem>> =
        entries
            .iter()
            .rev()
            .find_map(|entry: &ChatEntry| match entry {
                ChatEntry::Todos { items, .. } => Some(items),
                _ => None,
            });
    if let Some(items) = last_todos {
        blocks.push(Block {
            text: todo_block(items),
            is_user: false,
        });
    }
    fit_with_flag(blocks)
}

fn retro_block(entry: &ChatEntry, traces: &RuleTraces) -> Option<Block> {
    let ChatEntry::Tool {
        tool_use_id,
        tool,
        target,
        state,
        ..
    } = entry
    else {
        return entry_block(entry);
    };
    if let Some(HookBlock { reason, .. }) = traces.block_reason(tool_use_id) {
        return Some(Block {
            text: format!("Hook-Sperre: {tool} {target} — {reason}"),
            is_user: false,
        });
    }
    let label = match state {
        ToolState::Failed => "Werkzeug fehlgeschlagen",
        ToolState::Interrupted => "Werkzeug abgebrochen",
        ToolState::Running | ToolState::Done => return None,
    };
    Some(Block {
        text: format!("{label}: {tool} {target}"),
        is_user: false,
    })
}

/// Alle Werkzeug-Aufrufe, davon fehlgeschlagen, davon abgebrochen.
pub fn tool_counts(entries: &[ChatEntry]) -> (u32, u32, u32) {
    let (mut all, mut failed, mut interrupted) = (0, 0, 0);
    for entry in entries {
        if let ChatEntry::Tool { state, .. } = entry {
            all += 1;
            match state {
                ToolState::Failed => failed += 1,
                ToolState::Interrupted => interrupted += 1,
                ToolState::Running | ToolState::Done => {}
            }
        }
    }
    (all, failed, interrupted)
}

/// Ob die erste Nachricht der Session den Skill `session-review` aufruft — solche Sessions gehören
/// zu einer früheren Retro und gehen nicht in die nächste ein. Beginnt sie mit der Nachricht einer
/// anderen Session, ist sie keine Retro-Session.
pub fn is_retro_session(entries: &[ChatEntry]) -> bool {
    let first_message = entries.iter().find(|entry: &&ChatEntry| {
        matches!(
            entry,
            ChatEntry::User { .. } | ChatEntry::SessionMessage { .. }
        )
    });
    matches!(
        first_message,
        Some(ChatEntry::User { skill: Some(SkillRef { name, .. }), .. }) if name == RETRO_SKILL
    )
}

/// Der Inhalt von `session-<N>.md`.
pub fn session_file(
    number: u32,
    name: &str,
    status: SessionStatus,
    model: ModelId,
    entries: &[ChatEntry],
    traces: &RuleTraces,
) -> String {
    let (transcript, is_truncated) = retro_transcript(entries, traces);
    let (tools, failed, interrupted) = tool_counts(entries);
    format!(
        "# Session #{number} – {name}\n\
         Status: {}\n\
         Modell: {}\n\
         Werkzeug-Aufrufe: {tools}, davon fehlgeschlagen: {failed}, abgebrochen: {interrupted}\n\
         Hook-Sperren: {}\n\
         Skills: {}\n\
         Gekürzt: {}\n\n\
         {transcript}\n",
        status_label(status),
        model.cli_id(),
        traces.hook_blocks.len(),
        skills_line(traces),
        if is_truncated { "ja" } else { "nein" },
    )
}
