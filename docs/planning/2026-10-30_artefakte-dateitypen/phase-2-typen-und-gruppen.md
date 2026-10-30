# Phase 2 — Typ-Erkennung, Gruppierung nach Session, Medien

**Rating:** heikel (Sicherheitsmodell der Betrachter, Schema-Änderung, Kontrakt für Phasen 3–5) · **Wave:** 2 · **Aufwand:** L

Die Liste nimmt jede Datei und jeden Nicht-Zubehör-Ordner auf, erkennt den Typ, gruppiert nach anlegender Session und zeigt Bilder, Videos, Ton und die Rest-Karte. Danach tragen die Phasen 3–5 nur noch Betrachter ein.

## Kontext — vor dem Start lesen

- `README.md` dieses Plans, Abschnitt **Kontrakt → Artefakte** (verbindlich: `ArtifactKind`, Felder von `Artifact`, Zubehör-Regel, Betrachter-Adresse, Gruppierung).
- `artifacts/artefakte-alle-dateitypen.html` in diesem Plan — Entwurf der gruppierten Liste (Mock-up „Gruppierung nach Session“) und Typ-Katalog. Verbindlich für die Liste: aufklappbare Gruppe je Session mit Titel `#<n> <name>` und Anzahl rechts, eigene Session als erste Gruppe mit Zusatz „· diese Session“ in Akzentfarbe, „Ohne Zuordnung“ zuletzt; je Eintrag Typ-Abzeichen (Großbuchstaben, Festbreitenschrift, Rahmen) links, Titel, darunter Dateiname · Uhrzeit.
- `docs/decisions/026-artefakte.md`, `docs/decisions/035-kommentare-artefakt-bild.md` — Sandbox, CSP, Kommentar-Brücke.
- `src-tauri/src/artifacts/mod.rs` — `ARTIFACT_EXTENSIONS` (Zeile 20), Namensprüfung (Zeilen 65–77), `list` (ab Zeile 82), `latest_owner` (Zeilen 160–176), `title_of`. **Verstanden:** `list` nimmt heute nur `.html`/`.htm` direkt im Ordner; Unterordner sind stillschweigend Zubehör; der Besitzer ist die zuletzt schreibende Session.
- `src-tauri/src/artifacts/model.rs`, `src-tauri/src/artifacts/server.rs` (`answer`, `content_type`, `with_bridge`, `artifact_csp`), `src-tauri/src/artifacts/tools.rs` (aus Phase 1).
- `src-tauri/src/db/session_files.rs` (Upsert Zeilen 18–20, `load_for`), `src-tauri/src/db/migrations/007_session_changes.sql`, `src-tauri/src/db/migrations.rs` (Registrierung der Migrationen), `src-tauri/src/sessions/registry/artifacts.rs`.
- `src-tauri/src/sessions/registry.rs` — `use_artifacts` und der Zweig `Write` in `apply_event` (Karte `ChatEntry::Artifact`), `artifact_file_of` in `artifacts/mod.rs`.
- `src-tauri/src/agents/claude/process.rs` — `artifacts_prompt` (Anweisung an den Agenten); `src-tauri/src/codex/instructions.rs` (dieselbe Anweisung für Codex).
- `src/features/artifacts/` — `ArtifactsView.tsx` (Liste Zeilen 203–249), `ArtifactFrame.tsx`, `ArtifactFullscreen.tsx`, `ArtifactCard.tsx`, `artifactTexts.ts` (`metaLine`), `useProjectArtifacts.ts`; `src/stores/artifacts.ts`; `src/lib/artifacts.ts` (`artifactUrl`).
- `src-tauri/src/file_links/` und `src-tauri/src/commands/file_links.rs` — Öffnen über das Opener-Plugin (Vorbild für „Mit Standardprogramm öffnen“ und „Im Explorer zeigen“).
- Projektregeln: `docs/conventions/rust.md`, `react.md`, `typescript.md`, `tailwind.md`; Mode-Skill `mode-web-frontend` (Idiotensicherheit, a11y).
- Fehlerklassen geprüft: im Vault keine Entities zu Rust, Tauri, React — keine einschlägig.

## Entscheidungen

- **Betrachter-Seiten** liegen als `src-tauri/src/artifacts/viewers/<name>.js` und `viewers/shell.html`, eingebaut per `include_str!` in `viewers/mod.rs`. Der Server baut die Antwort: `shell.html` mit den Platzhaltern `__KIND__`, `__FILE__` (Dateiname als JSON-String, `</` ersetzt durch `<\/`) und `__SCRIPT__` (Inhalt der `.js` des Typs); danach `with_bridge`. Die Weiche `viewers::page(kind: ArtifactKind) -> Option<&'static str>` liefert für noch nicht gebaute Typen `None` → Rest-Karte. Phasen 3–5 ergänzen nur Dateien und Zeilen in dieser Weiche.
- **Bibliotheken** nur über https von `cdn.jsdelivr.net` mit exakter Version und `integrity` (SRI) + `crossorigin="anonymous"`; jeder Betrachter zeigt beim Ladefehler den Rohtext in `<pre>`. Grund: der Verwalter tauscht beim Update nur die exe (ADR 041) — gebündelte Dateien daneben kämen nicht mit; im exe eingebaute Bibliotheken (Mermaid, pdf.js zusammen mehrere MB) blähen jede Version auf. Ohne Internet läuft der Agent ohnehin nur lokal.
- **Medien in Phase 2 ohne Bibliothek:** `image.js` (Einpassen/1:1 per Klick, Zoom per Strg+Mausrad, Schachbrett hinter Transparenz, Umschalter heller/dunkler Hintergrund, Abmessungen und Größe in der Kopfzeile), `video.js` (`<video controls>`, Geschwindigkeit 0,5/1/1,5/2, Bild vor/zurück per `,`/`.` bei Pause), `audio.js` (`<audio controls>` plus Wellenform aus `AudioContext.decodeAudioData` auf `<canvas>`). Abspielfähig ohne Betrachter-Bibliothek, deshalb hier.
- **Rest-Karte** (`ArtifactOtherCard.tsx`, in der App gerendert — sie zeigt nur Metadaten aus dem Core, nie Dateiinhalt): Typ-Abzeichen, Dateiname, Größe, Änderung, Knöpfe „Mit Standardprogramm öffnen“ und „Im Explorer zeigen“.
- **Server-Endungen:** `content_type` ergänzt `pdf` → `application/pdf`, `mp3` → `audio/mpeg`, `wav` → `audio/wav`, `ogg` → `audio/ogg`, `m4a` → `audio/mp4`, `avif` → `image/avif`, `md` → `text/markdown; charset=utf-8`, `csv` → `text/csv; charset=utf-8`, `yaml`/`yml` → `text/yaml; charset=utf-8`; `svg` wird nur über den Bild-Betrachter (als `<img>`) gezeigt, nie direkt als Seite.
- **Ordner-Inhalt** wird über den Server ausgeliefert, wenn der Rahmen `<datei>/` ohne `view` aufruft: Website-Ordner liefern `index.html`; reine Ordner liefern in Phase 2 die Rest-Karte (Betrachter folgt in Phase 5).

## Abnahmekriterien

- [ ] Migration 16 fügt `session_files.first_touched_at REAL` hinzu; der Upsert setzt sie beim ersten Eintrag und lässt sie danach unverändert; Altbestand bleibt `NULL`.
- [ ] `artifacts::list` liefert jede sichtbare Datei und jeden Nicht-Zubehör-Ordner mit `kind`, `size`, `entries`; die drei Zubehör-Regeln greifen (geprüft am Ordner `.artefakte` dieses Vorhabens: `entwurf-umsetzung/`, `ablauf-recherche/`, `phasen-und-rollen/` fehlen in der Liste).
- [ ] `session_*` ist die anlegende Session, `last_session_*` nur gesetzt, wenn eine andere zuletzt schrieb; ohne `first_touched_at` gilt die zuletzt schreibende.
- [ ] Die Liste im Reiter ist nach Kontrakt gruppiert, Gruppen auf- und zuklappbar, Typ-Abzeichen je Eintrag; in der Vorhaben-Übersicht (ohne aktive Session) entfällt „diese Session“.
- [ ] Bild, Video und Ton werden in ihren Betrachtern gezeigt; `?view=1` liefert die Betrachter-Seite mit Kommentar-Brücke, ohne `?view` die Rohdatei.
- [ ] Jeder Typ ohne Betrachter zeigt die Rest-Karte; beide Knöpfe funktionieren.
- [ ] Kommentieren funktioniert auf Bild-, Video- und Ton-Betrachter (Element anklicken → Kommentar).
- [ ] Die Karte im Chat erscheint für jedes `Write` in `.artefakte`, mit Typ-Abzeichen.
- [ ] `artifact_list` (MCP) liefert `kind` aus `ArtifactKind`.
- [ ] Die Anweisung an Claude und Codex nennt, dass jedes Medium als Artefakt abgelegt werden darf, HTML für Interaktives bleibt, und die `_`-Regel für Zubehör-Ordner.

## Checkliste

- [ ] `src-tauri/src/db/migrations/016_session_files_first.sql`: `ALTER TABLE session_files ADD COLUMN first_touched_at REAL;` und in `migrations.rs` registrieren.
- [ ] `db/session_files.rs`: Upsert auf `INSERT … (session_id, path, touched_at, first_touched_at) VALUES (?1, ?2, ?3, ?3) ON CONFLICT … DO UPDATE SET touched_at = max(touched_at, excluded.touched_at)` (`first_touched_at` nicht anfassen); `load_for` liefert zusätzlich `first_touched_at: Option<f64>`. `ArtifactOwner.touched` wird `HashMap<String, (f64, Option<f64>)>` (letzte, erste); alle Leser von `load_for` anpassen (Grep nach `load_for`).
- [ ] `artifacts/model.rs`: `ArtifactKind` und neue Felder nach Kontrakt; Export in `src-tauri/examples/gen-bindings.rs`; `pnpm bindings`.
- [ ] `artifacts/mod.rs`: `ARTIFACT_EXTENSIONS` und Namensprüfung ersetzen durch `kind_of(path) -> ArtifactKind` (Tabelle: html/htm → Html; md/markdown → Markdown; json → Json; jsonl/ndjson → JsonLines; yaml/yml → Yaml; toml/ini/env und Dateiname `.env*` → Config; xml → Xml; csv/tsv → Csv; txt → Text; log → Log; diff/patch → Diff; mmd/mermaid → Mermaid; png/jpg/jpeg/gif/webp/avif/svg/ico → Image; mp4/webm → Video; mp3/wav/ogg/m4a → Audio; pdf → Pdf; Quelltext ts/tsx/js/jsx/mjs/py/rs/go/kt/java/cs/sql/sh/ps1/bat/cmd/css/scss/c/cpp/h/rb/php/swift → Code; sonst Other) und `is_accessory_dir(dir, name, html_files)` nach Kontrakt. `list` liest Dateien und Ordner, überspringt Namen mit `.`-Präfix (außer `.env*`), berechnet `size`/`entries` (Ordner: zwei Ebenen tief, höchstens 1000 Einträge gezählt). Titel: HTML wie bisher aus `<title>`, Markdown aus der ersten Zeile `# …`, sonst Dateiname ohne Endung.
- [ ] `artifacts/mod.rs`: `creator_owner` (kleinstes `first_touched_at`) neben `latest_owner`; `list` setzt `session_*` aus `creator_owner` mit Rückfall `latest_owner`, `last_session_*` wenn abweichend.
- [ ] `artifact_file_of` und der `Write`-Zweig in `registry.rs`: jede Datei direkt in `.artefakte` oder in einem Nicht-Zubehör-Ordner ergibt die Karte (bei Dateien in Ordnern: Karte für den Ordner).
- [ ] `artifacts/viewers/mod.rs`, `shell.html` (Grundgerüst: Kopfzeile mit Dateiname und Typ, Bühne, Rohtext-Rückfall, Tokens für hell/dunkel über `prefers-color-scheme`), `image.js`, `video.js`, `audio.js`; Weiche `page(kind)`.
- [ ] `artifacts/server.rs` `answer`: Query `view=1` erkennen (heute wird die Query abgeschnitten, Zeile 118 — Query vorher auswerten); bei `view=1` und `viewers::page(kind)` = `Some` → Betrachter-Seite als `text/html` mit `with_bridge` und gleicher CSP; Ordner-Ziel ohne `view` → `index.html` des Ordners, falls vorhanden, sonst 404. `content_type` nach Entscheidung erweitern.
- [ ] Commands `artifact_open_default(session_id, file)` und `artifact_reveal(session_id, file)` in `commands/artifacts.rs` (Pfad über `artifacts::resolve` prüfen, öffnen per Opener-Plugin wie `file_link_open`; Explorer: `opener.reveal_item_in_dir`); Wrapper in `src/lib/artifacts.ts`; in `lib.rs` registrieren.
- [ ] `src/features/artifacts/groupArtifacts.ts` (reine Funktion nach Kontrakt), `ArtifactGroupHeader.tsx`, `ArtifactKindBadge.tsx` (Kurztexte je Typ in `artifactTexts.ts`: HTML, SITE, DIR, MD, JSON, JSONL, YAML, CONF, XML, CSV, CODE, TXT, LOG, DIFF, BILD, VIDEO, TON, PDF, MMD, DATEI), `ArtifactOtherCard.tsx`; `ArtifactsView.tsx` rendert Gruppen statt flacher Liste; `src/stores/artifacts.ts` um `collapsed: Record<string, Record<string, boolean>>` (Vorhaben → Gruppen-Schlüssel) mit `toggleGroup`.
- [ ] `ArtifactFrame.tsx`/`ArtifactFullscreen.tsx`: Adresse je Typ — `Html` und `Site` wie bisher (Site: `<datei>/`), Typen mit Betrachter `<datei>?view=1`, sonst `ArtifactOtherCard` statt Rahmen. Welche Typen einen Betrachter haben, liefert der Core als Feld `viewable: bool` an `Artifact` (aus `viewers::page(kind).is_some()` oder `Html`/`Site`) — die Oberfläche hält keine eigene Liste.
- [ ] `artifactTexts.ts` `metaLine`: bei `lastSessionNumber` Zusatz „· zuletzt #<n>“; `ARTIFACTS_INFO` an neue Regeln anpassen.
- [ ] `ArtifactCard.tsx` (Chat): Typ-Abzeichen.
- [ ] `artifacts/tools.rs`: `kind` aus `ArtifactKind` (kebab-case).
- [ ] Anweisung `artifacts_prompt` in `process.rs` und `codex/instructions.rs`: „Möchte der Benutzer etwas zum Ansehen, lege es als Artefakt direkt in diesen Ordner: jede Datei, die einen Sachverhalt veranschaulicht — HTML-Seite, Markdown, Bild, Diagramm (Mermaid), PDF, Video, Ton, JSON, YAML, CSV oder ein Ordner. Interaktives als HTML. Dateiname in Kleinbuchstaben mit Bindestrichen. Dateien, die nur eine Seite braucht (Bilder, Skripte, Stylesheets), gehören in einen Unterordner, dessen Name mit _ beginnt. Zum Überarbeiten dieselbe Datei ändern.“ (übrige Sätze der bisherigen Anweisung zu Netz und Veröffentlichen bleiben).
- [ ] Prüfprogramm `src-tauri/examples/artifacts-probe.rs`: `artifacts-probe <Datenbank-Kopie> <Session-ID>` druckt die Liste wie `artifacts::list` (Datei, Typ, Größe, anlegende/letzte Session, Zubehör übersprungen); Aufruf in `AGENTS.md`.
- [ ] ADR `docs/decisions/047-artefakte-alle-dateitypen.md` (Definition „Artefakt“ aus der README, Zubehör-Regel, anlegende Session, Betrachter im Rahmen statt im App-Fenster, Bibliotheken über CDN mit SRI statt gebündelt, Rest-Karte); ADR 026 Kopf um „ergänzt durch ADR 047“.

## Definition of Done

- Abnahmekriterien belegt durch: `artifacts-probe` gegen eine Kopie der Datenbank (Ausgabe im Report-Back), `.schema session_files` vorher/nachher, Screenshots der gruppierten Liste (hell und dunkel), Bild-, Video-, Ton-Betrachter, Rest-Karte, Kommentar auf einem Bild.
- Sichtbares Ergebnis: Reiter „Artefakte“ zeigt alle Dateien gruppiert nach Session; Bilder/Videos/Ton laufen; alles andere hat die Rest-Karte.
- Artefakte: `artifacts/phase-2-*.png`, `artifacts/phase-2-probe.txt`.
- `pnpm check` grün, `pnpm bindings` gelaufen.
- Seven-Goals-Check; Schwerpunkt Sicherheit (kein Dateiinhalt im App-Fenster) und Wartbarkeit (eine Typ-Tabelle, eine Betrachter-Weiche).

## Report-Back
