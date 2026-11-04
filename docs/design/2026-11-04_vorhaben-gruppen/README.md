# Design-Entwurf: Vorhaben gruppieren

**Status:** abgenommen am 2026-11-04, Variante B (Gruppenköpfe). Variante A (Reiter) ist verworfen und steht im Entwurf nur zum Vergleich. Für die linke Leiste mit Gruppen und die Gruppenwahl auf der Seite „Neues Vorhaben“ ist dieser Entwurf verbindlich; alles andere regelt weiter der Entwurf [Vorhaben und TL;DR](../2026-09-30_vorhaben-und-tldr/README.md). Wo die Umsetzung abweichen muss, wird erst dieser Entwurf geändert, dann der Code.

## Ansehen

- Die Datei [mockup.html](mockup.html) ist eigenständig und läuft direkt im Browser (Doppelklick genügt, kein Server nötig).
- Bedienbar: Rechtsklick auf ein Vorhaben oder einen Gruppenkopf öffnet das Menü; Vorhaben lassen sich auf einen Gruppenkopf ziehen. Oben schalten „Variante“, „Zustand“ (mit Gruppen / ohne, also wie heute) und „Farbschema“ um.

## Gezeigt wird

- Die Leiste mit Gruppenköpfen: Vorhaben ohne Gruppe stehen oben, darunter je Gruppe ein einklappbarer Kopf; eingeklappt zeigt der Kopf den dringendsten Status der Gruppe.
- Ohne Gruppen sieht die Leiste aus wie heute.
- Das Menü eines Vorhabens („In Gruppe verschieben ▸ …“, „Neue Gruppe …“) und das Menü eines Kopfes.
- Ziehen eines Vorhabens auf einen Kopf; beim Ziehen erscheint oben die Fläche „Ohne Gruppe“.
- Die Seite „Neues Vorhaben“ mit der Gruppenwahl.
- Dunkles und helles Farbschema.

## Entscheidungen des Nutzers (2026-11-04)

1. **Variante B:** gruppieren mit Köpfen in der einen Liste, keine Reiter.
2. **Der Knopf „Neues Vorhaben“ bleibt, wo er ist, und startet ohne Gruppe.** Auf der Seite „Neues Vorhaben“ wählt man optional eine Gruppe.
3. **Ziehen und Ablegen:** Ein Vorhaben wird auf eine Gruppe gezogen und ihr damit zugewiesen oder die Gruppe wird gewechselt.
4. **Der Ein-/Ausklapp-Zustand eines Gruppenkopfs ist nicht flüchtig:** er überlebt den Neustart.

## Gestaltung

- Maße, Farben und Schriften folgen `src/styles/theme.css` und den Stilen der Leiste; Farbe trägt nur der Status.
- Eine Liste, kein zweites Navigationselement: „Chat first“, keine Dauer-Elemente ohne Nutzen.

## Platzhalter im Entwurf

- Alle Namen, Gruppen, Zahlen und Zeiten sind Beispieldaten.
- Die Menüpunkte „Umbenennen“ und „Archivieren“ haben im Entwurf keine Funktion.
- Variante A ist nur zum Vergleich da und wird nicht gebaut.
- Der Entwurf nutzt HTML5-Ziehen, weil er im Browser läuft; die App setzt das Ziehen mit Zeiger-Ereignissen um.

## Abweichungen vom Entwurf

- Die Menüs hängen als Popover am Vorhaben bzw. Kopf (wie das bestehende Vorhaben-Menü) statt an der Mausposition.
- „Neue Gruppe …“ fragt nicht per Eingabefenster, sondern legt die Gruppe „Neue Gruppe“ an, weist das Vorhaben zu und öffnet die Umbenennung am Kopf (Feld wie bei Vorhaben und Sessions; Enter speichert, Esc behält „Neue Gruppe“). Dasselbe gilt für das Umbenennen einer Gruppe; der Menüpunkt heißt dort „Umbenennen“ statt „Gruppe umbenennen“.
- Der Hinweis „Vorhaben bleiben erhalten (→ Ohne Gruppe)“ steht nicht als Zeile im Menü, sondern als `title` an „Gruppe löschen“.
- Die Vorhaben-Zeilen in einer Gruppe behalten ihren eigenen Chevron zum Auf-/Zuklappen der Sessions; der Entwurf lässt dafür nur Platz (20 px) und zeigt den Chevron nicht.
- Ein eingeklappter Kopf zeigt den Status nur, wenn er nicht „fertig“ oder „abgebrochen“ ist; der Entwurf nennt „fertig“ ebenso, „abgebrochen“ kennt er nicht.
- Das Ziehen läuft mit Zeiger-Ereignissen statt HTML5-Ziehen (Begründung in `docs/conventions/react.md`). Das Folgebild ist deshalb ein kleines Schild mit dem Namen des Vorhabens statt des Abbilds der Zeile.
- Das Ziel ist einfacher hervorgehoben: Akzentton und je eine Linie links und rechts über alle sichtbaren Zeilen der Gruppe, ohne obere und untere Kante und ohne Rundung, weil jede Zeile der virtualisierten Liste für sich liegt und die Gruppe kein gemeinsames Element hat.
- Vor dem Ziehen bleibt der Cursor der Hand-Cursor der Zeile; der Entwurf zeigt schon beim Überfahren „grab“. Beim Ziehen gilt „grabbing“ im ganzen Fenster.
- Über den Entwurf hinaus: Die Liste scrollt beim Ziehen am oberen und unteren Rand von selbst, und die Fläche „Ohne Gruppe“ steht als eigene Zeile ganz oben in der Liste, ist bei gescrollter Liste also erst über dieses Scrollen erreichbar.
- „Neues Vorhaben“, Überschrift von Abschnitt 3: „Agent und Gruppe“ statt „Modell, Modus und Gruppe“, solange das Feld da ist; sonst bleibt sie „Agent“.
- „Neues Vorhaben“, Feld „Gruppe“ und Hinweissatz erscheinen nur, wenn es mindestens eine Gruppe gibt; ohne Gruppen bleibt die Seite wie zuvor. Die erste Gruppe entsteht über das Menü eines Vorhabens in der Leiste.
- „Neues Vorhaben“, „Neue Gruppe …“ fragt nicht per Eingabefenster: Das Menü zeigt statt seiner Zeilen ein Namensfeld. Enter legt die Gruppe an und wählt sie, Esc führt zu den Zeilen zurück.
- „Neues Vorhaben“, das Menü ist ein Popover wie bei „Modell“ und „Modus“, mit der Kopfzeile „Gruppe für dieses Vorhaben“; das Feld trägt keinen Pfeil „▾“, weil auch die beiden anderen Felder keinen tragen.
- „Neues Vorhaben“, die Zusammenfassung unten nennt zusätzlich die gewählte Gruppe.
- Das Menü eines Vorhabens und das Kopf-Menü haben keine Trennlinie vor „Neue Gruppe …“ bzw. „Gruppe löschen“; das Menü auf „Neues Vorhaben“ hat sie.
- Bekannte Eigenheit: Die Fläche „Ohne Gruppe“ kommt erst beim Start des Ziehens dazu und verschiebt alle Zeilen um ihre Höhe, die Zeile unter dem Zeiger wechselt also. Damit ein kurzes Anreißen nicht die Zeile darüber trifft, bricht Loslassen näher als 16 px am Druckpunkt ab, und so nah gibt es auch keine Hervorhebung. Beseitigt ist das Verschieben damit nicht: Wer weiter als 16 px zieht, sieht das Ziel hervorgehoben und legt dort ab.
- Über den Entwurf hinaus: Umbenennen (F2, `/rename`) eines Ziels in einer eingeklappten Gruppe klappt die Gruppe auf und öffnet dann das Feld. Steht die Zeile danach nicht in der Liste, etwa bei einer Session in einem zugeklappten Vorhaben, endet das Umbenennen mit einer Meldung in der Leiste.
