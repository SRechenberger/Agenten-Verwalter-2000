# Verschiebbare Leisten

Die vier festen Leisten — Seitenleiste (Vorhaben), Dateibaum der Changes, Liste der Artefakte, Hintergrund-Leiste — bekommen wie in VS Code einen Griff an ihrer Kante: ziehen ändert die Breite, Doppelklick stellt die Standardbreite wieder her, die Breite überlebt einen Neustart.

| Phase | Inhalt | Rating | Status |
|---|---|---|---|
| 1 | Breiten im Core speichern (Einstellungen, ADR 056) | standard | complete |
| 2 | Griff, Breiten-Hook, Einbau in die vier Leisten, Doku | standard | complete |

## Entscheidungen

- **Speicherort: Tabelle `settings` in der Datenbank, plus Spiegel in `localStorage`** — dasselbe Muster wie das Farbschema ([ADR 012](../decisions/012-einstellungen-farbschema-und-listen.md)). Die Datenbank ist die Quelle (AGENTS.md Regel 3; ADR 012 verwirft reines `localStorage`, weil es mit dem WebView-Profil verloren geht). Der Spiegel `verwalter.panelWidths` wird beim ersten Rendern gelesen, damit die Leisten beim Start nicht von der Standardbreite auf die gemerkte springen. Verworfen: nur `localStorage` (Widerspruch zu ADR 012), nur Datenbank (sichtbarer Sprung bei jedem Start).
- **Grenzen und Standardbreiten leben nur in der Oberfläche** (`PANEL_BOUNDS` in `src/lib/panelWidths.ts`). Der Core speichert je Leiste `Option<u32>`; `None` = Standardbreite. Der Core lehnt nur Unsinn außerhalb 120–10000 ab (`CommandError`; 10000 deckt auch mehrere breite Monitore), sonst prüft er nichts — eine Grenze, zwei Orte, wäre Drift.
- **Gespeichert wird beim Loslassen**, nicht bei jeder Mausbewegung: während des Ziehens hält ein React-State die Breite, `pointerup` schickt genau ein `settings_update`.

| Leiste | Schlüssel `PanelId` | Standard | min | Griff an |
|---|---|---|---|---|
| Seitenleiste | `sidebar` | 256 | 200 | rechter Kante |
| Dateibaum der Changes | `changesTree` | 360 | 240 | rechter Kante |
| Liste der Artefakte | `artifactsList` | 300 | 220 | rechter Kante |
| Hintergrund-Leiste | `background` | 440 | 320 | linker Kante |

**Kein festes Maximum, aber nie breiter als das Fenster (Vorgabe Sascha):** eine Leiste darf genau den Platz ihrer **Füllfläche** übernehmen, mehr nicht. Füllfläche = der flexible Nachbar im selben Elternelement, markiert mit `data-resize-fill`: in `.app` ist das `app__main` (Chat), in `changes-view__body` die Diff-Fläche `changes-view__detail`, in `.artifacts` die Vorschau `artifacts__preview`. Obergrenze beim Ziehen = `panel.offsetWidth + fill.offsetWidth`, gelesen beim `pointerdown` bzw. Tastendruck. Die Füllfläche darf bis auf null schrumpfen; dadurch kann die Summe der Leisten nie die Fensterbreite überschreiten.

**Fenster wird kleiner:** reine CSS-Regel, kein JS. Leisten bekommen `flex-shrink: 1` und `min-width: <min>px` (statt heute `flex-shrink: 0`), Füllflächen `flex: 1 1 0; min-width: 0`. Weil die Füllfläche die Basis 0 hat, schrumpft sie zuerst auf null, danach schrumpfen die Leisten anteilig bis zu ihrem Minimum. Die gespeicherte Breite bleibt unverändert; wird das Fenster wieder größer, kehrt die Leiste zu ihr zurück. (Heutiges `flex-shrink: 0` existiert, damit breiter Chat-Inhalt die Leisten nicht zusammendrückt — das erledigt jetzt die Basis 0 der Füllfläche.)

## Finale AK

1. An der Kante jeder der vier Leisten zeigt der Mauszeiger `col-resize`; ziehen ändert die Breite live, zwischen dem Minimum der Tabelle und dem Punkt, an dem die Füllfläche (Chat, Diff, Artefakt-Bühne) null breit ist. Keine Leiste und keine Summe von Leisten ragt je über das Fenster hinaus, auch nicht nach Verkleinern des Fensters.
2. Doppelklick auf den Griff setzt die Standardbreite und speichert sie.
3. Nach einem Neustart der App haben die Leisten die zuletzt gezogene Breite, ohne sichtbaren Sprung beim Start.
4. Ziehen über der Artefakt-Ansicht (iframe) reißt nicht ab: der Griff behält die Maus bis zum Loslassen.
5. Während des Ziehens wird kein Text markiert.
6. Der Griff ist per Tastatur bedienbar: Fokus per Tab, Pfeil links/rechts ±16 px, `Home` = min, `End` = Obergrenze (Füllfläche null); gespeichert wird beim Loslassen der Taste (`keyup`).
7. Beim Überfahren erscheint nach kurzer Verzögerung eine 2-px-Linie in Akzentfarbe, während des Ziehens dauerhaft; der Tooltip lautet „Ziehen: Breite ändern · Doppelklick: Standardbreite“.
8. Schlägt das Speichern fehl, bleibt die gezogene Breite für diese Sitzung stehen und der Fehler landet in der Konsole (kosmetisch, wie die Titelleiste in `colorScheme.ts`).

## Phase 1 — Breiten im Core speichern

**Kontext:** `src-tauri/src/settings/model.rs` (`Settings`, `SettingsChange`), `src-tauri/src/settings/mod.rs` (`KEY_*`, `load`, `update`), `src-tauri/src/db/settings.rs` (`read_all`, `write`), `docs/decisions/012-einstellungen-farbschema-und-listen.md`, `docs/conventions/rust.md`. Fehlerklassen geprüft (React, TypeScript): keine einschlägig.

- [x] `model.rs`: `enum PanelId { Sidebar, ChangesTree, ArtifactsList, Background }` (`serde(rename_all = "camelCase")`, `TS`, Doc-Kommentar je Variante mit der Leiste); `struct PanelWidths { sidebar, changes_tree, artifacts_list, background: Option<u32> }` (`camelCase`, `TS`); Feld `pub panel_widths: PanelWidths` in `Settings` mit Doc-Kommentar „Breite je Leiste in Pixeln; `None` = Standardbreite der Oberfläche (ADR 056)“.
- [x] `SettingsChange::PanelWidth { panel: PanelId, value: Option<u32> }` — `None` löscht den Schlüssel (Doppelklick = Standard).
- [x] `mod.rs`: Schlüssel `panel_width.sidebar`, `panel_width.changes_tree`, `panel_width.artifacts_list`, `panel_width.background` (eine Funktion `panel_key(PanelId) -> &'static str`). `load`: unlesbarer oder fehlender Text → `None`. `update`: `Some(v)` außerhalb `120..=10000` → `CommandError` mit Satz „Leistenbreite außerhalb 120–10000 px“; `Some` schreibt, `None` löscht den Schlüssel. `db/settings.rs` kennt kein Löschen: `pub fn delete(connection: &Connection, key: &str) -> Result<(), CommandError>` neben `write` anlegen, Stil wie `write`.
- [x] `pnpm bindings` — erzeugt `PanelId.ts`, `PanelWidths.ts`, erweitert `Settings.ts`/`SettingsChange.ts`.
- [x] ADR `docs/decisions/056-verschiebbare-leisten.md` (Kontext / Optionen / Entscheidung / Konsequenzen) aus dem Abschnitt „Entscheidungen“ oben.
- [x] `pnpm check` grün (Clippy beanstandet ein ungenutztes Feld nicht, weil `Settings` serialisiert wird).

## Phase 2 — Griff, Hook, Einbau, Doku

**Kontext:** `src/lib/colorScheme.ts` (Muster Spiegel in `localStorage` mit `try/catch`), `src/stores/settings.ts`, `src/lib/settings.ts` (`updateSettings`), `src/app/App.tsx` (Sidebar, `BackgroundPanel`), `src/features/changes/ChangesView.tsx` (`changes-view__body` mit `FileTree`), `src/features/artifacts/ArtifactsView.tsx` (`artifacts__list`), die vier CSS-Dateien `src/app/Sidebar.css`, `src/features/changes/FileTree.css`, `src/features/artifacts/ArtifactsView.css`, `src/features/background/BackgroundPanel.css`, `docs/conventions/react.md`, `docs/conventions/tailwind.md`, `docs/code-map.md`, `docs/glossary.md`.

- [x] `src/lib/panelWidths.ts`: `PANEL_BOUNDS: Record<PanelId, { initial; min }>` mit den Werten der Tabelle; `clampPanelWidth(panel, value, max)` klemmt auf `[min, max]` (ist `max < min`, gilt `min`); beim Lesen aus Store/Spiegel nur nach unten auf `min` klemmen — die Obergrenze erzwingt beim Anzeigen die CSS-Regel; Spiegel `readRememberedPanelWidths()` / `rememberPanelWidths(widths)` unter `verwalter.panelWidths`, beide mit `try/catch` wie `colorScheme.ts`.
- [x] `src/lib/usePanelWidth.ts`: `usePanelWidth(panel: PanelId)` liefert `{ width, handleProps }`. Startwert: Wert aus `useSettingsStore` falls geladen, sonst Spiegel, sonst `initial`; immer durch `clampPanelWidth`. Kommt der Store-Wert nach dem ersten Rendern, gewinnt er (nicht während eines laufenden Ziehens). Speichern: `updateSettings({ kind: 'panelWidth', panel, value })`, Ergebnis in `setSettings`, danach `rememberPanelWidths`; Fehler → `console.error('Leistenbreite nicht gespeichert', reason)`.
- [x] `src/components/ResizeHandle.tsx` + `ResizeHandle.css` (BEM `resize-handle`): Props `edge: 'start' | 'end'` (Kante, an der die Leiste wächst), `width`, `min`, `onResize(width)`, (Obergrenze beim `pointerdown`/Tastendruck: Leiste = `previousElementSibling` bei `edge="end"`, `nextElementSibling` bei `edge="start"`; Füllfläche = `parentElement.querySelector(':scope > [data-resize-fill]')`; Obergrenze = Summe beider `offsetWidth`; `aria-valuemax` ebenso), `onCommit(width | null)`, `label`. Element `div` mit `role="separator"`, `aria-orientation="vertical"`, `aria-valuenow/min/max`, `aria-label` = Name der Leiste, `tabIndex={0}`, `title` = Tooltip aus AK 7. `pointerdown` → `setPointerCapture`, Startpunkt merken, Klasse `resizing` an `document.documentElement`; `pointermove` → neue Breite (`edge === 'end'`: Start + Δx, `'start'`: Start − Δx), geklemmt, `onResize`; `pointerup`/`lostpointercapture` → `onCommit(width)`, Klasse entfernen. `onDoubleClick` → `onCommit(null)`. Tastatur nach AK 6.
- [x] CSS: Griff `position: relative; flex-shrink: 0; width: 0` mit `::before` als 6 px breite Trefferfläche (`inset-block: 0; inset-inline: -3px`, `cursor: col-resize`, `z-index` über den Nachbarn) und `::after` als 2-px-Linie in `--color-accent` (`theme.css`), `opacity` 0 → 1 mit `transition-delay: 300ms` bei `:hover`, sofort bei `.resize-handle--active` und `:focus-visible`. Global in `ResizeHandle.css`: `:root.resizing { cursor: col-resize; user-select: none; } :root.resizing iframe { pointer-events: none; }`.
- [x] Einbau: Leiste bekommt `style={{ width }}`, das feste `width` in ihrer CSS-Datei bleibt als Fallback stehen; in ihrer CSS-Datei `flex-shrink: 0` → `flex-shrink: 1` plus `min-width` aus der Tabelle. Füllflächen bekommen `data-resize-fill` und in ihrer CSS `flex: 1 1 0; min-width: 0` — `app__main` (`App.tsx`/`App.css`), `changes-view__detail` (`ChangesView.tsx`, neben `FileTree`) und `artifacts__preview` (`ArtifactsView.tsx`, neben `artifacts__list`). Griff als **Geschwister** direkt neben der Leiste (nie in ihr — `FileTree` scrollt und virtualisiert):
  - `App.tsx`: `<ResizeHandle edge="end" …>` nach `<Sidebar>`; `<ResizeHandle edge="start" …>` vor `<BackgroundPanel>` innerhalb derselben Bedingung `isBackgroundOpen && visibleSession !== null`. `Sidebar` und `BackgroundPanel` bekommen eine Prop `width: number` und setzen sie am Wurzel-Element.
  - `ChangesView.tsx`: Griff nach `<FileTree>` in `changes-view__body`; `FileTree` bekommt Prop `width`.
  - `ArtifactsView.tsx`: Griff nach `<nav className="artifacts__list">`.
- [x] `docs/code-map.md`: Zeile „Verschiebbare Leisten“ → `src/lib/panelWidths.ts`, `src/lib/usePanelWidth.ts`, `src/components/ResizeHandle.tsx`; Core `settings/` (`PanelWidths`). `docs/glossary.md`: Begriff „Leiste“ (eine der vier seitlichen Flächen mit Griff), falls dort noch nicht geführt.
- [x] `pnpm check` grün.

## Wackelstellen

- 🟡 Zeiger-Fang über dem iframe der Artefakte in WebView2 — abgesichert doppelt (`setPointerCapture` und `pointer-events: none` auf iframes während des Ziehens); Beleg: in der Dev-App über die Artefakt-Seite ziehen.
- 🟡 Container-Abfragen der Kopfzeile (`@container main` in `SessionHeader.css`) reagieren jetzt auf gezogene Breiten — gewollt, aber beim Ziehen springen die Umbruchstufen sichtbar; Beleg: Sidebar und Hintergrund breit ziehen, Kopfzeile ansehen.

- 🟡 Ist der Chat auf null gezogen, sieht man ihn nicht mehr — gewollt; zurück per Ziehen oder Doppelklick. Beleg: Sidebar bis zum Anschlag ziehen, dann Fenster verkleinern — nichts ragt heraus, Griffe bleiben erreichbar.

## Summary

Vier Leisten (Seitenleiste, Dateibaum, Artefaktliste, Hintergrund) lassen sich per Griff ziehen; Doppelklick stellt die Standardbreite her; Speicherung in der Datenbank mit Spiegel in `localStorage` (ADR 056).

## Files touched

`src/lib/panelWidths.ts`, `src/lib/usePanelWidth.ts`, `src/components/ResizeHandle.{tsx,css}`, die vier Leisten samt CSS, `useSettings.ts` (Spiegel nachziehen), `src-tauri/src/settings/`.

## Commits

## Deviations from plan

`useSettings.ts` schreibt den Spiegel beim Laden der Einstellungen mit, damit er bei neuem WebView-Profil nicht leer bleibt.

## Follow-ups
