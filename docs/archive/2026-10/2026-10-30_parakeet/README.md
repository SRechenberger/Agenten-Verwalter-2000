# Parakeet statt Whisper

Ziel: Das Sprachdiktat erkennt mit NVIDIA Parakeet TDT 0.6B v3 (ONNX, int8, nur CPU) statt mit Whisper (whisper.cpp, small-q5_1). Aufnahme, Pausen-Schnitt, Stille-Prüfung, Zwischentext-Ereignisse und Bedienung bleiben, wie sie sind; ausgetauscht werden Erkennung, Modell-Download und Build-Voraussetzungen. Whisper verschwindet vollständig (Entscheidung des Users: „ersetzen“, nicht wählbar).

| # | Phase | Rating | Wave | Aufwand | Status |
|---|---|---|---|---|---|
| 1 | [Messlauf: Parakeet gegen Whisper auf derselben Aufnahme, Go/No-Go](phase-1-messlauf.md) | heikel | 1 | M | complete (Go) |
| 2 | [Core: Modellsatz laden, Parakeet erkennt, whisper-rs raus](phase-2-core-umstellen.md) | heikel | 2 | L | complete |
| 3 | [Spracheinstellung und Whisper-Texte aus Einstellungen und Oberfläche entfernen](phase-3-einstellung-und-texte.md) | standard | 3 | S | complete |
| 4 | [Build, CI, Doku, ADR 049, Release](phase-4-aufraeumen-und-release.md) | mechanisch | 4 | S | complete |

**Warum rein sequenziell:** Phase 1 entscheidet, ob der Plan überhaupt weiterläuft (Gate). Phase 2 und 3 ändern beide `src-tauri/src/voice/mod.rs`, `src-tauri/src/commands/voice.rs` und über die Bindings dieselben TS-Dateien; Phase 3 baut auf der in Phase 2 gestrichenen Whisper-Abhängigkeit auf; Phase 4 fasst die fertigen Ergebnisse zusammen. Keine disjunkten Datei-Mengen, also keine Wellen.

**Rahmen:** direkt auf `main`, ein Commit je Phase, Scope `voice`. Keine Migration (Einstellungs-Schlüssel `voice_language` bleibt als toter Eintrag in der Datenbank liegen, wird nirgends mehr gelesen). Keine automatisierten Tests und keine Smoke-Checkliste (Projektstand, wie die Nachbarpläne). `pnpm check` grün vor jedem Commit. Planabschluss mit Versions-Tag (Minor-Version, siehe [releases.md](../../conventions/releases.md)). Schätzung: zwei bis drei Sessions, rund 5–7 Stunden, davon Phase 2 die Hälfte; Unsicherheit liegt in Phase 1 (Bau von ONNX Runtime auf diesem Rechner).

**Nummern:** ADR 049 (Phase 4). Belegt bzw. reserviert und nicht zu verwenden: ADR 034, 037, 038 (geparkte Pläne), 043, 044, 045 (Notiz pro Option), 046, 047 (Artefakte aller Dateitypen), 048 (Umsetzung). Keine Migration.

**Nicht drin:** GPU-Beschleunigung (DirectML/CUDA) — wäre ein Folgeplan, falls die CPU-Zeit trotz Parakeet nicht reicht; Sprachwahl (Parakeet TDT erkennt die Sprache selbst, der Aufruf nimmt keinen Sprachparameter); Wortschatz-Hinweis (Parakeet TDT kennt keinen); wählbare zweite Engine; Zeitstempel.

## Festgelegte Entscheidungen

- **Engine:** Crate `parakeet-rs` 0.4.3 (MIT/Apache-2.0, pinnt `ort =2.0.0-rc.15`), Modell `parakeet-tdt-0.6b-v3` als int8-ONNX. Quelle der Gewichte: Hugging-Face-Repo `Discourse/parakeet-tdt-0.6b-v3-onnx`, festgepinnt auf Revision `b4ee2464a71b0a705915856c351b1cc19340e6b6`. Verwendet werden `encoder-model.int8.onnx` (ca. 652 MB), `decoder_joint-model.int8.onnx` (ca. 18 MB) und `vocab.txt`; die genauen Bytes und SHA-256 trägt Phase 1 in die Tabelle „Modellsatz“ unten ein. Gesamtgröße rund 670 MB statt 190 MB.
- **Nur CPU**, keine Zusatz-Features von `parakeet-rs` (kein `cuda`, `directml` usw.). Abnahme-Kriterium der Geschwindigkeit bleibt das aus ADR 009: ein ~10-s-Satz ist höchstens 5 s nach dem Stopp erkannt.
- **Fällt Phase 1 durch** (Gate nicht erreicht, Crate baut nicht, Modell lädt nicht), stoppt der Plan: kein stiller Wechsel auf eine andere Crate oder Engine, sondern 🔴 an den User mit den Messwerten. Whisper bleibt dann unangetastet.
- **Erkennungs-Thread statt geteiltem Zustand:** `ParakeetTDT::transcribe_samples` braucht `&mut self`. Der `Transcriber` hält das Modell deshalb in einem `Mutex`; ein auslaufender abgebrochener Durchlauf blockiert damit ein neues Diktat, bis er fertig ist (bei Whisper liefen beide nebeneinander). Hinnehmbar, weil der Abbruch Oberfläche und Mikrofon weiterhin sofort freigibt und ein Durchlauf nur Sekunden dauert.
- **Kein Abbruch mitten im Rechnen** (Parakeet hat keinen Abbruch-Rückruf wie whisper.cpp): `cancel` wird vor und nach dem Durchlauf geprüft, wie schon in `dictation.rs`. Zwangsschnitt (25 s) und Zusammenfassen liegengebliebener Abschnitte bleiben unverändert.
- **Wegfall ohne Ersatz:** Wortschatz-Hinweis (`BASE_PROMPT`, Repository-Namen, `prompt_tail`), Sprachwahl, Liste der Whisper-Halluzinationen („Untertitel im Auftrag des ZDF“). Die Stille-Prüfung (`silence.rs`) bleibt, ihr Kommentar nennt nicht mehr Whisper.
- **Altes Modell:** Nach erfolgreichem Download des neuen Satzes löscht der Core die Datei `ggml-small-q5_1.bin` im Modell-Ordner (genau dieser Name, sonst nichts), best effort. Wer nie neu lädt, behält sie, Diktat zeigt dann „Einrichten“ (neuer Satz fehlt).
- **Ablage:** `<Benutzerordner>\.verwalter\models\parakeet-tdt-0.6b-v3\` mit den drei Dateien in dem Layout, das Phase 1 als ladbar belegt.

## Kontrakt (Phase 1 füllt die Lücken, Phase 2 liest sie)

```rust
// src-tauri/src/voice/transcribe.rs — Außenansicht, bleibt für dictation.rs nutzbar
pub struct Transcriber { /* Mutex<ParakeetTDT> */ }
impl Transcriber {
    pub fn load(dir: &Path) -> Result<Transcriber, CommandError>;
    /// Erwartet 16 kHz mono. Leeres Ergebnis → `NoSpeech`, abgebrochen → `VoiceCancelled`.
    pub fn transcribe(&self, audio: &[f32], cancel: &Arc<AtomicBool>) -> Result<String, CommandError>;
}
// src-tauri/src/voice/model_file.rs
pub const MODEL_DIR_NAME: &str = "parakeet-tdt-0.6b-v3";
pub fn model_dir(app: &tauri::AppHandle) -> Result<PathBuf, CommandError>;
pub fn is_model_ready(dir: &Path) -> bool;          // alle Dateien da, je Größe stimmt
pub fn total_bytes() -> u64;                         // Summe der Dateigrößen
pub fn download(dir: &Path, cancel: &AtomicBool, on_progress: impl FnMut(u64, u64)) -> Result<(), CommandError>;
```

**Modellsatz** (Phase 1 trägt ein; Zeilen unverändert in `model_file.rs` als Konstanten):

| Datei (lokal) | Quelle (Pfad im Repo, fest gepinnt) | Bytes | SHA-256 |
|---|---|---|---|
| `encoder-model.int8.onnx` | `encoder-model.int8.onnx` | 652183999 | `6139d2fa7e1b086097b277c7149725edbab89cc7c7ae64b23c741be4055aff09` |
| `decoder_joint-model.int8.onnx` | `decoder_joint-model.int8.onnx` | 18202004 | `eea7483ee3d1a30375daedc8ed83e3960c91b098812127a0d99d1c8977667a70` |
| `vocab.txt` | `vocab.txt` | 102132 | `ba8e4007c65f4bb4358ffe2ecc13d9ccc7a10351151065242b5c3a943e685742` |

## Finale AK (Gesamtergebnis)

- [ ] `grep -ri whisper src src-tauri/src src-tauri/Cargo.toml .cargo .github README.md` findet nichts mehr außer dem Hinweis auf die Ablösung in der Doku (ADR 009 bleibt als Historie, Kopf „abgelöst durch 049“).
- [ ] `cargo run --manifest-path src-tauri/Cargo.toml --example parakeet-probe -- <WAV>` erkennt eine deutsche ~10-s-Aufnahme über denselben `Transcriber` wie die App in höchstens 5 s und gibt den Text aus (Zahl steht im Report-Back von Phase 2).
- [ ] In der laufenden App (`pnpm tauri dev`): Menü am Mikrofon zeigt „Einrichten“ mit der neuen Größe, Download läuft mit Fortschritt bis „Bereit“, Prüfsummen stimmen; das alte Modell ist danach gelöscht.
- [ ] Einstellungen → Spracheingabe enthält keine Sprachwahl mehr; kein toter Knopf, kein Text über Whisper.
- [ ] Ein sauberer Bau (`cargo clean`, dann `pnpm check`) gelingt **ohne CMake und ohne LLVM** auf diesem Rechner, oder die Doku nennt genau die, die noch gebraucht werden (und warum).
- [ ] `pnpm check` grün, Code-Map, Glossar, README, PROJECT, linting.md, ADR 009 (Kopf) und ADR 049 nachgezogen, Versions-Tag gesetzt.

## Risiken (Top 3)

- 🟡 **ONNX Runtime auf diesem Rechner:** `ort` lädt oder baut native Bibliotheken; VS 2019 Build Tools und die Linux-CI sind ungeprüft. Mitigation: Phase 1 baut zuerst nur das Probe-Programm und prüft `cargo tree -p ort` und den Linux-Schritt in CI vor dem Umbau.
- 🟡 **RAM und Ladezeit:** int8-Encoder 652 MB auf Platte, Arbeitsspeicher beim Laden nicht gemessen. Mitigation: Phase 1 misst Ladezeit und Spitzen-Speicher; Grenzen siehe Gate.
- 🟡 **Qualität auf Fachwörtern:** Ohne Wortschatz-Hinweis kann „Worktree“, „Commit“ oder „Verwalter“ schlechter erkannt werden als mit Whisper plus Hinweis. Mitigation: Phase 1 vergleicht beide auf einem Satz mit diesen Wörtern; das Urteil trifft der User beim Lesen der Gegenüberstellung.

## Summary

Das Diktat erkennt mit Parakeet TDT 0.6B v3 (ONNX, int8, CPU) statt mit Whisper. Gemessen: 8,3-s-Satz in 0,5 s, Wortfehler 10 % gegen 25 %. Whisper, CMake, LLVM und die Compiler-Flags sind aus dem Bau verschwunden; es gibt keine Sprachwahl und keinen Wortschatz-Hinweis mehr. ADR 049, Version 0.40.0.

## Files touched

`src-tauri/src/voice/` (Erkennung, Modell-Download), `commands/voice.rs`, Einstellungen und Bindings (Sprachwahl raus), `VoiceButton`, `.cargo/config.toml` (gelöscht), `.github/workflows/`, Doku (ADR 009/049, linting, README, PROJECT, Glossar, Code-Map).

## Commits

Phase 1 `9f90bb5`, Phase 2 `d3ebd4b`, Phase 3 `589178b`…`f72773e`, Phase 4 siehe Branch `feature/parakeet-release`.

## Deviations from plan

Phase 4 lief im Arbeitsbaum auf einem Feature-Branch statt direkt auf `main` (Hauptbaum durch parallele Session belegt). Schätzung 5–7 h; Ist nicht erfasst. Zusatz-Voraussetzung gegenüber der Annahme: MSVC ab 14.4x (VS 2019 und 14.38 scheitern am Link).

## Follow-ups

- Der User diktiert einmal in der echten App (Download, ein Satz mit „Worktree“, „Commit“, „Verwalter“); Auffälligkeiten hier nachtragen.
- Erster Tag-Lauf der CI: Linux (`cmake clang` ungeprüft), `ort-sys`-Download, Exe-/Installer-Größe messen.
- Gesamt-Review vor dem Merge (Plan hat eine mechanische Phase).
- Ladezeit 3,5–4,0 s beim ersten Diktat; falls störend, Modell beim Start vorladen.
