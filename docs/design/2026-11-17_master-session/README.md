# Design-Entwurf: Master-Session einer Umsetzung

**Status:** freigegeben am 2026-11-17 mit dem Plan „Master-Session“ (Taiga-Story 99, [ADR 069](../../decisions/069-master-session.md)). Gebaut ist der Kern (Phase 99.1: Migration 30, Melden und Annehmen getrennt, Werkzeug `implementation_review_station`, Wecken am Zugende, Sperren für Stationen, Master wechseln, Bindings und die mechanischen Fälle für den Zustand „gemeldet“); die Oberfläche folgt in Phase 99.2, die Abnahme in der echten App in 99.A.

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

Folgen mit Phase 99.2.
