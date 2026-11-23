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
| 6 bis 9 | für Lane M: Kasten im Chat für einen Vorschlag per MCP, Änderung als Diff, Vorschlag mit Syntaxfehler, beantwortete Zeile |

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

## Abweichungen vom Mockup

- **Editor (Bild 5):** Der Vorlagentext ist ein einfaches Textfeld in Festbreitenschrift; die Tags sind nicht farbig hervorgehoben, und die fehlerhafte Stelle ist nicht im Text markiert. Die Meldung unter dem Feld nennt Zeile und Spalte.
- **„Text ersetzen?“ (Bild 3):** ein Dialog ohne Kopfleiste und ohne ✕; Esc und „Abbrechen“ lassen alles, wie es war.
- **Sperrzeile:** ein Schloss-Symbol statt des Emoji.
- **Liste und Menü:** Der Vorlagentext steht ohne Kommentare in einer Zeile.
- **„Vorlagen in den Einstellungen bearbeiten …“** öffnet die Einstellungen in ihrer ersten Kategorie, nicht direkt bei „Agent“.
