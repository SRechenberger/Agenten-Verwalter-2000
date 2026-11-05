# Phase 2 — Codex: `/compact` und `/codex-review` im Übersetzer

## Kontext

Lesen: [README.md](README.md) (Abschnitt „Belegt“, Codex-Punkte; Entscheidungen „Der Codex-Review heißt `/codex-review`“, „`/compact` in Autark und Codex“, „Ausgabe nach dem Zusammenfassen“; Kontrakt `compact_boundary`), Protokoll der Probe [artifacts/codex-probe.log](artifacts/codex-probe.log) (Reihenfolge der Benachrichtigungen; das Probe-Skript daneben in `artifacts/codex-probe.cjs`), [ADR 031](../../decisions/031-codex.md), [docs/knowledge/codex-app-server.md](../../knowledge/codex-app-server.md), [docs/conventions/rust.md](../../conventions/rust.md).

Code: `src-tauri/src/codex/session.rs` (`Session`, `ActiveTurn`, `start_turn`, `interrupt`, `handle_notification`, `item_started`, `item_completed`, `token_usage_updated`, `turn_completed`, `result_error`, `SILENT_NOTIFICATIONS`), `src-tauri/src/codex/input.rs` (`user_input`), `src-tauri/src/codex/items.rs` (`SILENT_TYPES`), `src-tauri/src/codex/tokens.rs` (`TokenCount::from_notification`), `src-tauri/src/standalone/output.rs` (`compact_boundary` aus Phase 1).

Heute: `start_turn` macht aus jeder Nachricht `turn/start`; „/compact“ kommt als Text beim Modell an. `turn/started` steht in `SILENT_NOTIFICATIONS`. `contextCompaction` steht in `items::SILENT_TYPES` und erzeugt keine Zeile. `turn_completed` prüft die Turn-ID nicht und schließt den laufenden Turn ab.

## AK der Phase

1. Erster Text genau `/compact` oder `/compact <Text>` → `thread/compact/start {threadId}` statt `turn/start`; `<Text>` wird ignoriert. Nach `turn/completed` stehen auf stdout `compact_boundary` mit `trigger: "manual"`, `pre_tokens` (Stand vor dem Zusammenfassen, sonst `null`) und `post_tokens` (Stand danach, sonst `null`), danach `result` mit Erfolg.
2. Ein Zusammenfassen, das Codex mitten in einem gewöhnlichen Turn selbst auslöst (`contextCompaction` ohne `/compact`), schreibt dieselbe Zeile mit `trigger: "auto"`.
3. Erster Text genau `/codex-review` oder `/codex-review <Text>` → `review/start` mit `delivery: "inline"` und `target: {type: "custom", instructions}`, Anweisung wie unten. Die Antwort von Codex erscheint wie jede Antwort als Text im Verlauf (über das vollständige `agentMessage`), am Ende `result` mit Erfolg.
4. `/codex-review` ohne Repository-Ordner (`self.add_dirs` leer) → `result_error` mit `/codex-review braucht mindestens ein Repository in der Session.`, kein Aufruf an Codex.
5. Nach `/compact` meldet `get_context_usage` den Stand nach dem Zusammenfassen, nicht 0.
6. Esc während `/compact` oder `/codex-review` schickt `turn/interrupt` mit der ID aus `turn/started` und meldet sofort `result_aborted`.
7. `enteredReviewMode` und `exitedReviewMode` erzeugen keine Zeile und keine Meldung „noch nicht übersetzt“ auf stderr.
8. Jede andere Nachricht (auch `/review`, `/compactx`, Skills) verhält sich wie vorher.

## Checkliste

- [ ] `input.rs`: neues `pub enum Command { Compact, Review(String) }` und `pub fn command(content: &Value) -> Option<Command>`. Erster Textblock wie in `user_input` (`blocks_of`, erster Block mit `type == "text"`), `trim_start()`. Danach `/compact` exakt oder `/compact` + Leerraum → `Compact`; `/codex-review` exakt → `Review(String::new())`; `/codex-review` + Leerraum → `Review(rest.trim().to_owned())`; sonst `None`. Konstanten `COMPACT_COMMAND = "/compact"`, `REVIEW_COMMAND = "/codex-review"`.
- [ ] `input.rs`: `pub fn review_instructions(add_dirs: &[PathBuf], extra: &str) -> String`. Ergebnis: `Prüfe die noch nicht committeten Änderungen (git status, git diff, neue Dateien) in diesen Ordnern:\n- <Pfad 1>\n- <Pfad 2>\nNenne Fehler, Risiken und Verbesserungen jeweils mit Datei und Zeile. Ändere nichts.`, bei nicht leerem `extra` zusätzlich `\n\nZusätzlich: <extra>`.
- [ ] `session.rs` `ActiveTurn`: Feld `kind: TurnKind` mit `enum TurnKind { Message, Compact, Review }` und Feld `compact_before: Option<u64>`. Bestehende Konstruktion in `start_turn` setzt `kind: TurnKind::Message, compact_before: None`.
- [ ] `session.rs` `start_turn`: vor `input::user_input`:
  ```rust
  match input::command(content) {
      Some(input::Command::Compact) => return self.start_compact(),
      Some(input::Command::Review(extra)) => return self.start_review(&extra),
      None => {}
  }
  ```
- [ ] `session.rs` `start_compact(&mut self)`: `self.rpc.call("thread/compact/start", json!({ "threadId": self.thread_id }), CALL_TIMEOUT)`. Bei `Ok`: `self.turn = Some(ActiveTurn { id: String::new(), last_text: String::new(), error: None, is_aborted: false, kind: TurnKind::Compact, compact_before: None })`. Bei `Err(e)`: `self.output.line(&self.result_error(&e))`.
- [ ] `session.rs` `start_review(&mut self, extra: &str)`: leer `add_dirs` → `result_error` wie AK 4. Sonst `self.rpc.call("review/start", json!({ "threadId": …, "delivery": "inline", "target": { "type": "custom", "instructions": input::review_instructions(&self.add_dirs, extra) } }), CALL_TIMEOUT)`. Die Turn-ID aus `result.turn.id` lesen, wie in `start_turn`. Dann `ActiveTurn { id, …, kind: TurnKind::Review, compact_before: None }`; Fehler → `result_error`.
- [ ] `session.rs`: `"turn/started"` aus `SILENT_NOTIFICATIONS` nehmen (Array-Länge anpassen) und in `handle_notification` einen Zweig ergänzen. Ist ein Turn offen und `params.turn.id` vorhanden, wird diese ID in `turn.id` übernommen. Bei gewöhnlichen Turns ist sie dieselbe wie aus der Antwort. Beim Zusammenfassen gibt es sonst keine, und beim Review ist sie die, die gerade läuft (Probe: Antwort A, `turn/started` B). `interrupt` mit leerer `turn.id` (Esc vor `turn/started`) schickt kein `turn/interrupt`, meldet aber `result_aborted`.
- [ ] `session.rs` `item_started`: vor `items::started` — ist der Typ `contextCompaction` und ein Turn offen, setze `turn.compact_before = self.tokens.map(|count| count.input + count.output)` und gib zurück. Laut Probe kommt `thread/tokenUsage/updated` mit dem neuen Stand erst danach.
- [ ] `session.rs` `item_completed`: vor `items::completed` — ist der Typ `contextCompaction` und ein Turn offen, schreibe `output::compact_boundary(trigger, pre, post)` über `self.output.line`. Dabei ist `trigger` gleich `"manual"` bei `TurnKind::Compact` und `"auto"` sonst. `pre` ist `turn.compact_before` als `u32` (gesättigt), `post` ist `self.tokens.map(|count| count.input + count.output)` als `u32`. Danach zurückgeben.
- [ ] `items.rs`: `SILENT_TYPES` um `"enteredReviewMode"` und `"exitedReviewMode"` erweitern (Länge anpassen). Das Ergebnis des Reviews kommt laut Probe zusätzlich als vollständiges `agentMessage` und wird darüber angezeigt.
- [ ] `tokens.rs` `from_notification`: sind `inputTokens` und `outputTokens` von `last` beide 0, aber `last.totalTokens > 0`, gilt `input = totalTokens`, `output = 0`. Laut Probe meldet Codex nach `contextCompaction` `last = {totalTokens: 10841, inputTokens: 0, outputTokens: 0}`. Doc-Kommentar mit diesem Grund.
- [ ] Doc: `docs/knowledge/codex-app-server.md`: Tabelle „Benutzte Methoden“ um `thread/compact/start` und `review/start` ergänzen. „Benutzte Benachrichtigungen“ um `turn/started` (ID-Übernahme) und die Elemente `contextCompaction`, `enteredReviewMode`, `exitedReviewMode`. Unter „Eigenheiten“ die belegten Abläufe aus der README (zwei IDs beim Review, `last` nach dem Zusammenfassen) samt Stand `0.162.0-alpha.17.2` und Datum.
- [ ] Doc: `docs/code-map.md` Zeile „Codex“: `input` „… mit `/name`-Aufrufen, Bildern und den Befehlen `/compact`, `/codex-review`“.
- [ ] Doc: `docs/decisions/031-codex.md` unter Konsequenzen ein Satz: „`/compact` und `/codex-review` setzt der Übersetzer selbst um (ADR 034).“
- [ ] `pnpm check` grün; Commit `feat(codex): /compact und /codex-review im Übersetzer`.

## Report-Back
