# Design-Entwurf: Master-Session einer Umsetzung

**Status:** freigegeben am 2026-11-17 mit dem Plan „Master-Session“ (Taiga-Story 99, [ADR 070](../../decisions/070-master-session.md)). Gebaut ist der Kern (Phase 99.1: Migration 30, Melden und Annehmen getrennt, Werkzeug `implementation_review_station`, Wecken am Zugende, Sperren für Stationen, Master wechseln, Bindings und die mechanischen Fälle für den Zustand „gemeldet“); die Oberfläche folgt in Phase 99.2, die Abnahme in der echten App in 99.A.

## Ansehen

- Die Datei [mockup.html](mockup.html) ist eigenständig und läuft direkt im Browser (Doppelklick genügt, kein Server nötig).
- Oben wählen „Szenario“ (Gemeldet · Master entscheidet, Rückgaben ausgeschöpft, Master ausgefallen), „Farbschema“ und die Variante der Leiste; ein Klick auf eine Station wählt sie, „Master: … · ändern“ im Kopf der Umsetzung öffnet die Auswahl.
- Sprungmarken für Bilder und Links: `#theme=light|dark&scen=normal|cap|gone&side=a&sel=<Station>&pop=1` (`side`: Variante der Leiste, `sel`: Station in der Kartei, `pop=1`: Auswahl des Masters offen).

## Begriffe

- **Master:** die Session, die eine Umsetzung leitet; anfangs ihr Ersteller. Er entscheidet über die Meldungen seiner Stationen.
- **Gemeldet:** Die Station hat mit `implementation_complete_station` an den Master berichtet; er entscheidet, sobald ihr Zug beendet ist.
- **Rückgabe:** Der Master schickt die Meldung mit einer Nachricht zurück; höchstens dreimal, danach entscheidet der Benutzer.
- **Freigeben:** Der Master gibt eine Station „durch dich“ frei; sie bleibt beim Benutzer.

Verbindlich sind die Einträge im [Glossar](../../glossary.md).

## Gezeigt wird

| Teil | Inhalt | Phase |
| :- | :- | :- |
| Zustand „gemeldet“ | hellblau wie „läuft“ in der Dringlichkeit; in Zeile, Zeitleiste und Kartei | 99.2 (Zustand und Bindings in 99.1) |
| Abschnitt „Bericht an den Master“ in der Kartei | Betreff, Notiz, Befund, Rückgaben „1 von 3“ | 99.2 (Felder der Ansicht in 99.1) |
| Rückgaben ausgeschöpft | „wartet auf dich“, Annehmen und Zurückgeben für den Benutzer; „Zurückgeben“ gesperrt, solange das Feld leer ist | 99.2 (Befehle in 99.1) |
| Marke „freigegeben“ | an einer Station „durch dich“, die der Master freigegeben hat | 99.2 (Feld `released` in 99.1) |
| Kopf der Umsetzung | „Master: #N · ändern“ mit Auswahl der Sessions des Vorhabens, Warnung bei Ausfall | 99.2 (Befehl `implementation_set_master`, Felder `masterSessionId` und `masterMissing` in 99.1) |
| Leiste | die Umsetzung als eine Zeile unter ihrer Master-Session; darunter nur Stationen, die dich brauchen (Variante A) | 99.2 |
| Symbole | Umsetzungen eckig, Sessions und Stationen rund, dieselben Farben | 99.2 |
| Chat des Masters | die Weck-Nachricht des Verwalters und seine Antwort mit dem Werkzeug | 99.1 (Wortlaut, gemessen) |

## Entscheidungen des Nutzers

| # | Entscheidung (2026-11-17) |
| :- | :- |
| U1 | Nach der Meldung ist die Station **gemeldet**. Der Master entscheidet erst, wenn ihr Zug beendet ist: Abgabe, Fehler oder Abbruch. Eine Pause ist kein Zugende. |
| U2 | Ein Werkzeug des Masters mit drei Antworten: **annehmen**, **zurückgeben** (höchstens drei Mal, danach entscheidet der Benutzer), **freigeben** (Stationen „durch dich“). |
| U3 | Der Verwalter weckt den Master bei jeder Meldung, jeder Rückfrage, jedem Fehler und jeder Antwort ohne Meldung einer Station. Eine Antwort ohne Meldung geht nur an den Master. |
| U4 | Rückfragen und Rechte-Abfragen einer Station beantwortet weiter der Benutzer. |
| U5 | Kein `report_to_master`, kein `get_master_session`, kein `delegate`. Die erste Nachricht der Station nennt den Master. |
| U6 | Die Meldung verlangt eine Notiz an den Master. `result` (eine Zeile, höchstens 200 Zeichen) bleibt und ist der Betreff. |
| U7 | Stationen sprechen nur mit ihrem Master: `verwalter_send_message_to` an andere und `verwalter_open_session` sind für Stationen gesperrt. |
| U8 | Fällt der Master aus (gelöscht, Fehler), bestimmt der Benutzer einen neuen: jede Session des Vorhabens außer Stationen, jederzeit im Kopf der Umsetzung. Der neue Master bekommt die Umsetzung und die offenen Meldungen. Eine Kontingent-Pause ist kein Ausfall. |
| U9 | Seitenleiste: Die Umsetzung steht als **eine Zeile** unter ihrer Master-Session; darunter nur Stationen, die dich brauchen (Variante A). Ändert [ADR 065](../../decisions/065-stationen-in-der-sidebar.md). |

## Annahmen

- Die Notiz steht bis zu den Referenzen (Story 147) vollständig in der Weck-Nachricht.
- Der Master ist eine gewöhnliche Session; ein Wechsel schreibt keine Nachricht an den alten Master.
- Eine Station mit Fehler oder Abbruch bekommt die Rückgabe über Neustart und Senden.
- Der Entwurf zeigt die Variante A der Leiste als gewählte; die anderen stehen zum Vergleich da.

## Platzhalter im Entwurf

- Namen der Vorhaben, Sessions und Stationen sind ausgedacht.
- Die Nachrichten im Chat des Masters und die Antworten des Werkzeugs sind gekürzt.
- Die Zeiten („vor 4 min“) stehen fest und folgen keiner Uhr.

## Abweichungen vom Entwurf

| Stelle | Entwurf | Gebaut | Grund |
| :- | :- | :- | :- |
| Master in der Leiste | „◆ Master“ hinter dem Namen der Session | nur „◆“ (Tooltip „Master: leitet die Umsetzung unter ihr“) | Der Name der Session kürzt sich sonst in der schmalen Leiste; die Zeilenhöhe bleibt. |
| Marken in der Zeitleiste | zwischen Name und Zustand | rechts im Zustand, vor dem Zustandstext („zurückgegeben 1/3“, „✓ freigegeben“) | Dort stehen schon die Chips „setzt automatisch fort“ und „Kontingent“; die Zeile wird nicht höher. |
| Rückgaben über drei | nicht gezeigt | „zurückgegeben 4“ ohne „von 3“, sobald der Benutzer über die Grenze des Masters hinaus zurückgibt | Der Benutzer gibt ohne Grenze zurück; „4 von 3“ wäre falsch. Die 3 steht nur als Beschriftung in `masterTexts.ts`, ob der Master noch darf, sagt `returnsLeft`. |
| Zeile in der Kartei | „gemeldet 11:42 · Zug beendet 11:44 · …“ | „gemeldet 17.11., 22:38 · zurückgegeben n von 3“ | Der Core liefert nur `reportedAt`, nicht das Ende des Zugs; Datum und Uhrzeit wie an anderen Stellen (`stampText`). |
| Punkt „gemeldet“ | einfacher Punkt in Hellblau | Punkt in Hellblau mit „↑“ | Ohne Farbe unterscheidbar; die Punkte der Zeitleiste tragen ihr Zeichen im Punkt. |
| Symbol der Umsetzung bei „gemeldet“ | Ring mit Kern in Hellblau | Zeichen von „läuft“, Farbe nach `urgency` | In der Dringlichkeit steht „gemeldet“ wie „läuft“ (ADR 070); nur die Station trägt die eigene Farbe. |
| Warnleiste bei Ausfall | mit Link „Master ändern“ | ohne Link | „ändern“ steht im Kopf, daneben, in derselben Ansicht. |
| Auswahl des Masters | „#6 Architekt (neu)“ als Beispiel | „+ Neue Session als Master“ legt eine Session an und macht sie zum Master; Stationen stehen gesperrt mit „(Station)“, der heutige Master trägt „◆“ | Der Brief verlangt den Eintrag; eine neue Session gibt es sonst nicht. |
| Leiste B, C und „heute“ | zum Vergleich | nicht gebaut | Verworfene Varianten. |
| Eckige Symbole | Glyph-Funktion des Entwurfs | `StatusIcon` mit `shape="square"`; gleiche Formbedeutung, Farben wie die runden; „Fehler“ als Rahmen mit Ausrufezeichen (das runde Symbol trägt ein Dreieck), „Übergabe“ und „Skript“ als Rahmen mit Pfeil bzw. Klammern | Der Entwurf kennt diese beiden Zustände nicht. |

## Stand der Sichtprüfung

Phase 99.2, in der App mit Wegwerf-`HOME`, einer Kopie der Datenbank und der Attrappe der Kommandozeile, 1800 × 1000, Hell und Dunkel, einzeln mit dem Entwurf verglichen:

- **Gesehen (Hell und Dunkel):** „gemeldet“ in Zeile und Kartei mit Bericht, Befund, „zurückgegeben 1 von 3“ und dem Hinweis auf den Master; Rückgaben erschöpft mit „Annehmen“ und „Zurückgeben“ (leere und nur aus Leerzeichen bestehende Nachricht sperrt den Knopf); „✓ vom Master freigegeben“ in Kartei und Zeitleiste; Kopf „Master: #1 Architekt · ändern“; Auswahl offen; Master ausgefallen (Warnleiste, roter Name, „Wartet auf einen neuen Master“); Leiste Variante A mit der Umsetzung unter ihrem Master, bei ausgefallenem Master im Vorhaben; eckiges Symbol für „wartet auf dich“ in Leiste und Kopf.
- **Gesehen, nur im Gebrauch:** „Zurückgeben“ durch den Benutzer (die Station arbeitet wieder, die Meldung entfällt, die Zählung steigt) und Master wechseln (Warnung weg, Umsetzung wandert unter den neuen Master).
- **Nicht gesehen:** „Annehmen“ durch den Benutzer; die gekürzte Liste (ab fünf Sessions) mit weggeklapptem Master (nur im Selbsttest); die eckigen Symbole für läuft, Fehler, pausiert, neu und fertig (nur der Quelltext, kein Bild); Windows und macOS.
