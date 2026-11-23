# Design-Entwurf: Prompt-Vorlagen

**Status:** gewählt am 2026-11-23 (Taiga-Story 58; [ADR 075](../../decisions/075-prompt-vorlagen.md)). Aufbau und Stil wie [2026-11-22_nutzernachrichten-einreihen](../2026-11-22_nutzernachrichten-einreihen/README.md).

## Ansehen

[mockup.html](mockup.html) ist eigenständig und läuft direkt im Browser. Der Schalter oben wechselt Farbschema, Lage des Namensfelds, den Editor zwischen gültig und Syntaxfehler und die Darstellung des Diffs. Bild 2 lässt sich ausprobieren: tippen, Haken setzen, Namen ändern, „OK“ oder „Verwerfen“. Alle Namen und Texte sind Beispieldaten. Die gestrichelte Umrandung markiert, was neu ist. Ohne Vorlagen sieht „Neues Vorhaben“ aus wie bisher.

## Bilder

| Bild | Zeigt |
|---|---|
| 1A | Drop-down als eigene Zeile „Vorlage · keine ▾“ über dem Auftrag, Muster wie „Gruppe“ |
| 1B | Knopf ▤ in der Leiste des Eingabefelds neben + und /, Menü mit den Vorlagen und „Vorlagen in den Einstellungen bearbeiten …“ |
| 2A | Formular über dem Auftrag, einspaltig: Kopf „Bugfix · Formular“ mit „Verwerfen“ und „OK“, Textfelder mit Hilfetext, Checkbox; Auftrag gesperrt mit Sperrzeile; Namensfeld unter dem Auftrag mit „aus der Vorlage“ oder „von Hand“; Hinweis am Start |
| 2B | Formular neben dem Auftrag, zweispaltig, Seite breiter |
| 3 | Nachfrage „Text ersetzen?“, wenn schon Text im Feld steht |
| 4 | Einstellungen › Agent › Prompt-Vorlagen: Liste unter den Agententypen, Marke „Namensvorlage“, „Bearbeiten“ und „Löschen“ |
| 5 | Editor mit Formular-Vorschau, gültig oder mit Syntaxfehler samt Zeile und Spalte |
| 6 bis 9 | Kasten im Chat für einen Vorschlag per MCP, Änderung als Diff, Vorschlag mit Syntaxfehler, beantwortete Zeile |

## Wahl

Es gilt (Wahl des Benutzers, 2026-11-23): Drop-down **B** (Knopf ▤ in der Leiste), Formular **A** (über dem Auftrag), Namensfeld **B** (unter dem Auftrag), Diff **A** (untereinander, Lane M). Die Syntax prüfen Oberfläche und Core. Maßgeblich für diese Lane sind die Bilder 1B, 2A, 3, 4 und 5.

## Umsetzung

Screenshots der echten App unter [umsetzung/](umsetzung/), aufgenommen mit eigener Datenbank und drei Beispielvorlagen (Bugfix, Taiga-Story, Review):

| Datei | Zeigt | Bild im Mockup |
|---|---|---|
| [01-ohne-vorlagen.png](umsetzung/01-ohne-vorlagen.png) | „Neues Vorhaben“ ohne Vorlagen: kein ▤, der bisherige Hinweis und Satz zum Namen | Hinweis über Bild 1 |
| [02-einstellungen-liste.png](umsetzung/02-einstellungen-liste.png) | Einstellungen › Agent › Prompt-Vorlagen mit drei Vorlagen, Marke „Namensvorlage“ | 4 |
| [03-editor.png](umsetzung/03-editor.png) | Editor „Vorlage bearbeiten“ zu „Bugfix“ mit Formular-Vorschau: „Fehler“ mit Hilfetext, „Betroffene Komponente“, Checkbox „Mit Test“, Namensfeld | 5, gültig |
| [04-editor-syntaxfehler.png](umsetzung/04-editor-syntaxfehler.png) | getippter offener Abschnitt: „Zeile 8, Spalte 1: Der Abschnitt „offen“ ist nicht geschlossen.“, Vorschau und Speichern gesperrt | 5, Syntaxfehler |
| [05-editor-name-doppelt.png](umsetzung/05-editor-name-doppelt.png) | Speichern mit dem Namen „review“: Der Core lehnt ab, „Der Name „review“ gehört schon einer anderen Vorlage.“ | Text zu Bild 5 |
| [06-menue.png](umsetzung/06-menue.png) | Menü am Knopf ▤ mit den Vorlagen und dem Weg in die Einstellungen | 1B |
| [07-formular-dunkel.png](umsetzung/07-formular-dunkel.png), [07-formular-hell.png](umsetzung/07-formular-hell.png) | Formular offen über dem Auftrag, Auftrag gesperrt mit Sperrzeile, `/` und Diktat aus, mehrzeiliges Feld, `<b> & "x"` unverändert, Namensfeld „aus der Vorlage“; hell und dunkel | 2A |
| [08-start-gesperrt.png](umsetzung/08-start-gesperrt.png) | „Vorhaben starten“ gesperrt mit „Erst das Formular mit „OK“ schließen.“; Strg+Enter davor startete nichts | 2A, Fuß |
| [09-name-von-hand.png](umsetzung/09-name-von-hand.png) | Name von Hand geändert („von Hand“), danach im Formular weitergetippt: Der Auftrag folgt, der Name bleibt | 2A, Namensfeld |
| [10-nach-ok.png](umsetzung/10-nach-ok.png) | nach „OK“: Auftrag frei und ergänzt, Namensfeld bleibt, Start frei | 2A, nach „OK“ |
| [11-nachfrage.png](umsetzung/11-nachfrage.png) | neue Wahl „Taiga-Story“ nach eigener Änderung: „Text ersetzen?“ | 3 |
| [12-neue-wahl.png](umsetzung/12-neue-wahl.png) | nach „Ersetzen“: neues Formular, neues Namensfeld „aus der Vorlage“ | 3, danach |
| [13-verwerfen.png](umsetzung/13-verwerfen.png) | „Verwerfen“: der Text von vor der Wahl, ohne Namensfeld | 2A, nach „Verwerfen“ |

In 09, 10, 11 und 13 steht `>b>` statt `<b>`: Der Klick-Helfer des Laufs tippte `<` dort mit Umschalt; in 07 ist es korrigiert, die App gibt die Zeichen unverändert weiter.

### Vorschläge per MCP (Bilder 6 bis 9)

Aufgenommen wie die Bilder der Agententypen ([ADR 063](../../decisions/063-agententypen-per-mcp.md)): eigener Benutzerordner, eigener Port, privater Bildschirm, Klicks über XTEST; statt `claude` lief eine Attrappe, die Vorschläge kamen über `mcp-probe` mit dem Schlüssel der Session. Kein Kontingent verbraucht. Fenster 1280 × 800.

| Datei | Zeigt | Bild im Mockup |
|---|---|---|
| [14-vorschlag-neu-dunkel.png](umsetzung/14-vorschlag-neu-dunkel.png) | neuer Vorschlag „Refactoring“: Name, Namensvorlage, Vorlagentext eingeklappt mit Zähler, Formular, Knöpfe, Hinweis zum Eingabefeld | 6 |
| [15-vorschlag-bearbeiten-dunkel.png](umsetzung/15-vorschlag-bearbeiten-dunkel.png) | „Bearbeiten …“: Editor „Vorschlag bearbeiten“ mit Untertitel und „Übernehmen“ | 6, Text |
| [16-uebernommen-mit-aenderungen-dunkel.png](umsetzung/16-uebernommen-mit-aenderungen-dunkel.png) | nach „Übernehmen“ im Editor mit geändertem Namen: „Übernommen mit Änderungen“ (`edited: true`) | 9 |
| [17-aenderung-diff-kopf-dunkel.png](umsetzung/17-aenderung-diff-kopf-dunkel.png), [17-aenderung-diff-dunkel.png](umsetzung/17-aenderung-diff-dunkel.png), [17-aenderung-diff-kopf-hell.png](umsetzung/17-aenderung-diff-kopf-hell.png), [17-aenderung-diff-hell.png](umsetzung/17-aenderung-diff-hell.png) | Änderung an „Bugfix“ in der Ansicht „Diff“: Namensvorlage und Vorlagentext untereinander, Formular mit „+ ☐ Mit Review“, „Unverändert: Name.“; Kopf und unteres Ende, dunkel und hell | 7 |
| [18-aenderung-vorher-dunkel.png](umsetzung/18-aenderung-vorher-dunkel.png), [19-aenderung-nachher-dunkel.png](umsetzung/19-aenderung-nachher-dunkel.png) | „Vorher“ und „Nachher“ mit allen Feldern und dem Formular | 7 |
| [20-verwerfen-begruendung-hell.png](umsetzung/20-verwerfen-begruendung-hell.png) | „Verwerfen“ öffnet „Begründung für Claude (optional)“ | 6, Text |
| [22-veraltet-hell.png](umsetzung/22-veraltet-hell.png) | „Übernehmen“, nachdem die Vorlage in der Datenbank geändert wurde: Meldung „veraltet“, nichts gespeichert, Kasten offen | — |
| [23-freitext-verworfen-hell.png](umsetzung/23-freitext-verworfen-hell.png) | Text im Eingabefeld verwirft den offenen Vorschlag mit diesem Text | — |
| [24-beantwortet-hell.png](umsetzung/24-beantwortet-hell.png) | die Zeilen nach der Antwort: „Übernommen mit Änderungen“, zweimal „Verworfen“ mit Begründung, „Übernommen“, „Pausiert“ mit „Der Agent wurde gestoppt; nichts gespeichert.“ | 9 |
| [25-vorschlag-gezeigt-hell.png](umsetzung/25-vorschlag-gezeigt-hell.png) | „Vorschlag zeigen“ an einer verworfenen Änderung | 9 |

Bild 8 hat in der App kein Bild: Ein Vorschlag mit Syntaxfehler öffnet keinen Kasten. `mcp-probe` bekam `isError: true` mit „text: Zeile 2, Spalte 1: Der Abschnitt „Mit Review“ ist nicht geschlossen.“; ein leerer Name zusammen mit einem Fehler in der Namensvorlage kam als zwei Zeilen zurück.

Belegt im Lauf (Ergebnis aus `mcp-probe`, Datenbank mit `sqlite3`): `prompt_template_list` liefert die Vorlagen ganz; übernommen nach Bearbeiten (`edited: true`, gespeichert der neue Name), Verwerfen mit Begründung, veraltet (nichts angewendet, Kasten offen), freier Text verwirft, „1“ und Enter außerhalb des Eingabefelds lassen den Kasten offen, Übernehmen (`edited: false`, gespeichert), „Pausieren“ bei offenem Kasten („Der Kasten wurde geschlossen, ohne dass der Nutzer entschieden hat.“, nichts gespeichert); die Selbstfreigabe des Vorhabens war dabei ganz an. In 20 und 24 fehlt das Komma der Begründung: Der Klick-Helfer tippt keins.

## Abweichungen vom Mockup

- **Editor (Bild 5):** Der Vorlagentext ist ein einfaches Textfeld in Festbreitenschrift; die Tags sind nicht farbig hervorgehoben, und die fehlerhafte Stelle ist nicht im Text markiert. Die Meldung unter dem Feld nennt Zeile und Spalte.
- **„Text ersetzen?“ (Bild 3):** ein Dialog ohne Kopfleiste und ohne ✕; Esc und „Abbrechen“ lassen alles, wie es war.
- **Sperrzeile:** ein Schloss-Symbol statt des Emoji.
- **Liste und Menü:** Der Vorlagentext steht ohne Kommentare in einer Zeile.
- **„Vorlagen in den Einstellungen bearbeiten …“** öffnet die Einstellungen in ihrer ersten Kategorie, nicht direkt bei „Agent“.
- **Kasten (Bilder 6 und 7):** Der Vorlagentext steht wie der Systemprompt der Agententypen eingeklappt mit „Ganz zeigen“ und Zähler, auch in „Vorher“ und „Nachher“; das Mockup zeigt ihn dort ganz. Das Formular steht als Chips wie die Stationsnamen der Agententypen; in „Diff“ steht es, sobald sich Text oder Namensvorlage ändern, neue Felder mit „+“, wegfallende durchgestrichen. Der Schalter „Diff | Vorher | Nachher“ ist der Segment-Schalter der Einstellungen; nach dem Aushängen beim Scrollen steht er wieder auf „Diff“.
- **Karte im Dashboard:** Ein Vorschlag heißt dort wie jeder Kasten eines Werkzeugs „Freigabe Nachricht“ (wie bei den Agententypen); ihr Fenster verweist auf den Chat der Session.
- **„In den Einstellungen öffnen“** öffnet die Kategorie Agent; eine Kategorie mit Hinweis (etwa ein nicht lesbares Konto) geht wie bei den Agententypen vor.
