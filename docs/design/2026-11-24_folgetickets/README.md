# Design-Entwurf: Folgetickets 0.77.0

**Status:** gewählt am 2026-11-24 (Taiga #219 bis #225). Aufbau und Stil wie [2026-11-23_prompt-vorlagen](../2026-11-23_prompt-vorlagen/README.md).

## Ansehen

[mockup.html](mockup.html) ist eigenständig und läuft direkt im Browser. Der Schalter oben wechselt das Farbschema; bei Bild 5 lässt sich sehen, wie der Betrachter der App folgt. Die gestrichelte Umrandung markiert, was neu ist.

## Betrachter (#223, #225)

| Bild | Zeigt |
|---|---|
| 4 | Kommentar-Marke ganz im Bild: heute liegt sie mit `left = rect.right − 10` und `top = rect.top − 10` halb außerhalb des Fensters, sobald ein Element bis an den Rand reicht; neu bleibt sie an der Ecke ihres Elements und wird ins Fenster geschoben. Dasselbe gilt für das Schild beim Überfahren im Kommentarmodus |
| 5 | Betrachter folgen dem Schema der App: heute bleibt der Betrachter dunkel, wenn die App auf Hell steht und das System dunkel ist; neu trägt die Adresse des Rahmens `scheme=light` oder `scheme=dark`, bei „System“ fehlt der Parameter |

Es gilt (Wahl des Benutzers, 2026-11-24): Bild 4 wie gezeigt, Bild 5 **auch für HTML-Artefakte**. Die Brücke setzt `data-theme="light"` oder `data-theme="dark"` an `<html>`, wenn die App ein Schema gewählt hat; bei „System“ setzt sie nichts. Eine Seite, die ihre Farben unter `:root[data-theme="dark"]` und unter `@media (prefers-color-scheme: dark)` mit `:root:not([data-theme="light"])` schreibt, folgt damit der App; eine Seite, die nur auf `prefers-color-scheme` hört, bleibt beim System. Die Anweisung an die Agenten nennt die Konvention in einem Satz.

Bekannte Grenze: Ein Link auf eine andere Seite innerhalb eines Artefakts trägt den Parameter nicht, die Folgeseite folgt dann dem System.
