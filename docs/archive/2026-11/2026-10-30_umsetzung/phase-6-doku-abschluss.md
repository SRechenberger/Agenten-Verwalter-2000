# Phase 6 — Doku-Abschluss: Code-Map, Glossar, AGENTS.md, Prüfprogramm

Rating **mechanisch** · Wave 6 · Aufwand S · Status: complete

## Kontext

- [README.md](README.md): Kontrakt (Dateien und Namen), Entscheidungen (Begriffe).
- Report-Backs der Phasen 1–5 (Abweichungen übernehmen, nicht den Plan abschreiben).
- [docs/code-map.md](../../code-map.md) (Tabelle, Zeilen-Muster „Nebenfragen“), [docs/glossary.md](../../glossary.md) (Zeilen-Muster „Session-Werkzeuge“, „Selbstfreigabe“), [AGENTS.md](../../../AGENTS.md) (Tabelle „Befehle“, Zeile `side-probe` als Muster), `docs/decisions/048-umsetzung.md` (Phase 1), `knowledge/topics/docs-workflow.md` (welche Docs nach Code-Änderungen nachgezogen werden).
- Fehlerklassen (Vault): keine einschlägig (Doc-only).

## Abnahmekriterien

1. `docs/code-map.md`: Features-Liste um `implementation` (Umsetzung) ergänzt; neue Tabellenzeile „Umsetzung (Phasen und Stationen eines Plans, Werkzeuge für den Agenten, Rückfragen mit Lagebericht; ADR 048)“ mit Oberfläche (`src/features/implementation/` mit den Komponenten, `src/stores/implementation.ts`, `src/lib/implementation.ts`, `src/app/SidebarImplementation.tsx`, Zeilenart in `buildSidebarRows.ts`, Zweig in `App.tsx`) und Core (`src-tauri/src/implementation/` mit `model`, `tools`, `start`, `schedule`, `changes`, `report`; `commands/implementation.rs`; `db/implementations.rs`, Migration 19; `sessions/registry/implementation.rs`; Eingriff am Haken `turn_completed` in `update` und in `send`; Prüfprogramm `examples/implementation-probe.rs`); Zeile „Persistenz“ um Migration 19; Zeile „Sessions“ um `station_of`/`inquiry_of`; Zeile „Hinweise im Verlauf“ um `ReportUpdated`; Kopfzeile „Stand“ um einen Halbsatz.
2. `docs/glossary.md`: neue Einträge **Umsetzung** („Ein Plan im Verwalter, als Phasen mit Stationen angelegt über das Werkzeug `implementation_create`; der Verwalter startet jede Station als eigene Session und zeigt den Ablauf als Zeitleiste mit Kartei. Höchstens eine unfertige je Vorhaben. Im Code `implementation` (ADR 048).“), **Phase (Umsetzung)** („Abschnitt einer Umsetzung mit Schlüssel (`1`, `2a`), Titel, Beschreibung und Stationen; Phasen derselben Welle laufen gleichzeitig.“), **Station** („Ein Schritt einer Phase — Test, Umsetzung, Review, Fix-Runde, Gesamt-Review, Merge, Abnahme —, der als Session läuft; ihr Zustand ist der der Session; fertig heißt per `implementation_complete_station` gemeldet, Stationen „durch dich“ per Knopf „Erledigt“.“), **Welle** („Phasen mit derselben Wellennummer, die zugleich starten; die Phase danach wartet auf alle.“), **Lagebericht** („Programmatischer Stand einer Umsetzung als Text: Phasen, Stationen, offene Frage, Plan-Änderungen, geänderte Dateien; hängt an jeder Rückfrage und steht hinter dem Knopf „Lagebericht“.“), **Rückfrage (Umsetzung)** („Reguläre Session im Vorhaben mit dem Lagebericht als Anhang, zum Lesen, Erklären und Umplanen; ändert keinen Code — eine Regel für den Agenten, keine Sperre.“), **Plan-Änderung** („Vorschlag aus einer Session über `implementation_set_phases` mit der vollständigen Liste der kommenden Phasen; wirkt erst nach „Übernehmen“ auf der Karte „Plan ändern?“; neu, vorgezogen, entfallen.“); Eintrag **Session-Werkzeuge** um den Satz „In einer Station einer Umsetzung startet die nächste Station der Verwalter; `verwalter_open_session` legt dort eine reguläre Session an.“
3. `AGENTS.md`: Zeile für `cargo run --manifest-path src-tauri/Cargo.toml --example implementation-probe -- <Datenbank-Kopie> create <Session-ID> <JSON> | show <ID> | complete <ID> <Station-ID> "<Ergebnis>" [--fix] | set-phases <ID> <JSON> | report <ID>` mit Wirkung; Critical Rule 4 um den Halbsatz „, eine Station einer Umsetzung wird nach dem Neustart nicht von selbst gestartet“.
4. `docs/PROJECT.md`: Meilenstein/Scope-Zeile für die Umsetzung, falls dort Meilensteine geführt werden (nachlesen; sonst nichts).
5. Die README dieses Plans: Bottom-Sektionen noch leer (füllt die Archivierung); Status der Phasen 1–6 auf `complete`.

## Checkliste

- [x] Code-Map nach AK 1.
- [x] Glossar nach AK 2.
- [x] AGENTS.md nach AK 3.
- [x] PROJECT.md nach AK 4 (führt keine Meilensteine nach 6b, nichts zu ergänzen).
- [ ] `pnpm check` grün (Prettier über Markdown läuft nicht, trotzdem einmal), Commit `docs(implementation): Code-Map, Glossar und Prüfprogramm zur Umsetzung`.

## Definition of Done

- AK 1–5 erfüllt; **Reviewable Artifact:** `git diff --stat` des Commits und die neuen Glossar-Zeilen im Report-Back; **User-visible outcome:** Ein neuer Leser findet die Umsetzung über Code-Map und Glossar.
- Seven-Goals-Check (ein Satz zu Wartbarkeit reicht).

## Report-Back

Die Zeile „Umsetzung“ in der Code-Map stand schon (aus Phase 2–5); ergänzt wurden Stand-Halbsatz, Feature-Liste, Migration 19, `station_of`/`inquiry_of`, `ReportUpdated`, `start.rs`, die Eingriffe in `auto_handoff::consider` und `send` sowie der Zweig in `App.tsx`. Glossar: sieben neue Einträge, „Einstiegszeile“ und „Automatisch weiter“ um den Satz zu Stationen erweitert (zusätzlich eine Abgrenzung „Rückfrage (Umsetzung)“ ↔ Rückfrage des Agenten). AGENTS.md: Zeile `implementation-probe` mit dem tatsächlichen Aufruf (JSON-Datei statt JSON-Text), Regel 4 ergänzt. PROJECT.md führt keine Meilensteine nach 6b — unverändert. `chrono` steht in keiner Abhängigkeitsliste der Docs, daher kein Eintrag.
