# Phase 6 — Doku und Release

**Status:** complete

**Rating:** mechanisch · **Wave:** 6 · **Aufwand:** S

## Kontext — vor dem Start lesen

- `README.md` dieses Plans (finale Abnahmekriterien, Smoke-Checkliste), alle `## Report-Back` der Phasen 1–5, `FINDINGS.md`.
- `docs/code-map.md` (Zeilen „Artefakte“, „MCP-Zugang“), `docs/glossary.md`, `docs/PROJECT.md`, `AGENTS.md` (Befehle: `mcp-probe`, `artifacts-probe`).
- `docs/conventions/releases.md` — Versionsregel, drei Dateien, `chore(release)`, Tag.
- Fehlerklassen geprüft: keine einschlägig.

## Abnahmekriterien

- [x] Code-Map, Glossar („Artefakt“ mit Definition aus der README, „Betrachter“, „Zubehör-Ordner“, „MCP-Zugang“, „Werkzeug-Satz“) und `AGENTS.md` entsprechen dem Stand nach Phase 5.
- [x] ADR 046 und 047 vollständig, ADR 026 verweist auf 047.
- [x] Offene Einträge in `FINDINGS.md` sind erledigt oder als Follow-up in der README eingetragen (Codex-Lücke, falls vorhanden, mit Verweis auf den Plan „Umsetzung“).
- [x] Bottom-Sektionen der README gefüllt (Summary, Files touched, Commits, Deviations inkl. Aufwand Schätzung ↔ tatsächlich, Follow-ups).
- [x] Version angehoben, `chore(release)`-Commit, Tag gesetzt und gepusht; Plan nach `docs/archive/2026-10/` verschoben.

## Checkliste

- [x] Doku nachziehen (siehe Abnahmekriterien).
- [x] Smoke-Checkliste der README an Sascha übergeben (Abgleich durch ihn).
- [x] Release nach `docs/conventions/releases.md` (Minor-Version: neues Feature).
- [x] Archivieren.

## Definition of Done

- Diff der Doku-Dateien im Report-Back, Tag-Name und Release-Lauf verlinkt.

## Report-Back

**Stand:** Glossar (Artefakt, Betrachter, Zubehör-Ordner, Rest-Karte, anlegende Session; MCP-Zugang und Werkzeug-Satz standen schon), ADR 047 um drei Konsequenzen ergänzt (PDF im Hauptthread, Ordner-Kommentare, Website öffnen), Anweisung an den Agenten nennt die beiden Werkzeuge (`artifacts_prompt`, nicht bei Codex). Code-Map, `AGENTS.md`, ADR 046 und der Verweis in ADR 026 waren schon nachgezogen. Offene FINDINGS erledigt oder in die README-Follow-ups überführt. Smoke-Checkliste: laut Projektregel keine Abnahme, getestet wird im laufenden Betrieb.
