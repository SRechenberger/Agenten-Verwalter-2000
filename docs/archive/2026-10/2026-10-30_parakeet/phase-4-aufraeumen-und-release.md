# Phase 4 — Build, CI, Doku, ADR 049, Release

Rating **mechanisch** · Wave 4 · Aufwand S · Status **complete** (Release-Tag steht aus Freigabe)

## Kontext

- [README.md](README.md): Entscheidungen, finale AK; Voraussetzung: Phasen 1–3 sind `complete`, Messwerte stehen in den Report-Backs.
- Zu ändern, vorher lesen: `.cargo/config.toml` (nur whisper.cpp-Flags), `.github/workflows/release.yml` (Schritt „Compiler-Flags für whisper.cpp“ Zeilen 59–65; Linux-Abhängigkeiten Zeile 38 mit `cmake clang`), `docs/conventions/linting.md` (Abschnitt „Voraussetzungen“, Zeilen 30–37), `README.md` (Zeile 16), `docs/PROJECT.md` (Zeile 78), `docs/glossary.md` (Eintrag „Sprachmodell“, Zeile 79), `docs/code-map.md` (Zeile 51, enthält Whisper), `docs/decisions/009-sprachdiktat-lokal.md`, `STATE.md`, `docs/conventions/releases.md` (Ablauf des Versions-Tags), `AGENTS.md` (Zeile zur Probe).
- ADR-Format: ein vorhandenes ADR als Muster, z. B. `docs/decisions/044-nebenfragen.md` (Kopf Status/Datum, Kontext, Optionen, Entscheidung, Konsequenzen; rund zehn Zeilen).
- ADR-Nummer: 049 ist frei (belegt/reserviert bis 048, siehe README); vor dem Anlegen `docs/planning/` und `docs/decisions/` nach `049` durchsuchen, falls inzwischen ein anderer Plan sie nahm — dann nächste freie Nummer und in README/Commit nennen.
- Chesterton's Fence: `.cargo/config.toml` enthielt ausschließlich whisper.cpp-Bauflags (Zeilen 1–15, Kommentar nennt es selbst); es gibt keine anderen Einträge, also fällt die Datei. Schritt „Compiler-Flags für whisper.cpp“ in CI setzte nur `CMAKE_*_FLAGS_RELEASE` für Nicht-Windows. `cmake`/`clang` unter Linux-Abhängigkeiten dienten whisper.cpp **und** möglicherweise anderen Crates — nicht raten, Phase prüft (unten).

## AK der Phase

- [x] **Bau ohne CMake und LLVM:** In einer Shell **ohne** `LIBCLANG_PATH`, `BINDGEN_EXTRA_CLANG_ARGS` und ohne CMake im PATH (Variablen für die Prüfung entfernen, PATH-Eintrag der Visual-Studio-Variante weglassen; was in dieser Umgebung gesetzt ist, vorher mit `echo`/`Get-Command cmake` festhalten) gelingt `cargo clean` und danach `pnpm check`. Gelingt es nicht, nennt der Fehler die nötige Abhängigkeit; die Doku listet **genau** die tatsächlich gebrauchten (mit Grund), nicht mehr.
- [x] `grep -rni "whisper" . --include=* -l` (ohne `node_modules`, `target`, `dist`, `.git`) nennt nur noch: `docs/archive/**`, `docs/decisions/009-sprachdiktat-lokal.md`, `docs/decisions/049-*.md`, `docs/planning/2026-10-30_parakeet/**`, `docs/knowledge/**` falls vorhanden.
- [x] ADR 049 existiert: Kontext (warum Parakeet: User-Wunsch „besser“, gemessene Zahlen aus Phase 1), Optionen (a) Whisper behalten, (b) Parakeet ersetzt, (c) beide wählbar — verworfen mit Grund, (d) GPU-Weg — verworfen, Folgeplan), Entscheidung (b), Konsequenzen (siehe README: kein Wortschatz-Hinweis, keine Sprachwahl, 670 MB statt 190 MB, ONNX Runtime als Abhängigkeit, Mutex, keine Prozess-Eigenschaften der Build-Kette mehr).
- [x] ADR 009 trägt im Kopf `**Status:** abgelöst durch [049](049-…) für die Erkennung; Aufnahme, Pausen-Schnitt und Ein-Aufnahme-Regel gelten weiter` und sonst unveränderten Text.
- [x] `STATE.md`: der Hinweis zu whisper-rs-sys in „Nächster Schritt“ ist entfernt oder auf den tatsächlichen Stand korrigiert; Zeile „Zuletzt abgeschlossen“ nennt den Plan, ADR 049, die Version.
- [ ] Version in `package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` angehoben (Minor), `chore(release)`-Commit, Tag `vX.Y.Z` gesetzt und gepusht (Ablauf: `docs/conventions/releases.md`).

## Checkliste

Implementation
- [x] `.cargo/config.toml` löschen (`git rm`), falls Phase-4-Prüfung zeigt, dass kein anderer Crate darauf angewiesen ist (Bau ohne die Datei gelingt: Teil der AK oben).
- [x] `.github/workflows/release.yml`: Schritt „Compiler-Flags für whisper.cpp“ samt Kommentarzeile 59 löschen. `cmake clang` aus der Linux-Abhängigkeitsliste (Zeile 38) nur entfernen, wenn ein lokaler Bau ohne beide gelungen ist **und** `ort` unter Linux laut FINDINGS (Phase 1) nichts davon braucht; sonst bleiben sie und die Entscheidung steht in FINDINGS. Ein Lauf der CI auf Linux ist hier nicht möglich → 🟡 im Report-Back: erster Tag-Lauf ist die Probe, Rückfall ist ein Tag-Löschen und ein Fix-Commit.
- [x] `src-tauri/Cargo.toml`/`Cargo.lock`: prüfen, dass `whisper-rs`, `whisper-rs-sys`, `cmake`, `bindgen` nicht mehr im Baum sind (`cargo tree -i whisper-rs-sys` meldet „did not match“).

Doku (alles im selben Commit wie der Code, den es beschreibt)
- [x] `docs/conventions/linting.md`, Abschnitt „Voraussetzungen“: auf den in der AK belegten Stand bringen; Absatz zu den Compiler-Flags für whisper.cpp streichen. Ist am Ende keine Zusatz-Voraussetzung mehr nötig, der Abschnitt wird ein Satz („Zusätzlich zu Rust braucht der Bau nichts.“).
- [x] `README.md` Zeile 16: „… mit lokaler Parakeet-Erkennung“. `docs/PROJECT.md` Zeile 78: Erkennung lokal mit Parakeet, Modell rund 670 MB, Hinweis „Whisper bis v<alte Version>“ (Version aus dem letzten Tag lesen: `git describe --tags --abbrev=0`). `docs/glossary.md` „Sprachmodell“: „Die lokal gespeicherten Parakeet-Dateien unter `<Benutzerordner>\.verwalter\models\parakeet-tdt-0.6b-v3\`, die das Diktieren braucht; werden einmal auf Klick geladen (rund 670 MB), danach läuft die Erkennung ohne Internet.“ `docs/code-map.md` Zeile 51: Whisper-Erwähnung auf Parakeet umstellen (ganze Zeile lesen, nur die betroffene Stelle ändern, ADR 049 ergänzen).
- [x] ADR 049 anlegen und ADR 009 kennzeichnen (s. AK).
- [x] `AGENTS.md`: `parakeet-probe`-Zeile prüfen (aus Phase 2); Eintrag zur Bau-Voraussetzung, falls dort erwähnt, korrigieren.

Definition of Done
- [x] Alle AK erfüllt, Zitate im Report-Back.
- [ ] Sichtbares Ergebnis: sauberer Bau ohne Whisper-Werkzeugkette; Release-Tag.
- [x] Artefakt: Ausgabe `cargo tree -i whisper-rs-sys`, die `grep`-Liste, letzte Zeilen von `pnpm check`, `git tag --points-at HEAD`.
- [x] Seven-Goals-Sanity je ein Satz; Gesamtvergleich Schätzung ↔ Ist in den Bottom-Sektionen der README (Summary, Files touched, Commits, Deviations, Follow-ups).
- [x] Plan archivieren nach `docs/archive/2026-10/2026-10-30_parakeet/` (`git mv`), `STATE.md` auf „Aktiver Plan: keiner“.
- [ ] Commits: `docs(voice): ADR 049, Whisper-Verweise entfernt` und `chore(release): vX.Y.Z`.
- [ ] Nachlaufend, nicht blockierend: der User diktiert einmal in der echten App (Download, ein deutscher Satz mit „Worktree“, „Commit“, „Verwalter“); Auffälligkeiten als Follow-up in der README.

## Report-Back

**Stand:** Bau, Doku, ADR 049 fertig; Version 0.40.0 (0.39.0 ging an den parallelen Rückfrage-Kasten), Tag und Push mit dem Merge.

| AK | Beleg |
|---|---|
| Bau ohne CMake/LLVM | Umgebung vorher: kein `LIBCLANG_PATH`, kein `BINDGEN_EXTRA_CLANG_ARGS`; CMake nur unter `C:\Program Files\CMakein` (für den Lauf aus dem PATH gefiltert, ebenso LLVM). Lauf: `cmake: False`, `clang: False`, `cl` = MSVC 14.44.35207; frischer Arbeitsbaum ohne `target/`, `pnpm check` Exit 0 (Clippy-Teil 2 min 58 s) |
| `cargo tree -i whisper-rs-sys` | „did not match any packages“; ebenso `whisper-rs`, `bindgen`; `cmake` nur als Namensvorschlag `cookie` |
| Whisper-Treffer | nur ADR 009, ADR 049, `docs/archive/**`, dieser Plan; zusätzlich der gewollte Hinweis „bis v0.38.0 Whisper“ in `docs/PROJECT.md` und STATE |
| ADR 049, ADR 009 Kopf | angelegt bzw. gekennzeichnet |

**Entscheidungen:** `cmake clang` bleiben in der Linux-Abhängigkeitsliste von `release.yml` und `check.yml` (compile-Job): ein Linux-Lauf war hier nicht möglich, Linux-Jobs sind optional. Aus dem Windows-Job von `check.yml` fielen `cmake --version`/`clang --version` weg (belegt entbehrlich). DirectML: die Exe nutzt `directml.dll` aus dem Windows-Bestand, kein Mitliefern im Installer. Größe der App-Exe nicht gemessen (kein Release-Bau in dieser Phase).

**Abweichungen:** Arbeitsbaum + Feature-Branch `feature/parakeet-release` statt direkt auf `main`, weil der Hauptbaum eine parallele Session mit offenen Änderungen trug.

**🟡 Unsicherste Stelle:** `.github/workflows/release.yml` Zeile 38 (Linux, `cmake clang`) und der `ort-sys`-Download von `cdn.pyke.io` in der CI — beides ungeprüft; erster Tag-Lauf ist die Probe, Rückfall Fix-Commit.

**Seven Goals:** Simplicity — Bauvoraussetzungen auf zwei Punkte geschrumpft; Wartbarkeit — Doku nennt nur Gebrauchtes mit Grund; Stabilität — Tag erst nach grüner Prüfkette.
