# 056 — Verschiebbare Leisten

**Status:** angenommen · **Datum:** 2026-11-04

## Kontext

Die vier festen Leisten — Seitenleiste (Vorhaben), Dateibaum der Changes, Liste der Artefakte, Hintergrund-Leiste — haben eine feste Breite. Wie in VS Code sollen sie einen Griff an ihrer Kante bekommen: ziehen ändert die Breite, Doppelklick stellt die Standardbreite wieder her, die Breite überlebt einen Neustart.

## Optionen

- **Speicherort:** (a) Tabelle `settings` mit Spiegel in `localStorage`; (b) nur `localStorage`; (c) nur Tabelle `settings`.
- **Grenzen:** (i) Mindest- und Standardbreiten nur in der Oberfläche, der Core prüft nur grob; (ii) Grenzen in Core und Oberfläche.
- **Speichern:** (A) bei jeder Mausbewegung; (B) einmal beim Loslassen.

## Entscheidung

- **(a) Tabelle `settings` plus Spiegel `verwalter.panelWidths` in `localStorage`** — dasselbe Muster wie das Farbschema ([ADR 012](012-einstellungen-farbschema-und-listen.md)). Die Datenbank ist die Quelle (AGENTS.md Regel 3); (b) widerspräche ADR 012, weil reines `localStorage` mit dem WebView-Profil verloren geht. (c) ließe die Leisten bei jedem Start von der Standardbreite auf die gemerkte springen; der Spiegel wird beim ersten Rendern gelesen.
- **(i) Grenzen und Standardbreiten leben nur in der Oberfläche** (`PANEL_BOUNDS` in `src/lib/panelWidths.ts`). Der Core speichert je Leiste `Option<u32>`, `None` = Standardbreite, und lehnt nur Werte außerhalb 120–10000 px ab (`CommandError`; 10000 deckt mehrere breite Monitore). Grenzen an zwei Orten wären Drift.
- **Schlüssel** `panel_width.sidebar`, `panel_width.changes_tree`, `panel_width.artifacts_list`, `panel_width.background`. Fehlt ein Schlüssel oder ist sein Text unlesbar, gilt `None`. `SettingsChange::PanelWidth { panel, value }` mit `value = None` löscht den Schlüssel (Doppelklick = Standard).
- **(B) Gespeichert wird beim Loslassen**: während des Ziehens hält ein React-State die Breite, `pointerup` schickt genau ein `settings_update`.
- **Kein festes Maximum, aber nie breiter als das Fenster:** eine Leiste darf genau den Platz ihrer Füllfläche übernehmen (der flexible Nachbar im selben Elternelement, markiert mit `data-resize-fill`). Obergrenze beim Ziehen = Breite der Leiste plus Breite der Füllfläche minus deren `min-width`, gelesen beim Anfassen.
- **Fenster wird kleiner:** reine CSS-Regel. Leisten haben `flex-shrink: 1` und `min-width`, Füllflächen `flex: 1 1 0; min-width: 0` — die Füllfläche schrumpft zuerst auf null (hat sie ein eigenes `min-width`, wie die Liste des Dashboards mit 320 px, nur bis dahin), danach die Leisten bis zu ihrem Minimum. Die gespeicherte Breite bleibt unverändert und gilt wieder, wenn das Fenster wächst.

| Leiste | `PanelId` | Standard | Minimum | Griff an |
|---|---|---|---|---|
| Seitenleiste | `sidebar` | 256 | 200 | rechter Kante |
| Dateibaum der Changes | `changesTree` | 360 | 240 | rechter Kante |
| Liste der Artefakte | `artifactsList` | 300 | 220 | rechter Kante |
| Hintergrund-Leiste | `background` | 440 | 320 | linker Kante |

## Konsequenzen

- Eine fünfte Leiste ist ein neuer Eintrag in `PanelId`, `PanelWidths`, `panel_key` und `PANEL_BOUNDS`, keine Migration.
- Löscht der Benutzer den WebView-Speicher, springen die Leisten beim nächsten Start einmal von der Standardbreite auf die gemerkte, bis die Einstellungen geladen sind.
- Ist der Chat auf null gezogen, sieht man ihn nicht mehr; zurück per Ziehen oder Doppelklick.
- Die Container-Abfragen der Kopfzeile reagieren auf die gezogenen Breiten.

## Nachtrag: Kategorien-Leiste der Einstellungen (Nachbesserung 0.78.0)

Aus der Abnahme von 0.78.0 kam die Bitte, die Kategorien-Leiste der Einstellungen wie die Seitenleiste verschiebbar zu machen. Sie ist die fünfte Leiste: `PanelId::SettingsNav` (`settingsNav`), Schlüssel `panel_width.settings_nav`, Standard 216, Minimum 160, Griff an der rechten Kante, Füllfläche ist der Inhalt rechts. Keine Migration. Label und Zusammenfassung kürzen mit „…“, damit bei 160 px kein waagerechter Scrollbalken entsteht.

## Nachtrag: Trenner im Dashboard und in der Umsetzung (Nachbesserung 0.78.0)

Aus der Abnahme von 0.78.0 kam die Bitte, alle festen senkrechten Trenner verschiebbar zu machen. Drei weitere Leisten: `PanelId::DashboardFolders` (`dashboardFolders`, Schlüssel `panel_width.dashboard_folders`, Standard 210, Minimum 160, Griff rechts, Füllfläche ist die Liste), `PanelId::DashboardReader` (`dashboardReader`, `panel_width.dashboard_reader`, Standard 430, Minimum 340, Griff links, die alte Obergrenze von 430 px fällt weg) und `PanelId::ImplementationTimeline` (`implementationTimeline`, `panel_width.implementation_timeline`, Standard 640 statt der halben Breite, Minimum 380, Griff rechts, Füllfläche ist die Kartei). Die Liste des Dashboards bleibt mindestens 320 px breit; die Leisten daneben schrumpfen zuerst. Die Umsetzung gibt die Breite als CSS-Variable `--timeline-width` weiter, damit die schmale Ansicht unter 900 px (alles untereinander, ohne Griff) sie überschreiben kann. Ziehen einer Dashboard-Leiste drückt die andere nicht, weil der Griff das Minimum der Liste von der Obergrenze abzieht. Keine Migration.

## Nachtrag: Einklappen, Seitenleiste minimieren (Taiga 261)

Die Seitenleiste der App lässt sich zu einer schmalen Symbolleiste minimieren; derselbe Mechanismus („eingeklappte Leisten“) trägt später das Einklappen weiterer Leisten. Keine Migration.

- **Zustand (V1):** `Settings.collapsed_panels: Vec<PanelId>` in der Reihenfolge von `PanelId`, Änderung `SettingsChange::PanelCollapsed { panel, collapsed }`. Gespeichert als Schlüssel `panel_collapsed.<Name wie in panel_width.*>` mit dem Wert `true`; `collapsed: false` löscht ihn, fehlt er, ist die Leiste offen. Die gezogene Breite (`panel_width.*`) bleibt beim Einklappen gespeichert und gilt beim Aufklappen wieder.
- **Oberfläche:** Spiegel in `localStorage` unter `verwalter.collapsedPanels` (Helfer in `src/lib/panelWidths.ts`), beim ersten Rendern gelesen, damit eine minimierte Leiste beim Start nicht kurz offen aufblitzt. Hook `usePanelCollapsed(panel)` in `src/lib/usePanelCollapsed.ts` liefert `{ isCollapsed, toggle }`, schaltet sofort um und speichert dann über `updateSettings`.
- **Bedienung (L2–L4):** Der Knopf „Seitenleiste minimieren (Strg+B)“ sitzt rechts in der Kopfzeile der Leiste, oben in der Symbolleiste steht „Seitenleiste aufklappen (Strg+B)“. Strg+B ist ein globales Kürzel in `App.tsx` wie Strg+N. Im Terminal verarbeitet xterm die Taste selbst und gibt sie an die Shell; `terminalInstances.ts` bleibt unverändert. Im Eingabefeld des Chats wirkt das Kürzel.
- **Symbolleiste (L3, L6):** 44 px breit (`--sidebar-minimized-width` in `Sidebar.css`) mit Aufklappen, Neu, Dashboard mit Zähler, Archiv und Einstellungen mit den Hinweispunkten; jedes Symbol hat Tooltip und `aria-label` wie der Knopf der offenen Leiste. Minimiert gibt es keine Liste der Vorhaben und keinen Griff zum Ziehen.

## Nachtrag: Dashboard einklappen und untereinander (Taiga 260)

Im Dashboard lassen sich Ordnerspalte und Lesebereich unabhängig einklappen, und der Lesebereich steht nach Einstellung rechts neben oder unter dem Postfach. Keine Migration.

- **Höhe und Achse (V2):** `PanelId::DashboardReaderHeight` (`dashboardReaderHeight`, Schlüssel `panel_width.dashboard_reader_height`) ist die einzige Leiste mit einer Höhe statt einer Breite; sie läuft über `PanelWidths`, `usePanelWidth` und `PANEL_BOUNDS` wie die übrigen (P4). `ResizeHandle` bekommt `axis?: 'x' | 'y'` (Standard `x`); mit `y` misst er `offsetHeight`, `clientY` und das `min-height` der Füllfläche, nimmt Pfeil hoch und runter und zeigt `row-resize`. Die acht Einbauten mit Breite bleiben unverändert.
- **Anordnung:** `Settings.dashboard_reader_placement: ReaderPlacement` (`right` | `bottom`, Standard `right`), Schlüssel `dashboard_reader_placement`, Änderung `SettingsChange::DashboardReaderPlacement`. Sie steht in der Einstellungsseite unter „Darstellung“ wie `terminalPlacement` (P2); im Dashboard gibt es keinen eigenen Umschalter. Die Anordnung hat denselben Spiegel (`verwalter.dashboardReaderPlacement`, geschrieben in `useSettings` nach dem Laden und in `useSettingsUpdate` nach dem Speichern, beim ersten Rendern gelesen), sonst springt der Lesebereich beim Start mit „unten“ von rechts nach unten.
- **Einklappen (P1, P5):** Zwei Knöpfe „Ordnerspalte“ und „Lesebereich“ rechts in der Kopfzeile, immer da, mit `aria-pressed` (= sichtbar). Der Zustand läuft über die eingeklappten Leisten (`dashboardFolders`, `dashboardReader`, Nachtrag Taiga 261). Ein eingeklappter Bereich wird nicht gerendert, sein Griff auch nicht; die Liste füllt die Fläche. Die gezogene Breite bleibt gemerkt.
- **Filter bei eingeklappter Ordnerspalte (P3):** Der Filter bleibt aktiv; die Kopfzeile nennt „Ordner: <Name>“ mit dem Namen aus der Ordnerspalte (`folderName` in `postfach.ts`). Bei „Alles“ steht kein Hinweis.
- **Aufbau „untereinander“:** Die Ordnerspalte bleibt links; rechts davon steht eine Spalte (`postfach__rightcol`, Füllfläche der Ordner-Spalte) mit der Liste oben (Mindesthöhe 160 px), dem Griff mit `axis="y"` und dem Lesebereich unten. Tastatur, Auswahl, Aufklappen (▾) und Mehrfachauswahl sind in jeder Anordnung gleich; bei eingeklapptem Lesebereich ist das Aufklappen in der Liste der Weg zum Lesen (P6).

| Leiste | `PanelId` | Standard | Minimum | Griff an |
|---|---|---|---|---|
| Lesebereich des Dashboards (unten) | `dashboardReaderHeight` | 320 | 200 | oberer Kante |
