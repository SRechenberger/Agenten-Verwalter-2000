//! „Plane Vorhaben“: die feste erste Nachricht eines Planungszugs (ADR 062). Der Text liegt im
//! Verwalter, nicht in einem Skill, und führt keine Feldnamen des Schemas: Die Beschreibung von
//! `implementation_create` ist ihre einzige Quelle, damit sich das Schema ändern lässt, ohne dass
//! der Text bricht.
use super::tools::{AGENT_TYPES, CREATE};
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
