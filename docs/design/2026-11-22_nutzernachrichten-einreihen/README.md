# Design-Entwurf: Eigene Nachrichten einreihen

**Status:** Entwurf vom 2026-11-22, Wahl offen (Taiga-Story 190; [ADR 068](../../decisions/068-globale-warteschlange.md), D6). Aufbau und Stil wie [2026-11-21_warteschlange-sichtbar](../2026-11-21_warteschlange-sichtbar/README.md).

## Ansehen

[mockup.html](mockup.html) ist eigenständig und läuft direkt im Browser. Hell und Dunkel stehen auf jedem Bild nebeneinander. Alle Namen und Zahlen sind Beispieldaten. Farben, Abstände und Schriften stammen aus `theme.css`, `UserMessage.css`, `Composer.css`, `CacheColdHint.css`, `QueueChip.css` und `Sidebar.css`. Die gestrichelte Umrandung markiert, was neu ist. Der Platz „n“ kommt aus der Warteschlange; die Oberfläche rechnet keine Reihenfolge.

## Bilder

| Bild | Zeigt |
|---|---|
| 1A | wartende eigene Nachricht im Chat einer ruhenden Session; Marke „wartet · Platz 2“ klein unter der Blase, Rahmen der Blase gestrichelt in der Wartefarbe |
| 1B | dasselbe mit der Marke als Chip im Kopf der Blase |
| 2A | Hinweis als Leiste im Kopf des Eingabefelds wie `CacheColdHint`: „Deine Nachricht wartet auf einen freien Platz (Platz 2)“ mit „Jetzt senden“ |
| 2B | Hinweis als Knopf links neben dem Senden-Knopf, ohne zusätzliche Zeile |
| 3 | zwei wartende Nachrichten (Platz 2 und 3), die erste mit Anhang; die Leiste nennt beide, „Jetzt senden“ schickt beide in ihrer Reihenfolge |
| 4 | „/compact“ wartet: Blase mit Befehlszeichen und derselben Marke, derselbe Hinweis |
| 5 | nach dem Senden: Marke und Hinweis sind weg, die Blase ist eine normale |
| 6 | Leiste („✉ Platz 2“ an der Zeile der Session) und Liste der Warteschlange; Eintrag als „Nachricht · <Session>“ (A) oder „Eigene Nachricht · <Session>“ (B) |

## Was die Bilder zeigen

- **Marke:** A ist ruhiger und verschiebt den Text der Blase nicht, nimmt aber eine eigene Zeile unter der Blase. B steht in der Blase wie die Marke „n warten“ der Leiste, macht die Blase eine Zeile höher. Beide bleiben ohne Farbe lesbar (Wort, Uhr-Symbol, gestrichelter Rahmen).
- **Hinweis:** A braucht eine Zeile Höhe, bleibt aber auch in schmalen Fenstern lesbar und trägt den vollen Satz mit Platz. B spart die Höhe, ist aber enger (zwei Texte in der Leiste des Eingabefelds) und nennt nur „wartet · Platz 2“.
- **Liste:** Eine eigene Nachricht und eine Session-Nachricht hießen beide „Nachricht · <Session>“, werden aber verschieden bedient (nur die eigene lässt sich mit „Jetzt senden“ freigeben). Darum die Empfehlung B, „Eigene Nachricht · <Session>“.

## Texte (Vorschläge, nur hier)

- **`SLOTS_TITLE`:** „Eigene Nachrichten an ruhende Sessions warten, wenn die Grenze erreicht ist; mit ‚Jetzt senden‘ gehen sie trotzdem sofort hinaus. Antworten auf Rückfragen, laufende Sessions und erste Nachrichten starten sofort und zählen mit.“
- **Info-Text von „Gleichzeitige Agenten max.“:** „Eigene Nachrichten an ruhende Sessions warten auf einen freien Platz; ‚Jetzt senden‘ schickt sie sofort ab. Nachrichten an laufende Sessions, erste Nachrichten und Antworten auf Rückfragen gehen sofort und zählen mit.“
- **Tooltip von „Jetzt senden“:** „Schickt die wartenden Nachrichten dieser Session sofort ab, auch über der Grenze.“

## Offen: Wahl der Varianten

Empfehlung: Marke **A**, Hinweis **A**, Liste **B**.

Es gilt: (die Wahl trägt der Master nach dem Gespräch mit dem Benutzer ein)
