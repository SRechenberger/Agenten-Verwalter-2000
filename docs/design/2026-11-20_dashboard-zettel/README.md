# Design-Entwurf: Dashboard-Zettel

**Status:** Entwurf vom 2026-11-20, wartet auf die Freigabe des Nutzers (Taiga-Issues 181, 182, 183, Story 194; [ADR 071](../../decisions/071-dashboard.md)). Aufbau und Stil wie [2026-11-17_dashboard](../2026-11-17_dashboard/README.md).

## Ansehen

[mockup.html](mockup.html) ist eigenständig und läuft direkt im Browser. Hell und Dunkel schaltet der Schalter oben um, oder die Sprungmarke `#theme=light` bzw. `#theme=dark`. Alle Namen und Zahlen sind Beispieldaten.

## Bilder

| Bild | Ticket | Zeigt |
|---|---|---|
| 1 | #182 | Popup einer Antwort-Karte mit Markdown (Absätze, Liste, Code, Tabelle), daneben der heutige Rohtext; eine breite Tabelle (scrollt waagerecht) und eine mit schmalen Spalten |
| 2 links | #183 | Popup „Unterbrochen – Kontingent aufgebraucht“ einer Station mit „Gelesen“ |
| 2 rechts | #183 | Liste mit „Nur, was einen Agenten anhält“: Rückfrage zuerst, pausierte Karten dahinter |
| 3 | #181 | Offenes Popup, dessen Karte von „Antwort liegt vor“ zu „Fragt“ wechselt; es bleibt offen, die getippte Antwort bleibt |
| 4 | #194 | Dashboard-Knopf: heute, A (ganz oben), B (fester Kopf der Liste), C (im Fuß), D (in der Marke, Vorschlag) |

## Offen: Stelle des Dashboard-Knopfs

Variante D (Vorschlag): Zahl und Quadrate rechts in der Kopfzeile der Marke. Sie spart eine Zeile, ist immer sichtbar und lässt „Neues Vorhaben“ an seinem Platz. **Es gilt:** noch keine, der Nutzer wählt.
