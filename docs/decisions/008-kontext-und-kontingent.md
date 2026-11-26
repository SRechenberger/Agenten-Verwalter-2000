# 008 — Kontext und Kontingent anzeigen: Steueranfragen der Kommandozeile, Hilfsprozess fürs Kontingent

**Status:** angenommen · **Datum:** 2026-09-30

## Kontext

Die Session-Kopfzeile zeigt bisher nur einen Kontext-Balken (belegt / Fenster). Gebraucht werden eine Aufschlüsselung, was den Kontext belegt, und das Kontingent des Claude-Abos (5-Stunden- und Wochenfenster samt Verbrauchs-Treibern) — wie in den Fenstern „Context usage“ und „Account & Usage“ der VS-Code-Erweiterung. Die Protokoll-Fakten stehen in [claude-stream-json.md](../knowledge/claude-stream-json.md), Abschnitt „Kontext und Kontingent abfragen“. Randbedingung: Die UI zeigt, was die Kommandozeile meldet, nie eigene Schätzungen (AGENTS.md, Regel 2).

## Optionen

- **Kontext:** (a) eigene Rechnung aus den `usage`-Zeilen; (b) Steueranfrage `get_context_usage` an den Agenten der Session.
- **Speicher der Aufschlüsselung:** (a) SQLite; (b) nur im Speicher des Core.
- **Kontingent:** (a) kurz gestarteter Hilfsprozess mit `get_usage`; (b) Anfrage an den Agenten einer laufenden Session; (c) die `rate_limit_event`-Zeilen mitlesen.

## Entscheidung

- **Kontext: (b).** Die Kommandozeile kennt die Kategorien (Systemprompt, Werkzeuge, Memory-Dateien, Skills, Nachrichten); eine eigene Rechnung könnte sie nicht nachbilden. Der Core fragt am Ende jeder Antwort (`TurnEnded`, außer nach Abbrechen) und auf Wunsch der Oberfläche, solange der Agent zuhört.
- **Speicher: (b).** Die Aufschlüsselung ist ein Zwischenstand; nach einem App-Neustart zeigt das Fenster bis zur nächsten Antwort nur die Zahlen des Balkens. Ein beendeter Agent (Ruhe-Timer) lässt die letzte Aufschlüsselung stehen.
- **Kontingent: (a).** Ein `claude.exe` mit `--strict-mcp-config --no-session-persistence` im Arbeitsverzeichnis `<Benutzerordner>\.verwalter` beantwortet eine einzige Anfrage `get_usage` (rund 1,4 s) und wird beendet. Zeitlimit 20 s, nie zwei Abrufe gleichzeitig, ohne `force` höchstens ein Abruf je 30 s. (b) scheidet aus, weil es ohne laufenden Agenten nichts liefert und einen zweiten Weg für dieselbe Zahl bräuchte; (c) scheidet aus, weil der Inhalt der Zeilen nicht belegt ist.
- **Nachsichtiges Lesen:** `get_usage` ist im SDK als experimentell markiert; jedes Feld ist optional, ein unlesbares Feld ist leer, nie ein Fehler der ganzen Antwort.
- **Namen:** Feature `context` (Kontext-Aufschlüsselung je Session) und Feature `usage` (Kontingent des Abos); in der Oberfläche heißt Usage „Kontingent“.

## Konsequenzen

- Das Format beider Antworten kann sich mit jeder Claude-Version ändern; das Lesen ist darauf ausgelegt, eine Änderung als fehlende Anzeige statt als Absturz zu zeigen.
- Der Hilfsprozess kostet rund 1,4 s und einen kurzlebigen Prozess je Abruf; die Oberfläche fragt deshalb nur beim Einblenden der Kopfzeile, beim Öffnen des Fensters und alle 5 Minuten bei sichtbarem App-Fenster.
- Die Aufschlüsselung überlebt keinen Neustart.
- In einer lokalen Betriebsart keine Kontingent-Abfrage ([ADR 016](016-betriebsarten-und-lokales-modell.md)).
- Die Prozentwerte kommen zusätzlich live aus jeder Antwort in „Claude“ (`rate_limit_event`), samt Warnung bei knappem Kontingent ([ADR 033](033-warnhinweise.md)); der Hilfsprozess liefert weiter Plan, Aufschlüsselung und alle übrigen Fenster.

## Nachtrag: Fenster in der Zeile (Taiga 270)

- **Anlass:** Seit 2026-11-26 liefert das Ultra-Abo auf `get_usage` kein Session-Limit mehr: `rate_limits.five_hour` ist `null`, `limits` nennt nur `weekly_all` (und `weekly_scoped`). Die Zeile suchte nur `session` und blieb leer. Der Kern liest die Antwort richtig; `apply_rate_limit` und `usage_snapshot` bleiben, der Testfall steht in `stats.rs`.
- **Welche Fenster:** Eine reine Funktion (`shownWindows` in `usageWindows.ts`) liefert höchstens zwei Fenster, zuerst 5 Stunden, dann Woche. Claude: `session` und `weekly_all`; Codex: nach `windowMinutes` 300 und 10 080. Alles andere zeigt die Zeile nicht, auch `weekly_opus`, `weekly_sonnet` und Codex-Fenster anderer Dauer; das Popover listet weiter alle.
- **Zeile (V2):** Ein Platz von 32 px mit einem dünnen Balken je gezeigtem Fenster übereinander (oben 5 Stunden, unten Woche, je 3 px, 2 px Abstand), dann das Kürzel des höheren Fensters („5h“, „Wo“) und seine Zahl. Farbe und Symbol folgen dem höchsten Wert; ohne Fenster bleibt nur der Name. Der Tooltip nennt nach der ersten Zeile je Fenster Prozent und Rücksetzzeit.
