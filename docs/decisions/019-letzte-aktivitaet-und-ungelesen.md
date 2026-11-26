# 019 — Sidebar nach letzter Aktivität, Hinweis auf Ungelesenes

**Status:** angenommen · **Datum:** 2026-10-02

## Kontext

Die Sidebar verteilte die Vorhaben auf drei Gruppen („Braucht dich“, „Läuft“, „Abgeschlossen“), innerhalb einer Gruppe stand das zuletzt angelegte Vorhaben oben und die Sessions darin aufsteigend nach Nummer. Wer mehrere Vorhaben parallel laufen lässt, sucht so ständig nach dem, in dem gerade etwas passiert ist, und sieht nicht, in welcher Session der Agent abgegeben hat, während er woanders war.

## Optionen

- **Was als Aktivität zählt:** (a) jeder neue Chat-Eintrag; (b) nur Senden des Users und Abgeben des Agenten (Rückfrage, fertig, Fehler).
- **Wo die Zeitpunkte liegen:** (a) nur im Speicher; (b) in SQLite, Spalten der Tabelle `sessions`.

## Entscheidung

**(b) und (b).**

- **Aktivität = Senden oder Abgeben.** Werkzeug-Aufrufe und Zwischentexte zählen nicht: bei (a) tauschten gleichzeitig laufende Vorhaben im Sekundentakt die Plätze.
- **Gruppen entfallen,** eine Liste, das Vorhaben mit der jüngsten Aktivität oben, darin die Sessions ebenso. Den Status zeigt weiter das Symbol an der Zeile.
- **Ungelesen** heißt: letzte Aktivität jünger als der Zeitpunkt, an dem der User die Session zuletzt gesehen hat. Gesehen ist eine Session, solange die Oberfläche sie zeigt (Chat- oder Changes-Reiter); Übersicht, Einstellungen und „Neues Vorhaben“ zeigen keine Session. Eigene Nachrichten machen nie ungelesen.
- **Speicherort:** `last_activity_at` und `seen_at` in SQLite (Critical Rules 3 und 4: Persistentes in die Datenbank, überlebt den Neustart). „Wird gerade gezeigt“ liegt nur im Speicher des Core (`SessionRegistry.viewed`, `SessionState.is_viewed`), die Oberfläche meldet es über `session_set_viewed`.
- **Altbestand:** Migration 8 setzt die letzte Aktivität auf die letzte Nutzernachricht (sonst `created_at`) und markiert jede bestehende Session als gesehen.

## Konsequenzen

- Die Liste bleibt während der Läufe ruhig; sie bewegt sich erst beim Senden, bei einer Rückfrage, beim Fertigwerden oder bei einem Fehler.
- Nach dem Update leuchtet nichts auf.
- Nach dem Start öffnet die App die zuletzt aktive Session automatisch; sie gilt damit als gesehen, ihr Punkt verschwindet beim Start (überholt: siehe Nachtrag unten).
- Die Übersichtskarten eines Vorhabens bleiben nach Nummer sortiert.

## Nachtrag (Dashboard, [ADR 071](071-dashboard.md))

Die App startet auf dem Dashboard. Es zeigt keine Session und gilt wie die Übersicht, die Einstellungen und „Neues Vorhaben“ als Seite ohne Session; deshalb markiert der Start keine Session mehr als gelesen, und der Punkt der zuletzt aktiven Session bleibt bis zum ersten Öffnen stehen. Die Auswahl der neuesten Session (`holdSession`) bleibt im Hintergrund, bis der Benutzer eine Session wählt.

## Nachtrag: Reihenfolge von Hand (Taiga 101)

Die Reihenfolge nach letzter Aktivität wird in diesem Punkt abgelöst; Ungelesen-Punkt und Status-Symbol bleiben. Aktivität (Senden, Rückfrage, Fertigwerden, Fehler) verschiebt keine Zeile mehr.

- **R1 Position:** Die Spalte `projects.position` (Migration 40, kleiner = weiter oben) hält eine Folge für alle Vorhaben; die Gruppe wählt nur aus. Die Migration vergibt sie in der alten Reihenfolge (jüngste Aktivität der Sessions, ohne Sessions der Anlegezeitpunkt, dann `created_at`), der erste Start zeigt also dasselbe wie vorher.
- **R2 Neues Vorhaben:** bekommt `MIN(position) - 1` (`next_top_position`) und steht oben in seiner Gruppe, auf jedem Weg, der ein Vorhaben anlegt, auch über die Fernsteuerung. Ein aus dem Archiv zurückgeholtes Vorhaben ebenso (`restore`): Seine alte Position könnte mit einer vergebenen zusammenfallen.
- **R3 Ein Befehl:** `project_move` ändert Position und Gruppe in einer Transaktion (`move_to`): vor ein Vorhaben (dessen Gruppe) oder ans Ende einer Gruppe bzw. von „Ohne Gruppe“; danach sind die Positionen der nicht archivierten Vorhaben 0, 1, 2 …. `project_set_group` bleibt für das Menü „In Gruppe verschieben“ und lässt die Position stehen. Klasse wie `project_set_group` (nie vom Gast an den Host, `remote/access.rs`). Ein Vorhaben eines Hosts ändert beim Gast nur seine lokale Gruppe; seine Position ist dort fest (`i32::MAX`: hinter den eigenen Vorhaben).
- **R4 Ziehen:** immer an (auch ohne Gruppen), beim Umbenennen aus. Ziele: zwischen zwei Vorhaben mit Einfügelinie (obere Hälfte einer Zeile davor, untere samt Kindzeilen dahinter), Gruppenkopf (an den Anfang der Gruppe), Fläche „Ohne Gruppe“ (ans Ende). Die Gruppen bleiben in Anlegereihenfolge. Ein Vorhaben eines Hosts zielt beim Gast nur auf Gruppenkopf und „Ohne Gruppe“, nie zwischen Vorhaben (`dropSpotFor`).
- **R5 Tastatur:** „Nach oben“ und „Nach unten“ im Menü der Zeile, innerhalb der eigenen Gruppe, über `project_move`. Vorhaben eines Hosts haben beides nicht und zählen in der eigenen Reihenfolge nicht mit: „Nach unten“ ist beim letzten eigenen Vorhaben einer Gruppe aus, auch wenn eines des Hosts folgt.
- **R6 Keine Umsortierung:** Die Leiste ordnet nur nach `position`; die Sortierung nach Aktivität (`sessionsByActivity`, `projectActivity`) ist entfernt.
- **R7 Sessions:** in der Leiste nach Nummer absteigend (`sessionsByNumber`); das Kürzen bleibt (ab 5 Sessions die ersten 3, dazu angeheftete und Master).
