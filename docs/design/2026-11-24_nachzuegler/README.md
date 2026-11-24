# Design-Entwurf: Nachzügler 0.77.0

**Status:** gewählt am 2026-11-24 (Taiga #228, #230, #231). Aufbau und Stil wie [2026-11-24_folgetickets](../2026-11-24_folgetickets/README.md).

## Agda (#230)

[agda.html](agda.html) ist eigenständig und läuft direkt im Browser. Es zeigt einen Block ` ```agda ` im Chat und einen Diff einer `.agda`-Datei, jeweils heute (ungefärbt) und neu, in hell und dunkel.

Gewählt wurde eine eigene, rein lexikalische Grammatik `agda` für highlight.js (`src/lib/agdaLanguage.ts`), weil es dafür kein gepflegtes Paket gibt und der Alias auf Haskell Bezeichner wie `map-with` zerreißt. Sie färbt nur mit Klassen, die `syntax.css` schon kennt: Schlüsselwörter (auch `∀`, `→`, `λ`), `Set` und `Prop` als Typ, Kommentare, Pragmas und Löcher (`{! !}` und ein allein stehendes `?`) in der Kommentarfarbe, dazu Zeichenketten und Zahlen. Chat und Diff nutzen dieselbe Sprachliste (`common`, `powershell`, `agda`), der Diff färbt damit nebenbei auch PowerShell; `.lagda.md` und `.lagda.tex` behalten die Färbung der äußeren Sprache.

## Fenster (#228, #231)

[fenster.html](fenster.html) ist eigenständig und läuft direkt im Browser; Farbschema und Beispieldaten wie dort. Bild 1 zeigt die Kopfzeile der Dashboard-Karte bei 205 px Breite, heute und neu.

- **Fokus (#228):** Ein Klick auf den Hintergrund schließt den Dialog erst bei `click`, und nur, wenn auch das `pointerdown` davor auf dem Hintergrund lag. Die Standardaktion von `mousedown` setzt den Fokus auf `body` und läuft damit vor dem Schließen; danach setzen `onClose` und die Rückgabe an das Element vor dem Dialog den Fokus. Ein Ziehen vom Feld auf den Hintergrund oder umgekehrt schließt nicht.
- **Kopfzeile (#231):** Die Art der Karte bleibt einzeilig (`white-space: nowrap`, `flex: none`), die Kopfzeile darf umbrechen (`flex-wrap: wrap`, kleiner `row-gap`). Was nicht mehr passt, etwa die Marke „✓ vom Master freigegeben“, rutscht in die zweite Zeile; passt alles in eine Zeile, ändert sich nichts.
