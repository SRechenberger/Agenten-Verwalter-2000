//! Anweisungen und Antwort-Schemas für den Einmal-Aufruf von Haiku.
use std::time::Duration;

/// Längste gemessene Dauer (30 s) × 3; auch ein Transkript an der Obergrenze brauchte nur 17 s
/// (docs/knowledge/claude-stream-json.md, „Einmal-Aufruf im Druckmodus“).
pub const TLDR_TIMEOUT: Duration = Duration::from_secs(90);

pub const SESSION_SYSTEM_PROMPT: &str = "Du fasst den Verlauf einer Arbeitssitzung zwischen einem Entwickler (Nutzer) und einem Coding-Agenten (Claude) zusammen. Antworte auf Deutsch, knapp und sachlich, ohne Einleitung. title: Titel der Session, höchstens 40 Zeichen, eine Wortgruppe statt eines Satzes, ohne Anführungszeichen und ohne Schlusspunkt. short: ein Satz, höchstens 160 Zeichen, was die Session tut oder getan hat. goal: worum es geht, ein Satz. done: was erledigt ist, höchstens zwei Sätze. ongoing: woran zuletzt gearbeitet wurde und was noch läuft; leer, wenn nichts läuft. open: was offen ist; beginnt mit „Deine Entscheidung:“, wenn der Agent auf eine Entscheidung oder Antwort des Nutzers wartet; leer, wenn nichts offen ist. openNeedsUser: true genau dann, wenn open eine Entscheidung oder Antwort des Nutzers verlangt. Erfinde nichts, was nicht im Verlauf steht.";

pub const SESSION_SCHEMA: &str = r#"{"type":"object","properties":{"title":{"type":"string"},"short":{"type":"string"},"goal":{"type":"string"},"done":{"type":"string"},"ongoing":{"type":"string"},"open":{"type":"string"},"openNeedsUser":{"type":"boolean"}},"required":["title","short","goal","done","ongoing","open","openNeedsUser"],"additionalProperties":false}"#;

pub const PROJECT_SYSTEM_PROMPT: &str = "Du fasst ein Vorhaben zusammen, das aus mehreren nacheinander gelaufenen Arbeitssitzungen (Sessions) eines Entwicklers mit einem Coding-Agenten besteht. Du bekommst je Session ihre Nummer, ihren Namen, ihren Status und ihre Kurzfassung. Antworte auf Deutsch, knapp und sachlich, ohne Einleitung. title: Titel des Vorhabens, höchstens 40 Zeichen, eine Wortgruppe statt eines Satzes, ohne Anführungszeichen und ohne Schlusspunkt. summary: zwei Sätze, worum es geht und wo es steht. status: der Stand in einem Satz; nenne Sessions als #Nummer. open: das Wichtigste, was offen ist, mit #Nummer; leer, wenn nichts offen ist. next: der nächste sinnvolle Schritt in einem Satz; leer, wenn unklar. openNeedsUser: true genau dann, wenn open eine Entscheidung des Nutzers verlangt. Erfinde nichts.";

pub const PROJECT_SCHEMA: &str = r#"{"type":"object","properties":{"title":{"type":"string"},"summary":{"type":"string"},"status":{"type":"string"},"open":{"type":"string"},"next":{"type":"string"},"openNeedsUser":{"type":"boolean"}},"required":["title","summary","status","open","next","openNeedsUser"],"additionalProperties":false}"#;

pub const TITLE_SYSTEM_PROMPT: &str = "Du vergibst den Titel für eine Arbeitssitzung zwischen einem Entwickler und einem Coding-Agenten. Du bekommst die erste Nachricht des Entwicklers. Antworte auf Deutsch. title: höchstens 40 Zeichen, eine Wortgruppe statt eines Satzes, ohne Anführungszeichen und ohne Schlusspunkt; sagt, worum es geht. Erfinde nichts.";

pub const TITLE_SCHEMA: &str = r#"{"type":"object","properties":{"title":{"type":"string"}},"required":["title"],"additionalProperties":false}"#;

pub const HANDOFF_SYSTEM_PROMPT: &str = "Du schreibst die Übergabe einer Arbeitssitzung zwischen einem Entwickler (Nutzer) und einem Coding-Agenten (Claude) an eine neue Sitzung, die ohne diesen Verlauf weitermacht. Du bekommst nur den Gesprächstext, keine Werkzeug-Ausgaben. Antworte auf Deutsch, sachlich, ohne Einleitung; jedes Feld ist leer, wenn der Verlauf nichts dazu enthält. goal: worum es geht, höchstens zwei Sätze. done: was erledigt ist, als Stichpunkte mit „- “ je Zeile. decisions: getroffene Entscheidungen und verworfene Wege samt Grund, als Stichpunkte. current: woran zuletzt gearbeitet wurde und wie weit es ist, höchstens drei Sätze. open: offene Punkte und unbeantwortete Fragen, als Stichpunkte; beginnt mit „Deine Entscheidung:“, wenn der Agent auf den Nutzer wartet. next: der nächste Schritt, ein Satz. references: im Verlauf genannte Dateien, Pläne, Branches, Tickets und Befehle, die die neue Sitzung braucht, als Stichpunkte, wörtlich wie im Verlauf. Erfinde nichts, was nicht im Verlauf steht.";

pub const HANDOFF_SCHEMA: &str = r#"{"type":"object","properties":{"goal":{"type":"string"},"done":{"type":"string"},"decisions":{"type":"string"},"current":{"type":"string"},"open":{"type":"string"},"next":{"type":"string"},"references":{"type":"string"}},"required":["goal","done","decisions","current","open","next","references"],"additionalProperties":false}"#;

/// Die erste Nachricht kann ein langer Einfüge-Text sein; für einen Titel genügt der Anfang.
pub const TITLE_INPUT_CHARS: usize = 2000;
