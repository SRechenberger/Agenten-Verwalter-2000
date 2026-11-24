# Design-Entwurf: Nachzügler 0.77.0

## Agda (#230)

[agda.html](agda.html) ist eigenständig und läuft direkt im Browser. Es zeigt einen Block ` ```agda ` im Chat und einen Diff einer `.agda`-Datei, jeweils heute (ungefärbt) und neu, in hell und dunkel.

Gewählt wurde eine eigene, rein lexikalische Grammatik `agda` für highlight.js (`src/lib/agdaLanguage.ts`), weil es dafür kein gepflegtes Paket gibt und der Alias auf Haskell Bezeichner wie `map-with` zerreißt. Sie färbt nur mit Klassen, die `syntax.css` schon kennt: Schlüsselwörter (auch `∀`, `→`, `λ`), `Set` und `Prop` als Typ, Kommentare, Pragmas und Löcher (`{! !}` und ein allein stehendes `?`) in der Kommentarfarbe, dazu Zeichenketten und Zahlen. Chat und Diff nutzen dieselbe Sprachliste (`common`, `powershell`, `agda`), der Diff färbt damit nebenbei auch PowerShell; `.lagda.md` und `.lagda.tex` behalten die Färbung der äußeren Sprache.
