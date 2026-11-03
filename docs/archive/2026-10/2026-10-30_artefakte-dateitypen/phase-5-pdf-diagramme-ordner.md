# Phase 5 — PDF, Mermaid, Ordner

**Rating:** standard · **Wave:** 5 · **Aufwand:** L

PDF-Seitenansicht, Mermaid-Dateien als Diagramm und Ordner als Dateibaum mit Vorschau. Website-Ordner (`index.html`) laufen seit Phase 2.

## Kontext — vor dem Start lesen

- `README.md` dieses Plans, **Kontrakt → Artefakte**; `docs/decisions/026-artefakte.md` (Sandbox ohne `allow-same-origin`: die Seite hat eine undurchsichtige Herkunft), `docs/decisions/047-artefakte-alle-dateitypen.md`.
- `src-tauri/src/artifacts/server.rs` — `answer` mit `view=1` und Ordner-Zweig aus Phase 2, `artifact_csp`.
- `src-tauri/src/artifacts/viewers/` — `common.js`, `markdown.js` (Mermaid-Aufruf als Vorbild).
- `src-tauri/src/artifacts/mod.rs` — `resolve` (Pfadgrenze), `kind_of`.
- Fehlerklassen geprüft: keine Vault-Entities einschlägig.

## Entscheidungen

- **PDF:** `pdfjs-dist@4.4.168/build/pdf.min.mjs` laden per `<script type="module" src=… integrity=… crossorigin="anonymous">`; danach `globalThis.pdfjsLib` verwenden. Als Erstes prüfen, ob die Datei `globalThis.pdfjsLib` setzt (`console.log(typeof globalThis.pdfjsLib)` in einer Probe-Seite); falls nicht: `const pdfjsLib = await import(url)` ohne SRI und Eintrag in `FINDINGS.md`. Ein Worker von fremder Herkunft ist in der Sandbox nicht startbar; `GlobalWorkerOptions.workerSrc` auf `pdf.worker.min.mjs` derselben Version setzen und, falls der Worker scheitert, `pdfjsLib.getDocument({ …, disableWorker: true })` (Ausführung im Hauptthread). Darstellung: Seitenleiste mit Vorschaubildern (links, ab 720 px), Seiten untereinander auf `<canvas>` mit Textebene für Suche und Markieren, Zoom −/+/Breite, Suchfeld; je Seite `id="p<n>"` als Kommentar-Anker. Ohne pdf.js: Hinweis plus Knopf „Mit Standardprogramm öffnen“ (die Rest-Karte der App bleibt dafür zuständig — der Betrachter zeigt nur den Satz „PDF-Anzeige nicht geladen — oben „Im Browser öffnen“ nutzen.“).
- **Mermaid-Datei:** wie der Mermaid-Block in Markdown; zusätzlich Zoom per Strg+Mausrad, Verschieben per Ziehen, Umschalter „Diagramm | Quelltext“; Knoten-`id`s aus Mermaid bleiben Anker.
- **Ordner:** Server-Erweiterung `GET <ordner>/?list=1` → JSON `{ entries: [ { path, kind, size } ] }` (rekursiv bis 3 Ebenen, höchstens 2000 Einträge, Zubehör-Regeln gelten nicht innerhalb), jeder Pfad über `resolve` geprüft. Betrachter `folder.js`: Dateibaum links, rechts ein `<iframe>` mit `<ordner>/<pfad>?view=1` (bzw. ohne `view` für HTML), gleiche Sandbox-Attribute wie der äußere Rahmen. Enthält der Ordner nur Bilder → Galerie mit Vorschaubildern (Raster, Klick öffnet Bild-Betrachter).
- `artifacts::kind_of` und `viewers::page` werden für Dateien **innerhalb** eines Ordners mitbenutzt; der Server liefert Betrachter-Seiten deshalb auch für Pfade mit Unterordnern.

## Abnahmekriterien

- [x] Ein 30-seitiges PDF zeigt Seiten, Vorschaubilder, Zoom und Suche; Text ist markierbar. (Textebene vorhanden; Markieren nicht von Hand probiert.)
- [x] Scheitert der Worker, lädt das PDF trotzdem — geändert: es gibt gar keinen Worker mehr, pdf.js rechnet im Hauptthread (siehe Report-Back).
- [x] `.mmd` zeigt das Diagramm mit Zoom und Verschieben.
- [x] Ein Ordner mit gemischten Dateien zeigt Baum und Vorschau mit den Betrachtern aus Phase 2–4; ein reiner Bildordner zeigt die Galerie.
- [x] `?list=1` außerhalb des Ordners (`..`) liefert 403. (Belegt bis `resolve_folder` per Probe-Test; die HTTP-Antwort selbst lief nicht in der App.)
- [x] Kommentar auf Seite 3 eines PDFs bleibt nach „Neu laden“ an Seite 3. (Anker `#p3` mit `data-anchor` ist nach jedem Laden da; die Brücke in der App nicht gelaufen.)

## Checkliste

- [x] `viewers/pdf.js`, `viewers/mermaid.js`, `viewers/folder.js`; Weiche `page` um `Pdf`, `Mermaid`, `Folder`.
- [x] `server.rs`: `?list=1` für Ordner; Betrachter-Seiten für Pfade in Unterordnern (`folder.rs` neu).
- [x] Probe-Dateien in `artifacts/proben/`: `beispiel.pdf` (30 Seiten, z. B. aus `beispiel.md` gedruckt), `ablauf.mmd`, Ordner `gemischt/` und `bilder/`.
- [x] Code-Map: Zeile „Artefakte“ um `?list=1` ergänzen.

## Definition of Done

- Screenshots in `artifacts/phase-5-*.png`; `curl`-Ausgabe für `?list=1` und den 403-Fall im Report-Back.
- Sichtbares Ergebnis: PDFs, Diagramme und ganze Ordner sind im Reiter durchblätterbar.
- `pnpm check` grün; Seven-Goals-Check.

## Report-Back

**Stand:** umgesetzt, im Browser-Prüfstand belegt (Edge headless über das DevTools-Protokoll in echter Zeit, Seiten als `CSP: sandbox` ausgeliefert), in der App nicht gelaufen. `pnpm check` grün.

**Entscheidungen / Abweichungen:**
- PDF ohne Worker: ein Worker von fremder Herkunft startet unter der CSP des Servers nicht, meldet aber auch keinen Fehler — `getDocument` blieb hängen. pdf.js v4 hat kein `disableWorker` mehr; deshalb lädt der Betrachter `pdf.worker.min.mjs` als Modul mit Prüfsumme (`globalThis.pdfjsWorker`) und rechnet im Hauptthread. Der geplante Test „falscher `workerSrc`“ entfällt damit.
- Ordner-Betrachter nutzt `tree.js` nicht (eigener schlichter Baum, `needs_tree` unverändert); Seiten im Ordner laufen in einem zweiten sandboxed `<iframe>`, der Betrachter zeigt dann zwei Kopfleisten.
- `?list=1` liefert zusätzlich `truncated`; Eintrags-`kind` wie `ArtifactKind`, Unterordner `folder`. Verborgene Namen und Symlinks fehlen.
- Website: „Mit Standardprogramm öffnen“ öffnet jetzt `index.html` im Browser statt den Ordner im Explorer.
- Ein Ordner nur aus Bildern (auch mit Unterordnern) zeigt das Raster; Klick → Bild-Betrachter, „‹ Raster“ zurück.

**Offen / unsicherste Stelle:** `viewers/folder.js` `createPreview` und die Brücke — Kommentare auf Inhalte in der Ordner-Vorschau erreichen die App nicht (die innere Brücke meldet an die Ordner-Seite, nicht an die App). Ordner-Kommentare gehen nur auf Baumzeilen und Raster-Kacheln. In der App prüfen: verschachtelter Rahmen, PDF im Hauptthread (Dauer bei großen Dateien), Zoom per Strg+Mausrad.

**Prüfstand:** Mini-Server in Node, der `viewers::render` und den Ordner-Zweig nachbaut; `list_folder`/`resolve_folder` gegen die echten Proben in einem verworfenen Rust-Test (Liste identisch zum Prüfstand; `..`, `../x`, `/etc`, `C:/Windows` → `FileNotAllowed`, Datei als Ordner → `FileNotFound`). Screenshots `artifacts/phase-5-*.png`.
