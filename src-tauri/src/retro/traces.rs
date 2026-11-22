//! Regel-Spuren (ADR 053): was eine Session über die Regeln des Nutzers verrät — geladene Skills
//! und Hook-Sperren. Der Verwalter sammelt hier, der Skill `session-review` urteilt.
use std::collections::BTreeMap;

use rusqlite::Connection;

use crate::agents::event::{ChatEntry, ToolState};
use crate::db::tool_details;
use crate::error::CommandError;
use crate::skills::model::SkillRef;

/// Das Werkzeug, mit dem der Agent einen Skill lädt.
const SKILL_TOOL: &str = "Skill";
/// So beginnt die Ausgabe eines Aufrufs, den ein Hook gesperrt hat: `PreToolUse:Bash hook error: …`.
const HOOK_PREFIX: &str = "PreToolUse:";
const HOOK_MARKER: &str = " hook error: ";
const MAX_REASON_CHARS: usize = 200;

/// Was eine Session über die Regeln des Nutzers verrät: geladene Skills und Hook-Sperren.
pub struct RuleTraces {
    /// Skill-Name → (Aufrufe über das Menü, Aufrufe durch den Agenten), sortiert nach Name.
    pub skills: BTreeMap<String, (u32, u32)>,
    /// In der Reihenfolge des Verlaufs.
    pub hook_blocks: Vec<HookBlock>,
    /// Aufruf-ID eines fehlgeschlagenen Aufrufs ohne Hook-Sperre → eine Zeile Fehlertext, höchstens
    /// 200 Zeichen. Ohne ihn lässt sich nicht sagen, woran etwa eine Stationsmeldung scheiterte.
    pub tool_errors: BTreeMap<String, String>,
}

pub struct HookBlock {
    pub tool_use_id: String,
    pub tool: String,
    pub target: String,
    /// Eine Zeile, höchstens 200 Zeichen.
    pub reason: String,
}

impl RuleTraces {
    pub fn block_reason(&self, tool_use_id: &str) -> Option<&HookBlock> {
        self.hook_blocks
            .iter()
            .find(|block: &&HookBlock| block.tool_use_id == tool_use_id)
    }
}

/// Ein Durchlauf über den Verlauf; die gespeicherte Ausgabe wird nur für fehlgeschlagene
/// Werkzeug-Aufrufe geladen. Ein Aufruf ohne Zeile in `tool_details` gilt als nicht gesperrt.
pub fn collect(
    connection: &Connection,
    session_id: &str,
    entries: &[ChatEntry],
) -> Result<RuleTraces, CommandError> {
    let mut traces = RuleTraces {
        skills: BTreeMap::new(),
        hook_blocks: Vec::new(),
        tool_errors: BTreeMap::new(),
    };
    for entry in entries {
        match entry {
            ChatEntry::User {
                skill: Some(SkillRef { name, .. }),
                ..
            } => traces.skills.entry(name.clone()).or_default().0 += 1,
            ChatEntry::Tool {
                tool,
                target,
                tool_use_id,
                state,
                ..
            } => {
                if tool == SKILL_TOOL {
                    traces.skills.entry(target.clone()).or_default().1 += 1;
                }
                if *state != ToolState::Failed {
                    continue;
                }
                let Some(detail) = tool_details::load(connection, session_id, tool_use_id)? else {
                    continue;
                };
                if let Some(reason) = parse_hook_block(&detail.output) {
                    traces.hook_blocks.push(HookBlock {
                        tool_use_id: tool_use_id.clone(),
                        tool: tool.clone(),
                        target: target.clone(),
                        reason,
                    });
                } else if let Some(text) = one_line(&detail.output) {
                    traces.tool_errors.insert(tool_use_id.clone(), text);
                }
            }
            _ => {}
        }
    }
    Ok(traces)
}

/// Die Ausgabe einer Hook-Sperre im Wortlaut von Claude Code; `parse_hook_block` liest sie zurück.
pub fn hook_block_output(tool: &str, reason: &str) -> String {
    format!("{HOOK_PREFIX}{tool}{HOOK_MARKER}{reason}")
}

/// Der Sperrgrund, wenn die Ausgabe von einem Hook stammt, sonst `None`. Die einzige Stelle, die
/// das Präfix kennt: ändert Claude Code es, fällt die Erkennung hier aus.
pub fn parse_hook_block(output: &str) -> Option<String> {
    let trimmed = output.trim_start();
    if !trimmed.starts_with(HOOK_PREFIX) {
        return None;
    }
    let (_, reason) = trimmed.split_once(HOOK_MARKER)?;
    one_line(reason)
}

/// Der Text auf eine Zeile gebracht und auf `MAX_REASON_CHARS` gekürzt; `None`, wenn er leer ist.
fn one_line(text: &str) -> Option<String> {
    let joined = text.replace(['\r', '\n'], " ");
    let joined = joined.trim();
    if joined.is_empty() {
        return None;
    }
    if joined.chars().count() <= MAX_REASON_CHARS {
        return Some(joined.to_owned());
    }
    let shortened: String = joined.chars().take(MAX_REASON_CHARS).collect();
    Some(format!("{shortened}…"))
}

/// `mode-planning (Agent ×2), project-committing (Menü ×1, Agent ×1)` oder `keine`.
pub fn skills_line(traces: &RuleTraces) -> String {
    if traces.skills.is_empty() {
        return "keine".to_owned();
    }
    let parts: Vec<String> = traces
        .skills
        .iter()
        .map(|(name, (menu, agent)): (&String, &(u32, u32))| {
            let mut origins: Vec<String> = Vec::new();
            if *menu > 0 {
                origins.push(format!("Menü ×{menu}"));
            }
            if *agent > 0 {
                origins.push(format!("Agent ×{agent}"));
            }
            format!("{name} ({})", origins.join(", "))
        })
        .collect();
    parts.join(", ")
}

#[cfg(test)]
mod tests {
    use super::{hook_block_output, parse_hook_block};

    #[test]
    fn claude_wording_yields_the_reason() {
        let output = "PreToolUse:Bash hook error: [cmd]: Sperre zum Test";
        assert_eq!(
            parse_hook_block(output).as_deref(),
            Some("[cmd]: Sperre zum Test")
        );
    }

    #[test]
    fn text_without_prefix_or_marker_is_no_block() {
        assert_eq!(parse_hook_block("Von einem Hook blockiert."), None);
        assert_eq!(parse_hook_block("PreToolUse:Bash failed"), None);
    }

    #[test]
    fn written_block_reads_back_as_its_reason() {
        let output = hook_block_output("Bash", "Von einem Hook blockiert.");
        assert_eq!(
            output,
            "PreToolUse:Bash hook error: Von einem Hook blockiert."
        );
        assert_eq!(
            parse_hook_block(&output).as_deref(),
            Some("Von einem Hook blockiert.")
        );
    }
}
