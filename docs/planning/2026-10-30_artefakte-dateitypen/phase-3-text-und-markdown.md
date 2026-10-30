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

- [ ] `.txt`, `.log`, Quelltext (`.ts`, `.py`, `.rs`, `.sql`, `.ps1` geprüft), `.diff` und `.md` öffnen ihren Betrachter; Zeilennummern bei allem außer Markdown.
- [ ] Quelltext hat Syntaxfarben in hell und dunkel; Umbruch umschaltbar.
- [ ] Markdown zeigt Überschriften, Tabellen, Code mit Farben, Mermaid-Block als Diagramm, Aufgabenlisten, Inhaltsverzeichnis; `<script>` im Markdown wird nicht ausgeführt.
- [ ] Ohne Internet zeigt jeder dieser Betrachter den Rohtext mit Zeilennummern.
- [ ] Eine 20-MiB-Logdatei öffnet in unter 3 s mit Hinweis „Datei gekürzt“.
- [ ] Kommentar auf Zeile 12 einer Code-Datei bleibt nach „Neu laden“ an Zeile 12.

## Checkliste

- [ ] `viewers/common.js`, Platzhalter `__COMMON__` in `shell.html` und `viewers/mod.rs`.
- [ ] `viewers/text.js` (Text, Code, Log, Diff — eine Datei, Zweig nach `__KIND__`), `viewers/markdown.js`.
- [ ] Weiche `page`: `Text`, `Code`, `Log`, `Diff` → `text.js`; `Markdown` → `markdown.js`.
- [ ] Probe-Dateien `artifacts/proben/` dieses Plans: `beispiel.md` (mit Tabelle, Code, Mermaid, Aufgabenliste, `<script>alert(1)</script>`), `beispiel.log`, `beispiel.ts`, `beispiel.diff`, Skript `erzeuge-grosses-log.ps1` für die 20-MiB-Datei.
- [ ] Code-Map: Zeile „Artefakte“ um `viewers/` (Weiche, Betrachter je Typ) ergänzen.

## Definition of Done

- Screenshots je Betrachter (hell/dunkel) und der Rohtext-Rückfall ohne Netz in `artifacts/phase-3-*.png`; Zeitmessung der 20-MiB-Datei im Report-Back; Nachweis, dass `alert(1)` nicht feuert.
- Sichtbares Ergebnis: Markdown-Berichte und Logs des Agenten sind im Reiter lesbar.
- `pnpm check` grün; Seven-Goals-Check.

## Report-Back
