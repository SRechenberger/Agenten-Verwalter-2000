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
- **Fenster wird kleiner:** reine CSS-Regel. Leisten haben `flex-shrink: 1` und `min-width`, Füllflächen `flex: 1 1 0; min-width: 0` — die Füllfläche schrumpft zuerst auf null, danach die Leisten bis zu ihrem Minimum. Die gespeicherte Breite bleibt unverändert und gilt wieder, wenn das Fenster wächst.

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
