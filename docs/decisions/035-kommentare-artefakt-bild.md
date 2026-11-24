# ADR 035: Kommentare an Artefakten und Bildern

## Kontext

Review-Kommentare lassen sich bisher nur an Diff-Zeilen heften. Änderungen an HTML-Artefakten
und Rückmeldungen zu Screenshots brauchen ebenfalls einen konkreten Ort, der mit der nächsten
Nachricht an den Agenten geht.

## Optionen

1. Ein Bildschirmfoto des Artefakts über WebView2 kommentieren. Verworfen: Dafür wäre eine neue
   Abhängigkeit nötig, obwohl der Agent HTML und nicht Pixel ändert.
2. Ein kleines Skript an die Artefakt-Seite hängen und Element-Klicks per `postMessage` melden.
   Gewählt: Das Ziel bleibt ein CSS-Pfad samt Elementanfang und sichtbarer Beschriftung.
3. Nur einen Kommentar an die ganze Artefakt-Datei heften. Verworfen: Das ist für konkrete
   Überarbeitungen zu grob.

Bild-Kommentare speichern einen ganzzahligen Rechteckbereich in den Pixelkoordinaten des
Originalbilds. Ein Bild aus einer früheren Nachricht wird beim Senden erneut als Bildblock an den
Agenten angehängt.

## Entscheidung

Artefakt-Kommentare referenzieren Datei, CSS-Pfad und den gekürzten Anfang des Elements. Die
Artefakt-Seite kommuniziert im Kommentiermodus über `window.parent.postMessage`; der Verwalter
akzeptiert Nachrichten nur aus dem eigenen iframe und prüft das Protokollkennzeichen. Bildbereiche
referenzieren das Bild und seine Originalabmessungen.

Alle Kommentararten bleiben in derselben Review-Liste. Alte Einträge ohne Ziel werden als
Diff-Kommentare interpretiert.

## Konsequenzen

- Jede HTML-Antwort des Artefakt-Servers trägt ein angehängtes Kommentarskript.
- Der Agent erhält den markierten Elementanfang oder Bildbereich mit der Nachricht.
- Ein Bild aus einer früheren Nachricht kann erneut an den Agenten gesendet werden; der Core
  erlaubt dafür nur kanonische Dateien unter `<Workspace>\\.anhaenge`.
- Der persistierte Chat-Eintrag enthält nur die regulären Anhänge der aktuellen Nachricht; erneut
  angehängte Bilder sind ausschließlich Teil der Agentennachricht.

## Nachtrag 2026-11-23: Entwurf und Marken (Taiga #30, #32)

- Ist zu einer Session des Vorhabens ein Kommentar-Kasten zu einem Artefakt offen (Kennung `artifact|…`), ändert `artifact_show` die Auswahl nicht; der Text, den der Benutzer gerade tippt, bliebe sonst nicht beim Artefakt, zu dem er gehört. Der Sprung der Ansicht bleibt wie bisher. Ein Kasten in den Changes sperrt nichts.
- Die Brücke merkt sich die zuletzt empfangenen Marken und setzt sie neu, wenn sich das DOM unter `body` ändert (ein Durchlauf je Frame). Betrachter bauen ihr DOM erst nach dem Laden und beim Umschalten von Roh auf formatiert neu; die Marken kamen vorher zu früh und blieben weg. Die Overlays hängen an `documentElement`, lösen den Beobachter also nicht aus.

## Nachtrag 2026-11-24: Marke und Schild im Fenster (Taiga #223)

Die Marke einer Zeile, die bis an den Rand reicht, lag halb außerhalb des Fensters, an der obersten Zeile auch oben; dem Schild beim Überfahren ging es oben genauso. Beide bleiben an der Ecke ihres Elements, werden aber ins Fenster geschoben: die Marke mit `left` höchstens Fensterbreite − 22 und `top` mindestens 0, das Schild mit `top` mindestens 0 und `left` höchstens Fensterbreite − Breite des Schilds. Gemessen wird am Dokument (`clientWidth`, `clientHeight`), nicht an `innerWidth`: Die Scrollleiste des Fensters zählt `innerWidth` mit, und die Marke läge darunter. Marken scrollen weiter mit ihrem Element; der Beobachter aus dem Nachtrag oben löst nicht aus, denn die Overlays hängen an `documentElement`.
