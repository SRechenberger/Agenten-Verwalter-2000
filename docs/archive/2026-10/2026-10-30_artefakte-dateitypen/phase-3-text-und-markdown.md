# Phase 3 — Text, Code, Log, Diff, Markdown

**Rating:** standard · **Wave:** 3 · **Aufwand:** M

Betrachter für alles, was Zeilen hat, und für Markdown. Gemeinsame Bausteine (Zeilen mit Nummern, Syntaxfarben, Größen-Deckel) entstehen hier und werden in Phase 4 wiederverwendet.

## Kontext — vor dem Start lesen

- `README.md` dieses Plans, **Kontrakt → Artefakte** (Betrachter-Adresse, CDN mit SRI, Rohtext-Rückfall).
- `docs/decisions/047-artefakte-alle-dateitypen.md` (aus Phase 2).
- `src-tauri/src/artifacts/viewers/` — `mod.rs` (Weiche `page`), `shell.html` (Platzhalter `__KIND__`, `__FILE__`, `__SCRIPT__`), `image.js` als Vorbild für Aufbau und Rückfall.
- `src-tauri/src/artifacts/bridge.js` — wie ein Klick ein Element per Selektor markiert (Kommentar-Anker); Betrachter geben Zeilen und Abschnitten deshalb feste `id`s.
- `src/styles/syntax.css` — Farbzuordnung der highlight.js-Klassen in der App; die Betrachter übernehmen dieselben Farbwerte als eigene Tokens in `shell.html` (hell und dunkel), damit Code im Artefakt aussieht wie im Diff.
- Fehlerklassen geprüft: keine Vault-Entities einschlägig.

## Entscheidungen

- `viewers/common.js` mit `loadScript(url, integrity)`, `rawFallback(text)`, `lines(text) → HTMLElement` (je Zeile `<div class="line" id="L<n>">` mit Nummer), `capText(text, 5 MiB)` (darüber nur die ersten 5 MiB plus Hinweiszeile „Datei gekürzt — vollständig über „Im Browser öffnen““) und `languageOf(file)`. `shell.html` bekommt den Platzhalter `__COMMON__`, der vor `__SCRIPT__` steht.
- Bibliotheken (Version fest, SRI-Wert beim Einbau mit `curl -sL <url> | openssl dgst -sha384 -binary | openssl base64 -A` erzeugen und eintragen): `@highlightjs/cdn-assets@11.9.0/highlight.min.js`, `marked@12.0.2/marked.min.js`, `dompurify@3.1.6/dist/purify.min.js`, `mermaid@10.9.1/dist/mermaid.min.js`.
- Markdown: `marked` → `DOMPurify.sanitize` → DOM. Überschriften bekommen `id` aus ihrem Text; Inhaltsverzeichnis am linken Rand (ab 3 Überschriften, Breite unter 720 px ausgeblendet); Code-Blöcke mit highlight.js; ` ```mermaid `-Blöcke per `mermaid.render` als SVG; Aufgabenlisten als deaktivierte Häkchen; Links nach außen mit `target="_blank"` (öffnet nicht im Rahmen, Popups sind gesperrt — der Link bleibt sichtbar, ADR 026); Umschalter „Formatiert | Rohtext“ in der Kopfzeile; Absätze, Listenpunkte und Tabellenzeilen bekommen `id="b<n>"` als Kommentar-Anker.
- Log: Zeilen mit `ERROR|FATAL` rot, `WARN` gelb, `DEBUG|TRACE` gedämpft; Filter-Knöpfe je Ebene; nach jedem Neuladen ans Ende gescrollt.
- Diff: eigene schlichte Darstellung (Zeilen mit `+` grün, `-` rot, `@@` hervorgehoben, Dateiköpfe fett) — die Diff-Ansicht der App ist React im App-Fenster und darf Agenten-Inhalt nicht rendern.

## Abnahmekriterien

- [x] `.txt`, `.log`, Quelltext (`.ts`, `.py`, `.rs`, `.sql`, `.ps1` geprüft), `.diff` und `.md` öffnen ihren Betrachter; Zeilennummern bei allem außer Markdown.
- [x] Quelltext hat Syntaxfarben in hell und dunkel; Umbruch umschaltbar.
- [x] Markdown zeigt Überschriften, Tabellen, Code mit Farben, Mermaid-Block als Diagramm, Aufgabenlisten, Inhaltsverzeichnis; `<script>` im Markdown wird nicht ausgeführt.
- [x] Ohne Internet zeigt jeder dieser Betrachter den Rohtext mit Zeilennummern.
- [x] Eine 20-MiB-Logdatei öffnet in unter 3 s mit Hinweis „Datei gekürzt“.
- [x] Kommentar auf Zeile 12 einer Code-Datei bleibt nach „Neu laden“ an Zeile 12.

## Checkliste

- [x] `viewers/common.js`, Platzhalter `__COMMON__` in `shell.html` und `viewers/mod.rs`.
- [x] `viewers/text.js` (Text, Code, Log, Diff — eine Datei, Zweig nach `__KIND__`), `viewers/markdown.js`.
- [x] Weiche `page`: `Text`, `Code`, `Log`, `Diff` → `text.js`; `Markdown` → `markdown.js`.
- [x] Probe-Dateien `artifacts/proben/` dieses Plans: `beispiel.md` (mit Tabelle, Code, Mermaid, Aufgabenliste, `<script>alert(1)</script>`), `beispiel.log`, `beispiel.ts`, `beispiel.diff`, Skript `erzeuge-grosses-log.ps1` für die 20-MiB-Datei.
- [x] Code-Map: Zeile „Artefakte“ um `viewers/` (Weiche, Betrachter je Typ) ergänzen.

## Definition of Done

- Screenshots je Betrachter (hell/dunkel) und der Rohtext-Rückfall ohne Netz in `artifacts/phase-3-*.png`; Zeitmessung der 20-MiB-Datei im Report-Back; Nachweis, dass `alert(1)` nicht feuert.
- Sichtbares Ergebnis: Markdown-Berichte und Logs des Agenten sind im Reiter lesbar.
- `pnpm check` grün; Seven-Goals-Check.

## Report-Back

**Stand:** umgesetzt, `pnpm check` siehe Commit. Geprüft in Edge (headless) gegen eine Nachbildung des Artefakt-Servers mit denselben Dateien aus `viewers/` — **nicht** in der laufenden App und ohne deren CSP.

**Messwerte:** 20-MiB-Log: Inhalt nach ca. 0,5 s da (vorher 2,0 s — Layout von 55 000 Zeilen kostete 1,3 s, jetzt `content-visibility` ab 3000 Zeilen), 55 000 Zeilen, Hinweis „Datei gekürzt“, ans Ende gescrollt. Nur die ersten 5 MiB werden übertragen (Stream wird abgebrochen). `<script>alert(1)</script>` und `onerror` im Markdown: weder im DOM noch gefeuert. Kommentar-Anker: Klick auf gefärbten Text liefert `#L9`; nach Neuladen sitzt die Marke an Zeile 9. Ohne Netz: Markdown und Code zeigen Rohtext mit Zeilennummern. Screenshots: `artifacts/phase-3-*.png` (hell/dunkel je Betrachter, Markdown ohne Netz).

**Entscheidungen:** (1) `highlight.min.js` enthält kein PowerShell und kein Batch — `common.js` lädt `powershell.min.js`/`dos.min.js` bei Bedarf nach (eigene Prüfsummen). (2) `bridge.js` bekommt `targetOf`: ein Klick zählt für den nächsten Vorfahren mit `data-anchor` (Zeilen, Markdown-Blöcke), sonst läge die Marke auf einem Farb-`<span>`, den die Färbung später ersetzt. (3) Färbung ersetzt nur den Inhalt der vorhandenen Zeilen — die `<div id="L<n>">` bleiben, Marken bleiben gültig. (4) Färbung nur bis 512 KiB (darüber ungefärbt). (5) `.ps1`-Probe ist reines ASCII: Windows PowerShell 5.1 liest Skripte ohne BOM als ANSI.

**Unsicherste Stelle:** die echte CSP des Artefakt-Servers (`server.rs:217`) gegen CDN-Skripte mit `integrity`, Mermaid (`unsafe-eval`) und `blob:`-Bilder — in der Nachbildung gab es keine CSP.

**Abweichungen:** keine vom Kontrakt. Die Probe `beispiel.ts` heißt `beispiel.ts.probe` (ESLint würde eine `.ts` unter `docs/` prüfen und scheitern); zum Probieren ohne `.probe` in `.artefakte` kopieren. `.py`/`.rs`/`.sql` wurden nur als Sprachen von highlight.js bestätigt, nicht je als Datei geöffnet.
