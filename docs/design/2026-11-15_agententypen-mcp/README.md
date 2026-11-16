# Design-Entwurf: Agententypen per MCP vorschlagen

**Status:** freigegeben am 2026-11-15 mit dem Plan „Agententypen per MCP vorschlagen“ (Taiga-Story 109, [ADR 063](../../decisions/063-agententypen-per-mcp.md)). Gebaut sind das begrenzte Lesen der Dateien beim Start (Phase 1), die Werkzeuge mit Kasten-Mechanik und Befehl (Phase 2) und die Oberfläche des Kastens samt Dialog (Phase 3). `mockup.html` ist seit der Freigabe unverändert; wo die gebaute Oberfläche abweicht, stehen die Abweichungen unten, und die Bilder in `umsetzung/` zeigen die gebaute App.

## Ansehen

- Die Datei [mockup.html](mockup.html) ist eigenständig und läuft direkt im Browser (Doppelklick genügt, kein Server nötig).
- Oben wählen „Schritt“ (1 bis 10) einen Zustand und „Farbschema“ hell oder dunkel; jeder Knopf im Kasten und im Dialog ist klickbar. Sprungmarke: `mockup.html#schritt=4`.

## Gezeigt wird

| Schritt | Inhalt | Gebaut |
| :- | :- | :- |
| 1 | Neuer Typ vorgeschlagen: alle Felder, Dateien hervorgehoben, Systemprompt eingeklappt | Phase 3 |
| 2, 3 | Verwerfen mit Begründung, danach die Zeile „Verworfen“ | Phase 3 |
| 4 | „Vorschlag bearbeiten“: der Dialog der Einstellungen, vorbefüllt und voll bedienbar, mit dem neuen Hinweis unter „Dateien“ | Phase 3 |
| 5 | Übernehmen mit Fehler (Name vergeben): Meldung im Kasten, er bleibt offen | Phase 3 |
| 6 | Übernommen | Phase 3 |
| 7 | Änderung an einem bestehenden Typ: nur geänderte Felder, alt und neu | Phase 3 |
| 8 | Veraltet: der Typ wurde seit dem Vorschlag geändert | Phase 3 |
| 9 | Abgebrochen | Phase 3 |
| 10 | Startfehler auf der Karte bei einer FIFO oder einer zu großen Datei | Phase 1 (die Karte zeigte Startfehler schon vorher) |

## Umsetzung

Die Bilder in [`umsetzung/`](umsetzung/) stammen aus der gebauten App (Linux, Fenster 1280 × 800, dunkel und hell). Sie entstanden in einem Lauf mit eigenem Benutzerordner, eigenem Port und privatem Bildschirm, die Klicks über XTEST. Statt `claude` lief eine Attrappe, die Vorschläge kamen über `mcp-probe` mit dem Schlüssel der Session; Kontingent wurde nicht verbraucht. Unter Windows ist nichts davon gelaufen.

| Bild | Zeigt |
| :- | :- |
| `schritt-1-neuer-typ-dunkel.png`, `schritt-1-neuer-typ-hell.png` | Schritt 1 |
| `schritt-1-ganz-gezeigt-dunkel.png` | Schritt 1 nach „Ganz zeigen“ |
| `schritt-2-verwerfen-begruendung-dunkel.png` | Schritt 2: Feld „Begründung für Claude (optional)“ mit „Verwerfen“ und „Zurück“ |
| `schritt-3-verworfen-dunkel.png`, `schritt-3-vorschlag-gezeigt-dunkel.png` | Schritt 3, zugeklappt und nach „Vorschlag zeigen“ |
| `schritt-4-dialog-oben-dunkel.png` | Schritt 4: Titel, Untertitel und vorbefüllte Felder |
| `schritt-4-dialog-dateien-dunkel.png`, `schritt-4-dialog-dateien-hell.png` | Schritt 4: eine Datei per ✕ entfernt, eine per Pfad und „Hinzufügen“, ein Skill per Enter; darunter der neue Hinweis |
| `schritt-4-dialog-rueckfrage-hell.png` | Esc mit ungespeicherten Änderungen: dieselbe Rückfrage wie in den Einstellungen |
| `schritt-5-fehler-dunkel.png` | Schritt 5: ein Typ „Critical-Review“ wurde nach dem Vorschlag angelegt |
| `schritt-6-uebernommen-dunkel.png`, `schritt-6-uebernommen-mit-aenderungen-dunkel.png` | Schritt 6, direkt übernommen und nach Speichern im Dialog |
| `schritt-7-aenderung-dunkel.png`, `schritt-7-aenderung-prompt-dunkel.png` | Schritt 7 wie im Entwurf, dazu eine Änderung an Systemprompt und Skills |
| `schritt-8-veraltet-dunkel.png` | Schritt 8 |
| `schritt-9-abgebrochen-dunkel.png` | Schritt 9 nach „Session abbrechen“ (darüber eine Zeile nach „Pausieren“) |
| `freitext-verworfen-dunkel.png` | Text im Eingabefeld bei offenem Vorschlag verwirft ihn mit dem Text als Begründung |
| `einstellungen-geoeffnet-dunkel.png` | „In den Einstellungen öffnen“ zeigt die Kategorie Agent mit dem übernommenen Typ |
| `einstellungen-dialog-unveraendert-dunkel.png` | Der Dialog aus den Einstellungen heißt weiter „Agententyp bearbeiten“ und speichert mit „Speichern“ |

Belegt im Lauf (Werkzeug-Ergebnis aus `mcp-probe`, Datenbank mit `sqlite3`): Verwerfen mit Begründung (`rejected` mit dem Text, nichts gespeichert), Übernehmen (`accepted`, `edited: false`), Speichern im Dialog mit entfernter Datei (`accepted`, `edited: true`, gespeichert ist der bearbeitete Stand), Übernehmen einer Änderung, veraltet (nichts angewendet, Kasten offen), Pausieren und Abbrechen bei offenem Kasten (Fehler „Der Kasten wurde geschlossen, ohne dass der Nutzer entschieden hat.“, nichts gespeichert), Freitext verwirft, Speichern aus den Einstellungen.

## Abweichungen vom Entwurf

- **Hinweis zum Eingabefeld:** Unter den Knöpfen steht „Text im Eingabefeld unten verwirft den Vorschlag mit dieser Begründung.“, denn freier Text verwirft einen offenen Vorschlag (ADR 063); der Entwurf zeigt den Satz nicht.
- **Knöpfe** sind die der Einstellungen (30 px hoch statt 28 px); „Übernehmen“ zeigt „Übernimmt …“, solange der Befehl läuft.
- **Meldungen im Kasten** stehen als „Feld: Meldung“ mit dem Text des Core. „Veraltet“ steht ohne Feld und endet wie im Core mit „Verwirf den Vorschlag.“; der Entwurf hängt „Claude kann neu vorschlagen.“ an. Ein Fehler des Befehls steht als „Nicht beantwortet: …“ in derselben Liste. Die Liste bleibt bis zur nächsten Antwort stehen, auch wenn man inzwischen „Verwerfen“ öffnet; sie, das offene Feld „Begründung“ und ihr Text überstehen auch, dass der Verlauf den Kasten beim Scrollen aushängt.
- **Zähler des Systemprompts:** im Zahlenformat der App („252 / 16.000“) und in UTF-16-Einheiten wie im Dialog; der Entwurf schreibt „16 000“.
- **Änderung:** Ein geänderter Systemprompt steht alt (durchgestrichen) über neu, „Ganz zeigen“ klappt beide auf. Wegfallende Stationen und Skills stehen durchgestrichen hinter den übrigen. Geänderte Dateien zeigen die neue Liste hervorgehoben; der Entwurf zeigt diese Fälle nicht.
- **Leere Felder** heißen „keine“, ein leerer Systemprompt „keiner“.
- **Geschlossen ohne Entscheidung:** Neben „Abgebrochen“ kennt der Core „Pausiert“ (Knopf „Pausieren“) und „Nicht beantwortet“; alle drei stehen als graue Marke mit „Der Agent wurde gestoppt; nichts gespeichert.“.
- **„In den Einstellungen öffnen“** öffnet die Kategorie Agent (eine Kategorie mit Hinweis, etwa ein nicht lesbares Konto, geht wie sonst vor) und fehlt bei einem entfernten Vorhaben, denn der Typ liegt dann beim Host.
- **Name in der beantworteten Zeile:** Sie nennt immer den Namen des Vorschlags. Hat der Dialog den Namen geändert, steht dort „Übernommen mit Änderungen“, gespeichert ist aber der neue Name. Den gespeicherten Namen kennt die Zeile nicht: Die Antwortzeile des Core trägt ihn nicht, und den Core ändert diese Phase nicht; ein Wert nur im Speicher der Oberfläche ginge beim Neuladen und für einen Gast verloren. „In den Einstellungen öffnen“ zeigt den gespeicherten Typ.
- **„Vorschlag zeigen“** klappt die Felder unter der Zeile auf („Vorschlag ausblenden“ klappt zu).
- **Dialog:** derselbe `AgentTypeDialog` wie in den Einstellungen (Breite, mittig, nur der Rumpf rollt, Rückfrage bei Ungespeichertem, Text in den Eingabefeldern gilt beim Übernehmen), siehe die Abweichungen in [`../2026-11-07_agententypen/README.md`](../2026-11-07_agententypen/README.md). Der Untertitel nennt die Nummer der Session. Prüfmeldungen stehen am Feld, „veraltet“ am Namen. Wird der Kasten anders beantwortet, während der Dialog offen ist (Stopp, freier Text), schließt der Dialog. Bei einem entfernten Vorhaben fehlt „Datei wählen …“: Der Dateidialog zeigte die Dateien des Gastes, gespeichert und gelesen wird der Pfad aber auf dem Host; der Pfad wird dann getippt.
- **Gesperrt** sind die Knöpfe während einer Antwort und solange ein Gast steuert oder der Host fehlt.

## Platzhalter im Entwurf

- „Datei wählen …“ setzt im Entwurf einen Beispielpfad ein; die App öffnet den Dateidialog des Systems.
- Alle Namen, Pfade und Texte sind Beispieldaten.
