# 004 — Persistenz: Chat-Einträge als JSON in SQLite, Sessions überleben, Agent-Prozesse nicht

**Status:** angenommen · **Datum:** 2026-09-28

## Kontext

Bis Meilenstein 2a liegen Sessions und Verlauf im Speicher des Core; ein App-Neustart verliert alles. Das Konzept ([konzept.md](../konzept.md), Abschnitte 29–31) sieht SQLite mit den Tabellen `sessions`, `messages`, `events` vor und verlangt, dass ein UI-Absturz weder Agent noch Session noch Worktree verliert. Zwei Punkte passen nicht zur Wirklichkeit der Anbindung ([ADR 003](003-claude-anbindung.md)): Die Oberfläche arbeitet mit `ChatEntry`-Einträgen, nicht mit rohen Agent-Ereignissen, und `claude.exe` beendet sich, sobald seine Standardeingabe schließt — mit dem Ende der App endet also auch der Agent-Prozess.

## Optionen

- **Speicherformat:** (a) `messages` und `events` wie im Konzept, der Verlauf wird beim Laden aus Ereignissen neu berechnet; (b) eine Zeile pro `ChatEntry` (`session_id`, `seq`, JSON-Nutzlast), genau das, was die Oberfläche anzeigt.
- **Agent-Prozesse über das App-Ende hinaus:** (a) ein losgelöster Wächterprozess, der `claude.exe` hält und über eine benannte Pipe mit der App spricht; (b) Session überlebt, Prozess wird bei Bedarf mit `--resume` neu gestartet.
- **Reihenfolge:** (a) Meilenstein 3 (Worktrees) vor 4 (Persistenz), wie in [PROJECT.md](../PROJECT.md); (b) 4 vor 3.

## Entscheidung

- **(b) Eine Zeile pro `ChatEntry`.** Die Oberfläche liest genau diese Form; eine Neuberechnung aus Ereignissen wäre ein zweiter Übersetzer, der mit dem ersten auseinanderlaufen kann. Eine `events`-Tabelle kommt erst, wenn ein Meilenstein sie braucht (Git-Invalidierung in Meilenstein 5). Änderungen an einem Eintrag (Werkzeug fertig, Rückfrage beantwortet) überschreiben die Zeile mit gleicher `seq`.
- **(b) Session überlebt, Prozess nicht.** Beim Start lädt der Core alle nicht archivierten Sessions; war eine aktiv (`starting`, `running`, `waiting`), wird sie `paused`, offene Rückfragen tragen „Nicht beantwortet“, laufende Werkzeug-Zeilen „unterbrochen“. Der Agent startet erst mit der nächsten Nachricht, mit `--resume` und dem vollen Verlauf. Ein Wächterprozess (a) wäre die einzige Weise, Agenten über das App-Ende hinaus laufen zu lassen; er ist ein eigenes Vorhaben mit eigener Fehlerklasse (verwaiste Prozesse, Pipe-Wiederaufbau) und gehört nicht in „ohne Datenverlust weiterarbeiten“. Die Regel „Agent-Prozesse überleben die UI“ wird deshalb zu „**Sessions** überleben die UI“.
- **Ruhende Agenten beenden.** Eine Session im Status `completed` oder `paused` gibt ihren Prozess (rund 390 MB) nach einer Frist frei — Standard 30 Minuten, per `VERWALTER_IDLE_SECONDS` änderbar. Die nächste Nachricht startet ihn mit `--resume` neu. Hintergrundprozesse, die der Agent selbst gestartet hat (Dev-Server), enden damit: Das Beenden trifft seine ganze Prozessgruppe, und eine angenommene Station gibt ihren Agenten sofort frei ([ADR 074](074-agentenprozesse-beenden.md)).
- **`--resume` nur nach erstem Kontakt.** Claude kennt eine Session erst nach `system/init`; `--resume` auf eine unbekannte ID bricht mit „No conversation found“ ab (belegt am 2026-09-28). Die Session merkt sich deshalb `has_agent_history` (mindestens einmal `system/init` gesehen) und startet sonst mit `--session-id`.
- **(b) Meilenstein 4 vor 3.** Meilenstein 3 muss Repositories, Branch und Worktree-Pfad je Session dauerhaft speichern. Ohne Datenbank wäre das eine Wegwerf-Ablage im Speicher oder in einer Datei, die Meilenstein 4 wieder ersetzt.
- **Technik:** `rusqlite` mit eingebautem SQLite (`bundled`, keine System-Bibliothek nötig), eine Verbindung hinter einem Mutex, WAL-Modus, Schema über nummerierte Migrationen (`PRAGMA user_version`), Datei `<Benutzerordner>\.verwalter\verwalter.db`. Geschrieben wird unter der Session-Sperre, in der Reihenfolge Session-Sperre → Datenbank-Sperre, nie umgekehrt.

## Konsequenzen

- Nach einem harten Beenden der App fehlt höchstens der Eintrag, der in dem Augenblick geschrieben wurde; die Laufzeit-Summe einer gerade laufenden Antwort wird nicht nachgetragen.
- Wird ein Eintrag nicht geschrieben (Datenbankfehler), landet die Meldung im Protokoll der Session, nicht im Chat. Beim Laden gilt der Verlauf bis zur ersten Lücke in `seq`.
- Der Verlauf einer Session liegt nach dem ersten Zugriff vollständig im Speicher des Core (nicht der Oberfläche); die Oberfläche liest weiter seitenweise. Bei sehr langen Verläufen ist das die Stelle für eine spätere Umstellung auf Lesen direkt aus SQLite.
- Wer eine Session archiviert, blendet sie aus; die Daten bleiben, Worktrees ohne offene Änderungen räumt seit [ADR 005](005-repositories-und-worktrees.md) das Archivieren auf.
