# Phase 1 — Kern: die älteste offene Anfrage vollständig an die Station

Rating **standard** · Status: pending

## Kontext

- [README.md](README.md): Entscheidungen „Datenweg“ und „Nachzug bei mehreren offenen Anfragen“, Kontrakt (Rust-Teil).
- `src-tauri/src/implementation/model.rs` — `StationView` (Feld `question`).
- `src-tauri/src/implementation/mod.rs` — `SessionLive` (Feld `question`), `station_view` (setzt `question` nur bei `StationState::Asking`), Importe `use model::{…}`.
- `src-tauri/src/sessions/registry/implementation.rs` — `live_of` (liest `state.pending.first()`), Import `use crate::agents::event::{Attachment, Question}`.
- `src-tauri/src/implementation/report.rs` — `now_lines` (Zeile unter „fragt dich:“ aus `station.question`).
- `src-tauri/src/sessions/registry.rs` — `PendingRequest` (Felder `question_kind`, `questions`, `input`, `seq`; privat, aber aus dem Untermodul `registry::implementation` lesbar), `apply_answer`, `set_status` (kehrt bei gleichem Status sofort zurück und setzt nur dann `outbox.summary_dirty`), `update` (aus `summary_dirty` wird die Zusammenfassung, die als `session://changed` rausgeht und in die Datenbank geschrieben wird).
- `src-tauri/src/agents/event.rs` — `Question`, `QuestionKind`, `ChatEntry::Question` (Vorbild für die Feldnamen).
- `src-tauri/examples/gen-bindings.rs` — Liste der exportierten Typen (`StationView::export_all`).
- `src/features/implementation/StationCard.tsx` — `renderQuestion` liest `station.question`.
- [docs/conventions/rust.md](../../conventions/rust.md), [linting.md](../../conventions/linting.md).

## Abnahmekriterien

1. `StationView` hat kein Feld `question` mehr, sondern `open_question: Option<OpenQuestion>` nach Kontrakt; gesetzt genau dann, wenn der Zustand `Asking` ist und die Session eine offene Anfrage hat, und zwar mit der ältesten (`pending.first()`), allen ihren Fragen samt Optionen, Art und `seq` des Chat-Eintrags.
2. Der Lagebericht schreibt unter „Phase … fragt dich:“ weiterhin die erste Zeile der ersten Frage — Wortlaut unverändert.
3. Nach jeder Antwort über `apply_answer` geht die Zusammenfassung der Session raus, auch wenn danach noch eine Anfrage offen ist und der Status `waiting` bleibt.
4. `src/lib/bindings/OpenQuestion.ts` existiert; `StationView.ts` trägt `openQuestion: OpenQuestion | null` und kein `question` mehr.
5. Die Karte zeigt bis Phase 3 wie bisher den Text der ersten Frage.
6. `pnpm check` grün.

## Checkliste

- [ ] `model.rs`: Struct `OpenQuestion` nach Kontrakt mit Doc-Kommentar „Älteste offene Rückfrage oder Rechte-Abfrage einer Session, so wie der Chat sie zeigt.“; `Question` und `QuestionKind` in den bestehenden Import `use crate::agents::event::{Attachment, ModelId};` aufnehmen. In `StationView` das Feld `question` samt Kommentar ersetzen durch `open_question: Option<OpenQuestion>` mit Kommentar „Älteste offene Anfrage der Session; nur bei `Asking`.“
- [ ] `mod.rs`: in `SessionLive` `question` ersetzen durch `open_question: Option<OpenQuestion>` (Kommentar „Älteste offene Anfrage — Rückfrage oder Rechte-Abfrage.“); `OpenQuestion` in `use model::{…}` aufnehmen; in `station_view` `question: …` ersetzen durch `open_question: session.filter(|_| state == StationState::Asking).and_then(|session: &SessionLive| session.open_question.clone())`.
- [ ] `registry/implementation.rs`, `live_of`: `let open_question = state.pending.first().map(|(request_id, request): &(String, PendingRequest)| OpenQuestion { seq: request.seq, request_id: request_id.clone(), question_kind: request.question_kind, questions: request.questions.clone() });` und im `SessionLive { … }` `open_question` statt `question`. `OpenQuestion` in den Import `use crate::implementation::model::{…}` aufnehmen; `Question` aus `use crate::agents::event::{Attachment, Question}` entfernen, falls danach unbenutzt (der Compiler meldet es).
- [ ] `report.rs`, `now_lines`: `if let Some(question) = &station.question { entries.push(format!("{INDENT}{}", first_line(question))); }` ersetzen durch `if let Some(question) = station.open_question.as_ref().and_then(|open: &OpenQuestion| open.questions.first()) { entries.push(format!("{INDENT}{}", first_line(&question.question))); }`; `OpenQuestion` importieren wie die übrigen Modelltypen in der Datei.
- [ ] `registry.rs`, `apply_answer`: direkt nach `state.set_question_answer(outbox, seq, label);` die Zeile `outbox.summary_dirty = true;` mit Kommentar „Die Karte der Station zeigt die älteste offene Anfrage; bleibt danach eine offen, ändert sich der Status nicht — die Zusammenfassung muss trotzdem raus.“ `set_status` bleibt unverändert (die frühe Rückkehr verhindert Zusammenfassungen ohne Änderung bei jedem Ereignis).
- [ ] `examples/gen-bindings.rs`: `OpenQuestion` in den Import `implementation::model::{…}` aufnehmen, `OpenQuestion::export_all(&cfg)?;` direkt vor `StationView::export_all(&cfg)?;`. Danach `pnpm bindings`.
- [ ] `StationCard.tsx`, `renderQuestion` (Übergang bis Phase 3): `const text: string | undefined = station.openQuestion?.questions[0]?.question;` — bei `station.state !== 'asking' || text === undefined` `null`, sonst `<p className="station-card__question">{text}</p>`.
- [ ] Doc: noch nichts (Phase 3).
- [ ] `pnpm check` grün, Commit `feat(implementation): offene Anfrage vollständig an die Station`.

## Report-Back
