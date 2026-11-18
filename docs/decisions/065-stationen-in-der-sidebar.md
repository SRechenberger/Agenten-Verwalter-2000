# 065 — Aktive Stationen als Zeilen in der Sidebar

**Status:** angenommen · **Datum:** 2026-11-16

## Kontext

Der Weg in den Chat einer Station führte über zwei bis vier Klicks: Vorhaben aufklappen, Umsetzung öffnen, Station auf der Zeitleiste anwählen, „Chat öffnen“. Im Chat einer Station war in der Sidebar nichts markiert. [ADR 048](048-umsetzung.md) hatte der Station bewusst keine Zeile gegeben.

Die Idee kommt aus Upstream, das in Phasen nacheinander plant. Der Fork plant seit [ADR 061](061-constraint-ablaeufe.md) aus Kanten: bereiter Satz, Warteschlange hinter `max_parallel`, Lanes ohne Reihenfolge. Was „aktiv“ und „erste aktive Stelle“ heißt, muss deshalb auf Kanten neu festgelegt werden.

## Optionen

- **(a) Aktive Stationen als Zeilen unter ihrer Umsetzung** — gewählt: ein Klick bis in den Chat, die Markierung der Sidebar folgt der Station.
- **(b) Den Chat der Station in die Umsetzung einbetten** — verworfen: zwei Chat-Oberflächen mit eigenem Verlauf, Eingabe und Rückfrage-Kasten.
- **(c) Nur die Statuszeile am Vorhaben klickbar machen** — verworfen: trifft bei mehreren Stationen gleichzeitig nur eine, und der Weg zurück bleibt unmarkiert.

## Entscheidung

- **Aktiv** heißt hier: Zustand in `LIVE_STATES` (`implementationOrder.ts`): bereit, startet gleich, startet, läuft, wartet auf Skript, setzt automatisch fort, fragt dich, wartet auf dich, Fehler. Bereit heißt nach ADR 061: alle Vorgänger-Kanten fertig gemeldet; eine bereite Station in der Warteschlange zählt mit. Draußen bleiben `pending` (ein Vorgänger fehlt noch) und `done`. Das ist weiter als `StationView::is_active` im Core, das nur zählt, was einen Platz unter `max_parallel` belegt; beide Begriffe bleiben getrennt.
- **Reihenfolge:** die der Zeitleiste (`flatStations`): nach Stufe (`level`), innerhalb einer Stufe in Plan-Reihenfolge, also eine topologische Reihenfolge der Kanten, nicht Lane für Lane.
- **Aufgeklappt:** unter jeder Umsetzung stehen ihre aktiven Stationen, eine Ebene tiefer. **Zugeklappt:** nur die in `NEEDS_USER_STATES` (fragt dich, wartet auf dich), auf Höhe der Umsetzungen. Gruppen, Ablegefläche und Sortierung nach Aktivität bleiben unverändert; eine Stationszeile gehört zur Gruppe ihres Vorhabens (`dropGroup`).
- **Ein Klick** wählt die Session der Station; eine Station ohne Session (bereit, Countdown, „durch dich“) öffnet die Umsetzung mit ihrer Karte vorn (`useOpenStation`).
- **Genau eine Markierung:** im Chat einer Station ihre Stationszeile, sonst — fertig oder ohne Zeile — die Zeile ihrer Umsetzung.
- **Zeile:** Stationsname mit Session-Nummer, Lane-Schlüssel und Kurzzustand (`stationMeta`, Farbe aus `stationTone`, also mit Startfehler); der Tooltip nennt Lane und den Zustand der Zeitleiste (`rowStateText`, mit Platz in der Warteschlange und „startet neu“).
- **Rückweg-Leiste:** über dem Chat einer Station führt eine schmale Leiste zurück zur Umsetzung, nennt die Lane der Station und führt mit ‹ › zur vorigen bzw. nächsten aktiven Station in der Reihenfolge der Zeitleiste (`StationTrail`). „Vorige“ und „nächste“ heißen dort: früher bzw. später in `flatStations`, also nicht unbedingt Vorgänger oder Nachfolger über eine Kante.
- **Zeitleiste öffnet bei der ersten aktiven Stelle:** Beim Öffnen einer Umsetzung steht der Kopf der Stufe oben, in der die erste aktive Station in der Reihenfolge der Zeitleiste liegt; ohne aktive Station die Stufe der ersten nicht fertigen, bei fertigem Plan bleibt alles stehen (`openingLevel`). Danach scrollt nichts mehr von selbst; nur die Pfeiltasten holen eine Zeile außerhalb des Sichtbaren gerade so ins Bild. Im schmalen Layout rollt die Zeitleiste nicht selbst. Upstream öffnet bei der ersten Phase mit aktiver Station; der Fork zeigt keine Phasen-Köpfe, sondern Stufen, und eine Lane mit aktiver Station kann in einer späteren Stufe liegen als die aktive Station einer anderen Lane. Die Stufe der ersten aktiven Station lässt deshalb jede aktive Station unter dem oberen Rand.
- **`Ctrl J`:** springt über alle Vorhaben zur Session oder Station, die am längsten auf dich wartet (erst „fragt“, dann „wartet auf dich“); erneut gedrückt zur nächsten, am Ende von vorn. Eine kurze Einblendung nennt die Stelle („1 von 3 warten auf dich“). Im Terminal bleibt `Ctrl J` ein Zeilenumbruch; sonst belegt der Fork die Taste nicht. Die Liste wird bei jedem Druck neu berechnet (`src/features/waiting/`). Bereite Stationen in der Warteschlange und Stationen mit Startfehler warten nicht auf dich, sondern auf einen Platz bzw. auf ▶, und stehen nicht in der Liste.
- Ändert den Punkt „Sidebar“ aus ADR 048: Stationen bekommen weiterhin keine **Session**-Zeile, aktive aber eine Stationszeile.

## Nachtrag: Variante A der Master-Session ([ADR 070](070-master-session.md))

- **Aktiv:** „Aktiv“ bleibt `LIVE_STATES` (mit „gemeldet“), bestimmt aber nicht mehr, welche Stationen eine Zeile bekommen: Variante A ersetzt „alle aktiven Stationen“. Unter der Umsetzung stehen nur Stationen in `asking`, `waitsForYou` und `error` (`BELOW_IMPLEMENTATION_STATES`); alle anderen erreichst du über die Zeitleiste. Zugeklappt gilt weiter `NEEDS_USER_STATES`.
- **Platz:** Die Umsetzung steht als eine Zeile unter ihrer Master-Session, ihre Stationen eine Ebene tiefer; ohne Master oder bei ausgefallenem bleibt sie im Vorhaben. Ein weggeklappter Master bleibt sichtbar.
- **Rückfall-Markierung:** Zeigt die Hauptfläche den Chat einer Station ohne eigene Zeile (das sind jetzt auch laufende und gemeldete), trägt ihre Umsetzung die Markierung (`stationSessionIds` in `Sidebar.tsx`).

## Nachtrag: Zustand der Session ([ADR 070](070-master-session.md))

- Unter der Umsetzung steht eine Station auch dann, wenn nicht `state`, sondern `live` in `asking`, `waitsForYou` oder `error` liegt: eine fertige oder gemeldete Station, deren Session wieder fragt, pausiert oder scheitert (`isBelowImplementation`). Eine pausierte Station trägt dort den Grund als Chip („Kontingent bis HH:MM“).

## Konsequenzen

- Der Baum wird bei parallelen Stationen länger: je aktiver Station eine Zeile; eine lange Warteschlange steht ganz in der Sidebar.
- `pending` und `done` bleiben draußen; fertige Stationen erreicht man weiter über die Zeitleiste.
- Keine Migration: die Zeilen werden aus der Ansicht der Umsetzung abgeleitet.
