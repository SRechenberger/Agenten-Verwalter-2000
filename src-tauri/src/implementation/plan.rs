//! „Plane Vorhaben“: die feste erste Nachricht eines Planungszugs (ADR 062). Der Text liegt im
//! Verwalter, nicht in einem Skill, und führt keine Feldnamen des Schemas: Die Beschreibung von
//! `implementation_create` ist ihre einzige Quelle, damit sich das Schema ändern lässt, ohne dass
//! der Text bricht. Dazu der Wächter: Er prüft am Ende eines Planungszugs, ob der Plan angelegt
//! wurde, und erinnert sonst den Agenten.
use super::tools::{AGENT_TYPES, CREATE, EDIT, SET_PHASES};
use crate::mcp_access::AGENT_TOOL_PREFIX;

/// Die Nachricht an die planende Session: der Auftrag des Benutzers, unverändert, hinter dem festen
/// Teil. Er steht in der ersten Person, denn der Benutzer hat „Plane Vorhaben“ gedrückt; ein Satz, der
/// wie eine fremde Anweisung klingt, lehnt das kleinste Modell ab.
pub fn plan_prompt(task: &str) -> String {
    format!(
        "Ich möchte ein Vorhaben planen, und der Plan soll im Umsetzungstool des Verwalters \
         landen: Sobald er steht, legst du ihn mit {AGENT_TOOL_PREFIX}{CREATE} an, nie als \
         Fließtext im Chat. Bis dahin darfst du mir Fragen stellen.\n\
         \n\
         So ist der Plan aufgebaut: Ein Vorhaben besteht aus Lanes, das sind Stränge, die dem \
         Vorhaben seine Gliederung geben. Eine Lane besteht aus Stationen. Eine Station ist ein \
         Schritt, den der Verwalter als eigene Session mit frischem Kontext startet und dessen \
         Ende der Agent selbst meldet. Das übliche Muster einer Lane ist Test, Umsetzung, \
         Review, PR; lass Stationen weg, die zur Aufgabe nicht passen, und teil die Aufgabe in \
         mehrere Lanes, wenn sich Teile unabhängig voneinander bauen lassen.\n\
         \n\
         Die Reihenfolge ergibt sich aus Bedingungen, nicht aus Wellen: Eine Station nennt, worauf \
         sie wartet, und was nicht voneinander abhängt, darf gleichzeitig laufen. Welche Angaben \
         es dafür und für alles Weitere gibt, steht in der Beschreibung von \
         {AGENT_TOOL_PREFIX}{CREATE}; halte dich an sie.\n\
         \n\
         Jede Station konfigurierst du einzeln: Typ, Prompt, Modell und Denkaufwand. Lies vor dem \
         Vorschlag mit {AGENT_TOOL_PREFIX}{AGENT_TYPES} die Agententypen aus den Einstellungen und \
         empfiehl je Station einen Typ. Modell und Denkaufwand nennst du nur dort, wo die Station \
         vom Typ abweichen soll, mit kurzer Begründung.\n\
         \n\
         Mein Auftrag:\n\
         {task}"
    )
}

/// Höchstens so viele Erinnerungen je Planungszug; danach der Hinweis „Kein Plan im Umsetzungstool“.
pub const MAX_PLAN_REMINDERS: u8 = 2;

/// Die feste Erinnerung des Wächters, wie `RESUME_TEXT` bei „Automatisch fortsetzen“. In der ersten
/// Person wie `plan_prompt`.
pub const PLAN_REMINDER_TEXT: &str = "Ich sehe noch keinen Plan im Umsetzungstool. Leg ihn bitte \
     jetzt mit mcp__verwalter__implementation_create an, nicht als Fließtext im Chat. Fehlt dir \
     dafür noch eine Angabe von mir, frag mich mit AskUserQuestion.";

/// Der Wächter eines Planungszugs; nur im Speicher der Session, nach einem Neustart weg.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanGuard {
    /// Der Auftrag des Benutzers, für „Erneut“.
    pub task: String,
    /// `implementation_create`, `implementation_set_phases` oder `implementation_edit` wurde aufgerufen.
    pub called: bool,
    /// Schon gesendete Erinnerungen.
    pub reminders: u8,
}

impl PlanGuard {
    pub fn new(task: &str) -> PlanGuard {
        PlanGuard {
            task: task.to_owned(),
            called: false,
            reminders: 0,
        }
    }
}

/// Was am Ende eines Zugs geschieht.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuardStep {
    Nothing,
    Remind,
    Notice,
}

/// Die Entscheidung des Wächters am Ende eines abgeschlossenen Zugs. `None`: kein Planungszug, oder
/// der Hinweis hat die Prüfung schon beendet.
pub fn guard_step(guard: Option<&PlanGuard>) -> GuardStep {
    match guard {
        None => GuardStep::Nothing,
        Some(guard) if guard.called => GuardStep::Nothing,
        Some(guard) if guard.reminders < MAX_PLAN_REMINDERS => GuardStep::Remind,
        Some(_) => GuardStep::Notice,
    }
}

/// Rückt den Wächter am Ende eines abgeschlossenen Zugs weiter: Eine Erinnerung zählt mit, der
/// Hinweis nimmt ihn weg und gibt ihn heraus (sein Auftrag gehört in den Hinweis). Damit endet die
/// Prüfung dieses Zugs schon mit dem Hinweis; „Trotzdem weiter“ erledigt nur noch den Hinweis.
pub fn advance_guard(guard: &mut Option<PlanGuard>) -> (GuardStep, Option<PlanGuard>) {
    let step = guard_step(guard.as_ref());
    match step {
        GuardStep::Nothing => (step, None),
        GuardStep::Remind => {
            if let Some(guard) = guard {
                guard.reminders += 1;
            }
            (step, None)
        }
        GuardStep::Notice => (step, guard.take()),
    }
}

/// Nur der Name zählt, nicht das Ergebnis des Aufrufs; so gilt der Wächter für jedes Schema.
/// `implementation_edit` zählt mit: In einem Vorhaben mit unfertiger Umsetzung lehnt
/// `implementation_create` immer ab, und der Plan kommt als Vorschlag auf die Karte „Plan ändern?“.
pub fn is_plan_tool(tool: &str) -> bool {
    tool.strip_prefix(AGENT_TOOL_PREFIX)
        .is_some_and(|name: &str| name == CREATE || name == SET_PHASES || name == EDIT)
}
