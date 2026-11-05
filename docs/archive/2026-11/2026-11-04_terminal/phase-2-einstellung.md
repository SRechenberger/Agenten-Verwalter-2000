# Phase 2 — Einstellung „Terminal“ (mechanisch)

## Kontext

- [README](README.md): Entscheidungen, Kontrakt (Teil `TerminalPlacement`), finale AK 4.
- Muster 1:1: `ColorScheme` und `color_scheme` in `src-tauri/src/settings/model.rs` und `src-tauri/src/settings/mod.rs` (`KEY_COLOR_SCHEME`, `DEFAULT_SETTINGS`, `stored_or`, Zweig in `update`); Oberfläche `src/features/settings/ColorSchemeSegment.tsx` und die Zeile „Farbschema“ im Abschnitt „Darstellung“ in `src/features/settings/SettingsView.tsx`.
- `src-tauri/examples/gen-bindings.rs`, [ADR 012](../../decisions/012-einstellungen-farbschema-und-listen.md).

## AK der Phase

- Finale AK 4 erfüllt; ohne gespeicherten Wert lädt `bottom`, 260, 460; Höhe und Breite haben keine eigene Zeile in den Einstellungen (gezogen wird am Dock, Phase 3); `pnpm check` grün; `TerminalPlacement.ts` generiert, `Settings.ts`/`SettingsChange.ts` tragen das neue Feld bzw. den neuen Fall.

## Checkliste

- [x] `src-tauri/src/settings/model.rs`: `pub enum TerminalPlacement { Bottom, Right }` mit denselben Derives und `serde`-Attributen wie `ColorScheme`, Doc-Kommentar „Wo das Terminal-Dock sitzt (ADR 055): unter der Hauptfläche oder als Spalte rechts.“; Feld `pub terminal_placement: TerminalPlacement` am Ende von `Settings`; Fall `TerminalPlacement { value: TerminalPlacement }` am Ende von `SettingsChange`.
- [x] `src-tauri/src/settings/model.rs`: zusätzlich Felder `pub terminal_height: u32` und `pub terminal_width: u32` (Doc-Kommentar: „Gezogene Höhe bzw. Breite des Terminal-Docks in Pixeln.“) und Fälle `TerminalHeight { value: u32 }`, `TerminalWidth { value: u32 }` in `SettingsChange`.
- [x] `src-tauri/src/settings/mod.rs`: `const KEY_TERMINAL_PLACEMENT: &str = "terminal_placement";`, `KEY_TERMINAL_HEIGHT = "terminal_height"`, `KEY_TERMINAL_WIDTH = "terminal_width"`; in `DEFAULT_SETTINGS` `terminal_placement: TerminalPlacement::Bottom`, `terminal_height: 260`, `terminal_width: 460`; `terminal_placement` laden über `stored_or` wie `color_scheme`; Höhe/Breite laden wie `mark_unsourced` (`parse::<u32>().ok()`, sonst Standard); `update`-Zweige schreiben `value.to_string()`; Höhe/Breite unter 100 bzw. über 10000 → `CommandError::Internal("Terminalgröße außerhalb 100–10000 px".to_owned())`.
- [x] `src-tauri/examples/gen-bindings.rs`: `TerminalPlacement` exportieren; `pnpm bindings`.
- [x] `src/features/settings/TerminalPlacementSegment.tsx` als Kopie des Musters `ColorSchemeSegment.tsx` (gleiche CSS-Klassen, keine eigene CSS-Datei), Optionen `bottom` → „Unten“, `right` → „Rechts“.
- [x] `src/features/settings/SettingsView.tsx`: im Abschnitt „Darstellung“ nach „Farbschema“ eine `SettingRow` mit `label="Terminal"`, `info="Wo das Terminal neben dem Chat erscheint: unten unter der Ansicht oder als Spalte rechts. Ein- und ausblenden mit dem Knopf >_ oder Strg+Ö."`, darin `TerminalPlacementSegment` mit `save({ kind: 'terminalPlacement', value }).catch(() => undefined)`.
- [x] Commit `feat(settings): Platz und Größe des Terminals speichern`.

## Report-Back
