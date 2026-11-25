# 076 — Umsetzungen je Session, Agenten öffnen keine Sessions

**Status:** umgesetzt · **Datum:** 2026-11-25 · Taiga #216, #217 · Migration 36 · Nachträge in [ADR 052](052-session-werkzeuge.md) und [ADR 070](070-master-session.md) (D9, D11)

## Kontext

Ein Agent konnte mit `verwalter_open_session` Sessions öffnen (ADR 052), und ein Vorhaben hatte höchstens eine unfertige Umsetzung (ADR 048). Zwei Sessions eines Vorhabens konnten deshalb nicht je eine Umsetzung führen, und ein Agent, der Arbeit verteilen wollte, hatte zwei Wege: das Werkzeug und die Umsetzung. Der Nutzer will nur den zweiten (#216), und mehrere Umsetzungen je Vorhaben, jede unter ihrem Master (#217). Entwurf: `docs/design/2026-11-25_umsetzungen-je-session/` (`mockup.html`, `README.md`).

## Entscheidungen (S1 bis S9)

- **S1 (Ticket #216):** Keine Session öffnet Sessions per Werkzeug. Die Wege der Oberfläche und `verwalter_send_message_to` bleiben.
- **S2:** `verwalter_open_session` ist aus `SessionTools::tools()` gelöscht, nicht ein Werkzeug, das immer ablehnt: Ein Werkzeug, das immer ablehnt, kostet jeden Aufruf einen Zug, und die Sichtbarkeit gibt es nur je Werkzeugsatz (`rpc.rs`). Ein Aufruf liefert „Unbekanntes Werkzeug“. `ToolRequest::OpenSession` (alte Einträge im Verlauf), die Spalte `projects.self_release_open_session` und `SelfRelease.open_session` bleiben, damit alte Zeilen lesbar bleiben und Gäste ein älteres Format lesen. Die Zeile „Selbstfreigabe für neue Sessions“ entfällt in Übersicht und Einstellungen. Den Hinweis auf die zwei Wege trägt `session_tools_prompt` (Vertrag V1): „Neue Sessions legst du nicht selbst an. Soll eine andere Session arbeiten, leg mit mcp__verwalter__implementation_create eine Umsetzung an; führst du schon eine, schlag mit mcp__verwalter__implementation_edit eine Änderung ihres Plans vor.“
- **S3:** Höchstens **eine unfertige** Umsetzung je Master-Session. Nach dem Ende oder dem Abbruch (beide setzen `finished_at`) darf dieselbe Session eine neue anlegen. Migration 36 legt den partiellen UNIQUE-Index `implementations_one_unfinished_per_master` auf `implementations(master_session_id) WHERE finished_at IS NULL` an; der Bestand erfüllt ihn, weil es bisher höchstens eine je Vorhaben gab. `create_implementation` prüft vor dem Anlegen unter derselben Datenbank-Sperre (`unfinished_led_by`) und nennt die laufende Umsetzung: „Du führst schon die Umsetzung „<Name>“. Ändere ihren Plan mit implementation_edit, oder leg eine neue an, wenn sie fertig ist.“ Die Grenze „eine je Vorhaben“ (ADR 048) fällt weg.
- **S4:** Ein Masterwechsel (`implementation_set_master`) auf eine Session, die eine andere unfertige Umsetzung führt, wird abgelehnt: „Die Session führt schon die Umsetzung „<Name>“.“ Der `MasterPicker` sperrt sie mit „führt schon „<Name>““ (`unfinishedLedBy` in `masterTexts.ts`); er bekommt dafür die Liste der Umsetzungen von `useImplementations`.
- **S5:** Gelöschte oder archivierte Master bleiben „Master ausgefallen“ mit Auswahl; keine Änderung.
- **S6:** Eine Station legt keine Umsetzung an: „Eine Station legt keine Umsetzung an. Melde dein Ergebnis mit implementation_complete_station.“ Die Prüfung steht in `create_implementation`, nach der Prüfung der Befehle des Plans (ein Plan mit Fehler nennt zuerst den Fehler). Die Sperre `open_refusal` (ADR 070, D9) entfällt mit dem Werkzeug.
- **S7:** `implementation_of_session` sucht der Reihe nach: die unfertige Umsetzung, die die Session führt; die ihrer Station; die ihrer Rückfrage (`inquiry_of`); die zuletzt geführte, auch eine fertige oder abgebrochene. Es gibt **keinen** Rückfall auf das Vorhaben mehr, denn ein Vorhaben kann mehrere haben. Ohne Treffer: „Du führst keine Umsetzung. Leg eine mit implementation_create an.“ (`NO_IMPLEMENTATION`). Die Beschreibungen von `implementation_get` und `implementation_cancel` nennen die Reihenfolge.
- **S8:** Der Reiter „Changes“ der Umsetzung zeigt weiter die Changes des Vorhabens und heißt jetzt „Changes im Vorhaben“. Eine Reichweite nur über die Sessions der Umsetzung ist ein Folge-Ticket; der Lagebericht zählt `changed_files` weiter für das Vorhaben.
- **S9:** Der Vault-Skill `architect-workflow` nennt `verwalter_open_session`. Die Umsetzung ändert keine Skill-Datei, sondern legt einen Patch-Vorschlag mit alten und neuen Zeilen im Vault ab; den Skill spielt der Architekt ein.

## Folgen

- Die Leiste setzt Umsetzungen schon je Master (`sidebarPlacement.ts`); zwei Umsetzungen eines Vorhabens stehen unter zwei Mastern. Das Dashboard schlüsselt nach Umsetzungs-ID.
- Zwei Umsetzungen eines Vorhabens teilen sich den Reiter „Changes im Vorhaben“ (S8) und den Lagebericht-Zähler.
- Stationen und Rückfragen finden ihre Umsetzung über Station und `inquiry_of`; ein Master einer fertigen Umsetzung über „zuletzt geführt“.
- Wirkung auf Tests: Die Tests zu `verwalter_open_session` in `sessions/registry.rs` und `implementation/master.rs` prüfen jetzt, dass das Werkzeug fehlt; der Test zur Wahl der Umsetzung (`a_master_gets_its_open_implementation_not_a_cancelled_one`) legt die abgebrochene zuerst an, weil der Index nur eine unfertige je Master zulässt, und prüft statt des Rückfalls aufs Vorhaben, dass es keinen gibt.

## Messung (F4)

Wortlaut des neuen Satzes: siehe S2. Verfahren wie ADR 061 und 070: `claude -p --model haiku` (Version 2.1.296), `--tools ""`, `--setting-sources ""`, `--no-session-persistence`, Wegwerf-`HOME` mit verlinktem `.claude`, ohne `CLAUDE*`-Variablen, Systemprompt `prompt.txt` aus `implementation-probe --wake-texts` als `--append-system-prompt`, 2026-11-25. 18 Läufe: drei Aufträge („eine andere Session soll die Tests schreiben“, „starte eine neue Session für den Review“, „öffne eine Session für die Migration“), je dreimal ohne eigene Umsetzung und dreimal mit dem Vorsatz „Du führst schon die Umsetzung „Release“.“; jeweils mit dem Nachsatz „Sag in einem Satz, mit welchem Werkzeug du das tust, und ruf es noch nicht auf.“

- **Verweigerungen: 0 von 18.** Kein Lauf nannte `verwalter_open_session`.
- **Ohne eigene Umsetzung (9 Läufe):** 9 von 9 nennen `implementation_create`.
- **Mit einer führenden Umsetzung (9 Läufe):** 5 nennen nur `implementation_edit`, 2 beide, 2 nur `implementation_create`. Alle 18 Antworten nennen eines der beiden Werkzeuge; der Satz „führst du schon eine, schlag … vor“ trifft Haiku also nicht immer (2 von 9 würden eine zweite anlegen, was der Core mit dem Satz aus S3 ablehnt und zu `implementation_edit` zurücklenkt).
- Zwei frühere Durchgänge mit je 18 Läufen ohne den Nachsatz (nur der Auftrag, bzw. „Sag in einem Satz, was du als Nächstes tust“) verweigerten ebenfalls 0 Mal; ohne Werkzeuge rief Haiku zuerst `implementation_agent_types` und `verwalter_get_sessions` auf, und es sagte meist nur „ich lege eine Umsetzung an“, ohne das Werkzeug zu nennen. Die Messung belegt daher nur, dass Haiku den Satz annimmt und das Werkzeug richtig wählt, wenn er danach gefragt wird; echte Werkzeuge standen nicht zur Verfügung.

Die Rohdaten liegen im Scratchpad der Umsetzungs-Session.

## Prüfung

Rust: `cargo test --lib implementation::` mit `two_unfinished_implementations_of_one_project_run_under_two_masters`, `a_second_unfinished_implementation_from_the_same_session_is_refused`, `after_the_end_the_same_session_may_create_another_implementation`, `a_station_creates_no_implementation`, `a_master_change_to_a_leading_session_is_refused`, `an_inquiry_session_gets_its_implementation_before_the_last_led_one`, `a_master_gets_its_open_implementation_not_a_cancelled_one` (Reihenfolge ohne Rückfall aufs Vorhaben), `the_database_index_allows_one_unfinished_implementation_per_master`, `migration_36_adds_the_index_to_a_database_of_version_35`; `a_station_may_not_open_a_session` in `implementation/master.rs` prüft, dass `tools/list` das Werkzeug nicht nennt. Oberfläche: `pnpm selftest` (Sperre im Picker in `sidebarPlacement.selftest.ts`).
