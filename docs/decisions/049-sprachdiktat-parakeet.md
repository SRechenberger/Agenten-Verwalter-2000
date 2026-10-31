# 049 — Sprachdiktat mit Parakeet statt Whisper

**Status:** angenommen · **Datum:** 2026-10-30

## Kontext

Das Diktat erkannte bisher mit Whisper small ([ADR 009](009-sprachdiktat-lokal.md)). Der Nutzer wollte eine bessere Erkennung. Gemessen am 2026-10-30 auf derselben Aufnahme (Windows-Sprachausgabe, Release-Profil, beide Engines mit `min(Kerne, 8)` Threads, zweiter Lauf): Whisper brauchte 10,7–12,2 s je Abschnitt und verfehlte damit die 5-s-Grenze aus ADR 009 schon vorher; Parakeet TDT 0.6B v3 erkannte denselben 8,3-s-Satz mit Fachwörtern in 0,5 s, bei 10 % Wortfehlern gegen 25 % bei Whisper.

## Optionen

- **(a) Whisper behalten** — verworfen: zu langsam auf dem Entwicklungsrechner, Wortfehlerrate höher.
- **(b) Parakeet ersetzt Whisper** — gewählt.
- **(c) Beide wählbar** — verworfen: doppelter Download, doppelter Code, und der Nutzer will keine Wahl (Entscheidung „ersetzen“).
- **(d) GPU-Weg (DirectML/CUDA)** — verworfen: die CPU reicht mit Abstand; falls nicht, ein eigener Plan.

## Entscheidung

- **Engine:** Crate `parakeet-rs` mit ONNX Runtime (`ort`), Modell `parakeet-tdt-0.6b-v3` als int8, nur CPU. Gewichte aus `Discourse/parakeet-tdt-0.6b-v3-onnx`, festgepinnte Revision, drei Dateien mit Größe und SHA-256 in `voice/model_file.rs`.
- **Ablage:** `%USERPROFILE%\.verwalter\models\parakeet-tdt-0.6b-v3\`; nach erfolgreichem Download löscht der Core die alte Datei `ggml-small-q5_1.bin`.
- **Ein Modell im Speicher, hinter einem `Mutex`:** `transcribe_samples` braucht `&mut self`. Ein abgebrochener, noch rechnender Durchlauf hält ein neues Diktat auf, bis er fertig ist. `cancel` wird vor und nach dem Durchlauf geprüft; mitten im Rechnen lässt sich nicht abbrechen.
- **Aufnahme, Pausen-Schnitt, Stille-Prüfung, Ein-Aufnahme-Regel und Zwischentext-Ereignisse** bleiben wie in ADR 009.

## Konsequenzen

- **Kein Wortschatz-Hinweis mehr:** „Repository“ und „Worktree“ erkennt Parakeet nicht sicher (Messlauf: „Reparatory“, „Worktee“). Whisper traf sie mit Hinweis auch nicht durchgängig.
- **Keine Sprachwahl:** Parakeet erkennt die Sprache selbst. Die Einstellung `voice_language` bleibt als toter Eintrag in der Datenbank.
- **Satzzeichen und Großschreibung** setzt das Modell selbst; die Liste der Whisper-Halluzinationen entfällt.
- **670 MB statt 190 MB** beim Einrichten (Download rund 9 Minuten gemessen), Laden 3,5–4,0 s statt 0,2 s, Arbeitsspeicher-Spitze bis 757 MB statt 518 MB. Das erste Diktat nach dem Start wartet die Ladezeit ab.
- **ONNX Runtime als Abhängigkeit:** `ort-sys` lädt beim Bau eine fertige statische Bibliothek von `cdn.pyke.io` (Windows x64 nur mit DirectML) — der Bau braucht Netz dorthin. Die Exe importiert `directml.dll`, `d3d12.dll` und `dxgi.dll` aus dem Windows-Bestand. Gelinkt wird nur mit einem aktuellen v143-Werkzeugsatz (MSVC ab 14.4x); VS 2019 und 14.38 scheitern mit LNK2019.
- **Die Bau-Kette wird einfacher:** kein CMake, kein LLVM, keine Compiler-Flags in `.cargo/config.toml` mehr.
- Code: `src-tauri/src/voice/` (`transcribe.rs`, `model_file.rs`), `examples/parakeet-probe.rs`.
