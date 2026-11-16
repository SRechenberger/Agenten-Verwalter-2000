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
- Ändert den Punkt „Sidebar“ aus ADR 048: Stationen bekommen weiterhin keine **Session**-Zeile, aktive aber eine Stationszeile.

## Konsequenzen

- Der Baum wird bei parallelen Stationen länger: je aktiver Station eine Zeile; eine lange Warteschlange steht ganz in der Sidebar.
- `pending` und `done` bleiben draußen; fertige Stationen erreicht man weiter über die Zeitleiste.
- Keine Migration: die Zeilen werden aus der Ansicht der Umsetzung abgeleitet.
