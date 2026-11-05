# Rückfragen auf der Karte der Station beantworten

Ziel: Stellt eine Station einer Umsetzung eine Rückfrage (AskUserQuestion) oder eine Rechte-Abfrage, zeigt ihre große Karte in der Kartei denselben Rückfrage-Kasten wie der Chat und nimmt dort die Antwort entgegen — mit Ziffern, Notiz je Option, eigener Antwort je Frage und „Antworten“. Heute zeigt die Karte nur den Text der ersten Frage und schickt in den Chat: der Kern reicht nur diesen Text durch (`live_of` in `src-tauri/src/sessions/registry/implementation.rs`), und das Antworten wurde beim Bau der Umsetzung bewusst im Chat gelassen (Abweichung vom Entwurf, `docs/archive/2026-11/2026-10-30_umsetzung/phase-2-zeitleiste-und-kartei.md`, Absatz nach den Abnahmekriterien). Dieser Plan holt den Entwurf nach.

Kontext für jeden Umsetzer: [AGENTS.md](../../../AGENTS.md), [docs/code-map.md](../../code-map.md) (Zeilen „Chat“ und „Umsetzung“), [docs/glossary.md](../../glossary.md) (Rückfrage, Rechte-Abfrage, Station — „Rückfrage (Umsetzung)“ ist etwas anderes und bleibt unberührt), [docs/conventions/](../../conventions/), [ADR 045](../../decisions/045-rueckfrage-kasten.md) (Rückfrage-Kasten), [ADR 048](../../decisions/048-umsetzung.md) (Umsetzung). Entwurf der Karte: `docs/archive/2026-11/2026-10-30_umsetzung/artifacts/entwurf-umsetzung.html`, Abschnitt 1, Station im Zustand „fragt dich“. Fehlerklassen geprüft (Vault: React, TypeScript): keine einschlägig; für Rust, Tauri und Zustand gibt es keine Entity. Einschlägig aus dem Bestand: Zustand v5 (Selektor mit neuem Objekt je Aufruf) — Prüfpunkt in Phase 2.

| # | Phase | Rating | Status |
|---|---|---|---|
| 1 | Kern: die älteste offene Anfrage vollständig an die Station, Nachzug bei mehreren offenen Anfragen | standard | pending |
| 2 | Oberfläche: Antworten auf Rückfragen als gemeinsamer Baustein aus dem Chat lösen | standard | pending |
| 3 | Oberfläche: Karte beantwortet Rückfragen; ADR 055, Doku | standard | pending |

**Rahmen:** strikt 1 → 2 → 3. Keine Migration (die Ansicht der Umsetzung wird nicht gespeichert). ADR 055 ist für diesen Plan reserviert.

## Festgelegte Entscheidungen

- **Datenweg:** Die Ansicht der Station (`StationView`) trägt statt des Fragetexts die ganze älteste offene Anfrage ihrer Session (`OpenQuestion`: `seq`, Anfrage-ID, Art, alle Fragen samt Optionen), nur im Zustand `asking`. Der Kern hält die Anfrage ohnehin im Speicher (`SessionState.pending`); die Ansicht wird schon heute bei jeder Änderung einer Station-Session neu geladen (`useImplementations`). Verworfen: die Karte lädt die Frage aus dem Verlauf der Session wie der Chat — braucht je Station im Fokus eine Seite Verlauf plus eigenes Abo auf `chat://entry` und eine zweite Wahrheit über „offen“.
- **Nachzug bei mehreren offenen Anfragen:** Beantwortet der Benutzer eine von mehreren offenen Anfragen, bleibt der Status `waiting` und es geht heute keine Zusammenfassung raus — die Karte zeigte weiter die schon beantwortete. `apply_answer` markiert die Zusammenfassung deshalb immer als geändert.
- **Ein Antwortweg, zwei Orte:** Entwürfe, Senden, Fehlerzeile und Tasten liegen in einem gemeinsamen Baustein (`useQuestionAnswer`, `useQuestionKeys`), den Chat und Karte benutzen. Halbe Antworten liegen im Chat-Store je Session und Anfrage-ID, nicht mehr im Zustand von `ChatTimeline` — so überlebt eine angefangene Notiz den Wechsel zwischen Karte und Chat. Der Kasten selbst bleibt `QuestionBlock` aus dem Chat (ADR 045), statt der einfachen Knopfreihe aus dem älteren Entwurf: eine dritte Antwort-Oberfläche wäre Dopplung.
- **Nach dem Senden** bleibt der Kasten gesperrt, bis die Ansicht die Anfrage nicht mehr meldet; dazwischen liegt eine Runde zum Kern (`listImplementations`), in der ein zweiter Klick sonst „Rückfrage nicht offen“ meldete.
- **Fehler** beim Antworten gehen wie heute an die Fehlerzeile der Session (`sessionErrors`); die Karte zeigt diese Zeile unter dem Kasten (`SessionActionError`).
- **Tasten:** Ziffern 1–9 und Enter wirken auf der Karte wie im Chat, solange die Karte mit offener Anfrage sichtbar ist; Chat und Umsetzung sind nie gleichzeitig gemountet (`renderMain` in `src/app/App.tsx`).
- **Satz „Oder schreib unten eine eigene Antwort.“** steht nur im Chat (darunter liegt die Eingabeleiste), nicht auf der Karte.

## Kontrakt

```rust
// src-tauri/src/implementation/model.rs, derive(Debug, Clone, Serialize, Deserialize, TS), serde camelCase
/// Älteste offene Rückfrage oder Rechte-Abfrage einer Session, so wie der Chat sie zeigt.
pub struct OpenQuestion {
    pub seq: u32,
    pub request_id: String,
    pub question_kind: QuestionKind, // crate::agents::event
    pub questions: Vec<Question>,    // crate::agents::event
}
// StationView: Feld `question: Option<String>` entfällt, dafür
pub open_question: Option<OpenQuestion>, // nur bei StationState::Asking
// SessionLive (implementation/mod.rs): Feld `question` entfällt, dafür
pub open_question: Option<OpenQuestion>,
```

```ts
// src/lib/bindings/OpenQuestion.ts (generiert): { seq: number, requestId: string, questionKind: QuestionKind, questions: Array<Question> }
// StationView.openQuestion: OpenQuestion | null

// src/stores/chat.ts — zusätzlich
questionDrafts: Record<string, Record<string, QuestionDraft>>; // Session-ID → Anfrage-ID → Entwurf
setQuestionDraft: (sessionId: string, requestId: string, draft: QuestionDraft) => void;
clearQuestionDraft: (sessionId: string, requestId: string) => void;

// src/features/chat/useQuestionAnswer.ts
export interface QuestionAnswering {
  draftFor: (entry: QuestionEntry) => QuestionDraft;
  isSending: (entry: QuestionEntry) => boolean;
  pick: (entry: QuestionEntry, questionIndex: number, optionIndex: number) => void;
  submit: (entry: QuestionEntry, answer: QuestionAnswer) => void;
  storeDraft: (entry: QuestionEntry, draft: QuestionDraft) => void;
}
export function useQuestionAnswer(sessionId: string): QuestionAnswering;

// src/features/chat/useQuestionKeys.ts
export function useQuestionKeys(entry: QuestionEntry | null, answering: QuestionAnswering): void;

// src/lib/keyTargets.ts
export function isTextTarget(target: EventTarget | null): boolean;
export function isControlTarget(target: EventTarget | null): boolean;

// QuestionBlock: neue Pflicht-Prop
showsComposerHint: boolean;
```

## Finale Abnahmekriterien

1. Jede offene Rückfrage und Rechte-Abfrage einer Station lässt sich auf ihrer großen Karte beantworten, ohne den Chat zu öffnen, mit allem, was der Chat-Kasten kann (Ziehharmonika, Ziffern, Notiz je Option, eigene Antwort je Frage, „Antworten“/Enter).
2. Der Chat beantwortet Rückfragen genau wie vorher; neu ist nur, dass ein halber Entwurf den Wechsel zur Karte und zurück überlebt.
3. Sind mehrere Anfragen offen, zeigt die Karte nach jeder Antwort die nächste.
4. Der Lagebericht ist unverändert.
5. `pnpm check` grün; keine Migration; ADR 055, Code-Map und Glossar nachgezogen.

## Nicht-Ziele

Die Faktenzeile des Entwurfs unter der Karte (Modell, Läuft seit, Zuletzt), „fragt seit N min“, Rückfragen in der Übersicht des Vorhabens oder in der Sidebar, eine breitere Kartei-Spalte (sie ist schon `1fr`; die Karte wächst in die Höhe, die Spalte rollt).

## Wackelstellen

- Zeitfenster zwischen erfolgreicher Antwort und neuer Ansicht — gedeckt durch die Sperre nach dem Senden. Beleg im laufenden Betrieb: auf der Karte antworten; der Kasten bleibt gesperrt, bis „läuft“ erscheint, kein Fehlersatz.
- Zwei gleichzeitig offene Anfragen — Nachzug über die Zusammenfassung. Beleg im laufenden Betrieb: eine Station im Modus mit Rechte-Abfragen zwei Werkzeugaufrufe parallel stellen lassen, den ersten auf der Karte beantworten; die Karte zeigt sofort den zweiten.

## Summary

## Files touched

## Commits

## Deviations from plan

## Follow-ups
