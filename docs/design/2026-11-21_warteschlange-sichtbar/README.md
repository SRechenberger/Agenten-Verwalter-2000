# Design-Entwurf: Belegung der Plätze sichtbar

**Status:** Entwurf vom 2026-11-21, Wahl getroffen: Variante C (Taiga-Story 188; [ADR 068](../../decisions/068-globale-warteschlange.md)). Aufbau und Stil wie [2026-11-20_dashboard-zettel](../2026-11-20_dashboard-zettel/README.md).

## Ansehen

[mockup.html](mockup.html) ist eigenständig und läuft direkt im Browser. Hell und Dunkel stehen auf jedem Bild nebeneinander. Alle Namen und Zahlen sind Beispieldaten. Farben, Abstände und Schriften stammen aus `theme.css`, `Sidebar.css`, `QueueChip.css` und `SlotsMeter.css`; die Leiste hat ihre echte Breite von 256 px, der Fuß also 235 px Platz in der Zeile.

## Bilder

Jede Variante zeigt dieselben sechs Zustände, je Hell und Dunkel:

| Zustand | Zeigt |
|---|---|
| 1 | nichts belegt, nichts wartet: nur der Dashboard-Knopf wie heute |
| 2 | 2 von 4 belegt, nichts wartet |
| 3 | 4 von 4 belegt, 3 warten |
| 4 | 5 von 4 belegt, über der Grenze |
| 5 | ohne Grenze, 3 von ∞ |
| 6 | Grenzfall (nicht im Auftrag, zugefügt): 12 von 12 belegt, 14 warten |

| Bild | Zeigt |
|---|---|
| A | eigene Marke der Belegung (Zellen und „2/4“) links neben „n warten“; zwei Marken, beide öffnen die Liste |
| B | eine gemeinsame Marke: „2 von 4 belegt“, mit Wartenden „· 3 warten“, über der Grenze „5 von 4 · über der Grenze“ |
| C | schmaler Füllbalken über der Zeile (ein Segment je Platz, rechts „2/4“); Marke „n warten“ und Knopf darunter wie heute |
| D | eigene Variante: eine Marke aus zwei Teilen, links Belegung (Zellen, „2/4“), rechts angeheftet „3 warten“; ein Knopf, ein Fokus |
| Liste | das geöffnete Popover nach einem Klick, mit Wartenden und ohne Wartende (dann der Hinweis „Nichts wartet“), oben die Belegung im Wortlaut von `slotsText` mit den Zellen von `slotCells` und dem Weg zur Grenze |

Die gestrichelte Umrandung markiert, was neu ist. Der Block `UsageSection` fehlt in den Bildern; er steht zwischen Zeile und Archiv und ändert sich nicht.

## Was die Bilder zeigen

- **Platz:** In 235 px passt D mit 4 von 4 und „3 warten“ noch in eine Zeile mit dem Dashboard-Knopf, A nicht mehr (der Knopf rutscht in die nächste Zeile, die Leiste wird höher). Bei 12 Plätzen brechen A und D die Zeile um, in D reicht die Marke über die Leiste hinaus. B passt bis zu „12 von 12 belegt · 14 warten“ in die Zeile und schiebt erst dann den Knopf in die nächste. C braucht immer eine eigene Zeile, aber nur, solange etwas belegt ist.
- **Text:** Die lange Form `slotsText` („Plätze 5 von 4 (App) · über der Grenze“, 38 Zeichen) passt nicht in die Marke. Die Umsetzung braucht daneben eine kurze Form aus denselben Bausteinen (`queue.used`, `queue.limit`, `isOverLimit`), die lange bleibt Titel, `aria-label` und Kopfzeile der Liste.
- **Farbe:** Bernstein heißt heute „wartet“ (`QueueChip`) und „über der Grenze“ (`SlotsMeter`). Alle Varianten behalten das; eine Belegung ohne Wartende ist neutral umrandet.
- **Liste:** Die Kopfzeile enthält den Link „Grenze in den Einstellungen“ aus `SlotsMeter`; ohne ihn wäre die Kopfzeile nur Anzeige. Fällt er weg, bleibt die Liste sonst gleich.

## Offen: Wahl der Variante

Empfehlung: **B**. Sie sagt in Worten, was die Zahl bedeutet, braucht keine neue Form und keinen Platz für Zellen, und der Knopf bleibt in den üblichen Fällen in der Zeile. D ist die kompakte Alternative, wenn die Zellen sichtbar bleiben sollen.

Es gilt: Variante C, mit dem Link „Grenze in den Einstellungen“ in der Kopfzeile der Liste.
