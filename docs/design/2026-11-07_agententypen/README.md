# Design-Entwurf: Agententypen

**Status:** freigegeben am 2026-11-08 mit dem Plan „Agententypen für Stationen“ (Taiga-Story 55, [ADR 060](../../decisions/060-agententypen.md)). Der Entwurf zeigt drei Teile: die Einstellungen (Schritte 1 bis 8), die Planung (Schritte 9 bis 11) und die Umsetzung (Schritte 12 bis 19). Gebaut sind bisher die Einstellungen; Planung und Umsetzung folgen in späteren Phasen und sind hier nur Entwurf. Wo die Umsetzung abweichen muss, wird erst dieser Entwurf geändert, dann der Code; `mockup.html` bleibt nach der Abnahme unverändert, die Abweichungen stehen unten.

## Ansehen

- Die Datei [mockup.html](mockup.html) ist eigenständig und läuft direkt im Browser (Doppelklick genügt, kein Server nötig).
- Oben schalten „Ansicht“ (Einstellungen, Planung, Umsetzung), „Betriebsart“ und „Farbschema“ um; der Rundgang führt durch 19 Schritte, jeder Knopf in der Anwendung geht auch ohne ihn. Sprungmarken: `mockup.html#schritt=7&schema=hell&betriebsart=codex`.

## Gezeigt wird

| Schritte | Inhalt | Gebaut |
| :- | :- | :- |
| 1 bis 8 | Einstellungen › Agent: Abschnitt „Agententypen“ (leer, Dialog mit Pflichtangaben, Modell und Denkaufwand, Speichern, Liste mit drei Typen, doppelter Stationsname, Löschen) | Phase 3 (Einstellungen) |
| 9 bis 11 | Planung: der Planer liest die Typen, legt einen Plan mit Typ und Begründung je Station vor, `implementation_create` mit `agent`, `effort` und `prompt` | Phase 5 (Werkzeuge) |
| 12 bis 19 | Umsetzung: Karte vor dem Start, Abweichungen einer Station, Dialog „Einstellungen der Station“, läuft, Start scheitert, nach der Korrektur, andere Betriebsart | Phase 6 (Karte und Stationsdialog) |

## Entscheidungen der Einstellungen

- **Ein Abschnitt in der Kategorie Agent,** hinter „Agent“, als eine Zeile des Katalogs mit voller Breite statt Beschriftung und Knopf (ADR 057): Die Liste braucht die Breite. Der ⓘ-Text und die Suche kommen aus dem Katalog.
- **Ein Dialog statt Bearbeiten in der Zeile:** Ein Typ hat sieben Felder. „Bearbeiten“ öffnet denselben Dialog wie „Agententyp anlegen“; Umbenennen heißt: anderen Namen speichern.
- **Pflicht** sind Name und Beschreibung. Die Beschreibung ist für den Planer, der Systemprompt für den Agenten selbst.
- **Modell und Denkaufwand als Paar** oder „Standard“, im Menü der Eingabeleiste; die Auswahl gilt nur in der Betriebsart Claude (in den anderen steht ein Hinweis über der Liste).
- **Keine Prüfung der Dateien im Dialog:** Relative Pfade gelten gegen die Repositories eines Vorhabens, das es in den Einstellungen nicht gibt. Fehlt eine Datei, sagt es die Karte beim Start (Phase 4 und 6).
- **Die Meldungen kommen vom Core** und stehen an ihrem Feld; die Oberfläche rechnet nur die Zähler der Beschreibung und des Textes (ADR 060).

## Gestaltung

- Maße, Farben und Schriften folgen `src/styles/theme.css`; die Bausteine sind den vorhandenen nachgebaut (`SettingRow`, `SelectButton`, `ModelMenu` mit `EffortDots`, `Dialog`) und in der Umsetzung diese Bausteine selbst.
- Hell und dunkel: Der Entwurf folgt zunächst dem Farbschema des Systems und lässt sich oben umschalten.

## Platzhalter im Entwurf

- **„Beispiel einsetzen“** im Dialog füllt den Typ developer aus, damit man nicht tippen muss. Es gibt den Knopf in der App nicht.
- **Der Schalter „Datei … fehlt / liegt da“** oben im Entwurf (nur in den Schritten 17 und 18 sichtbar) tut so, als läge die Datei des Typs reviewer da oder nicht, damit der Startfehler der Karte und seine Korrektur zu sehen sind. In der App prüft der Core die Dateien beim Start (Phase 4), die Karte zeigt den Fehler (Phase 6).
- **„Datei wählen …“** hängt im Entwurf einen Beispielpfad an; die App öffnet den Dateidialog des Systems.
- Alle Namen, Pfade, Zahlen und die Liste der Skill-Vorschläge sind Beispieldaten.

## Umsetzung der Einstellungen

Die Bilder in [`umsetzung/`](umsetzung/) stammen aus der gebauten App (Linux, 1280 px Breite, dunkel und hell). Sie entstanden in einem Lauf mit eigenem Benutzerordner und Port 1421 auf einem privaten Bildschirm, die Klicks über XTEST. Unter Windows ist nichts davon gelaufen.

| Bild | Zeigt |
| :- | :- |
| `liste-leer-dunkel.png`, `liste-leer-hell.png` | Schritt 1: Abschnitt „Agententypen“ ohne Typen |
| `liste-drei-typen-dunkel.png`, `liste-drei-typen-hell.png` | Schritt 6: drei Typen |
| `dialog-fehler-dunkel.png`, `dialog-fehler-hell.png` | Schritt 2: „Speichern“ ohne Name und Beschreibung, die Meldungen des Core an den Feldern |
| `dialog-stationsname-vergeben-dunkel.png` | Schritt 7: „umsetzung“ gehört schon zum Typ „developer“ (getrimmt, ohne Beachtung der Schreibweise), dazu die fehlende Beschreibung |
| `dialog-modellmenue-dunkel.png`, `dialog-modellmenue-hell.png` | Schritt 4: das Modellmenü mit der Zeile „Denkaufwand“ in einem rollenden Dialog, nicht abgeschnitten |
| `loeschen-dunkel.png`, `loeschen-hell.png` | Schritt 8: „Löschen“ nennt die Folge |
| `dialog-esc-schliesst-nur-das-menue-dunkel.png` | Esc bei offenem Modellmenü: nur das Menü schließt, der Dialog behält seine Eingaben |
| `dialog-rueckfrage-dunkel.png`, `dialog-rueckfrage-hell.png` | Esc oder Klick auf den Hintergrund mit ungespeicherten Eingaben: der Dialog fragt, bevor er sie verwirft |
| `dialog-skills-vorschlaege-dunkel.png` | Vorschläge im Skill-Feld: die Skills und Befehle des Benutzers |
| `dialog-grenzen-dunkel.png` | Beschreibung mit 515 von 500 Zeichen und 21 Stationsnamen: der Zähler wird rot, die Meldungen des Core stehen an den Feldern |
| `dialog-text-zu-lang-hell.png` | Systemprompt mit 16.001 von 16.000 Einheiten: „Der Text ist länger als 16 000 Zeichen.“ |
| `dialog-datei-gewaehlt-hell.png` | „Datei wählen …“ trägt den absoluten Pfad aus dem Dateidialog ein |
| `liste-nach-neustart-dunkel.png` | Nach dem Neustart der App stehen die Typen noch da |
| `suche-dunkel.png` | Die Suche (ADR 057) findet „Typen“ über den ⓘ-Text („Systemprompt“) |
| `hinweis-codex-dunkel.png` | In der Betriebsart Codex steht der Hinweis über der Liste |

Belegt im Lauf: Anlegen (drei Typen), Ändern, Umbenennen (dieselbe ID, anderer Name), Löschen (über den Dialog, danach ist die Tabelle leer), doppelter Stationsname, fehlende Beschreibung, Neustart (zweimal), die Grenzen aus ADR 060 (500 Zeichen der Beschreibung gezählt in Codepunkten: 500 Emoji sind 500 von 500; 16 000 Einheiten des Textes gezählt in UTF-16-Einheiten; 20 Stationsnamen), ein getipptes führendes „/“ im Skill und Text in den Eingabefeldern, den „Speichern“ ohne Enter mitnimmt.

## Abweichungen vom Entwurf

- **Knöpfe und Auswahlknopf:** Die Knöpfe sind die der übrigen Einstellungen (30 px hoch statt 28 px), der Auswahlknopf für Modell und Denkaufwand im Dialog ist 300 statt 270 px breit. Die Chips einer Typ-Zeile brechen dadurch etwas früher um.
- **Dialog:** 600 statt 580 px breit, weil die Knöpfe der Einstellungen breiter sind und die Pfadzeile ihren Platzhalter sonst abschneidet. Er steht mittig über der App (der gemeinsame `Dialog`) statt oben, und nur der Rumpf rollt; Kopf und Fuß mit „Speichern“ bleiben sichtbar, wo der Entwurf die ganze Fläche rollen lässt.
- **Meldungen:** Die Prüfung liegt im Core. Der Dialog schickt die Eingabe beim Speichern und zeigt, was der Core meldet, an seinem Feld, in der Reihenfolge der Felder; der Entwurf prüft selbst und rechnet nach jeder Eingabe neu. In der Umsetzung verschwindet die Meldung eines Feldes, sobald man dieses Feld ändert; geprüft wird erst beim nächsten Speichern. Der Fuß zählt Felder, nicht Meldungen („2 Angaben fehlen oder sind ungültig“). Nach einem Speichern mit Meldungen rollt der Dialog das erste betroffene Feld ins Bild.
- **Zähler:** Die Beschreibung zählt getrimmt in Codepunkten, der Text in UTF-16-Einheiten, beides wie der Core. Der Zähler wird rot, sperrt „Speichern“ aber nicht, denn die Meldung kommt vom Core. Die Grenze steht im Zahlenformat der App („16.000“), der Entwurf schreibt „16 000“.
- **Text in den Eingabefeldern** von Stationen, Dateien und Skills gilt beim Speichern auch ohne Enter oder „Hinzufügen“; der Entwurf verwirft ihn. Ein getipptes führendes „/“ im Skill schneidet der Dialog ab, denn der Core lehnt es ab. Die Chip-Felder zeigen einen Fokusring.
- **Skill-Vorschläge** sind die Skills und Befehle des Benutzers (`~/.claude/skills`, `~/.claude/commands`) statt einer festen Liste. Plugin-Skills kennt diese Liste nicht; freier Text mit Enter geht immer. Die Liste öffnet, wenn das Feld den Fokus bekommt oder man tippt, und schließt bei Esc, Enter, Auswahl und beim Verlassen des Feldes.
- **„Datei wählen …“** öffnet den Dateidialog des Systems (`open` mit `directory: false`, die Berechtigung `dialog:allow-open` deckt Dateien) und trägt den absoluten Pfad ein.
- **Esc und Klick auf den Hintergrund** (der gemeinsame `Dialog` schließt sonst ohne Rückfrage): Bei ungespeicherten Eingaben fragt der Dialog „Die Eingaben sind nicht gespeichert. Verwerfen?“, mit „Weiter bearbeiten“ vorgewählt; ein zweites Esc heißt „Weiter bearbeiten“. Ohne Eingaben schließt er sofort, „Abbrechen“ verwirft ohne Rückfrage. Ist ein Menü oder die Liste der Vorschläge offen, schließt Esc zuerst nur sie. Beides entscheidet der Dialog selbst, `Dialog.tsx` und `Popover.tsx` bleiben unverändert.
- **Modellmenü im Dialog:** `ModelMenu` reicht `detached` und `className` an `Popover` weiter. Das Menü hängt dadurch am `body` und wird vom rollenden Rumpf nicht abgeschnitten; eine Regel hebt es über den Dialog (`--z-modal`).
- **Breite Zeile:** `SettingRow` kann eine Zeile mit voller Breite zeigen (`isWide`) und rechts neben dem Namen eine Aktion (`action`); der Entwurf zeichnet die Zeile so, die bisherigen Zeilen bleiben unverändert.
- **Über den Entwurf hinaus:** Die Zeile zeigt „Lädt …“, bis die Liste da ist, und einen Satz, wenn sie nicht ladbar ist oder das Abo der Änderungen fehlschlägt. Der Löschdialog zeigt einen Fehler des Core an, statt zu schließen.

## Bekannte Grenzen

- Das losgelöste Modellmenü liegt außerhalb der Fokusfalle des Dialogs: Tab aus dem letzten Eintrag des Menüs führt aus dem Dialog heraus. Esc schließt das Menü und gibt den Fokus an den Auswahlknopf zurück.
- Zähler und Core trimmen mit leicht verschiedenen Begriffen von Leerraum (U+0085 und U+FEFF); steht eines dieser Zeichen am Rand der Beschreibung, kann der Zähler um eins abweichen.
- Unter Windows nicht gelaufen: der Dateidialog, Pfade wie `~/…` und relative Pfade.
