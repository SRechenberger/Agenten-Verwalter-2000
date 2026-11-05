# Phase 1 — Autark: `/compact` von Hand

## Kontext

Lesen: [README.md](README.md) (Entscheidungen „`/compact` in Autark und Codex“, „Ausgabe nach dem Zusammenfassen“; Kontrakt `compact_boundary`), [ADR 017](../../decisions/017-autarker-agent.md), [docs/conventions/rust.md](../../conventions/rust.md).

Code: `src-tauri/src/standalone/session.rs` (`Session::start_turn`, `poll_turn`, `replace_history`, `finish_turn`, `interrupt`, `ActiveTurn`), `src-tauri/src/standalone/turn.rs` (`make_room`, `TurnEvent`, `Outcome`, `TurnJob::notify`), `src-tauri/src/standalone/compact.rs` (`compact`, `summarize`), `src-tauri/src/standalone/context.rs` (`ContextMeasure`), `src-tauri/src/standalone/output.rs` (`compact_boundary`, `result_success`).

Heute: `start_turn` schickt jede Nachricht nach `invocation::expand_message` als Benutzer-Nachricht an das Modell; „/compact“ ist kein Skill und kommt als Text an. Verdichtet wird nur automatisch in `make_room` (ab der Schwelle aus `context::compact_threshold`). `output::compact_boundary()` schreibt die Zeile ohne Angaben.

## AK der Phase

1. Eine Nachricht, deren erster Text nach `trim_start` genau `/compact` ist oder mit `/compact ` beginnt, startet keinen Turn mit dem Modell. Sie startet ein Zusammenfassen des Transkripts im Hintergrund-Thread; die Nachricht selbst kommt nicht ins Transkript.
2. Nach Erfolg stehen auf stdout in dieser Reihenfolge `{"type":"system","subtype":"compact_boundary","compact_metadata":{"trigger":"manual","pre_tokens":<vorher>,"post_tokens":<nachher>}}` und eine `result`-Zeile mit Erfolg (Text leer). Das Transkript auf der Platte ist ersetzt, und der nächste Turn rechnet mit der neuen Messung.
3. Text nach `/compact ` steht in der Anfrage an das Modell als Schwerpunkt: `Summarize the conversation above. Focus on: <Text>`.
4. Leeres Transkript → sofort `result` mit Erfolg, ohne Thread und ohne `compact_boundary`.
5. Esc während des Zusammenfassens bricht ab (Cancel-Flag), schreibt `result_aborted` wie bei einem Turn und lässt das Transkript unverändert.
6. Ein Fehler des Modells → `result_error` mit `Verdichten fehlgeschlagen: <Meldung>`, Transkript unverändert.
7. Das automatische Verdichten in `make_room` schreibt `compact_boundary` mit `trigger: "auto"`, `pre_tokens = before`, `post_tokens = measure.tokens`.
8. Nachrichten, die während des Zusammenfassens eintreffen, warten in `queue` wie während eines Turns.

## Checkliste

- [ ] `output.rs`: `compact_boundary` bekommt die Signatur aus dem Kontrakt der README; Doc-Kommentar „wie die Claude-Kommandozeile; der Verwalter liest die Angaben für die Trennlinie im Verlauf (Plan Warnhinweise)“. Ausgabe: `compact_metadata` mit `trigger`, `pre_tokens`, `post_tokens` (`None` → `null`).
- [ ] `turn.rs` `make_room`: Aufruf auf `output::compact_boundary("auto", Some(before), Some(measure.tokens))` umstellen; `compact::compact(…, None)` (neuer Parameter, siehe unten).
- [ ] `compact.rs`: `compact` und `summarize` bekommen `focus: Option<&str>`. In `summarize` wird die letzte Anfrage `SUMMARY_REQUEST` bei `Some(focus)` mit nicht leerem `focus.trim()` zu `format!("{SUMMARY_REQUEST} Focus on: {}", focus.trim())`.
- [ ] `session.rs`: Konstante `COMPACT_COMMAND: &str = "/compact"` und freie Funktion `compact_request(content: &Value) -> Option<String>`. Sie nimmt den ersten Textblock (wie `invocation::first_text`; die Funktion dort auf `pub(super)` stellen und wiederverwenden) und prüft `trim_start()`: genau `/compact` → `Some(String::new())`; beginnt mit `/compact` + Leerraum → `Some(rest.trim().to_owned())`; sonst `None`.
- [ ] `session.rs` `start_turn`: als erste Zeile `if let Some(focus) = compact_request(content) { self.start_compact(&focus); return; }`.
- [ ] `session.rs` neue Methode `start_compact(&mut self, focus: &str)`:
  - Ist `self.transcript.messages` leer → `output::result_success(&self.shared.session_id, "", &self.model, self.shared.context_window)` schreiben, Ende.
  - `before = self.measure.tokens`; Cancel-Flag, Kanal `(event_sender, event_receiver)` und `ActiveTurn { cancel, events: event_receiver, is_aborted: false, transcript_error: None }` wie in `start_turn`.
  - Thread `agent-compact` mit Klonen von `Arc<Shared>`, `self.model`, `self.system_prompt.text`, `self.transcript.messages`, `focus`, `before`. Darin: `compact::compact(&shared.base_url, &model, &history, &cancel, focus_option)`.
    - `Ok(compacted)`: `messages = [system] + compacted`, `measure = ContextMeasure::estimated(&messages)`, dann `shared.output.line(&output::compact_boundary("manual", Some(before), Some(measure.tokens)))`, danach `TurnEvent::Compacted(compacted)`, `TurnEvent::Measured(measure)` und `TurnEvent::Done(Outcome::Completed(String::new()))`.
    - `Err(LlmError::Cancelled)`: `Done(Outcome::Aborted)`.
    - `Err(LlmError::Failed(m))`: `Done(Outcome::Failed(format!("Verdichten fehlgeschlagen: {m}")))`.
  - Die Ereignisse gehen über denselben Weg wie `TurnJob::notify`. Gibt es keinen `TurnJob`, sendet der Thread direkt über `event_sender`; Sendefehler werden ignoriert.
  - `poll_turn` und `finish_turn` bleiben unverändert und nehmen die Ereignisse an. Ausnahme: `finish_turn` hängt bei `is_aborted` den `INTERRUPTED_MARKER` an das Transkript. Nach einem abgebrochenen Zusammenfassen darf er **nicht** angehängt werden, denn es gab keine Benutzer-Nachricht. Dafür bekommt `ActiveTurn` das Feld `is_compact: bool`; `finish_turn` überspringt den Marker bei `is_compact`.
- [ ] Spawn-Fehler des Threads → `result_error` wie in `start_turn`.
- [ ] Doc: `docs/code-map.md` Zeile „Autarker Agent“: bei `compact` ergänzen „auch von Hand über `/compact`“.
- [ ] Doc: `docs/decisions/017-autarker-agent.md` unter Konsequenzen ein Satz: „`/compact` fasst den Verlauf auch von Hand zusammen; Text danach ist ein Schwerpunkt (ADR 034).“
- [ ] `pnpm check` grün; Commit `feat(standalone): /compact von Hand und Tokenstände in compact_boundary`.

## Report-Back
