# Design-Entwurf: Automatisch fortsetzen nach Kontingent

**Status:** gewählt am 2026-11-15 (Story Taiga #56). Baut auf dem Entwurf 1 der Constraint-Abläufe auf ([2026-11-10_constraint-ablaeufe](../2026-11-10_constraint-ablaeufe/README.md)) und zeigt nur, was neu ist. Gilt für die Ansicht der Umsetzung ([ADR 048](../../decisions/048-umsetzung.md), [ADR 061](../../decisions/061-constraint-ablaeufe.md)) und die Kontingent-Karte im Chat ([ADR 051](../../decisions/051-kontingent-warnung.md)); Entscheidung und Kern: [ADR 064](../../decisions/064-automatisch-fortsetzen.md).

## Ansehen

- Die Datei [mockup.html](mockup.html) ist eigenständig und läuft direkt im Browser (Doppelklick genügt, kein Server nötig).
- Oben schalten „Szenario“ (Frisch angelegt, Läuft, Kontingent leer mit Schalter an, aus oder gemischt), „Kontingent wird zurückgesetzt“, „Beschriftung“ und „Farbschema“ um. Sprungmarken: `#scen=qon|qoff|qmix|fresh|run&theme=light|dark`. Gestrichelt umrandete Knöpfe und die Leiste darüber sind Simulation und gehören nicht zur App.

## Begriffe

- **Automatisch fortsetzen** (↻): Nach aufgebrauchtem Kontingent setzt eine Station 30 Sekunden nach dem Zurücksetzen von selbst fort. Schalter der Umsetzung in der Leiste, Wert je Station in Zeile und Kartei (wie die Umsetzung, an, aus), Standard in den Einstellungen.
- **Setzt automatisch fort:** Zustand einer Station, deren Session am Kontingent pausiert und deren Zeitgeber scharf ist.
- **Automatisch starten** bleibt der Schalter, der die nächste Station nach ihren Vorgängern startet ([ADR 061](../../decisions/061-constraint-ablaeufe.md)); er ist ein anderer Schalter.

## Gezeigt wird

- **Leiste:** der neue Schalter neben „Automatisch starten“; er schaltet alle Stationen, die noch nicht fertig sind.
- **Zeile und Kartei:** ↻ in der Zeile und im Abschnitt „Automatik“ die Wahl „wie Umsetzung · an · aus“, änderbar bis die Station fertig ist; ein Punkt am ↻ zeigt eine Abweichung von der Umsetzung. Die letzte Aktion gewinnt.
- **Warnleiste:** steht, solange mindestens eine Station am Kontingent wartet, in beiden Reitern; sie nennt, wie viele fortsetzen, wie viele auf dich warten und wie viele keine Rücksetzzeit kennen, und nach dem Zurücksetzen, wer noch auf dich wartet.
- **Zustand „setzt automatisch fort“:** grauer Ton mit ↻, nicht Bernstein (Bernstein bleibt „wartet auf dich“); die Station liegt nicht vorn.
- **Plätze:** Eine Station, die auf das automatische Fortsetzen wartet, hält ihren Platz unter „Parallel max.“; die bereite Station bleibt in der Warteschlange.
- **Einstellungen:** eine Zeile in der Kategorie „Agent“, ausgeliefert mit „aus“; sie gilt nur für neue Umsetzungen.
- **Kontingent-Karte im Chat einer Station:** Ihr Schalter schreibt denselben Wert wie ↻ in der Zeile, nur für diese Station; ohne bekannte Rücksetzzeit kein Schalter. Außerhalb einer Umsetzung gilt er nur für diese Pause, wie bisher.

## Entscheidungen des Nutzers

**2026-11-15**
1. Der Wert wird je Station gespeichert (leer = wie die Umsetzung, an, aus), nicht nur im Speicher der Session; er übersteht den Neustart der App.
2. „Automatisch fortsetzen“ heißt es überall: Leiste, Warnleiste, Zeile, Kartei, Einstellungen, Karte im Chat. „Automatisch weiter“ aus Entwurf 1 lässt sich im Mock nur noch zum Vergleich umschalten.

## Annahmen im Entwurf

- Der Kern liefert den Zustand und die Werte (Bindings `StationState` mit `waitsForQuota`, `StationView.autoResume` und `resumesAutomatically`, `ImplementationView.autoResume`, `Settings.defaultAutoResume`); die Warnleiste rechnet die Oberfläche aus den Sessions der Stationen.
- Eine neue Pause übernimmt den Schalter nur bei einer Rücksetzzeit in der Zukunft; nach einem Neustart der App wartet keine Station mehr auf das Kontingent.
- Rückfragen, die anlegende Session und Sessions außerhalb einer Umsetzung beginnen jede Pause mit „aus“.

## Platzhalter im Entwurf

- Alle Namen, Lanes, Modelle, Zeiten und Zähler sind Beispieldaten; das Zurücksetzen dauert im Mock 6 statt 30 Sekunden.
- Der Mock rechnet Zustände, Plätze und Warnleiste im Browser selbst; in der App kommen Zustand und Plätze aus dem Core.

## Abweichungen vom Entwurf

Stand der Oberfläche (Phase 56.2). Die Bilder in [umsetzung/](umsetzung/) stammen aus der echten App: eigener Ordner als `HOME`, privates Display, eine Attrappe der Claude-Kommandozeile mit den Fällen `quota` und `quota-ohne-zeit`; jedes ist gegen den Mock (`#scen=qon|qoff|qmix`, hell und dunkel) verglichen. Die Klicks (↻ in der Zeile, Wahl in der Kartei, Schalter in Leiste, Warnleiste, Karte im Chat und Einstellungen) liefen dort über XTest.

- **Leiste:** Dort stehen nur ↻ und der Schalter; der Name „Automatisch fortsetzen“ ist Tooltip und Name für Screenreader. Mit dem ganzen Namen (wie im Mock) brach die Leiste schon bei 1440 Pixel Fensterbreite in zwei Zeilen um. Dazu ist der Fortschrittsbalken nicht mehr fest 160 Pixel breit: er wächst bis 160 Pixel, bricht aber erst unter 60 Pixel um. So bleibt die Leiste überall einzeilig, wo sie es vorher war (gemessen mit dem CSS der App von 300 bis 1300 Pixel Hauptfläche). Einzige Ausnahme ist die schmale Fassung unter 700 Pixel: bei 515 bis 575 Pixel Hauptfläche bekommt sie eine Zeile mehr, weil der Fortschrittsbalken dort schon auf seiner Mindestbreite steht. Mit „Warteschlange: N“ war die Leiste bei 1440 Pixel Fensterbreite schon vorher zweizeilig. In der Warnleiste steht der ganze Name.
- **Zeile:** ↻ steht rechts im Zustand, neben der Mini-Aktion („Öffnen“, ▶), nicht hinter Modell und Denkaufwand: dort liegt der Titel-Knopf, und ein Knopf darf nicht in einem Knopf stehen. Mit „wartet auf dich“, dem Chip „Rücksetzzeit unbekannt“ und „Öffnen“ wird der Titel bei 1440 Pixel Fensterbreite gekürzt („Lane 4 · Pr…“).
- **Zustand nach dem Zurücksetzen:** Der Mock zählt „setzt fort in N s“ herunter. Die App hat diesen Zustand nicht, der Zeitgeber lebt im Kern: Die Station bleibt bis zum Senden bei „setzt automatisch fort“ mit dem Chip der Rücksetzzeit, danach läuft sie.
- **Warnleiste:** „Kontingent zurückgesetzt.“ erscheint erst, wenn jede bekannte Rücksetzzeit vorbei ist. Zwei Stationen, die zehn Sekunden auseinander ins Kontingent liefen, zeigen bis zur späteren Zeit noch „aufgebraucht“. Der Mock kennt nur eine Zeit. Die genannte Uhrzeit ist die früheste noch kommende; ist die Zeit einer fortsetzenden Station schon vorbei, die einer anderen aber nicht, heißt es „… setzen gleich fort“. Nach dem Fortsetzen bleibt die Leiste mit „… warten noch auf dich“ stehen, solange eine Station am Kontingent pausiert.
- **Plätze:** In der App rückt eine bereite Station nach, sobald Stationen mit Schalter aus oder ohne Rücksetzzeit „wartet auf dich“ zeigen. Im Lauf „gemischt“ startete so „Messung“, die im Mock in der Warteschlange bleibt; im Mock warten in diesem Szenario drei Stationen automatisch.
- **Kartei:** Die erste Wahl nennt den Wert der Umsetzung („wie Umsetzung (an)“), wie bei „Automatisch starten“; die Marke „neu“ des Mocks fehlt. Der Kopf der Kartei zeigt „setzt automatisch fort“ ohne Chip.
- **Einstellungen:** Die Zeile hat nur Namen, ⓘ und Schalter; den Untertitel des Mocks („Standard für neue Umsetzungen …“) kennt die Einstellungszeile nicht. Der ⓘ-Text ist der des Mocks. Gesehen sind Zeile, Suche („fortsetzen“) und das Speichern (`default_auto_resume` in der Datenbank `true`, dann `false`). Das Szenario „Frisch angelegt“ ist nicht im Bild: Eine Umsetzung legt nur ein Agent über das Werkzeug an; die Übernahme prüft der Kern (ADR 064, Tests).
- **Nicht gesehen:** der Reiter „Changes“ mit der Warnleiste (das Beispiel-Vorhaben hat kein Repository, also keinen Reiter); laut Code steht die Warnleiste außerhalb des Reiterinhalts. Die Sperre für Gäste der Fernsteuerung ist nur im Code geprüft.
