# Phase 1 — Messlauf: Parakeet gegen Whisper auf derselben Aufnahme

Rating **heikel** (Go/No-Go für den ganzen Plan; unbekannte Bau-Umgebung) · Wave 1 · Aufwand M

## Kontext (alles, was der Bearbeiter lesen muss)

- [README.md](README.md) dieses Plans: Entscheidungen, Kontrakt, Gate, Tabelle „Modellsatz“.
- `STATE.md` Zeile „Nächster Schritt“: Bau-Umgebung auf diesem Rechner (`cmd` mit vcvars64, `LIBCLANG_PATH`, `BINDGEN_EXTRA_CLANG_ARGS`, `%USERPROFILE%\.cargo\bin` im PATH) — gilt für jeden Cargo-Aufruf, solange `whisper-rs-sys` noch im Baum ist.
- `src-tauri/src/voice/transcribe.rs` (heutiger `Transcriber` mit Whisper, `transcribe(audio, prompt, language, cancel)`), `src-tauri/src/voice/dictation.rs` (`BASE_PROMPT`), `src-tauri/src/voice/resample.rs` (`to_mono_16k`).
- Muster für ein Probe-Programm: `src-tauri/examples/app-update-probe.rs` (Unterbefehle, Ausgabe auf stdout). Aufruf-Konvention steht in `AGENTS.md`, Tabelle „Befehle“.
- `docs/conventions/rust.md`, `docs/conventions/linting.md`.
- Vault Fehlerklassen (nur Abschnitt `## Fehlerklassen`): `knowledge/sprachen/rust.md`. Ertrag: eine Zeile im Report-Back („Fehlerklassen geprüft: …“).
- Crate-Fakten (belegt aus der README von `parakeet-rs` 0.4.3, Stand 2026-10-30): `ParakeetTDT::from_pretrained(<Ordner>, Some(config))`, `transcribe_samples(audio, 16000, 1, Some(TimestampMode::Sentences))`, Ergebnis hat `.text`; `ParakeetTDT` ist ein `&mut self`-Typ; kein Sprach-, kein Prompt-Parameter; Ordner enthält laut README `encoder-model.onnx`, `encoder-model.onnx.data`, `decoder_joint-model.onnx`, `vocab.txt` (das ist die **Vollgewichte**-Variante). Ob `from_pretrained` die int8-Dateien erkennt, ist **nicht belegt** — das klärt diese Phase.

## AK der Phase

- [x] `cargo run --manifest-path src-tauri/Cargo.toml --example parakeet-probe -- <WAV> <Parakeet-Ordner> <Whisper-Modell> --reference "<Satz>"` läuft durch und druckt je Engine: Ladezeit (ms), Erkennungszeit (ms), Audiolänge (s), Echtzeitfaktor, erkannten Text, Wortfehlerrate gegen den Referenzsatz (Prozent).
- [x] **Gate Go** nur wenn beides gilt: (a) Parakeet erkennt den ~10-s-Satz mit Fachwörtern in einem Durchlauf in höchstens 5000 ms, (b) seine Wortfehlerrate ist auf diesem Satz höchstens so hoch wie die von Whisper small. Sonst **No-Go**: Plan stoppt, 🔴 an den User mit den Messwerten, Cargo-Änderungen dieser Phase werden zurückgenommen.
- [x] Spitzen-Arbeitsspeicher beider Läufe ist gemessen (Stichprobe von `WorkingSet64` alle 200 ms) und im Report-Back genannt; über 2 GB bei Parakeet steht als 🟡 darin.
- [x] Die Tabelle „Modellsatz“ in der README ist gefüllt (lokaler Dateiname, Pfad im Hugging-Face-Repo, Bytes, SHA-256 — alle drei Dateien), das ladbare Ordner-Layout ist in FINDINGS als Eintrag „→ Phase 2“ beschrieben.
- [x] FINDINGS enthält für Phase 2: genaue Signatur von `from_pretrained` und `transcribe_samples` (aus dem Compiler, nicht aus der README), ob `ParakeetTDT` `Send` ist (Beleg: ein `fn assert_send<T: Send>() {}`-Aufruf im Probe-Programm kompiliert), woher ONNX Runtime kommt (heruntergeladen beim Bau / mitgebaut / dynamisch geladen — Beleg: `cargo tree -e features -p ort` und Blick in `src-tauri/target/debug/build/ort-*`), ob sich die Thread-Zahl einstellen lässt.

## Checkliste

Implementation
- [x] `git status` im Repo: Tree darf nur die fremden ungetrackten Pläne in `docs/planning/` und diesen Plan enthalten; sonst 🔴 (parallele Session).
- [x] `src-tauri/Cargo.toml`: `parakeet-rs = "0.4.3"` unter `[dependencies]` (Standard-Features), `hound = "3"` unter `[dev-dependencies]` (WAV lesen). `whisper-rs` bleibt in dieser Phase.
- [x] `src-tauri/examples/parakeet-probe.rs` neu. Aufruf: `<WAV> <Parakeet-Ordner> [<Whisper-Modell>] [--reference "<Text>"]`. Ablauf: WAV mit `hound` lesen (Ganzzahl durch 32768 teilen, Float unverändert), mit `verwalter_lib::voice::resample::to_mono_16k(&samples, channels, rate)` auf 16 kHz mono bringen. Parakeet: Ordner laden (Zeit messen), einmal erkennen (Zeit messen). Whisper (nur mit drittem Argument): `verwalter_lib::voice::transcribe::Transcriber::load(<Datei>)` und `transcribe(&mono, BASE_PROMPT, None, &cancel)` (Prompt = `verwalter_lib::voice::dictation::BASE_PROMPT`). Wortfehlerrate: kleine private Funktion `word_error_rate(reference, hypothesis) -> f64` (kleingeschrieben, Satzzeichen entfernt, Levenshtein über Wörter, geteilt durch Referenzwörter). Kopf des Programms mit Aufrufzeile als Kommentar, Stil wie `app-update-probe.rs`.
- [x] `AGENTS.md`, Tabelle „Befehle“: eine Zeile für `parakeet-probe`, Stil der Nachbarzeilen.
- [x] Testaufnahmen erzeugen (Skript im Scratchpad, nicht im Repo): PowerShell mit `Add-Type -AssemblyName System.Speech`, deutsche Stimme wählen (`GetInstalledVoices()` mit Kultur `de-DE`), `SetOutputToWaveFile`. Referenzsätze, wörtlich: (1) „Bitte lege einen neuen Branch im Repository Verwalter an, mach einen Commit und öffne einen Pull Request für den Worktree.“ (2) „Wie spät ist es, und wo habe ich die Datei mit den Einstellungen gespeichert?“ (3) kurz, ~3 s: „Setz das bitte um.“ Gibt es **keine** deutsche Stimme: 🔴 eine Zeile an den User, er nimmt Satz (1) mit dem Windows-Sprachrekorder auf und nennt den Pfad; Satz (1) ist dann die einzige Aufnahme.
- [x] Modell holen: Ordner `<Scratchpad>\parakeet-modell\` (neu, leer), je Datei `curl -L -o <Datei> https://huggingface.co/Discourse/parakeet-tdt-0.6b-v3-onnx/resolve/b4ee2464a71b0a705915856c351b1cc19340e6b6/<Pfad>` für `encoder-model.int8.onnx`, `decoder_joint-model.int8.onnx`, `vocab.txt`. `Get-FileHash -Algorithm SHA256` und Größe je Datei notieren. Whisper-Modell: liegt schon unter `%USERPROFILE%\.verwalter\models\ggml-small-q5_1.bin`; fehlt es, Pfad im Report-Back melden und die Whisper-Hälfte weglassen (Gate (b) dann nicht prüfbar → 🔴).
- [x] Layout-Versuche in dieser Reihenfolge, jeweils `from_pretrained`: (A) Ordner mit den drei Dateien unter ihren Originalnamen; (B) falls (A) nicht lädt: `encoder-model.int8.onnx` → `encoder-model.onnx`, `decoder_joint-model.int8.onnx` → `decoder_joint-model.onnx` umbenannt (Kopie im selben Ordner). Das erste, das lädt **und** Text liefert, ist das Layout; nennt keins Text, ist das ein No-Go-Grund (Fehlertext im Report-Back). Weitere Layouts (z. B. `.fp16.`-Dateien, Vollgewichte mit `.data`) nicht ausprobieren.
- [x] Messen: je Aufnahme ein Lauf Parakeet und ein Lauf Whisper, Lauf 1 verwerfen (Kaltstart des Dateisystems), Lauf 2 zählen; Release-Profil (`--release`), weil Debug-Läufe ONNX und whisper.cpp unfair bremsen.
- [x] Gate auswerten, README-Tabelle und FINDINGS füllen, Plan-Zeile „Phase 1“ auf `complete` bzw. bei No-Go den Stopp melden.

Doku
- [x] Tabelle „Modellsatz“ (README), FINDINGS-Eintrag, `AGENTS.md`-Zeile (s. o.). Keine Änderung an ADR/Code-Map in dieser Phase.

Definition of Done
- [x] Alle AK oben erfüllt, jedes mit Zahl oder Zitat im Report-Back.
- [x] Sichtbares Ergebnis: die Gegenüberstellung (je Satz: Zeit, Wortfehlerrate, beide Texte) als Tabelle im Report-Back; der User sieht, was Parakeet gegen Whisper leistet.
- [x] Artefakt: vollständige Ausgabe der Probe-Läufe (Befehlszeile und stdout), Ausgabe von `cargo tree -e features -p ort`.
- [x] `pnpm check` grün (die Probe wird mitgeprüft: Clippy `-D warnings` gilt auch für `examples/`).
- [x] Seven-Goals-Sanity je ein Satz: Token-Effizienz, Code-Qualität, Umsetzungsgenauigkeit, Stabilität, Wartbarkeit, Sicherheit (Downloads nur mit gepinnter Revision und geprüfter SHA-256), Simplicity.
- [x] Ein Commit `feat(voice): Messlauf Parakeet gegen Whisper` mit Probe, Cargo-Dateien, `AGENTS.md`, README/FINDINGS dieses Plans. Bei No-Go stattdessen kein Code-Commit, nur Plan-Dateien mit dem Ergebnis.

## Report-Back

**Ergebnis: Go.** Gate (a) Parakeet erkennt Satz 1 (8,3 s, Fachwörter) in 529 ms (Grenze 5000 ms). Gate (b) Wortfehlerrate Parakeet 10,0 % gegen Whisper small 25,0 %.

Aufnahmen: Windows-Sprachausgabe, Stimme „Microsoft Hedda Desktop“ (de-DE), 22,05 kHz mono. Release-Profil, je Satz und Engine zwei Läufe in eigenem Prozess, gezählt ist Lauf 2. Threads: beide `min(Kerne, 8)`.

| Satz | Engine | Laden | Erkennen | Echtzeitfaktor | WER | Speicher-Spitze | Text |
|---|---|---|---|---|---|---|---|
| 1 (8,30 s) | Parakeet | 3646 ms | 529 ms | 0,064 | 10,0 % | 757 MB | Bitte lege einen neuen Branch im Reparatory Verwalter an, mach einen Commit und öffne einen Pull Request für den Worktee. |
| 1 | Whisper | 248 ms | 11481 ms | 1,384 | 25,0 % | 518 MB | Bitte lege einen neuen Branch im Repository-Verwalter an, mach einen Commit und öffne einen Pull-Request für den Workty. |
| 2 (5,51 s) | Parakeet | 4028 ms | 453 ms | 0,082 | 0,0 % | 740 MB | Wie spät ist es? Und wo habe ich die Datei mit den Einstellungen gespeichert? |
| 2 | Whisper | 286 ms | 12216 ms | 2,216 | 0,0 % | 518 MB | Wie spät ist es, und wo habe ich die Datei mit den Einstellungen gespeichert? |
| 3 (1,95 s) | Parakeet | 3510 ms | 171 ms | 0,088 | 25,0 % | 685 MB | Setzt das bitte um! |
| 3 | Whisper | 239 ms | 10744 ms | 5,513 | 0,0 % | 517 MB | Setz das bitte um. |

- 🟡 Whisper braucht auf diesem Rechner im Messlauf 10,7–12,2 s je Abschnitt, auch für 2 s Audio; das verfehlt die 5-s-Grenze aus ADR 009 schon heute. Ursache nicht untersucht (kein Ziel dieser Phase).
- 🟡 Parakeet verwechselt „Repository“ und „Worktree“ (ohne Wortschatz-Hinweis) und setzt Satzzeichen selbst; Whisper mit Hinweis trifft „Repository“ und verfehlt „Worktree“ ebenfalls.
- 🟡 Ladezeit 3,5–4,0 s statt 0,2 s; Speicher unter 2 GB.
- Bau: erst nach Nachinstallieren von MSVC 14.44 (VS 2022 Build Tools) gelungen, Details in FINDINGS (→ Phase 4). Dafür hat der User der Installation zugestimmt.
- Layout (A) lädt, Signaturen, `Send`, Herkunft ONNX Runtime und Thread-Einstellung: FINDINGS (→ Phase 2/4).
- Fehlerklassen geprüft: Vault-Eintrag `knowledge/sprachen/rust.md` existiert nicht (Glob ohne Treffer); stattdessen selbst geprüft: die Probe hat keine `unwrap`, keine Indizes außerhalb der Levenshtein-Tabelle (Grenzen durch Konstruktion), Clippy `-D warnings` grün.
- Artefakte (außerhalb des Repos): Messlauf `messlauf.txt`, `cargo-tree-ort.txt`, Skripte `messen.ps1`, `env-run.cmd` im Scratchpad der Session.
- Seven Goals: Token — Probe ist ein Beispielprogramm, kein App-Code · Qualität — Clippy/fmt grün · Genauigkeit — Gate mit Zahlen belegt · Stabilität — App unverändert, Whisper bleibt · Wartbarkeit — Probe bleibt als Messwerkzeug, AGENTS-Zeile · Sicherheit — Download nur mit gepinnter Revision, SHA-256 notiert · Simplicity — keine Zusatz-Features von `parakeet-rs`.
