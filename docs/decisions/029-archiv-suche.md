# 029 — Archiv durchsuchen und wiederherstellen

**Status:** angenommen · **Datum:** 2026-10-08

## Kontext

Archivierte Vorhaben verschwanden bisher endgültig aus der Oberfläche: Verlauf und Datenbankzeilen blieben, aber es gab keinen Weg zurück und keine Möglichkeit, eine alte Lösung wiederzufinden, an deren Inhalt man sich nur noch ungefähr erinnert.

## Optionen

- **Ort:** (a) eigene Hauptansicht „Archiv“; (b) Dialog über der App, geöffnet aus dem Sidebar-Fuß.
- **Suche:** (a) nur Vorhaben-Namen; (b) Namen und Chat-Text, linear über die gespeicherten Einträge; (c) SQLite-FTS5-Index.
- **Groß/Klein:** (a) SQL-`LIKE`; (b) `to_lowercase()` in Rust.
- **Seiten:** (a) alle Treffer auf einmal; (b) Seiten zu 20 über `offset`.

## Entscheidung

**(b), (b), (b) und (b).**

- **Dialog statt Hauptansicht** — das Archiv ist selten gebraucht und hilft nicht beim laufenden Agent-Workflow (Critical Rule 1).
- **Lineare Suche ohne Index.** FTS5 bräuchte eine Migration und einen zweiten Datenbestand neben dem Verlauf; bei Archiven im einstelligen MB-Bereich lohnt das nicht. Reine Namenssuche fände nichts, woran man sich nur inhaltlich erinnert. Durchsucht werden `text` von Nutzernachricht, Antwort und Fehler sowie die Antwort auf eine Rückfrage — nicht Denken, Werkzeuge, Pfade oder JSON-Schlüssel. Eine billige Vorprüfung auf dem JSON (klein geschrieben) entscheidet, ob ein Eintrag überhaupt gelesen wird; bei Begriffen mit Zeichen, die JSON maskiert (`"`, `\`, Steuerzeichen), entfällt sie.
- **Groß/Klein in Rust.** `LIKE` kennt Groß/Klein nur für ASCII, „Über“ fände „über“ nicht. Fundstellen werden an Zeichengrenzen geschnitten; ein Zeichen, das klein geschrieben länger wird, verschiebt den Ausschnitt nicht.
- **Seiten zu 20, im Core festgelegt.** Der Aufrufer nennt nur `offset`; der Core liest bis zum 21. Treffer, um `hasMore` zu bestimmen, und bricht dann ab. Reihenfolge `archived_at` absteigend, `id` als Tiebreak. Höchstens 3 Ausschnitte je Vorhaben; ein Vorhaben außerhalb der Seite wird nur als Treffer erkannt (Name oder erster Ausschnitt).
- **Datenbank je Vorhaben bzw. Session kurz sperren**, nie für die ganze Suche: laufende Sessions schreiben über dieselbe Verbindung und sollen nicht warten.
- **Wiederherstellen** hebt `archived_at` für Vorhaben und Sessions in einer Transaktion auf. Vorher werden die Sessions aus der Datenbank gebaut (dieselbe Funktion wie beim App-Start); scheitert das, bleibt das Vorhaben archiviert. Der Verlauf lädt erst beim Öffnen einer Session, der Agent startet erst mit der nächsten Nachricht. Weil das Archivieren jede Session abbricht, setzt das Wiederherstellen abgebrochene Sessions zurück auf „fertig“ (mit Agent-Verlauf) bzw. „neu“ (ohne): sie sind wieder schreibbar und setzen per `--resume` fort; der Hinweis auf den abgelaufenen Cache lässt die Wahl zwischen Weiterschreiben und neuer Session.
- **Keine Migration** — `archived_at` gibt es in `projects` und `sessions` schon.

## Konsequenzen

- Die Suchzeit wächst linear mit dem archivierten Verlauf. Wird sie spürbar, ist FTS5 ein Folgeplan.
- `offset` zählt Vorhaben, die die Oberfläche gerade zeigt. Wird ein Vorhaben wiederhergestellt, rücken die übrigen nach; die Oberfläche muss danach mit der verkleinerten Anzahl weiterladen.
- App-Worktrees von Sessions vor ADR 010 fehlen nach dem Archivieren; der Agent-Start legt sie wie bisher neu an. Haupt-Checkouts und Ticket-Worktrees waren nie weg.
- Kein Löschen aus dem Archiv und keine Vorschau des Verlaufs.
