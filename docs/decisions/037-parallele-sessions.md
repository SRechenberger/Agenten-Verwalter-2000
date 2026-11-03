# 037 — Parallele Sessions im selben Ordner

**Status:** angenommen · **Datum:** 2026-10-22

## Kontext

Zwei Agenten, die im selben Ordner schreiben, überschreiben einander Änderungen und lesen halbfertigen Stand. Der Verwalter sieht beide Sessions, sagt es aber keiner von beiden. Auf dem Arbeitslaptop hängen dagegen oft mehrere Sessions am selben Repository, jede in ihrem eigenen Ticket-Worktree (ADR 010) — das ist der Normalfall und kein Grund zur Warnung.

## Optionen

- **(a) Warnung, sobald zwei Sessions dasselbe Repository angehängt haben.** Verworfen: meldet den Normalfall mehrerer Worktrees dauernd.
- **(b) Schreib-Ordner aus Ticket-Worktrees und geschriebenen Dateien.** Gewählt.
- **(c) Sperre statt Warnung.** Verworfen: der Verwalter hält keinen Agenten an.

## Entscheidung

- **Schreib-Ordner einer Session:** jeder Ticket-Worktree, den sie benutzt hat, und der Haupt-Checkout (bzw. Ordner ohne Git) eines angehängten Repositorys — Letzterer nur, wenn unter den geschriebenen Dateien der Session (ADR 014) eine liegt, die in keinem ihrer Ticket-Worktrees liegt. App-Worktrees zählen nicht, sie gehören je einer Session.
- **Nur laufende Agenten:** `Starting`, `Running`, `Waiting`, dazu ein Hauptagent, der auf einen Subagenten wartet. Eine Session, die gleich sendet, zählt schon mit.
- **Vergleich:** ganzer Ordner, über `attribution::normalize_path`.
- **Hinweis an die Oberfläche:** Command `session_overlaps`; die Oberfläche fragt alle 5 s und nach jedem `session://changed`.
- **Hinweis an den Agenten:** ein Satz, den `send` an die Nachricht hängt, wenn sich die Lage seit dem letzten Satz dieser Session geändert hat. Er steht nicht im Chat.

## Konsequenzen

- Die Oberfläche fragt alle 5 s; es gibt kein Ereignis zwischen Sessions.
- Dateien, die der Agent per Shell schreibt, zählen nur, soweit `session_files` sie kennt.
- Der Haupt-Checkout wird erst erkannt, wenn ein Schreibzugriff darauf vermerkt ist; vorher bleibt es still.
