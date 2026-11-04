//! Die Datei `befunde.md`: die Mini-Retros aller Sessions eines Laufs, je Session ein Abschnitt.
use crate::agents::event::ModelId;
use crate::retro::model::{Claim, Correction, Effect, Fact, Failure, SessionFindings};
use crate::retro::traces::HookBlock;
use crate::sessions::model::SessionStatus;
use crate::tldr::transcript::status_label;

const EMPTY_LIST: &str = "- keine";

/// Eine einbezogene Session mit dem Ergebnis ihrer Mini-Retro (Fehlertext, wenn sie scheiterte).
pub struct SessionResult {
    pub number: u32,
    pub name: String,
    pub status: SessionStatus,
    pub model: ModelId,
    /// Alle Werkzeug-Aufrufe, davon fehlgeschlagen, davon abgebrochen.
    pub counts: (u32, u32, u32),
    pub truncated: bool,
    /// Die fertige Zeile aus `traces::skills_line`.
    pub skills: String,
    pub hook_blocks: Vec<HookBlock>,
    pub outcome: Result<SessionFindings, String>,
}

pub fn render(project_name: &str, sessions: &[SessionResult]) -> String {
    let truncated = sessions
        .iter()
        .filter(|session: &&SessionResult| session.truncated)
        .count();
    let failed = sessions
        .iter()
        .filter(|session: &&SessionResult| session.outcome.is_err())
        .count();
    let mut lines: Vec<String> = vec![
        format!("# Befunde – {project_name}"),
        format!(
            "Sessions: {}, davon gekürzt: {truncated}, Mini-Retro fehlgeschlagen: {failed}",
            sessions.len()
        ),
    ];
    for session in sessions {
        lines.push(String::new());
        push_session(&mut lines, session);
    }
    lines.push(String::new());
    lines.join("\n")
}

fn push_session(lines: &mut Vec<String>, session: &SessionResult) {
    let (tools, failed, interrupted) = session.counts;
    lines.push(format!("## Session #{} – {}", session.number, session.name));
    lines.push(format!(
        "Status: {} · Modell: {} · Werkzeug-Aufrufe: {tools}, fehlgeschlagen: {failed}, abgebrochen: {interrupted} · Hook-Sperren: {} · Gekürzt: {} · Verlauf: session-{}.md",
        status_label(session.status),
        session.model.cli_id(),
        session.hook_blocks.len(),
        if session.truncated { "ja" } else { "nein" },
        session.number,
    ));
    lines.push(format!("Skills: {}", session.skills));
    lines.push(String::new());
    // Steht vor dem Ausgang der Mini-Retro: eine gescheiterte Mini-Retro soll die Sperren nicht verschlucken.
    push_rubric(
        lines,
        "Hook-Sperren",
        &session.hook_blocks,
        |item: &HookBlock| {
            format!(
                "{} {} — {}",
                one_line(&item.tool),
                one_line(&item.target),
                one_line(&item.reason)
            )
        },
    );
    let findings = match &session.outcome {
        Ok(findings) => findings,
        Err(error) => {
            lines.push(format!("Mini-Retro fehlgeschlagen: {}", one_line(error)));
            return;
        }
    };
    push_rubric(
        lines,
        "Korrekturen des Nutzers",
        &findings.corrections,
        |item: &Correction| format!("„{}“ → {}", one_line(&item.quote), one_line(&item.about)),
    );
    push_rubric(
        lines,
        "Fehlversuche und Wiederholungen",
        &findings.failures,
        |item: &Failure| format!("{} · „{}“", one_line(&item.what), one_line(&item.quote)),
    );
    push_rubric(
        lines,
        "Regel hat gewirkt",
        &findings.effects,
        |item: &Effect| format!("{} · „{}“", one_line(&item.what), one_line(&item.quote)),
    );
    push_rubric(
        lines,
        "Behauptungen ohne Beleg",
        &findings.unbacked_claims,
        |item: &Claim| format!("„{}“", one_line(&item.quote)),
    );
    push_rubric(
        lines,
        "Neue technische Fakten",
        &findings.facts,
        |item: &Fact| format!("{} · „{}“", one_line(&item.fact), one_line(&item.quote)),
    );
    push_rubric(lines, "Offen geblieben", &findings.open, |item: &String| {
        one_line(item)
    });
}

fn push_rubric<T>(
    lines: &mut Vec<String>,
    heading: &str,
    items: &[T],
    render_item: impl Fn(&T) -> String,
) {
    lines.push(format!("### {heading}"));
    if items.is_empty() {
        lines.push(EMPTY_LIST.to_owned());
        return;
    }
    for item in items {
        lines.push(format!("- {}", render_item(item)));
    }
}

/// Ein Zeilenumbruch im Zitat würde den Listenpunkt zerreißen.
fn one_line(text: &str) -> String {
    text.split(['\r', '\n'])
        .filter(|part: &&str| !part.trim().is_empty())
        .map(str::trim)
        .collect::<Vec<&str>>()
        .join(" ")
}
