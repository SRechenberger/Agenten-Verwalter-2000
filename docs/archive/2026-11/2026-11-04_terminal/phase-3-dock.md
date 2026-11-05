# Phase 3 — Oberfläche: Terminal-Dock (standard)

## Kontext

- [README](README.md): Entscheidungen, Kontrakt, finale AK.
- Entwurf: [artifacts/entwurf-terminal-anordnung.html](artifacts/entwurf-terminal-anordnung.html), Varianten A und B ([artifacts/README.md](artifacts/README.md)).
- `src/app/App.tsx` (`renderMain`, `<main className="app__main">`, `BackgroundPanel` als Geschwister, `isBackgroundOpen`/`toggleBackground`, `currentSession`, `isMainReplaced`, Tastenkürzel `Strg+;`), `src/app/App.css` (`.app` ist `display: flex`), `src/stores/background.ts` (`isOpen`, `toggle`), `src/app/SessionHeader.tsx` und `src/app/ProjectHeader.tsx` (Bereich `session-header__tools`, Knopf des Hintergrund-Panels als Muster samt `runningBackgroundCount`), `src/stores/settings.ts` (geladener Stand, `terminalPlacement` und `colorScheme`), `src/lib/artifacts.ts` und `src/lib/projects.ts` (Muster Wrapper und Abo), `src/lib/errors.ts` (`commandErrorText`), `src/styles/theme.css` (Tokens).
- Doku xterm.js 6 (`Terminal`, `ITheme`, `attachCustomKeyEventHandler`, `onData`, `hasSelection`, `getSelection`, `options.theme`) und `@xterm/addon-fit` (`FitAddon.fit`).
- [docs/conventions/react.md](../../conventions/react.md), [typescript.md](../../conventions/typescript.md), [tailwind.md](../../conventions/tailwind.md).
- Fehlerklassen (Vault TypeScript/React): keine einschlägig.

## AK der Phase

- Finale AK 1, 2, 5–11 erfüllt; `pnpm check` grün; jede Schaltfläche des Docks hat einen Tooltip (`title`).
- Entwurf A und B als Struktur: **A** Dock unter dem Inhalt in `app__main`, oben eine 5 px hohe Ziehkante, darunter Leiste (Terminal-Knöpfe mit „×“, „+“, rechts „Maximieren“ und „Ausblenden“), darunter xterm über die Resthöhe; **B** Dock als Spalte zwischen `app__main` und Hintergrund-Panel, links eine 5 px breite Ziehkante, gleiche Leiste ohne „Maximieren“.

## Checkliste

- [x] `pnpm add @xterm/xterm@6.0.0 @xterm/addon-fit@0.11.0 --save-exact`.
- [x] `src/lib/terminal.ts`: Wrapper aus dem Kontrakt; `attachTerminal(id, onOutput: (message: TerminalOutput) => void): Promise<string>` legt `new Channel<TerminalOutput>()` (`@tauri-apps/api/core`) an und setzt `onmessage`.
- [x] `src/stores/terminal.ts` (flüchtig): `isOpen: boolean` (false), `isMaximized: boolean` (false), `dragSize: number | null` (Größe während des Ziehens, sonst `null`), `selected: Record<string, string>` (Vorhaben-ID → Terminal-ID); Aktionen `toggle()`, `close()`, `setDragSize(px | null)`, `toggleMaximized()`, `select(projectId, terminalId)`. `close()` setzt auch `isMaximized = false`. Die gespeicherte Höhe/Breite kommt aus dem Settings-Store (`terminalHeight`, `terminalWidth`), nicht von hier.
- [x] `src/stores/background.ts`: Aktion `close()` ergänzen, falls nur `toggle` existiert.
- [x] `src/features/terminal/useProjectTerminals.ts`: lädt `listTerminals(projectId)` beim Mount und bei jedem `terminal://changed` mit passender `projectId`; liefert `{ terminals, error }`; `projectId === null` → leere Liste. Nach jedem Laden `prune(projectId, ids)`. Zusätzlich ein Abo auf `terminal://changed` für **andere** Vorhaben: lädt deren Liste einmal und ruft `prune` (räumt Instanzen auf, die z. B. beim Archivieren endeten).
- [x] `src/features/terminal/terminalTheme.ts`: `terminalTheme(): ITheme` aus `getComputedStyle(document.documentElement)`: `--color-bg-base` (background), `--color-fg-primary` (foreground, cursor), `--color-accent-subtle` (selectionBackground); `terminalFont(): string` aus `--font-mono`.
- [x] `src/features/terminal/terminalInstances.ts` (Ablage außerhalb von React, Modul-Variable `Map<string, TerminalInstance>`; importiert `@xterm/xterm/css/xterm.css`). `TerminalInstance = { projectId, term: Terminal, fit: FitAddon, element: HTMLDivElement, isOpened: boolean, lastCols: number, lastRows: number }`.
  - `acquire(terminalId, projectId): TerminalInstance`: vorhandene zurückgeben; sonst anlegen: `element = document.createElement('div')` (Klasse `terminal-pane__screen`), `new Terminal({ fontFamily: terminalFont(), fontSize: 13, cursorBlink: true, scrollback: 5000, theme: terminalTheme() })`, `FitAddon` laden; Eingabe, Tasten und Ausgabe (unten) genau einmal verdrahten; ablegen.
  - Ausgabe: Warteschlange `pending: TerminalOutput[]`, `ready = false`; `attachTerminal` mit Handler, der vor `ready` in `pending` legt, danach direkt schreibt; nach Auflösen `term.write(snapshot)`, `pending` abspielen, `ready = true`. `data` → `term.write(text)`; `exit` → `term.write('\r\n[Prozess beendet' + (code === null ? '' : `, Code ${code}`) + ']\r\n')`. Der Kanal bleibt angehängt, solange die Instanz lebt (auch ausgehängt).
  - Eingabe: `term.onData((data) => writeTerminal(id, data))`, Fehler in die Konsole.
  - Tasten: `attachCustomKeyEventHandler`, nur `keydown`: Kopier-Taste (`Strg+C`, macOS `Cmd+C`) bei `term.hasSelection()` → `navigator.clipboard.writeText(term.getSelection())`, `false`; Einfüge-Taste (`Strg+V` bzw. `Cmd+V`) → `false` ohne `preventDefault`; `Strg+Ö` → `false` (geht an das globale Kürzel); sonst `true`.
  - `show(instance, host)`: `host.appendChild(instance.element)`; beim ersten Mal `term.open(instance.element)` und `isOpened = true`; danach `fitAndResize(id)`, `term.refresh(0, term.rows - 1)`, `term.focus()`.
  - `hide(instance)`: `instance.element.remove()` — nichts sonst.
  - `fitAndResize(id)`: hat `element` Breite oder Höhe 0, nichts tun; sonst `fit.fit()`, bei geänderten `cols`/`rows` `resizeTerminal` (entprellt 50 ms) und `lastCols`/`lastRows` setzen.
  - `destroy(id)`: `detachTerminal`, `term.dispose()`, `element.remove()`, aus der Map.
  - `prune(projectId, ids: readonly string[])`: jede Instanz des Vorhabens, deren ID nicht in `ids` steht, `destroy`.
  - Farbschema: einmal beim Laden des Moduls auf den Settings-Store abonnieren; wechselt `colorScheme`, nach einem `requestAnimationFrame` bei allen Instanzen `term.options.theme = terminalTheme()`.
- [x] `src/features/terminal/TerminalPane.tsx` (Props `terminalId: string`, `projectId: string`), `TerminalPane.css` (Host füllt die Restfläche; `.terminal-pane__screen` 100 % hoch und breit):
  - Mount: `instance = acquire(terminalId, projectId)`, `show(instance, host)`, `ResizeObserver` auf den Host → `fitAndResize(terminalId)`.
  - Unmount: Observer trennen, `hide(instance)` — **kein** `destroy`, kein `detachTerminal`.
- [x] `src/features/terminal/TerminalDock.tsx` (Props `projectId: string`, `terminals: TerminalInfo[]`, `placement: TerminalPlacement`), `TerminalDock.css` (BEM `terminal-dock`, Modifikatoren `terminal-dock--bottom`, `terminal-dock--right`, `terminal-dock--maximized`; Tokens wie `session-header__tab`):
  - Größe: angezeigt wird `dragSize ?? gespeicherter Wert`, geklemmt auf die Grenzen (`--bottom` 120 px bis Höhe von `app__main` − 160 px, `--right` 320 px bis 60 % von `window.innerWidth`).
  - Ziehkante: `--bottom` oben (`cursor: row-resize`), `--right` links (`cursor: col-resize`), per Pointer-Capture; während des Ziehens `setDragSize(px)`; beim Loslassen `save({ kind: 'terminalHeight' | 'terminalWidth', value: Math.round(px) })` über `useSettingsUpdate`, danach `setDragSize(null)`; bei `--maximized` keine Ziehkante.
  - Leiste: je Terminal ein Knopf `<label> <number>`, bei `running === false` Zusatz „beendet“ in `--color-fg-muted`; aktiver hervorgehoben; „×“ (`title="Terminal schließen und Prozess beenden"`) → `closeTerminal`; „+“ (`title="Neues Terminal im Arbeitsordner des Vorhabens"`) → `openTerminal(projectId, 80, 24)`, danach auswählen; rechts „Maximieren“ (nur `bottom`, `title="Terminal maximieren"` bzw. „Wiederherstellen“) und „Ausblenden“ (`title="Terminal ausblenden (Strg+Ö)"`) → `close()`.
  - Fehler von `openTerminal`/`closeTerminal` als Satz über der Leiste in `--color-status-error` über `commandErrorText`.
  - Auswahl: gespeicherte aus dem Store, sonst das letzte. Ist die Liste nach dem ersten Laden leer → einmal `openTerminal` (Schutz per `useRef` gegen Strict Mode; je `projectId` neu scharf).
  - Darunter `<TerminalPane key={selectedId} terminalId={selectedId} projectId={projectId} />` über die Restfläche (nur das gewählte ist eingehängt; die übrigen leben in der Ablage weiter).
- [x] `src/app/App.tsx`:
  - `terminalProjectId = isMainReplaced ? null : (currentSession?.projectId ?? null)`; `useProjectTerminals(terminalProjectId)` einmal; `runningTerminalCount`.
  - `placement` aus dem Settings-Store, solange nicht geladen `'bottom'`.
  - `toggleTerminal()`: öffnet bzw. schließt; öffnet es bei `placement === 'right'` und ist das Hintergrund-Panel offen → Hintergrund `close()`. `onToggleBackground` an die Kopfzeilen wird zu: öffnet es den Hintergrund bei `placement === 'right'` und offenem Dock → Dock `close()`; dann `toggleBackground()`.
  - Tastenkürzel `Strg+Ö` (`event.ctrlKey && event.key.toLowerCase() === 'ö'`) neben dem bestehenden `Strg+;`, nur mit `terminalProjectId !== null`, `preventDefault`.
  - `bottom`: in `<main className="app__main">` den bisherigen Inhalt in `<div className="app__content">` (flex 1, `min-height: 0`, als Spalte) legen; bei `isOpen && isMaximized` bekommt er `app__content--hidden` (`display: none`, bleibt eingehängt); darunter `<TerminalDock key={terminalProjectId} … />` bei `isOpen && terminalProjectId !== null`.
  - `right`: `TerminalDock` mit denselben Props als Geschwister zwischen `<main>` und `BackgroundPanel`, unter derselben Bedingung.
- [x] `src/app/App.css`: `&__content` und `&__content--hidden`.
- [x] `src/app/SessionHeader.tsx` und `src/app/ProjectHeader.tsx`: Props `terminalCount: number`, `isTerminalOpen: boolean`, `onToggleTerminal: () => void`; Knopf `>_` im Bereich `session-header__tools` vor dem Knopf des Hintergrund-Panels (Übersicht: am Ende von `session-header__tools`), gleiche Klassen wie dieser, aktiv bei `isTerminalOpen`, Zahl nur bei `> 0`, `title="Terminal ein-/ausblenden (Strg+Ö)"`.
- [x] Doku: `docs/code-map.md` neue Zeile „Terminal (Pseudo-Terminal je Vorhaben, Dock unten/rechts; ADR 055)“ mit Oberfläche (`src/features/terminal/`, `src/stores/terminal.ts`, `src/lib/terminal.ts`, Anschluss in `App.tsx` und den Kopfzeilen, Einstellung `TerminalPlacementSegment`) und Core (`src-tauri/src/terminal/`, `commands/terminal.rs`, `sessions/registry/terminal.rs`, Exit-Hook in `lib.rs`, `settings/` Schlüssel `terminal_placement`), `terminal` in der Feature-Liste; `docs/glossary.md` Begriffe „Terminal“ und „Terminal-Dock“.
- [x] Commit `feat(terminal): Terminal-Dock unten oder rechts mit xterm.js`.
- [x] Planabschluss nach [releases.md](../../conventions/releases.md) (Minor-Version, Archiv, STATE.md).

## Report-Back

Umgesetzt wie geplant, Commit `50040a1`. `pnpm check` grün; Laufzeit ungeprüft (nicht in der echten App gelaufen).

Abweichungen: `show` bekommt die Terminal-ID als ersten Parameter (`fitAndResize` braucht sie); `TerminalDock` bekommt zusätzlich `isLoaded` und `listError`, damit der Auto-Start erst nach dem ersten Laden greift; `TerminalInstance` trägt `isDisposed` und `resizeTimer`; Grenzen des Docks liefert `useDockSpace`.
