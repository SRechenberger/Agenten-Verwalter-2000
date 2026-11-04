# Terminal im Vorhaben

Ziel: Jedes Vorhaben bekommt echte Terminals (Pseudo-Terminal im Core, xterm.js in der Oberfläche) im Workspace-Ordner des Vorhabens — genau dem Ordner, in dem der Agent arbeitet. Sie erscheinen **parallel zum Chat** in einem einblendbaren Terminal-Dock: wahlweise **unten** unter der Hauptfläche (Variante A, Standard) oder **rechts** als eigene Spalte (Variante B); umgeschaltet wird in den Einstellungen, der Wert liegt in der Datenbank und übersteht einen Neustart. Die Shell richtet sich nach dem Betriebssystem: Windows `cmd.exe`, macOS die Login-Shell des Users (Standard `zsh`), Linux die Login-Shell des Users (Standard `bash`). Entwurf: [artifacts/entwurf-terminal-anordnung.html](artifacts/entwurf-terminal-anordnung.html) (Varianten A und B gewählt am 2026-11-04, C und D verworfen).

Kontext für jeden Umsetzer: [AGENTS.md](../../../AGENTS.md), [docs/code-map.md](../../code-map.md) (Zeilen „Vorhaben“, „Sessions“, „Einstellungen“, „Hintergrund“, „Generierte Typen“, „Aufruf-Wrapper“, „Fehler am Command-Rand“, „Design-Tokens“), [docs/glossary.md](../../glossary.md), [docs/conventions/](../../conventions/) (rust, typescript, react, tailwind, linting, commits, releases), [ADR 012](../../decisions/012-einstellungen-farbschema-und-listen.md) (Einstellungen, Fehlersätze). Fehlerklassen geprüft (Vault: TypeScript, React): keine einschlägig; für Rust, Tauri und xterm.js gibt es keine Einträge.

| # | Phase | Rating | Status |
|---|---|---|---|
| 1 | [Core: Pseudo-Terminal je Terminal, Shell je Betriebssystem, Puffer, Commands, Lebensdauer; ADR 055](phase-1-core.md) | heikel | complete |
| 2 | [Einstellungen „Terminal“ (unten/rechts) und gemerkte Größe in Core und Einstellungsseite](phase-2-einstellung.md) | mechanisch | pending |
| 3 | [Oberfläche: Terminal-Dock unten/rechts, xterm.js, Knopf und Strg+Ö; Doku-Abschluss](phase-3-dock.md) | standard | pending |

**Rahmen:** strikt 1 → 2 → 3, direkt auf `main`, ein Commit pro Phase, Scope `terminal` (Phase 2: `settings`). `pnpm check` grün vor jedem Commit, `%USERPROFILE%\.cargo\bin` im PATH, `pnpm bindings` nach Phase 1 und 2, Bindings mitcommitten. Neue Abhängigkeiten genau diese: `portable-pty = "0.9.0"` (Core), `@xterm/xterm@6.0.0` und `@xterm/addon-fit@0.11.0` (Oberfläche, exakt ohne `^`). Keine Migration (Tabelle `settings` ist Schlüssel/Text). Keine automatisierten Tests, keine Smoke-Checkliste. Planabschluss mit Versions-Tag (Minor-Version). ADR-Nummer 055 (034 und 038 sind von geparkten Plänen reserviert, 054 ist die höchste auf Platte).

## Festgelegte Entscheidungen

- **Eingebettet, nicht extern:** Pseudo-Terminal im Core über `portable-pty` (Windows: ConPTY, macOS/Linux: Unix-PTY), Anzeige mit xterm.js.
- **Shell je Betriebssystem:** Windows `ComSpec`, sonst `cmd.exe`, ohne Argumente, Label `cmd` · macOS `SHELL`, sonst `/bin/zsh`, Argument `-l` · Linux `SHELL`, sonst `/bin/bash`, Argument `-l` · überall `TERM=xterm-256color`, `COLORTERM=truecolor`, Arbeitsordner = Workspace des Vorhabens.
- **Gehört dem Vorhaben:** alle Sessions eines Vorhabens teilen den Workspace und damit die Terminals; das Dock zeigt die Terminals des gerade sichtbaren Vorhabens (Session-Ansicht oder Übersicht) und wechselt mit ihm.
- **Mehrere Terminals je Vorhaben, höchstens 8 laufende.** Nummer je Vorhaben ab 1, nie wiederverwendet bis App-Ende. Titel `<Label> <Nummer>`.
- **Kein Reiter.** Das Dock blendet sich über einen Knopf `>_` in der Kopfzeile (Session und Übersicht, mit Zahl der laufenden Terminals) und über `Strg+Ö` ein und aus. Öffnet es sich in einem Vorhaben ohne Terminal, startet automatisch eines.
- **Variante A „Unten“ (Standard):** Dock unter der Hauptfläche (`app__main`), über der vollen Breite der Hauptfläche, links von einem offenen Hintergrund-Panel. Höhe per Ziehkante (120 px bis Höhe der Hauptfläche − 160 px, Start 260 px), Knopf „Maximieren“ füllt die Hauptfläche (Inhalt darüber bleibt eingehängt, nur ausgeblendet).
- **Variante B „Rechts“:** Dock als eigene Spalte zwischen Hauptfläche und Hintergrund-Panel. Breite per Ziehkante (320 px bis 60 % der Fensterbreite, Start 460 px). Dock und Hintergrund-Panel sind in B nie zugleich offen: Öffnen des einen schließt das andere. Kein Maximieren.
- **Einstellung `terminal_placement`** (`bottom` | `right`, Standard `bottom`) in der Tabelle `settings`, Zeile „Terminal“ im Abschnitt „Darstellung“. Wechsel wirkt sofort; ein offenes Dock springt an den neuen Platz, Terminals laufen weiter.
- **Größe wird gemerkt:** Höhe (unten) und Breite (rechts) liegen als `terminal_height` (Standard 260) und `terminal_width` (Standard 460) in der Tabelle `settings`, gespeichert beim Loslassen der Ziehkante, und überstehen einen Neustart. Liegt ein gespeicherter Wert außerhalb der aktuellen Grenzen (kleineres Fenster), wird er beim Anzeigen auf die Grenze geklemmt, nicht überschrieben.
- **Flüchtig bleiben:** offen/zu und maximiert (`src/stores/terminal.ts`); beim Start ist das Dock zu.
- **Neu aufgebaut wird nur nach `Strg+R`** (vom User festgelegt): Jede xterm-Instanz lebt in einer Ablage außerhalb von React (`src/features/terminal/terminalInstances.ts`) von ihrem ersten Anzeigen bis zum Ende ihres Terminals. Ausblenden, Wechsel zwischen Terminals, Vorhabenwechsel und Platzwechsel hängen nur ihr DOM-Element aus und wieder ein — Inhalt, Scrollstand und ein laufendes `htop`/`vim` (auch über `ssh`) bleiben unverändert. Nur nach einem Neuladen der Oberfläche spielt eine frische Instanz den Puffer aus dem Core (1 MiB) neu ab; dann können Vollbild-Programme verzerrt aussehen, bis sie neu zeichnen.
- **Speicher der Oberfläche:** wächst bewusst mit der Zahl offener Terminals (je Instanz bis 5000 Zeilen Verlauf), nicht nur mit dem Sichtbaren — Ausnahme von Critical Rule 3, im ADR 055 festgehalten; begrenzt durch höchstens 8 laufende je Vorhaben und das Entfernen mit „×“.
- **Nur im Speicher:** Terminals (und damit `ssh`-Verbindungen) enden mit „×“, Archivieren des Vorhabens oder App-Ende.
- **Zwischenablage:** `Strg+C` kopiert bei Markierung, sonst `^C`; `Strg+V` fügt ein (macOS `Cmd`). Einfügen über das native `paste`-Ereignis, das xterm.js selbst verarbeitet.

## Kontrakt

```rust
// src-tauri/src/terminal/model.rs — derive(Debug, Clone, Serialize, TS), #[serde(rename_all = "camelCase")]
pub struct TerminalInfo {
    pub id: String,                 // uuid v4
    pub project_id: String,
    pub number: u32,                // je Vorhaben ab 1
    pub label: String,              // "cmd", "zsh", "bash", …
    pub cwd: String,                // Workspace des Vorhabens
    #[ts(type = "number")] pub started_at: f64, // ms seit Epoche
    pub running: bool,
    pub exit_code: Option<u32>,
}

#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TerminalOutput { Data { text: String }, Exit { code: Option<u32> } }

pub struct TerminalChanged { pub project_id: String } // Nutzlast von "terminal://changed"

// src-tauri/src/settings/model.rs — derive wie ColorScheme
pub enum TerminalPlacement { Bottom, Right }        // serde: "bottom" | "right"
// Settings bekommt: pub terminal_placement: TerminalPlacement, pub terminal_height: u32, pub terminal_width: u32
// SettingsChange bekommt: TerminalPlacement { value: TerminalPlacement }, TerminalHeight { value: u32 }, TerminalWidth { value: u32 }
```

| Command (`src-tauri/src/commands/terminal.rs`) | Argumente | Ergebnis | Wirkung |
|---|---|---|---|
| `terminal_list` | `project_id: String` | `Vec<TerminalInfo>` nach `number` | — |
| `terminal_open` | `project_id: String, cols: u16, rows: u16` | `TerminalInfo` | startet Shell im Workspace; ab 8 laufenden `CommandError::Terminal("Höchstens 8 Terminals je Vorhaben laufen gleichzeitig.")`; Ereignis `terminal://changed` |
| `terminal_attach` | `id: String, on_output: Channel<TerminalOutput>` | `String` (Puffer) | hängt den Kanal an und liefert den Puffer unter derselben Sperre; ist die Shell beendet, folgt sofort `Exit` |
| `terminal_detach` | `id: String` | `()` | löst den Kanal; unbekannte ID still |
| `terminal_write` | `id: String, data: String` | `()` | Eingabe ins Pseudo-Terminal |
| `terminal_resize` | `id: String, cols: u16, rows: u16` | `()` | nur wenn `running` |
| `terminal_close` | `id: String` | `()` | beendet den Prozess, entfernt das Terminal; Ereignis `terminal://changed` |

Wrapper `src/lib/terminal.ts`: `listTerminals`, `openTerminal`, `attachTerminal`, `detachTerminal`, `writeTerminal`, `resizeTerminal`, `closeTerminal`, `onTerminalChanged`. Die Einstellung läuft über die bestehenden `settings_load`/`settings_update`.

## Finale AK

1. In Session-Ansicht und Übersicht steht in der Kopfzeile der Knopf `>_` mit Tooltip „Terminal ein-/ausblenden (Strg+Ö)“ und, wenn Terminals laufen, ihrer Zahl; Knopf und `Strg+Ö` blenden das Dock ein und aus.
2. Erstes Öffnen in einem Vorhaben ohne Terminal startet `cmd 1` (Windows) im Workspace; `cd` zeigt `…\.verwalter\workspaces\<id>` — den Arbeitsordner des Agenten.
3. macOS/Linux starten die Shell aus `SHELL` (sonst `/bin/zsh` bzw. `/bin/bash`) mit `-l` — belegt per Code-Blick auf die `cfg`-Zweige; Laufzeit dort ungeprüft.
4. Einstellungen → Darstellung → „Terminal“: Segment „Unten | Rechts“ mit Erklärung; Standard „Unten“; die Wahl übersteht einen Neustart der App.
5. „Unten“: Dock unter Chat/Changes/Artefakte/Übersicht, Höhe ab der ersten Version ziehbar, die gezogene Höhe übersteht einen Neustart; „Maximieren“ füllt die Hauptfläche und stellt per zweitem Klick zurück; ein offenes Hintergrund-Panel bleibt rechts daneben offen.
6. „Rechts“: Dock als Spalte rechts, Breite ab der ersten Version ziehbar, die gezogene Breite übersteht einen Neustart; Öffnen des Hintergrund-Panels schließt das Dock und umgekehrt.
7. „+“ öffnet ein weiteres Terminal; ein 9. laufendes wird mit dem Satz aus dem Kontrakt abgelehnt, sichtbar im Dock.
8. Ausblenden und wieder Einblenden, Wechsel zwischen Terminals, Vorhabenwechsel und Platzwechsel in den Einstellungen lassen ein laufendes `htop`/`vim` samt Scrollstand unverändert stehen; nach `Strg+R` sind Terminals und Ausgabe noch da (neu abgespielt).
9. Farben, Umlaute (`echo äöü`), Größenänderung (Spalten folgen, `mode con`), `Strg+C` bei laufendem `ping -t localhost`, Kopieren und Einfügen funktionieren.
10. `exit` lässt das Terminal mit „beendet“ stehen; „×“ entfernt es und beendet einen laufenden Prozess; Archivieren und App-Ende lassen keinen Shell-Prozess zurück.
11. Hell- und Dunkelmodus: das Terminal übernimmt Hintergrund, Schrift und Cursor aus den Tokens und wechselt mit dem Farbschema.
12. ADR 055, Glossar, Code-Map nachgezogen; `pnpm check` grün.

## Wackelstellen (Konfidenz)

- 🟡 **Enkelprozesse unter Windows:** ob ein in `cmd` gestartetes `pnpm dev` mit „×“/App-Ende mitstirbt, hängt am Schließen der ConPTY. Check: `ping -t localhost`, „×“, `Get-Process PING` — leer = gut; sonst Job-Objekt nachrüsten (FINDINGS).
- 🟡 **Letzte Ausgabezeilen unter Windows:** der Warte-Thread schließt die ConPTY nach Prozessende. Check: `cmd /c "echo eins & echo zwei & exit"` — beide Zeilen müssen stehen.
- 🟡 **Einfügen per `paste` in WebView2:** Check: `Strg+V` fügt genau einmal ein; doppelt oder gar nicht → Tasten-Handler anpassen bzw. `navigator.clipboard.readText` als Rückfall.

## Summary

## Files touched

## Commits

## Deviations from plan

## Follow-ups
