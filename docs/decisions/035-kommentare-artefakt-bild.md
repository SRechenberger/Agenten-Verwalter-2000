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
