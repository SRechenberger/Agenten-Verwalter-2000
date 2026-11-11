//! Die Datei `werkzeuge.md` (ADR 067): Teil A, der Querschnitt über die Werkzeug-Spuren aller
//! Sessions eines Laufs, ohne Modell; Teil B, die Werkzeug-Retro je Session.
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use super::findings::{one_line, push_rubric};
use super::model::{Friction, Routine, ScriptPurpose, StepSummary, ToolFindings};
use super::tools::{CallStatus, GROUND_TOOLS, SessionTrace, TraceCall, TraceStep, output_label};

/// Wiederkehrend heißt: mindestens so oft insgesamt …
const MIN_TOTAL: usize = 3;
/// … und in mindestens so vielen Sessions.
const MIN_SESSIONS: usize = 2;
/// Ab dieser Ausgabegröße gilt ein Aufruf als dick.
const BIG_OUTPUT_CHARS: usize = 20_000;
const MAX_REPEATED_CALLS: usize = 40;
const MAX_SEQUENCES: usize = 30;
const MAX_FAMILIES: usize = 30;
const MAX_BIG_OUTPUTS: usize = 15;
const MAX_SCRIPTS: usize = 40;
const MAX_FOLLOWERS: usize = 3;
const MIN_SEQUENCE_LENGTH: usize = 3;
const MAX_SEQUENCE_LENGTH: usize = 5;
/// Sperren mit gleichem Anfang des Grundes sind eine Gruppe.
const REASON_KEY_CHARS: usize = 80;
/// So viele Session-Nummern stehen in einer Zeile, danach `…`.
const MAX_LISTED_SESSIONS: usize = 8;

/// `retros` steht an derselben Stelle wie die Session in `traces`; Teil B steht nur, wenn
/// mindestens ein Eintrag `Some` ist.
pub fn render(
    project_name: &str,
    traces: &[SessionTrace],
    retros: &[Option<Result<ToolFindings, String>>],
) -> String {
    let call_count: usize = traces
        .iter()
        .map(|trace: &SessionTrace| calls_of(trace).count())
        .sum();
    let mut lines: Vec<String> = vec![
        format!("# Werkzeuge – {project_name}"),
        format!(
            "Sessions: {} · Aufrufe: {call_count} · Schwelle für Wiederholung: ≥{MIN_TOTAL}× in ≥{MIN_SESSIONS} Sessions · Spuren: spur-<N>.md",
            traces.len()
        ),
        String::new(),
        "## A Querschnitt".to_owned(),
    ];
    let render_text = |text: &String| text.clone();
    push_rubric(
        &mut lines,
        "Wiederholte gleiche Aufrufe",
        &repeated_calls(traces),
        render_text,
    );
    push_rubric(
        &mut lines,
        "Wiederkehrende Abfolgen",
        &recurring_sequences(traces),
        render_text,
    );
    push_rubric(
        &mut lines,
        "Hook-Sperren",
        &hook_blocks(traces),
        render_text,
    );
    push_rubric(
        &mut lines,
        "Werkzeug-Familien",
        &family_table(traces),
        render_text,
    );
    push_rubric(
        &mut lines,
        "Dicke Ausgaben",
        &big_outputs(traces),
        render_text,
    );
    push_rubric(
        &mut lines,
        "Inline-Skripte",
        &inline_scripts(traces),
        render_text,
    );
    if retros
        .iter()
        .any(|retro: &Option<Result<ToolFindings, String>>| retro.is_some())
    {
        lines.push(String::new());
        lines.push("## B Werkzeug-Retro je Session".to_owned());
        push_part_b(&mut lines, traces, retros);
    }
    lines.push(String::new());
    lines.join("\n")
}

fn calls_of(trace: &SessionTrace) -> impl Iterator<Item = &TraceCall> {
    trace
        .steps
        .iter()
        .flat_map(|step: &TraceStep| step.calls.iter())
}

/// Stellen der Sessions in `traces`; die Nummer allein ist nicht eindeutig.
type SessionSet = BTreeSet<usize>;

/// `#1, #4, …`: höchstens `MAX_LISTED_SESSIONS` Kennungen.
fn session_list(sessions: &SessionSet, traces: &[SessionTrace]) -> String {
    let mut listed: Vec<String> = sessions
        .iter()
        .take(MAX_LISTED_SESSIONS)
        .map(|position: &usize| format!("#{}", traces[*position].label))
        .collect();
    if sessions.len() > MAX_LISTED_SESSIONS {
        listed.push("…".to_owned());
    }
    listed.join(", ")
}

struct Occurrence {
    total: usize,
    sessions: SessionSet,
    display: String,
}

fn repeated_calls(traces: &[SessionTrace]) -> Vec<String> {
    let mut groups: HashMap<&str, Occurrence> = HashMap::new();
    for (position, trace) in traces.iter().enumerate() {
        for call in calls_of(trace).filter(|call: &&TraceCall| !call.exact_key.is_empty()) {
            let group = groups
                .entry(call.exact_key.as_str())
                .or_insert_with(|| Occurrence {
                    total: 0,
                    sessions: SessionSet::new(),
                    display: call.display.clone(),
                });
            group.total += 1;
            group.sessions.insert(position);
        }
    }
    let mut listed: Vec<(&str, &Occurrence)> = groups
        .iter()
        .filter(|(_, group): &(&&str, &Occurrence)| {
            group.total >= MIN_TOTAL && group.sessions.len() >= MIN_SESSIONS
        })
        .map(|(key, group): (&&str, &Occurrence)| (*key, group))
        .collect();
    listed.sort_by(|left: &(&str, &Occurrence), right: &(&str, &Occurrence)| {
        right
            .1
            .sessions
            .len()
            .cmp(&left.1.sessions.len())
            .then(right.1.total.cmp(&left.1.total))
            .then(left.0.cmp(right.0))
    });
    listed
        .into_iter()
        .take(MAX_REPEATED_CALLS)
        .map(|(_, group): (&str, &Occurrence)| {
            format!(
                "{}× in {} Sessions ({}): {}",
                group.total,
                group.sessions.len(),
                session_list(&group.sessions, traces),
                group.display
            )
        })
        .collect()
}

struct Sequence {
    families: Vec<String>,
    total: usize,
    sessions: SessionSet,
}

impl Sequence {
    fn contains(&self, shorter: &Sequence) -> bool {
        self.families
            .windows(shorter.families.len())
            .any(|window: &[String]| window == shorter.families.as_slice())
    }
}

/// Familie eines Grundwerkzeugs wie `Read:retro.rs` oder `Grep`.
fn is_ground_family(family: &str) -> bool {
    let tool = family.split(':').next().unwrap_or(family);
    GROUND_TOOLS.contains(&tool)
}

/// Ein Fenster zählt nur mit mindestens zwei verschiedenen Familien, davon mindestens zwei
/// außerhalb der Grundwerkzeuge — sonst besteht es aus `Grep → Grep → Read`.
fn is_meaningful_window(window: &[&str]) -> bool {
    let distinct: HashSet<&&str> = window.iter().collect();
    let content = window
        .iter()
        .filter(|family: &&&str| !is_ground_family(family))
        .count();
    distinct.len() >= 2 && content >= 2
}

fn recurring_sequences(traces: &[SessionTrace]) -> Vec<String> {
    let mut counts: HashMap<Vec<String>, (usize, SessionSet)> = HashMap::new();
    for (position, trace) in traces.iter().enumerate() {
        for step in &trace.steps {
            let families: Vec<&str> = step
                .calls
                .iter()
                .map(|call: &TraceCall| call.family.as_str())
                .collect();
            // Je Schritt zählt jedes Fenster einmal.
            let mut seen: HashSet<&[&str]> = HashSet::new();
            for length in MIN_SEQUENCE_LENGTH..=MAX_SEQUENCE_LENGTH {
                for window in families.windows(length) {
                    if !is_meaningful_window(window) || !seen.insert(window) {
                        continue;
                    }
                    let key: Vec<String> = window
                        .iter()
                        .map(|family: &&str| (*family).to_owned())
                        .collect();
                    let entry = counts.entry(key).or_default();
                    entry.0 += 1;
                    entry.1.insert(position);
                }
            }
        }
    }
    let mut candidates: Vec<Sequence> = counts
        .into_iter()
        .filter(
            |(_, (total, sessions)): &(Vec<String>, (usize, SessionSet))| {
                *total >= MIN_TOTAL && sessions.len() >= MIN_SESSIONS
            },
        )
        .map(
            |(families, (total, sessions)): (Vec<String>, (usize, SessionSet))| Sequence {
                families,
                total,
                sessions,
            },
        )
        .collect();
    candidates.sort_by(order_sequences_longest_first);
    let mut kept: Vec<Sequence> = Vec::new();
    for candidate in candidates {
        let is_covered = kept.iter().any(|longer: &Sequence| {
            longer.families.len() > candidate.families.len()
                && longer.sessions.len() >= candidate.sessions.len()
                && longer.contains(&candidate)
        });
        if !is_covered {
            kept.push(candidate);
        }
    }
    kept.sort_by(|left: &Sequence, right: &Sequence| {
        right
            .sessions
            .len()
            .cmp(&left.sessions.len())
            .then(right.families.len().cmp(&left.families.len()))
            .then(right.total.cmp(&left.total))
            .then(left.families.cmp(&right.families))
    });
    kept.into_iter()
        .take(MAX_SEQUENCES)
        .map(|sequence: Sequence| {
            format!(
                "{} Sessions, {}×: {}",
                sequence.sessions.len(),
                sequence.total,
                sequence.families.join(" → ")
            )
        })
        .collect()
}

fn order_sequences_longest_first(left: &Sequence, right: &Sequence) -> std::cmp::Ordering {
    right
        .families
        .len()
        .cmp(&left.families.len())
        .then(right.sessions.len().cmp(&left.sessions.len()))
        .then(right.total.cmp(&left.total))
        .then(left.families.cmp(&right.families))
}

#[derive(Default)]
struct BlockGroup {
    total: usize,
    sessions: SessionSet,
    /// Familie des nächsten Aufrufs im selben Schritt → wie oft.
    followers: BTreeMap<String, usize>,
}

fn hook_blocks(traces: &[SessionTrace]) -> Vec<String> {
    let mut groups: BTreeMap<String, BlockGroup> = BTreeMap::new();
    for (session_position, trace) in traces.iter().enumerate() {
        for step in &trace.steps {
            for (call_position, call) in step.calls.iter().enumerate() {
                let CallStatus::Blocked(reason) = &call.status else {
                    continue;
                };
                let key: String = reason.chars().take(REASON_KEY_CHARS).collect();
                let group = groups.entry(key).or_default();
                group.total += 1;
                group.sessions.insert(session_position);
                if let Some(next) = step.calls.get(call_position + 1) {
                    *group.followers.entry(next.family.clone()).or_default() += 1;
                }
            }
        }
    }
    let mut listed: Vec<(&String, &BlockGroup)> = groups.iter().collect();
    listed.sort_by(
        |left: &(&String, &BlockGroup), right: &(&String, &BlockGroup)| {
            right.1.total.cmp(&left.1.total).then(left.0.cmp(right.0))
        },
    );
    listed
        .into_iter()
        .map(|(reason, group): (&String, &BlockGroup)| {
            format!(
                "{}× in {} Sessions: {reason} — danach: {}",
                group.total,
                group.sessions.len(),
                followers_text(&group.followers)
            )
        })
        .collect()
}

/// Die drei häufigsten Folge-Familien als `Bash:git status ×2, Read:a.rs ×1`.
fn followers_text(followers: &BTreeMap<String, usize>) -> String {
    let mut ranked: Vec<(&String, &usize)> = followers.iter().collect();
    ranked.sort_by(|left: &(&String, &usize), right: &(&String, &usize)| {
        right.1.cmp(left.1).then(left.0.cmp(right.0))
    });
    if ranked.is_empty() {
        return "keine".to_owned();
    }
    ranked
        .into_iter()
        .take(MAX_FOLLOWERS)
        .map(|(family, count): (&String, &usize)| format!("{family} ×{count}"))
        .collect::<Vec<String>>()
        .join(", ")
}

#[derive(Default)]
struct FamilyStats {
    calls: usize,
    failed: usize,
    blocked: usize,
    output_chars: usize,
}

fn family_table(traces: &[SessionTrace]) -> Vec<String> {
    let mut families: BTreeMap<&str, FamilyStats> = BTreeMap::new();
    for call in traces.iter().flat_map(calls_of) {
        let stats = families.entry(call.family.as_str()).or_default();
        stats.calls += 1;
        stats.output_chars += call.output_chars;
        match call.status {
            CallStatus::Failed(_) => stats.failed += 1,
            CallStatus::Blocked(_) => stats.blocked += 1,
            CallStatus::Ok | CallStatus::Interrupted => {}
        }
    }
    let mut ranked: Vec<(&str, &FamilyStats)> = families
        .iter()
        .map(|(family, stats): (&&str, &FamilyStats)| (*family, stats))
        .collect();
    ranked.sort_by(
        |left: &(&str, &FamilyStats), right: &(&str, &FamilyStats)| {
            right.1.calls.cmp(&left.1.calls).then(left.0.cmp(right.0))
        },
    );
    ranked
        .into_iter()
        .take(MAX_FAMILIES)
        .map(|(family, stats): (&str, &FamilyStats)| {
            format!(
                "{family}: {} Aufrufe · {} Fehler · {} Sperren · {} Ausgabe",
                stats.calls,
                stats.failed,
                stats.blocked,
                output_label(stats.output_chars, false)
            )
        })
        .collect()
}

fn big_outputs(traces: &[SessionTrace]) -> Vec<String> {
    let mut big: Vec<(usize, &TraceCall)> = Vec::new();
    for (position, trace) in traces.iter().enumerate() {
        for call in calls_of(trace) {
            if call.output_chars > BIG_OUTPUT_CHARS || call.output_truncated {
                big.push((position, call));
            }
        }
    }
    big.sort_by(|left: &(usize, &TraceCall), right: &(usize, &TraceCall)| {
        right
            .1
            .output_chars
            .cmp(&left.1.output_chars)
            .then(left.0.cmp(&right.0))
            .then(left.1.step.cmp(&right.1.step))
            .then(left.1.index.cmp(&right.1.index))
    });
    big.into_iter()
        .take(MAX_BIG_OUTPUTS)
        .map(|(position, call): (usize, &TraceCall)| {
            format!(
                "#{} Schritt {}.{}: {} — {}",
                traces[position].label,
                call.step,
                call.index,
                output_label(call.output_chars, call.output_truncated),
                call.display
            )
        })
        .collect()
}

struct ScriptGroup {
    count: usize,
    sessions: SessionSet,
    label: String,
    lines: usize,
}

fn inline_scripts(traces: &[SessionTrace]) -> Vec<String> {
    let mut groups: BTreeMap<&str, ScriptGroup> = BTreeMap::new();
    for (position, trace) in traces.iter().enumerate() {
        for script in calls_of(trace).filter_map(|call: &TraceCall| call.script.as_ref()) {
            let group = groups
                .entry(script.hash.as_str())
                .or_insert_with(|| ScriptGroup {
                    count: 0,
                    sessions: SessionSet::new(),
                    label: script.label.clone(),
                    lines: script.lines,
                });
            group.count += 1;
            group.sessions.insert(position);
        }
    }
    let mut ranked: Vec<(&str, &ScriptGroup)> = groups
        .iter()
        .map(|(hash, group): (&&str, &ScriptGroup)| (*hash, group))
        .collect();
    ranked.sort_by(
        |left: &(&str, &ScriptGroup), right: &(&str, &ScriptGroup)| {
            right
                .1
                .sessions
                .len()
                .cmp(&left.1.sessions.len())
                .then(right.1.count.cmp(&left.1.count))
                .then(left.0.cmp(right.0))
        },
    );
    let rest = ranked.len().saturating_sub(MAX_SCRIPTS);
    let mut lines: Vec<String> = ranked
        .into_iter()
        .take(MAX_SCRIPTS)
        .map(|(hash, group): (&str, &ScriptGroup)| {
            let candidate = if group.count >= MIN_TOTAL && group.sessions.len() >= MIN_SESSIONS {
                " ← Kandidat"
            } else {
                ""
            };
            format!(
                "{}× in Sessions {}: {} ({hash}, {} Zeilen){candidate}",
                group.count,
                session_list(&group.sessions, traces),
                group.label,
                group.lines
            )
        })
        .collect();
    if rest > 0 {
        lines.push(format!("… und {rest} weitere"));
    }
    lines
}

fn push_part_b(
    lines: &mut Vec<String>,
    traces: &[SessionTrace],
    retros: &[Option<Result<ToolFindings, String>>],
) {
    for (trace, retro) in traces.iter().zip(retros) {
        let Some(outcome) = retro else {
            continue;
        };
        lines.push(String::new());
        lines.push(format!(
            "### Session #{} – {}",
            trace.label,
            one_line(&trace.name)
        ));
        match outcome {
            Ok(findings) => push_findings(lines, findings),
            Err(error) => lines.push(format!(
                "Werkzeug-Retro fehlgeschlagen: {}",
                one_line(error)
            )),
        }
    }
}

fn push_findings(lines: &mut Vec<String>, findings: &ToolFindings) {
    push_labeled(
        lines,
        "Ziele und Wege",
        &findings.steps,
        |item: &StepSummary| {
            format!(
                "Schritt {}: {} — {}",
                item.step,
                one_line(&item.goal),
                one_line(&item.path)
            )
        },
    );
    push_labeled(lines, "Handgriffe", &findings.routines, |item: &Routine| {
        format!(
            "[{}] {}: {} · ändert sich: {} · Vorschlag: {}",
            one_line(&item.kind),
            item.calls.join(", "),
            one_line(&item.recipe),
            one_line(&item.inputs),
            one_line(&item.proposal)
        )
    });
    push_labeled(lines, "Reibung", &findings.friction, |item: &Friction| {
        format!("{}: {}", item.calls.join(", "), one_line(&item.cause))
    });
    push_labeled(
        lines,
        "Skripte",
        &findings.scripts,
        |item: &ScriptPurpose| {
            let reuse = if item.reusable {
                "wiederverwendbar"
            } else {
                "einmalig"
            };
            format!(
                "{}: {} ({reuse})",
                one_line(&item.hash),
                one_line(&item.purpose)
            )
        },
    );
}

/// Wie `push_rubric`, aber mit der Zeile `<Bezeichnung>:` statt einer Überschrift.
fn push_labeled<T>(
    lines: &mut Vec<String>,
    label: &str,
    items: &[T],
    render_item: impl Fn(&T) -> String,
) {
    lines.push(format!("{label}:"));
    if items.is_empty() {
        lines.push("- keine".to_owned());
        return;
    }
    for item in items {
        lines.push(format!("- {}", render_item(item)));
    }
}
