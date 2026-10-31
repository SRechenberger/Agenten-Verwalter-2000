# STATE

**Aktiver Plan:** keiner.
**Prüfkette auf diesem Rechner:** `VsDevCmd.bat -arch=amd64` von VS 2022 Build Tools (MSVC ab 14.4x), `%USERPROFILE%\.cargo\bin` im PATH; kein CMake, kein LLVM nötig.
**Danach im Backlog:** `2026-10-22_parallele-sessions` oder `2026-10-22_umgebung` — unabhängig voneinander umsetzbar; beide fassen wie „Automatisch weiter“ die Funktion `send` an, wer später umsetzt, liest sie neu.
**Zuletzt abgeschlossen:** `docs/archive/2026-10/2026-10-30_parakeet/` (ADR 049, keine Migration), Release `v0.40.0`; die Diktat-Probe in der echten App (Download, Satz mit „Worktree“) steht aus; davor `docs/archive/2026-10/2026-10-30_rueckfrage-kasten/` (ADR 045, keine Migration), Release `v0.39.0`; die Selbstprüfung der Oberfläche beider Phasen (Wackelstellen, AK 10) steht aus; davor `docs/archive/2026-10/2026-10-29_nebenfragen/` (ADR 044, keine Migration), Release `v0.38.0`; die Selbstprüfung der Oberfläche (Phase 2 und 3, AK) in der echten App steht aus; davor `docs/archive/2026-10/2026-10-29_unveraenderte-zeilen/` (ADR 043, keine Migration), Release `v0.37.0`; davor `docs/archive/2026-10/2026-10-28_werkzeug-details/` (ADR 042, Migration 015), Release `v0.36.0`. Der volle Update-Durchlauf (Klick → Tausch → Neustart) ist noch nie in der echten App gelaufen.

**Geparkt seit 2026-10-22 (je mit Entwurf unter `artifacts/`):** `2026-10-22_parallele-sessions` (ADR 037), `2026-10-22_umgebung` (ADR 038). Reserviert: ADR 034 für `mitgelieferte-befehle`; keiner braucht eine Migration. **Geparkt seit 2026-10-30:** `2026-10-30_umsetzung` (ADR 048, Migration 17; Entwurf unter `artifacts/`; startet erst nach Phase 1 von `2026-10-30_artefakte-dateitypen`, ADR 046; letzte Phase ändert die Skills im Master `G:\Meine Ablage\U104_Agents`, nicht das Repo).

**Außerdem offen:** Plan `mitgelieferte-befehle` (ungetrackt in `docs/planning/`) übernehmen oder verwerfen; seine README verweist noch auf `../2026-10-21_warnhinweise/` (jetzt im Archiv). Kommt dort eine neue Herkunft dazu, braucht `skillUseKey` (`src/features/skills/skillUses.ts`) und `db/skill_uses.rs` einen eigenen Fall. Dessen generiertes `codex-probe.cjs` ist gezielt aus ESLint ausgeschlossen; `pnpm check` bleibt grün. Keine Smoke-Checklisten mehr: getestet wird im laufenden Betrieb, Releases folgen dem Plan-Abschluss ohne Abnahme.
**Linux-Laufzeitnachweis offen:** `linux-pfade` ist nur per Prüfkette belegt, nicht auf Linux/macOS gelaufen.

## Reihenfolge der geparkten Pläne in `docs/planning/`

1. ~~MCP-Dialog~~ — erledigt und archiviert (`v0.8.0`).
1a. ~~Sidebar nach letzter Aktivität~~ — erledigt und archiviert (`docs/archive/2026-10/2026-10-02_sidebar-aktivitaet.md`), Migration 8 und ADR 019.
1b. ~~Innere Repositories~~ — erledigt und archiviert (`docs/archive/2026-10/2026-10-02_innere-repositories/`, `v0.11.0`, ADR 020). Changes-Review: dessen FINDINGS trägt den Hinweis auf `changes/sources.rs`.
2. ~~Session-Changes~~ — erledigt und archiviert (`v0.9.0`).
3. ~~Changes-Review~~ — erledigt und archiviert (`docs/archive/2026-10/2026-10-01_changes-review/`, `v0.12.0`, ADR 015).
4. ~~Claude Code + LM Studio~~ — erledigt und archiviert (`docs/archive/2026-10/2026-10-01_claude-code-lokal/`, `v0.13.0`, ADR 016).
4a. ~~Claude-Konto~~ — umgesetzt und archiviert (`docs/archive/2026-10/2026-10-05_claude-konto.md`, ADR 021); `v0.14.0`.
5. ~~Autark~~ — umgesetzt und archiviert (`docs/archive/2026-10/2026-10-01_autarker-agent/`, ADR 017), Smoke offen, Release auf Zuruf.
6. ~~Dateiverweise im Chat~~ — auf Zuruf archiviert (`docs/archive/2026-10/2026-10-05_dateiverweise-im-chat.md`, ADR 023), Smoke offen; `v0.15.0` auf Zuruf gesetzt. Prüfprogramm einmal gegen das echte facepass auf dem Arbeitslaptop.
7. ~~MCP-Anmeldung~~ — auf Zuruf archiviert (`docs/archive/2026-10/2026-10-05_mcp-anmeldung.md`, ADR 024), Smoke offen; `v0.16.0` auf Zuruf gesetzt.
8. ~~Git-Werkzeuge~~ — umgesetzt und archiviert (`docs/archive/2026-10/2026-10-05_git-werkzeuge/`, ADR 025), Smoke offen, Release auf Zuruf (Minor-Version).
9. ~~Artefakte~~ — umgesetzt und archiviert (`docs/archive/2026-10/2026-10-05_artefakte/`, `v0.17.0`, ADR 026), Smoke offen.
10. ~~Session-Übergabe~~ — umgesetzt und archiviert (`docs/archive/2026-10/2026-10-05_session-uebergabe.md`, ADR 027, Migration 9), Smoke offen.
11. ~~Vorhaben-Retro~~ — umgesetzt und archiviert (`docs/archive/2026-10/2026-10-07_vorhaben-retro/`, ADR 028), Smoke offen, Release auf Zuruf (Minor-Version).
12. ~~Archiv-Suche~~ — auf Zuruf archiviert (`docs/archive/2026-10/2026-10-07_archiv-suche.md`, ADR 029), Smoke offen; `v0.22.0` auf Zuruf gesetzt.
13. ~~Browser-Zugriff~~ — umgesetzt und archiviert (`docs/archive/2026-10/2026-10-19_browser-zugriff.md`, ADR 030, Migration 10), Smoke offen, Release auf Zuruf.
14. ~~Codex als Betriebsart~~ — umgesetzt und archiviert (`docs/archive/2026-10/2026-10-19_codex/`, ADR 031, Migration 11), Smoke offen, Release auf Zuruf (Minor-Version).
15. ~~Wartet auf Skript~~ — umgesetzt und archiviert (`docs/archive/2026-10/2026-10-20_wartet-auf-skript.md`, ADR 032), Smoke offen, Release auf Zuruf.
16. ~~Warnhinweise~~ — umgesetzt und archiviert (`docs/archive/2026-10/2026-10-21_warnhinweise/`, ADR 033, Migration 12), Smoke offen, Release auf Zuruf (Minor-Version).

## Offen

- Alle archivierten Pläne sind bis `v0.30.0` ausgeliefert. Die Smoke-Abschnitte in den archivierten Plänen sind Altbestand und werden nicht mehr abgearbeitet.
- **ADR-Nummern:** Changes-Review 015, Claude Code lokal 016, Autark 017, „Ordner ohne Git“ 018 (Sidebar hat 019 bekommen), Innere Repositories 020, Claude-Konto 021, Scratchpad im Workspace 022, Dateiverweise im Chat 023, MCP-Anmeldung 024, Git-Werkzeuge 025, Artefakte 026, Session-Übergabe 027, Vorhaben-Retro 028, Archiv-Suche 029, Browser-Zugriff 030, Codex 031, Wartet auf Skript 032, Warnhinweise 033, Kommentare an Artefakt und Bild 035, Ohne Fundstelle 036, Skill-Kassensturz 039, Automatisch weiter 040, Werkzeug-Details 042, Unveränderte Zeilen 043 (vergeben); Nebenfragen 044 (reserviert), Zurück-Zeile 050 (vergeben).
