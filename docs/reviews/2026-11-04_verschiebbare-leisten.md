# Review: Verschiebbare Leisten (Phase 1 + 2)

Scope: `aa7489e..54f91d0` gegen die Finale AK des archivierten Plans. `pnpm check` grün (auch nach dem Fix unten).

## Ergebnis

Keine CRITICAL-Befunde. AK 1–8 sind im Diff erfüllt: Füllfläche (`data-resize-fill`, `flex: 1 1 0`), Tastatur (±16, Home/End, Speichern bei `keyup`), iframe-Fang (`setPointerCapture` plus `:root.resizing iframe { pointer-events: none }`), Speichern bei Fehler (Breite bleibt, `console.error`).

## Befunde

- **WARNING (behoben)** `ResizeHandle.tsx`: Ziehen startete bei der gemerkten Breite, nicht bei der tatsächlich gezeigten. Wurde die Leiste per CSS unter ihre gemerkte Breite gedrückt (kleines Fenster), sprang sie beim ersten Zug. Jetzt misst `measurePanel` die echte Breite beim `pointerdown`.
- **INFO** `TerminalDock.css:5`: Das Terminal rechts hat `flex-shrink: 0`. Sidebar (min 200) + Dock + Hintergrund (min 320) können in einem sehr schmalen Fenster die Breite übersteigen. Bestand vor dieser Arbeit, durch AK 1 („keine Summe ragt heraus“) aber berührt.
- **INFO** `aria-valuenow` zeigt die gemerkte, nicht die gezeigte Breite, solange das Fenster die Leiste drückt.
- **INFO** Keine Rust-Tests für `PanelWidth` (Bereichsprüfung, Löschen bei `None`); im Modul `settings` gibt es bisher keine.

## Nicht geprüft

Verhalten in WebView2 (Zeiger-Fang über dem iframe, Umbruchstufen der Kopfzeile) — nur im Code, nicht in der laufenden App.
