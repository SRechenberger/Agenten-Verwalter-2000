# Design-Entwurf: Folgetickets 0.77.0

**Status:** gewählt am 2026-11-24 (Taiga #219 bis #225). Aufbau und Stil wie [2026-11-23_prompt-vorlagen](../2026-11-23_prompt-vorlagen/README.md).

## Ansehen

[mockup.html](mockup.html) ist eigenständig und läuft direkt im Browser. Der Schalter oben wechselt das Farbschema; bei Bild 5 lässt sich sehen, wie der Betrachter der App folgt. Die gestrichelte Umrandung markiert, was neu ist.

## Betrachter (#223, #225)

| Bild | Zeigt |
|---|---|
| 4 | Kommentar-Marke ganz im Bild: heute liegt sie mit `left = rect.right − 10` und `top = rect.top − 10` halb außerhalb des Fensters, sobald ein Element bis an den Rand reicht; neu bleibt sie an der Ecke ihres Elements und wird ins Fenster geschoben. Dasselbe gilt für das Schild beim Überfahren im Kommentarmodus |

Es gilt (Wahl des Benutzers, 2026-11-24): Bild 4 wie gezeigt.
