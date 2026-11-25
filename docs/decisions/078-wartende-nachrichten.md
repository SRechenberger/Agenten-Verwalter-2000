# 078 — Wartende Nachrichten

**Status:** umgesetzt · **Datum:** 2026-11-25 · **Taiga:** Story 218 (Task 240)

## Kontext

Eine Nachricht an eine Session, die nicht sofort zugestellt wird, steht in `session_inbox` ([ADR 068](068-globale-warteschlange.md), [ADR 072](072-nachrichten-ueber-referenzen.md)). Sichtbar war nur die eigene Nachricht des Nutzers an eine ruhende Session (Taiga-Story 190) und eine fremde an eine ruhende Session als Eintrag der Warteschlange. Zeilen an laufende, fragende oder pausierte Sessions standen in keiner Liste, und eine wartende eigene Nachricht ließ sich nicht zurücknehmen.

## Entscheidungen

Die Entscheidungen N1 bis N9 stehen in `docs/design/2026-11-25_wartende-nachrichten/README.md`. Die Umsetzung:

- **`inbox_list`** (`sessions/registry/inbox.rs`, Klasse F, für Gäste gefiltert wie `agent_queue`) liest jede Zeile aus `session_inbox` und nennt Absender, Empfänger, Vorhaben, Betreff, Text, Eingang, Eintrag im Chat, `waitsFor` (`slot`, `turnEnd`, `answer`, `paused`, `start`) und bei `slot` den Platz. Der Grund folgt dem Zustand der Session: offene Rückfrage → `answer`; steht sie in der Warteschlange → `slot` mit ihrem Rang; neu → `start`; ruhend → `slot`; pausiert, abgebrochen oder gescheitert → `paused`; sonst → `turnEnd`.
- **`inbox://changed`** meldet am Ende jedes Durchgangs (`run_pass`) eine geänderte Liste, ohne Nutzlast. Jedes Einfügen und Löschen von Zeilen endet in einem Durchgang; die Meldung vergleicht die Liste mit der zuletzt gemeldeten (`inbox_changed`) und kommt deshalb auch, wenn sich Grund oder Platz ändern. Am Gast `remote://inbox-changed` mit `{ hostId }`, `FORWARDED` mit `Owner::EveryDevice`.
- **`chat_discard_queued(sessionId, entrySeq)`** (Klasse C) verwirft eine wartende eigene Nachricht: unter der Session-Sperre `queued` zurück und `discarded` an (`ChatEntry::User.discarded`, `serde(default)`), danach die Zeile löschen. Trägt der Eintrag `queued` nicht mehr, antwortet der Befehl `alreadySent` und ändert nichts. Die Freigabe (`deliver_user_messages`) sieht einen verworfenen Eintrag als `Stale`, löscht höchstens die Zeile und sendet nichts: Die Nachricht geht nie an den Agenten. Anhänge bleiben im Workspace.
- **Oberfläche:** „Verwerfen“ an der Blase und im Reiter „Nachrichten“, „verworfen“ und „Text übernehmen“ am verworfenen Eintrag, `ForeignWaiting` über dem Eingabefeld mit den fremden wartenden Nachrichten, Reiter „Plätze“ und „Nachrichten“ im Popover, alles flach (Regel Story 236).
- **Gast:** `remote_inbox_list(hostId)` (Klasse G), `inbox_list` liefert nur Nachrichten der Vorhaben, die das Gerät hält; keine Migration, keine Protokolländerung (ein älterer Host lehnt `inbox_list` ab, die Oberfläche zeigt dann nichts).

## Grenzen

- `WaitingMessage.text` (`serde(default)`) trägt den Text für den Chat; der Reiter „Nachrichten“ zeigt ihn nicht.
- Die Meldung `inbox://changed` hängt am Durchgang; eine Zeile, die gelöscht wird, ohne dass ein Durchgang folgt, wird erst mit dem nächsten gemeldet.
- Ein verworfener Eintrag zählt für TL;DR, Einordnung und Retro wie jeder Eintrag des Nutzers.
- Eine Messung des Satzes für den Agenten gibt es nicht: Die Lane ändert keinen Prompt.
