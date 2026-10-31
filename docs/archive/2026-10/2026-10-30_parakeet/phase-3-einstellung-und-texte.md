# Phase 3 — Spracheinstellung und Whisper-Texte entfernen

Rating **standard** · Wave 3 · Aufwand S · Status **complete**

## Kontext

- [README.md](README.md): Entscheidungen („Wegfall ohne Ersatz“: keine Sprachwahl); Voraussetzung: Phase 2 ist auf `main`.
- Zu ändern, vorher lesen: `src-tauri/src/settings/model.rs` (Feld `voice_language` ~Zeile 42, Variante `SettingsChange::VoiceLanguage` ~Zeile 91), `src-tauri/src/settings/mod.rs` (`KEY_VOICE_LANGUAGE`, `DEFAULT_SETTINGS.voice_language`, Block in `load`, Arm in `update`), `src/features/settings/SettingsView.tsx` (Sektion „Spracheingabe“, Zeilen 332–345, und der Import in Zeile 27), `src/features/settings/VoiceLanguageSegment.tsx`, `src/features/voice/VoiceSetup.tsx` (Zeile 12 `MODEL_SIZE_LABEL`, Zeile 92 Text), `src/features/voice/VoiceButton.tsx` (Zeile 161, Text bleibt: „Sprachmodell“ ist kein Whisper-Wort).
- `docs/conventions/typescript.md`, `react.md`, `rust.md`, ADR 012 (Einstellungen: wie Schlüssel gespeichert werden — `setting_rows`, kein Migrationszwang für verwaiste Schlüssel).
- Fehlerklassen geprüft (Vault: Rust, TypeScript, React): keine einschlägig über die üblichen Punkte hinaus; ungenutzter Import nach dem Löschen fängt `pnpm check`.
- Chesterton's Fence: `voice_language` wurde je Diktat gelesen, um Whisper die Sprache vorzugeben; mit Parakeet gibt es keinen Parameter dafür — Feld, Variante, Schlüssel und Oberfläche sind ohne Wirkung und fallen ganz. Der Datenbank-Eintrag alter Installationen bleibt liegen (harmlos, keine Migration, siehe README „Rahmen“).

## AK der Phase

- [x] `grep -rn "voice_language\|voiceLanguage\|VoiceLanguage" src src-tauri/src` liefert nichts.
- [x] Einstellungen zeigt keine Sektion „Spracheingabe“ mehr; die übrigen Sektionen stehen unverändert (Beleg per Quelltext: Block samt Import aus `SettingsView.tsx` gelöscht, Nachbarsektionen unberührt; kein Screenshot).
- [x] `pnpm bindings` erzeugt `Settings.ts` ohne `voiceLanguage` und `SettingsChange.ts` ohne Variante `voiceLanguage`; die zwei Dateien liegen in Commit `ccae7b6`.
- [x] `MODEL_SIZE_LABEL` = `'670 MB'`: 652183999 + 18202004 + 102132 = 670488135 Bytes, gerundet 670 MB.
- [x] `pnpm check` grün.

## Checkliste

Implementation
- [x] Rust: `voice_language` aus `Settings`, `SettingsChange::VoiceLanguage`, `KEY_VOICE_LANGUAGE`, Default-Feld, Lese-Block und Arm entfernt.
- [x] `pnpm bindings` ausgeführt, die zwei Binding-Dateien mitcommittet.
- [x] TS: Sektion „Spracheingabe“ samt Import gelöscht; `VoiceLanguageSegment.tsx` gelöscht (keine eigene CSS).
- [x] `VoiceSetup.tsx`: `MODEL_SIZE_LABEL` auf `'670 MB'`.
- [x] Finding aus Phase 2: ungenutzte Eigenschaft `sessionId` an `VoiceButton` samt den zwei Aufrufern entfernt.

Doku
- [x] Keine; Doku-Nachzug sammelt Phase 4.

Definition of Done
- [x] Alle AK erfüllt.
- [x] Sichtbares Ergebnis: Einstellungsseite ohne Spracheingabe; Mikrofon-Menü „Diktieren einrichten“ mit 670 MB.
- [x] Artefakt: `grep`-Prüfung leer, `pnpm check` Exit 0.
- [x] Statt eines Commits vier kleine (auf Wunsch des Users), alle auf `main`.

## Report-Back

- Commits: `589178b` Sprachwahl aus der Einstellungsseite, `ccae7b6` Einstellung aus Core und Bindings, `1c47aea` ungenutzte `sessionId`, `f72773e` Größenangabe 670 MB.
- Abweichung: vier statt einem Commit; parallele Session (Rückfrage-Kasten) arbeitete im selben Tree, deshalb Commits nur mit expliziten Pfaden. `pnpm check` lief erst grün, als deren Stand lint-sauber war; der erste Lauf war wegen deren Dateien rot.
- Nicht geprüft: Sichtprüfung in der laufenden App (kein Screenshot, kein `pnpm tauri dev`).
- Unsicherste Stelle: `src-tauri/src/settings/mod.rs` — alte Datenbankzeile `voice_language` bleibt liegen und wird nirgends gelesen; gewollt, keine Migration.
