# Design-Entwurf: Stationen bedienen

**Status:** Entwurf vom 2026-11-21, wartet auf die Wahl des Nutzers (Taiga-Stories 135, 136, 132; Issues 51, 53). Aufbau und Stil wie [2026-11-20_dashboard-zettel](../2026-11-20_dashboard-zettel/README.md).

## Ansehen

[mockup.html](mockup.html) ist eigenständig und läuft direkt im Browser. Hell und Dunkel schaltet der Schalter oben um, oder die Sprungmarke `#theme=light` bzw. `#theme=dark`. Alle Namen und Zahlen sind Beispieldaten.

## Bilder

| Bild | Ticket | Zeigt |
|---|---|---|
| 1 A | #135 | „Umsetzung starten“ als Hauptknopf links vor den Schaltern der Leiste, mit Zeilen der Zeitleiste; je drei Zustände: noch nicht begonnen, nach dem Klick (Grenze 2: zwei im Countdown, eine eingereiht), begonnen mit ausgeschalteter Automatik und bereiten Stationen |
| 1 B | #135 | Der Knopf ersetzt den Schalter „Automatisch starten“, solange die Umsetzung nicht begonnen hat; bei begonnener Umsetzung steht er neben dem Schalter |
| 1 C | #135 | Wie A, dazu derselbe Knopf an der Zeile des Ablaufs im Dashboard |
| 1 D | #135 | Vorschlag: Streifen unter der Leiste, der den Grund nennt („3 Stationen sind bereit und starten nicht von selbst“), mit dem Knopf darin |
| 2 oben | #136 | Zeile einer Station „durch dich“ mit „Erledigt“, freigegeben und nicht freigegeben |
| 2 Mitte | #136 | Rückfrage „Der Master hat diese Station nicht freigegeben. Trotzdem erledigt?“ in der Zeile und im Popup (Kartei gleich) |
| 2 unten | #136 | Marke der Freigabe in der Leiste (A: Chip in der zweiten Zeile, B: Häkchen) und an der Karte im Dashboard |
| 3 A–C | #132 | Marke „Plan ändern?“ in der Leiste: an der Umsetzung, an der Master-Session neben ◆, an beiden |
| 3 unten | #132 | Karte „Plan ändern?“ im Chat: heute, Vorschlag mit stärkerer Abhebung, und nach dem Sprung mit Fokusring |

## Offen: Wahl des Nutzers

- **#135:** A, B, C oder D. Empfehlung: **A**; D ergänzt A sinnvoll, wenn der Grund sichtbar sein soll. B lässt im Zustand „noch nicht begonnen“ den Schalter weg. C braucht dazu die Änderung der Karte im Dashboard (Lane D der Welle).
- **#136:** Marke der Freigabe in der Leiste, A oder B. Empfehlung: **A** (lesbar ohne Hinweisschrift).
- **#132:** A, B oder C. Empfehlung: **C**: bei zugeklappter Master-Gruppe bleibt die Marke sichtbar.
- **Karte „Plan ändern?“:** Vorschlag (getönter Grund, kräftiger Rahmen, Kopf mit Symbol und „wartet auf dich“) oder wie heute.

**Es gilt:** noch offen; der Master trägt die Wahl des Nutzers hier ein.
