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

Variante D (Vorschlag): Zahl und Quadrate rechts in der Kopfzeile der Marke. Sie spart eine Zeile, ist immer sichtbar und lässt „Neues Vorhaben“ an seinem Platz.

**Es gilt:** C (Wahl des Nutzers, 2026-11-20). Festlegungen des Architekten, die das Mockup nicht zeigt:
- Der Eintrag „Dashboard“ unter „Neues Vorhaben“ entfällt; der Knopf steht im Fuß der Leiste, links der Chip der Warteschlange, rechts der Knopf.
- Ohne Karten bleibt der Knopf stehen, nur mit dem Symbol, ohne Pille und Zahl.
- Ist die Warteschlange leer, fehlt ihr Chip; der Knopf behält seinen Platz rechts.
- Ohne Schriftzug tragen `aria-label` und `title` den Namen: „Dashboard“, mit Karten „Dashboard – N Aufgaben warten auf dich“. Ist das Dashboard offen, gilt `aria-current="page"` mit Hervorhebung.
- Die Zahl bleibt die aller offenen Karten, nicht der gefilterten.
- Die Karte „Fehler“ einer Station bekommt unter „Gelesen“ denselben Hinweis wie „Unterbrochen“.
