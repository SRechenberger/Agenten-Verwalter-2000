# Phase 2 — Core: Modellsatz laden, Parakeet erkennt, whisper-rs raus

Rating **heikel** (Concurrency: `Mutex` statt geteiltem Kontext, Abbruch ohne Rückruf; mehrdateiiger geprüfter Download) · Wave 2 · Aufwand L

## Kontext

- [README.md](README.md): Entscheidungen, Kontrakt (Signaturen), Tabelle „Modellsatz“ (von Phase 1 gefüllt). **Voraussetzung:** Phase 1 ist `complete` mit Go; der FINDINGS-Eintrag „→ Phase 2“ gibt Layout, Signaturen und `Send`-Befund vor — im Zweifel gilt FINDINGS vor dem Wortlaut hier.
- Zu ändern, vorher lesen: `src-tauri/src/voice/transcribe.rs`, `model_file.rs`, `dictation.rs`, `mod.rs`, `silence.rs`, `segmenter.rs`, `resample.rs`; `src-tauri/src/commands/voice.rs`; `src-tauri/src/settings/mod.rs` (nur die Prüfung mit `whisper_rs::get_lang_id`, Zeilen 79–85); `src/lib/voice.ts`, `src/features/voice/useDictation.ts`; `src-tauri/Cargo.toml`; `src-tauri/examples/parakeet-probe.rs` (aus Phase 1).
- `docs/conventions/rust.md`, `docs/conventions/linting.md`; ADR 009 (`docs/decisions/009-sprachdiktat-lokal.md`) für das Warum von Aufnahme, Pausen-Schnitt, Ein-Aufnahme-Regel — das bleibt.
- Chesterton's Fence, **verstanden und beibehalten:** `is_model_ready` vergleicht nur Größen (Hashen kostet Sekunden je Statusaufruf, die Prüfsumme gilt beim Download vor dem Umbenennen); `.part`-Datei plus Umbenennen nach bestandener Prüfsumme (nie eine halbe Datei unter dem Endnamen); `ureq::call()` macht aus HTTP-Status ab 400 einen Fehler (nicht abschalten); `TranscribingGuard` und je-Diktat-neues `cancel` in `mod.rs` (ein auslaufender abgebrochener Thread darf das nächste Diktat nicht sehen). **Verstanden und gestrichen:** `BASE_PROMPT`/`base_prompt`/`prompt_tail` (Whisper-Erkennungshilfe, Parakeet kennt keine), `HALLUCINATIONS` (Whisper-Untertitel-Abspänne, entstehen nur dort), `set_abort_callback_safe` mit Box-Kniff (whisper-rs-Eigenart).
- Vault Fehlerklassen (nur Abschnitt): `knowledge/sprachen/rust.md`. Prüfpunkte daraus als eigene AK-Zeile ergänzen, falls einschlägig (Nebenläufigkeit/Mutex-Vergiftung: `PoisonError::into_inner` wie im Bestand).

## AK der Phase

- [x] `cargo run --manifest-path src-tauri/Cargo.toml --release --example parakeet-probe -- download <Ordner>` lädt den Modellsatz in einen leeren Ordner, prüft jede SHA-256 und lässt danach genau die Dateien der Tabelle (keine `.part`) darin liegen; ein zweiter Aufruf lädt nichts neu (Dateien mit stimmender Größe werden übersprungen) und meldet „bereit“.
- [x] `… --example parakeet-probe -- recognize <Ordner> <WAV> [--reference "<Satz>"]` erkennt über den **App-`Transcriber`** (nicht über `parakeet_rs` direkt) und druckt Text, Erkennungszeit und Wortfehlerrate; die Zeit für Satz (1) aus Phase 1 liegt unter 5000 ms (Release), Text entspricht dem Phase-1-Ergebnis.
- [x] Ein Abbruch während des Downloads (`cancel`-Flag gesetzt) hinterlässt keine `.part`-Datei und meldet einen Abbruch, kein Erfolg. Beleg: Probe-Unterbefehl `download <Ordner> --cancel-after <Sekunden>` setzt das Flag nach der Zeit; danach `Get-ChildItem <Ordner>` zeigt keine `.part`.
- [x] `grep -rn "whisper" src-tauri/src src-tauri/Cargo.toml src-tauri/examples` liefert nichts (Kommentare eingeschlossen).
- [x] `is_model_ready(dir)` ist `false`, solange eine Datei fehlt oder eine falsche Größe hat; `true` nur mit allen. Beleg: Probe-Unterbefehl `ready <Ordner>` auf dem vollen und auf einem Ordner, aus dem eine Datei entfernt wurde.
- [x] Nach vollständigem Download löscht der Core `<Modell-Ordner>/ggml-small-q5_1.bin`, falls vorhanden; keine andere Datei wird gelöscht. Beleg: im Probe-Unterbefehl `download` mit Ordner, der eine Attrappe `ggml-small-q5_1.bin` enthält — danach weg, eine daneben liegende `anderes.bin` noch da. (Die Löschfunktion nimmt dafür den **Eltern-Ordner der Modelle** als Parameter: `remove_legacy_model(models_dir: &Path)`.)
- [x] `pnpm check` grün (der Bau braucht weder `whisper-rs-sys` noch CMake für diesen Crate; ob CMake/LLVM für andere Crates fällt, prüft Phase 4).

## Checkliste

Implementation
- [x] `src-tauri/Cargo.toml`: `whisper-rs` entfernen. `parakeet-rs`, `hound` (dev) bleiben aus Phase 1.
- [x] `src-tauri/src/voice/transcribe.rs` neu nach Kontrakt der README: `pub struct Transcriber { model: Mutex<ParakeetTDT> }`. `load(dir)`: `ParakeetTDT::from_pretrained(dir, <Config laut FINDINGS>)`, Fehler als `CommandError::Internal(format!("Sprachmodell: {error}"))`. `transcribe(audio, cancel)`: (1) `cancel` gesetzt → `Err(VoiceCancelled)`; (2) `model.lock()` mit `unwrap_or_else(PoisonError::into_inner)`; (3) `transcribe_samples(audio.to_vec(), 16_000, 1, <Zeitstempel-Modus laut FINDINGS, kleinster verfügbarer>)`, Fehler als `Internal(format!("Erkennung: {error}"))`; (4) `cancel` gesetzt → `Err(VoiceCancelled)`; (5) Text: Whitespace zu einzelnen Leerzeichen zusammenziehen (wie bisher `split_whitespace().join(" ")`); leer → `Err(NoSpeech)`. Modul-Kommentar: „Erkennung eines Abschnitts mit Parakeet (ONNX).“ Das `Mutex`-Warum als Kommentar: „`transcribe_samples` braucht `&mut self`; ein auslaufender abgebrochener Durchlauf hält die Sperre bis zu seinem Ende.“
- [x] `src-tauri/src/voice/model_file.rs`: `MODEL_FILE_NAME`, `MODEL_URL`, `MODEL_BYTES`, `MODEL_SHA256`, `model_path` ersetzen durch `struct ModelFile { name: &'static str, url: &'static str, bytes: u64, sha256: &'static str }`, `const MODEL_FILES: [ModelFile; 3]` (Werte **aus der README-Tabelle** kopieren, nichts anderes), `pub const MODEL_DIR_NAME: &str = "parakeet-tdt-0.6b-v3"`, `pub fn total_bytes() -> u64` (Summe), `pub fn model_dir(app) -> Result<PathBuf, CommandError>` (`data_dir(app)?.join(MODELS_DIR).join(MODEL_DIR_NAME)`, legt nichts an), `pub fn is_model_ready(dir)`, `pub fn download(dir, cancel, on_progress)` (je Datei, in Tabellenreihenfolge: Größe stimmt → überspringen und deren Bytes in den Fortschritt rechnen; sonst nach `<dir>/<name>.part` laden, SHA-256 prüfen, umbenennen; `on_progress(bisher_gesamt, total_bytes())`; bei Fehler/Abbruch die aktuelle `.part` löschen und früher fertige Dateien liegen lassen). Der Lade-Kern der alten Funktion `download_to_part` wird zu `download_file(file, part_path, cancel, offset, on_progress)` — gleiche Logik, gleiche Kommentare (call()-Fehlerverhalten, Reader ohne Größenlimit). `pub fn remove_legacy_model(models_dir: &Path)` löscht `models_dir/ggml-small-q5_1.bin` (Konstante `LEGACY_MODEL_FILE_NAME`), Fehler ignorieren mit Kommentar „Ein Fehler beim Aufräumen ändert nichts mehr am Ergebnis.“. Aufruf am Ende von `download`: `if let Some(models_dir) = dir.parent() { remove_legacy_model(models_dir); }`.
- [x] `src-tauri/src/voice/mod.rs`: Importe anpassen (`MODEL_BYTES` raus, `total_bytes`, `model_dir` rein; `use dictation::base_prompt` raus); `Transcriber::load(&model_dir(app)?)`; `model_state`, `start_download` (`total_bytes: total_bytes()`), `run_download` (`downloading_state(0, total_bytes())`, `download(&dir, …)`) auf `dir` statt `path`; `start(&self, app: AppHandle)` ohne `repository_names`, `language`, ohne Prompt; `dictation::run(app, segment_receiver, channels, rate, cancel)`. Kommentar in `start` („das Modell lässt sich mitten im Rechnen nicht sofort anhalten“) bleibt sinngemäß wahr, Wortlaut „Modell“ beibehalten.
- [x] `src-tauri/src/voice/dictation.rs`: `BASE_PROMPT`, `PROMPT_TAIL_CHARS`, `base_prompt`, `prompt_tail` löschen; `run` verliert `base_prompt` und `language`; Aufruf `transcriber.transcribe(&mono, &cancel)`; den Satz im Kommentar über dem Zusammenfassen („ein kurzer Abschnitt kostet Whisper fast so viel wie ein langer“) ersetzen durch „spart Aufrufe, wenn die Erkennung hinter dem Sprechen herhinkt“.
- [x] Kommentare ohne Whisper-Bezug: `resample.rs` Zeile 1 („… das die Erkennung erwartet: 16 kHz mono“), `segmenter.rs` Zeile 13 („Zwangsschnitt für Dauerredner, damit der Zwischentext nicht ausbleibt“), `silence.rs` Zeile 8 („… ab dem ein Stück an die Erkennung geht — darunter lohnt ein Durchlauf nicht“). Schwellenwerte unverändert.
- [x] `src-tauri/src/commands/voice.rs`: `voice_start(app, voice)` ohne `registry`, `database`, `session_id`; zugehörige Importe (`PathBuf`, `Arc`, `Database`, `SessionRegistry`, `settings`, `SessionRepository`) entfernen, soweit sonst ungenutzt. Der Kommentar „Je Diktat gelesen …“ entfällt mit.
- [x] `src/lib/voice.ts`: `startDictation(): Promise<void> { await invoke('voice_start'); }` (Parameter und Doc-Hinweis auf Session streichen). `src/features/voice/useDictation.ts` Zeile 178: `startDictation()`; wird `sessionId` im Hook danach nirgends mehr gebraucht, Parameter samt Aufrufer-Kette entfernen (`pnpm check` meldet es als ungenutzt; nur so weit gehen, wie der Compiler zwingt, und in FINDINGS notieren, was zurückbleibt).
- [x] `src-tauri/src/settings/mod.rs`: nur den Block `if let SettingsChange::VoiceLanguage { value: Some(code) } = &change && whisper_rs::get_lang_id(code).is_none() { … }` löschen (Phase 3 entfernt die Einstellung ganz). Bis dahin nimmt der Core jeden Sprachcode an und ignoriert ihn.
- [x] `src-tauri/examples/parakeet-probe.rs`: umbauen auf Unterbefehle `download <Ordner> [--cancel-after <s>]`, `ready <Ordner>`, `recognize <Ordner> <WAV> [--reference "<Satz>"]` — alle über `verwalter_lib::voice::model_file` und `…::transcribe::Transcriber` (Whisper-Teil und Direktaufruf von `parakeet_rs` entfallen, `word_error_rate` bleibt). `AGENTS.md`-Zeile der Probe entsprechend aktualisieren. Ob `model_file` aus der Bibliothek erreichbar ist, entscheidet `voice/mod.rs` (`pub mod model_file;` steht schon dort).
- [x] `src-tauri/Cargo.lock` ändert sich durch Cargo selbst; keine Handarbeit.

Doku
- [x] Nur Kommentare im Code (s. o.). Code-Map, Glossar, ADR, Build-Doku macht Phase 4 gesammelt — ausdrücklich nicht hier, damit der Commit klein bleibt.

Definition of Done
- [x] Alle AK oben erfüllt, jedes mit Zitat der Ausgabe im Report-Back.
- [x] Sichtbares Ergebnis: Ausgabe von `recognize` mit Text, Zeit, Wortfehlerrate für die Aufnahmen aus Phase 1.
- [x] Artefakt: Ausgaben der drei Probe-Unterbefehle samt `Get-ChildItem` des Ordners vor/nach dem Abbruch-Test; Ausgabe `pnpm check` (letzte Zeilen).
- [x] `pnpm bindings` ändert nichts (keine Typen über die Tauri-Grenze berührt); falls doch: Diff prüfen und mitcommitten.
- [x] Seven-Goals-Sanity je ein Satz (Stabilität: Mutex-Vergiftung abgefangen; Sicherheit: nur gepinnte URL, Prüfsumme vor Umbenennen, gelöscht wird genau eine benannte Datei).
- [ ] Ein Commit `feat(voice): Erkennung mit Parakeet statt Whisper`. Nach dem Commit: ein Satz im Report-Back, dass ein Diktat in der echten App (Mikrofon) **nachlaufend** vom User geprüft wird und nichts blockiert.

## Report-Back

**Stand:** umgesetzt, `pnpm check` grün, Commit wartet auf Freigabe. Ausgaben im Scratchpad der Session (`probe-abbruch.txt`, `probe-download.txt`, `check.log`).

| AK | Beleg |
|---|---|
| Download in leeren Ordner | `bereit nach 550610 ms (670488135 Bytes im Satz)`; Ordner danach genau `decoder_joint-model.int8.onnx` (18202004), `encoder-model.int8.onnx` (652183999), `vocab.txt` (102132), keine `.part` |
| Zweiter Aufruf lädt nichts | `vorher bereit: true` … `bereit nach 0 ms` |
| Abbruch | `--cancel-after 5`: `abgebrochen nach 5011 ms: Download: abgebrochen`, Exit 1, Ordner danach leer |
| `ready` | voll `bereit: ja`; ohne `vocab.txt` `bereit: nein`; zurückgelegt `bereit: ja` |
| Altmodell | Attrappe `ggml-small-q5_1.bin` im Eltern-Ordner nach dem Download weg, `anderes.bin` noch da; nach dem Abbruch beide noch da |
| `recognize` über App-`Transcriber` (Release, Lauf 2) | Satz 1 (8,30 s): Laden 3101 ms, Erkennen **536 ms**, WER 10,0 %; Satz 2 (5,51 s): 381 ms, 0,0 %; Satz 3 (1,95 s): 158 ms, 25,0 % — Texte wortgleich mit Phase 1 |
| Kein Whisper | Suche `whisper` (ohne Groß/Klein) in `src-tauri/src`, `src-tauri/examples`, `src-tauri/Cargo.toml`: keine Treffer |
| Prüfkette | `pnpm check` Exit 0; `pnpm bindings` ändert nichts |

**Entscheidungen:** Zeitstempel-Modus `None` (= `Tokens`, kleinster); der Text ist nach dem Zusammenziehen der Leerzeichen gleich dem aus `Sentences`. Fortschritt zählt über den ganzen Satz gegen die Tabellensumme statt gegen `content_length` der einzelnen Antwort. Die ONNX-Umgebung entsteht in `Transcriber::load` (FINDINGS: ohne sie lädt nichts). `VoiceButton` reicht `sessionId` nicht mehr weiter; die Eigenschaft im Interface und bei den Aufrufern bleibt (der Compiler zwingt nicht weiter, → FINDINGS Phase 3).

**Abweichungen:** keine vom Kontrakt.

**Unsicherste Stelle:** `src-tauri/src/voice/transcribe.rs` — die Sperre auf das Modell wird während der ganzen Erkennung gehalten; ein abgebrochenes Diktat, dessen Durchlauf noch rechnet, lässt das nächste Diktat bis zu dessen Ende warten (gewollt laut README, in der echten App nicht beobachtet).

**Seven Goals:** Stabilität — vergiftete Sperre wird mit `PoisonError::into_inner` weiterverwendet, kein `unwrap`; Sicherheit — nur gepinnte URLs, Prüfsumme vor dem Umbenennen, gelöscht wird genau eine benannte Datei; Simplicity — eine Tabelle statt vier Konstanten, die Lade-Schleife unverändert übernommen.

Ein Diktat in der echten App (Mikrofon) prüft der User nachlaufend; das blockiert nichts.
