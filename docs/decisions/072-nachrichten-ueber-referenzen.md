# 072 — Nachrichten über Referenzen: Betreff statt Volltext im Kontext des Empfängers

**Status:** angenommen und umgesetzt (Migration 32, Werkzeug `verwalter_read_message`, Pflichtfeld `subject`; Story Taiga #147, Phase 147.1).

## Kontext

Nachrichten zwischen Sessions, Rückgaben des Masters, die Notiz einer Meldung und die ersten Nachrichten einer Station standen im Ganzen im Kontext des Empfängers, auch wenn er den Inhalt nicht brauchte. Lange Texte füllten ihn.

## Entscheidung

Vom Benutzer entschieden (N8 bis N10, D5 bis D7):

- **Betreff und Referenz.** Der Agent bekommt eine Kopfzeile, den Betreff und eine Referenz `msg-<id>`; den Inhalt lädt er mit `verwalter_read_message { reference }`. Der Text an den Agenten hat drei Zeilen (`session_tools::reference`):

  ```text
  [Nachricht von Session <N> „<Titel>“, ID <id>]
  Betreff: <subject>
  Inhalt: msg-<id> — lies ihn mit verwalter_read_message, bevor du handelst.
  ```

- **Welche Wege.** `verwalter_send_message_to` (der Text), `verwalter_open_session` (erste Nachricht samt TL;DR), die Rückgabe des Masters (`implementation_review_station`, `return`; nur die des Masters, die des Benutzers bleibt direkt), die erste Nachricht einer Station (auch einer Fix-Runde; `/<skill>` bleibt allein die erste Zeile, Absender „Verwalter“, Betreff `Auftrag: Lane <lane> · <stage> in „<Umsetzung>“`) und die Notiz in „Gemeldet“ und „Gemeldet mit Befund“ (`Notiz: msg-<id> (lies sie mit verwalter_read_message)`, Betreff der Zeile ist das Ergebnis, der übrige Weck-Text bleibt). Alle anderen Weck-Texte und festen Texte („Unterbrochen“, „Fehler“, „Gestoppt“, „Fertig“, Frage, „Mach weiter.“, Neustart, neuer Master) bleiben direkt.
- **Eine Stelle.** `agent_message` und `station_message` in `sessions/registry/session_tools.rs` sind die einzigen Funktionen, die eine Zeile schreiben; `wake::message` und `start::first_message` bleiben unverändert (D5), die Referenz entsteht erst an der Zustellung, in `start_turn`. Den Text bauen die Funktionen in `session_tools/reference.rs`, dieselben nutzt die Probe `--wake-texts`.
- **Nur Claude.** Nur die Betriebsart Claude hat `verwalter_read_message`; in jeder anderen geht der ganze Text wie bisher (D6, `readable`).
- **Lesen.** Nur der Empfänger, solange seine Session besteht (N10): Tabelle `session_messages` mit `recipient_id … ON DELETE CASCADE`. Fremde, gelöschte und falsche Referenzen antworten „Unbekannte Referenz.“. Das Werkzeug ist lesend und in Nebenfragen erlaubt.
- **`subject` ist Pflicht** (N9): eine Zeile, 1 bis 120 Zeichen; leer, zu lang und mehrzeilig haben je einen eigenen Fehlersatz.
- **Chat (D5).** Der Empfänger sieht den ganzen Text, dazu den Betreff (`ChatEntry::SessionMessage.subject`, `ToolRequest::SendMessage.subject` und `::OpenSession.subject`, alle `Option` mit `serde(default)`; alte Einträge lesen sich). Bei „Gemeldet“ steht der Betreff schon im Text.
- **Posteingang.** Eine wartende Nachricht speichert ihren Betreff in zwei neuen Spalten von `session_inbox` (`subject`, und `note` für „Gemeldet“), nicht als Verweis auf `session_messages`: Die Zeile dort entsteht erst bei der Zustellung (sonst lägen Zeilen zu Nachrichten, die nie ankommen, und bei einer Änderung der Betriebsart zwischen Einreihen und Zustellen wäre die Referenz unlesbar). Eine Zeile ohne Betreff bleibt direkt.

## Messung (D7)

Haiku (`claude -p --model haiku`), Systemprompt der Session-Werkzeuge (`prompt.txt`), Wegwerf-`HOME` mit verlinktem `.claude`, ohne `CLAUDE*`-Variablen, MCP-Attrappe für `verwalter_read_message` (protokolliert jeden Aufruf). Je Text 6 Läufe, Texte aus `implementation-probe --wake-texts`.

| Text | Liest die richtige Referenz, bevor er handelt | Verweigerungen |
|---|---|---|
| `message-ref` (Nachricht mit Auftrag) | 6 von 6, als erstes Werkzeug | 0 |
| `reported-ref` („Gemeldet“ mit Notiz) | 6 von 6, als erstes Werkzeug | 0 |
| `first-message-ref` (Station, `/<skill>` vorn) | 6 von 6 vor jeder Arbeit; vorher nur `ToolSearch` und das Laden der Rolle (`Read` des Skills, durch die Zeile `/<skill>` verlangt) | 0 |

Beim dritten Text lädt das Modell zuerst die Rolle, die die erste Zeile verlangt, und liest dann die Referenz; danach beginnt erst die Arbeit (`Bash`/Suche). Streng als „erstes Werkzeug“ gelesen wären es 0 von 6; gemessen an „bevor er handelt“ sind es 6 von 6. Rohdaten: `raw-*.jsonl` und `calls-*.jsonl` im Scratchpad der Messung.

## Grenzen

- Die Messung gilt für Haiku und diese drei Texte mit einem Inhalt, der den Auftrag nicht vollständig beschreibt; sie belegt das Lesen, nicht die Güte der Arbeit danach.
- Eine Zeile `session_messages` wird nie gelöscht, außer mit der Session des Empfängers.
- Die Betriebsart wird bei der Zustellung geprüft; in einer Session, die nach dem Wechsel der Betriebsart weiterläuft, kann eine frühere Referenz unlesbar bleiben (der Chat hat den Text).
