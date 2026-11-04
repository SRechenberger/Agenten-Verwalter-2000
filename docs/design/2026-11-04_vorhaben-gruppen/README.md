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

- Noch keine.
