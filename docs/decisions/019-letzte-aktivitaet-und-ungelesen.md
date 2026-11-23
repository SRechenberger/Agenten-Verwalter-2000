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
