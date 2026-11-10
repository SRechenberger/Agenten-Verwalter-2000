# Design-Entwurf: Agententypen

**Status:** freigegeben am 2026-11-08 mit dem Plan „Agententypen für Stationen“ (Taiga-Story 55, [ADR 060](../../decisions/060-agententypen.md)). Der Entwurf zeigt drei Teile: die Einstellungen (Schritte 1 bis 8), die Planung (Schritte 9 bis 11) und die Umsetzung (Schritte 12 bis 19). Gebaut sind die Einstellungen (Phase 3), die Werkzeuge der Planung (Phase 5, ohne eigene Oberfläche) und die Umsetzung (Phase 6). Wo die Umsetzung abweichen muss, wird erst dieser Entwurf geändert, dann der Code; `mockup.html` bleibt nach der Abnahme unverändert, die Abweichungen stehen unten.

## Ansehen

- Die Datei [mockup.html](mockup.html) ist eigenständig und läuft direkt im Browser (Doppelklick genügt, kein Server nötig).
- Oben schalten „Ansicht“ (Einstellungen, Planung, Umsetzung), „Betriebsart“ und „Farbschema“ um; der Rundgang führt durch 19 Schritte, jeder Knopf in der Anwendung geht auch ohne ihn. Sprungmarken: `mockup.html#schritt=7&schema=hell&betriebsart=codex`.

## Gezeigt wird

| Schritte | Inhalt | Gebaut |
| :- | :- | :- |
| 1 bis 8 | Einstellungen › Agent: Abschnitt „Agententypen“ (leer, Dialog mit Pflichtangaben, Modell und Denkaufwand, Speichern, Liste mit drei Typen, doppelter Stationsname, Löschen) | Phase 3 (Einstellungen) |
| 9 bis 11 | Planung: der Planer liest die Typen, legt einen Plan mit Typ und Begründung je Station vor, `implementation_create` mit `agent`, `effort` und `prompt` | Phase 5 (Werkzeuge, keine eigene Oberfläche) |
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

## Bekannte Grenzen (Einstellungen)

- Das losgelöste Modellmenü liegt außerhalb der Fokusfalle des Dialogs: Tab aus dem letzten Eintrag des Menüs führt aus dem Dialog heraus. Esc schließt das Menü und gibt den Fokus an den Auswahlknopf zurück.
- Zähler und Core trimmen mit leicht verschiedenen Begriffen von Leerraum (U+0085 und U+FEFF); steht eines dieser Zeichen am Rand der Beschreibung, kann der Zähler um eins abweichen.
- Unter Windows nicht gelaufen: der Dateidialog, Pfade wie `~/…` und relative Pfade.

## Umsetzung der Karte und der Stationseinstellungen

Gebaut sind die Schritte 12 bis 19: die Zeile der Karte mit Typ, Modell und Denkaufwand samt Herkunft, der Chip „Zusatztext“, der Startfehler, der Typname in Zeitleiste und Kartei, „startet nicht“ als Meta und der Dialog „Einstellungen der Station“. Die Bilder stammen wie die oben aus der gebauten App (Linux, 1280 px Breite, Fenster 1280 × 1060 für die Dialoge, dunkel und hell), aus einem Lauf mit eigenem Benutzerordner, Port 1421 und einer Attrappe für `claude` und `codex` auf einem privaten Bildschirm; die Klicks gingen über XTEST. Unter Windows ist nichts davon gelaufen. Das Vorhaben ist das des Entwurfs („Export als Markdown“, drei Typen); die Startfehler stammen aus der App selbst: Die Datei `~/rollen/reviewer.md` des Typs „reviewer“ gibt es im Wegwerf-Ordner nicht, und die Station „Review“ nennt in einem zweiten Lauf den gelöschten Typ „ghost“.

| Bild | Zeigt |
| :- | :- |
| `karte-vor-dem-start-dunkel.png`, `karte-vor-dem-start-hell.png` | Schritt 12: Station „Test“ vor dem Start, Typ „tester“ (ohne Modell), Modell und Denkaufwand aus der allgemeinen Regel („Regel“: die Session mit der höchsten Nummer) |
| `karte-abweichungen-dunkel.png`, `karte-abweichungen-hell.png` | Schritt 13: das kritische Review mit Denkaufwand Max („Station“), Zusatztext · 131 Zeichen, Modell des Typs |
| `station-dialog-dunkel.png`, `station-dialog-hell.png` | Schritt 14: „Einstellungen der Station“ für dieses Review: je Zeile Wert, Herkunft und „Zurücksetzen“, unten die Vorschau |
| `station-dialog-abweichungen-dunkel.png` | Schritt 15: Haiku, Sehr hoch und ein eigener Skill gesetzt, die Vorschau nennt „(Station)“ je Wert |
| `station-dialog-modellmenue-dunkel.png`, `station-dialog-modellmenue-hell.png` | das Modellmenü mit dem Eintrag „Wie der Typ“, über dem rollenden Dialog, nicht abgeschnitten |
| `station-dialog-esc-schliesst-nur-das-menue-dunkel.png` | Esc bei offenem Modellmenü: nur das Menü schließt, der Dialog behält seine Eingaben |
| `station-dialog-rueckfrage-dunkel.png`, `station-dialog-rueckfrage-hell.png` | Esc oder Klick auf den Hintergrund mit ungespeicherten Eingaben: der Dialog fragt, bevor er sie verwirft |
| `station-dialog-fehler-dunkel.png`, `station-dialog-fehler-hell.png` | „Speichern“ mit einem Skill von 57 Zeichen: die Meldung des Core am Feld, der Fuß zählt „1 Angabe“ |
| `station-dialog-zusatztext-zu-lang-dunkel.png` | Zusatztext mit 4.001 von 4.000 Zeichen: der Zähler wird rot, die Meldung „prompt: höchstens 4000 Zeichen, nicht 4001“ steht am Feld |
| `station-dialog-geloeschter-typ-dunkel.png` | Die Station nennt einen Typ, den es nicht gibt: „ghost (gelöscht)“ bleibt in der Auswahl, die Vorschau nennt ihn |
| `karte-laeuft-dunkel.png` | Schritt 16: nach dem Start die Werte der Session (Haiku, Niedrig, `/review`), der Knopf „Einstellungen“ fehlt |
| `karte-startfehler-dunkel.png`, `karte-startfehler-hell.png` | Schritt 17: ▶ ohne die Datei des Typs: Fehlerblock im Fehlerton, Rand und Punkt rot, „startet nicht“ in der Zeitleiste, „Typ bearbeiten“ |
| `karte-geloeschter-typ-dunkel.png` | der Startfehler für einen gelöschten Typ: „ghost · gelöscht“ gestrichelt, der Text des Core, kein „Typ bearbeiten“ |
| `karte-typ-bearbeiten-dunkel.png` | „Typ bearbeiten“ öffnet den Dialog des Typs aus den Einstellungen an Ort und Stelle |
| `karte-nach-der-korrektur-dunkel.png` | Schritt 18: die Datei steht nicht mehr im Typ, ▶ startet die Station als „reviewer“, der Fehler ist weg |
| `karte-codex-dunkel.png`, `karte-codex-hell.png` | Schritt 19: Betriebsart Codex: der Typ steht, „Modell: wie die Session“; der ausdrückliche Denkaufwand der Station steht auch hier |

Belegt im Lauf (Datenbank und Argumente der Attrappe gelesen): „Einstellungen“ ersetzt den ganzen Satz (Haiku, Sehr hoch, Zusatztext, Skill; die Marke `edited` steht), „Zurücksetzen“ je Zeile und „Alles zurücksetzen“ (danach sind alle fünf Werte leer und die Karte zeigt die des Typs); der Start nimmt die gespeicherten Werte (`--model claude-haiku-5-5`, `--effort low`, die erste Nachricht beginnt mit `/review`, der Zusatztext steht am Ende des Systemprompts); nach dem Start fehlt der Knopf; ein Startfehler durch eine fehlende Datei und einer durch einen gelöschten Typ zeigen den Fehlerblock, nach der Korrektur startet ▶ die Station und `start_error` ist leer; die Meldungen des Core stehen am Feld (Skill, Zusatztext); Esc schließt zuerst das Menü, dann fragt der Dialog; Codex als Betriebsart.

### Entscheidungen zu den Stolpersteinen aus Phase 3

- **Esc und Klick auf den Hintergrund:** Der Dialog der Station fragt wie der der Typen („Die Eingaben sind nicht gespeichert. Verwerfen?“, „Weiter bearbeiten“ vorgewählt, ein zweites Esc heißt „Weiter bearbeiten“). Ohne Änderung schließt er sofort, „Abbrechen“ verwirft ohne Rückfrage. `Dialog.tsx` und `Popover.tsx` bleiben unverändert.
- **Modellmenü im rollenden Dialog:** Das Menü hängt am `body` (`detached`) und liegt über dem Dialog; Esc schließt zuerst nur das Menü, eine Auswahl schließt es auch. Belegt durch die beiden Bilder oben.
- **Pfeiltasten:** Die Kartei blättert mit ↑ und ↓. Mit einem Dialog über der Kartei blätterte ein Pfeil die Station dahinter weiter, und mit ihr verschwand der Dialog samt Eingaben; `ImplementationView` ignoriert die Tasten jetzt, solange irgendein Dialog offen ist, gleich wo der Fokus liegt. Das Ziel der Taste zu prüfen reichte nicht: Nach einem Klick auf Text im Dialog und nach „Zurücksetzen“ (der Knopf entfernt sich selbst) liegt der Fokus auf `body`.

### Abweichungen vom Entwurf (Schritte 12 bis 19)

- **Typname auch in der Kartei:** Der Typname steht klein und gedämpft hinter dem Stationsnamen in der Zeitleiste (wie im Entwurf) und in den kleinen Karten über und unter der Karte im Fokus (der Entwurf zeigt sie dort ohne; Auftrag der Phase, Punkt N6).
- **Startfehler:** Der Block nennt „Startet nicht.“, den Text des Core und den festen Satz „Es ist keine Session entstanden. Behebe die Ursache; danach startet ▶ die Station.“; der Entwurf nennt Datei und Abhilfe („Lege die Datei an oder ändere den Typ“), aber der Core kennt weitere Gründe (Typ gelöscht, Datei nicht lesbar, Text zu lang). Ein manueller Start gibt den Text zusätzlich als Fehler zurück, und das vorhandene Banner über der Zeitleiste („Start fehlgeschlagen: …“) zeigt ihn; der Entwurf kennt das Banner nicht.
- **Kopfzeile und Seitenleiste:** Bei einem Startfehler steht dort weiter „bereit“. Der Entwurf zeigt „Fehler“ mit rotem Punkt; die Dringlichkeit der Umsetzung kommt vom Core (Zustand der Stationen), der einen Startfehler nicht kennt, und der Core bleibt in dieser Phase unberührt.
- **„Typ bearbeiten“:** Der Knopf steht nur, wenn die Auflösung einen Typ fand, und öffnet den Dialog des Typs an Ort und Stelle. Ein gelöschter Typ hat keinen Dialog; dort hilft „Einstellungen“.
- **Nach dem Start:** Die Karte zeigt Modell und Denkaufwand der Session ohne Herkunft (Plan 4.4: der Nutzer kann sie in der Eingabeleiste geändert haben). Der Entwurf zeigt im Schritt 16 noch die aufgelösten Werte. In einer anderen Betriebsart als Claude zeigt die Karte auch danach nur ausdrückliche Werte der Station und sonst „wie die Session“.
- **Allgemeine Regel:** Der Core meldet sie nicht als Wert; die Karte und der Dialog lesen ihn aus der Session mit der höchsten Nummer im Vorhaben (`latestSessionOf`, wie `create_fresh_in_project`). Der Tooltip nennt die Session.
- **Dialog, Meldungen:** Der Core liefert einen Satz je Aufruf, und der Dialog setzt ihn an das Feld, mit dessen Namen er beginnt („prompt: …“, „skill: …“, „Der Agententyp …“); die Wortlaute sind die des Core („prompt: höchstens 4000 Zeichen, nicht 4001“), nicht die des Entwurfs („Der Text ist länger als 4 000 Zeichen.“). Alles andere („Die Station hat schon begonnen.“) steht im Fuß. Die Meldung eines Feldes verschwindet, sobald man es ändert. Der Zähler steht im Zahlenformat der App („4.000“, auch im Chip der Karte).
- **Dialog, Bausteine:** 600 px breit und mittig wie der Dialog der Typen, nur der Rumpf rollt; das Modell wählt man im `ModelMenu` ohne Denkaufwand-Zeile (der hat der Dialog eine eigene), der Typ in einem nativen Auswahlfeld. Ein Typ, den es nicht gibt, bleibt als „X (gelöscht)“ wählbar, damit man ihn sieht und ersetzen kann. Der lange Eintrag „Wie die allgemeine Regel (Max)“ bricht die Stufen des Denkaufwands in eine zweite Zeile um.
- **Skill-Vorschläge:** Eine native Liste (`datalist`) mit den Skills und Befehlen des Benutzers, wie im Dialog der Typen; Plugin-Skills kennt sie nicht, freier Text geht immer. Das native Popup erscheint nicht auf den Bildern.
- **Zusatztext:** Chip und Vorschau zählen den getrimmten Text in Codepunkten (so speichert ihn der Core), der Zähler und die Grenze den Text, wie er im Feld steht.
- **Info-Text:** Der ⓘ-Text des Abschnitts „Agententypen“ in den Einstellungen (Katalog) hat einen Satz mehr: „Auf der Karte einer Station ändert „Einstellungen“ … nur für diese Station.“ Der Entwurf zeigt den alten Text.
- **Platzhalter des Entwurfs:** Der Schalter „Datei fehlt / liegt da“ und „Beispiel einsetzen“ gibt es in der App nicht; der Startfehler kommt vom Core.

### Bekannte Grenzen (Karte und Dialog)

- Unter Windows nicht gelaufen: die Karte, der Dialog, der Start einer Station mit Typ, Pfade wie `~/…`.
- Die Typen im Dialog und für „Typ bearbeiten“ sind die dieses Rechners (Klasse G). Gehört das Vorhaben einem anderen Gerät, kann die Liste von der des Hosts abweichen; geprüft wird am Host.
- Löst der Countdown die Station aus, während der Dialog offen ist, verschwindet er mit seinen ungespeicherten Eingaben; die Station läuft dann mit den gespeicherten Werten.
- Das losgelöste Modellmenü liegt außerhalb der Fokusfalle des Dialogs (wie im Dialog der Typen): Tab aus dem letzten Eintrag führt aus dem Dialog heraus.
- Nach „Speichern“ im Dialog des Typs oder der Station bleibt ein alter Startfehler auf der Karte, bis ▶ die Station startet oder neu scheitert (Entscheidung N5): Der Text kann dann noch den Namen nennen, den man gerade ersetzt hat.
