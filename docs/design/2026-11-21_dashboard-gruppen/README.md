# Design-Entwurf: Dashboard-Gruppen und Marke „entfernt“

**Status:** Entwurf vom 2026-11-21, wartet auf die Wahl des Nutzers (Taiga-Story 186, Issue 198; [ADR 071](../../decisions/071-dashboard.md)). Aufbau und Stil wie [2026-11-20_dashboard-zettel](../2026-11-20_dashboard-zettel/README.md).

## Ansehen

[mockup.html](mockup.html) ist eigenständig und läuft direkt im Browser. Hell und Dunkel schaltet der Schalter oben um, oder die Sprungmarke `#theme=light` bzw. `#theme=dark`; die Marke „entfernt“ in den Bildern 2, 3 und 5 wählt `#mark=A` bis `#mark=D`. Die Schachtelung im Mockup rechnet wirklich (Gruppen in Bild 2 klappen auf Klick). Alle Namen und Zahlen sind Beispieldaten: vier Vorhaben (hoch, normal, niedrig und ein entferntes, hoch), zwei Gruppen („Verwalter“, „Sonstiges“).

## Bilder

| Bild | Ticket | Zeigt |
|---|---|---|
| 1 | #186 | Kopf der Liste: drei Schalter Gruppe, Vorhaben, Art in fester Reihenfolge und der Filter; alles aus, alles an, Fokus |
| 2 | #186 | Ganz geschachtelt (Gruppe → Vorhaben → Art) neben der flachen Liste; Vorhaben nach Priorität; eine Gruppe, ein Vorhaben und eine Art eingeklappt; „Sonstiges“ eingeklappt mit der Zahl der Karten und einer neuen Karte; der Kopf einer Gruppe in vier Zuständen |
| 3 | #186 | Teilweise: nur Vorhaben, nur Art, alles aus (flach wie heute) |
| 4 | #198 | Marke „entfernt“ an Karte, Zeile im Ablauf und Session, Varianten A, B, C und D, je mit der lokalen Entsprechung |
| 5 | #198 | Abläufe und Sessions: heute lokal zuerst und entfernte hinten, neu gemischt nach letzter Aktivität |

## Offen: Wahl des Nutzers

- **Marke „entfernt“:** A (⇄ mit Host-Name als Pille), B (Host als zweite Zeile), C (farbiger Rand links, Name im Tooltip) oder D.
    - **D (Vorschlag):** nur ⇄ in Akzentfarbe in der Kopfzeile von Karte, Ablauf und Session, wie in der Leiste; auf der Karte steht der Host vorn in der Herkunftszeile, in Ablauf und Session im `title`. Keine neue Zeile, keine neue Fläche, kein Platz in den vollen Zeilen der Abläufe.
    - A kostet Breite (die Pille bricht die Kopfzeile der Karte um, kürzt den Namen der Session), B kostet Höhe, C kollidiert auf der Karte mit dem Rand der Art.
    - In allen Varianten tragen `title` und `aria-label` den Namen des Hosts; die Marke folgt nur aus der `projectId`.
- **„+1“ an einer eingeklappten Gruppe:** zeigt, dass eine neue Karte dazukam, ohne die Gruppe zu öffnen. Das ist mehr als der Vertrag verlangt (Zahl der Karten, neue Karte öffnet nicht); der Store müsste die Zahl beim Einklappen merken. Ohne Wahl entfällt es.

## Festlegungen, die das Mockup zeigt

- Drei unabhängige Schalter im Stil des heutigen Dreier-Schalters (`aria-pressed`), das Kontrollkästchen „Gruppiert“ entfällt. Die Schachtelung folgt immer der Reihenfolge Gruppe → Vorhaben → Art, gleich welche Schalter an sind.
- Vorhaben stehen nach Priorität (▲ hoch zuerst, ▼ niedrig zuletzt, wie in der Leiste). Bei gleicher Priorität gilt die Reihenfolge der flachen Liste. Gruppe und Art behalten die Reihenfolge ihrer ersten Karte.
- Jeder Kopf einer Gruppe ist ein Knopf mit `aria-expanded`, mit Pfeil und der Zahl der Karten. Eingeklappt bleibt die Zahl sichtbar; der Zustand wird nicht gespeichert.
- Ein Hinweis, dass die Wahl bleibt, steht nicht im Kopf: nicht nötig.
- Die Zeit der letzten Aktivität in Bild 5 zeigt nur das Mockup; die Ansicht ordnet danach, ohne sie zu zeigen.

**Es gilt:** noch offen (Wahl des Nutzers).
