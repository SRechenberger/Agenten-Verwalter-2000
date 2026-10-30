# Plan: Umsetzung — Phasen und Stationen im Verwalter

Der Verwalter führt die Umsetzung eines Plans selbst: Die Planungs-Session legt nach der Freigabe über ein Werkzeug des Verwalters eine **Umsetzung** an (Phasen mit Stationen „Test · Umsetzung · Review“, je Station Modell und Auftrag). Der Verwalter startet jede Station als frische Session, sobald ihre Vorgänger fertig gemeldet haben, zeigt den Ablauf als Zeitleiste mit gekoppelter Kartei, nimmt Plan-Änderungen nur nach „Übernehmen“ an und gibt Rückfragen einen programmatischen Lagebericht mit. Die Einstiegszeile entfällt im Verwalter; im Terminal arbeiten die Skills wie heute. Entwurf: [artifacts/entwurf-umsetzung.html](artifacts/entwurf-umsetzung.html), vom User am 2026-10-30 freigegeben (Abschnitt 9 „Festgelegt“); Details zu den Dateien in [artifacts/README.md](artifacts/README.md).

Kontext für jeden Umsetzer: [AGENTS.md](../../../AGENTS.md), [docs/code-map.md](../../code-map.md) (Zeilen „Vorhaben“, „Sessions“, „Chat“ (Einstiegszeile, `HandoffBar`), „Anhänge“, „TL;DR“, „Nebenfragen“, „App-Rahmen“, „Sidebar-Zeilen“), [docs/glossary.md](../../glossary.md), [docs/conventions/](../../conventions/), [ADR 011](../../decisions/011-vorhaben-und-sessions.md) (Vorhaben und Sessions), [ADR 027](../../decisions/027-session-uebergabe.md) und [ADR 040](../../decisions/040-automatisch-weiter.md) (Einstiegszeile, Countdown, Haltegründe), ADR 046 (MCP-Zugang, aus dem Plan `2026-10-30_artefakte-dateitypen`). Fehlerklassen geprüft (Vault: TypeScript, React, Tailwind): keine einschlägig; für Rust, Tauri und SQLite gibt es keine Einträge.

## Überblick

| # | Phase | Rating | Wave | Aufwand | Status |
|---|---|---|---|---|---|
| 1 | [Core: Datenmodell, Werkzeuge anlegen/lesen/beschreiben, Zeile in der Sidebar, Lagebericht als Text; ADR 048, Migration 17](phase-1-core-und-werkzeuge.md) | heikel | 1 | L | pending |
| 2 | [Ansicht: Zeitleiste mit gekoppelter Kartei, Kopfzeile, Reiter Changes](phase-2-zeitleiste-und-kartei.md) | standard | 2 | L | pending |
| 3 | [Stationen starten: Fertigmeldung, Nachfolger, erste Nachricht, Countdown, Fix-Runde, Fehler](phase-3-stationen-starten.md) | heikel | 3 | L | pending |
| 4 | [Wellen (parallele Phasen) und Plan-Änderung mit Karte „Plan ändern?“](phase-4-wellen-und-plan-aenderung.md) | heikel | 4 | L | pending |
| 5 | [Rückfragen mit Lagebericht als Anhang, Knopf „Lagebericht“](phase-5-rueckfragen-und-lagebericht.md) | standard | 5 | M | pending |
| 6 | [Doku-Abschluss: Code-Map, Glossar, AGENTS.md, Prüfprogramm in AGENTS.md](phase-6-doku-abschluss.md) | mechanisch | 6 | S | pending |
| 7 | [Skills: Planung und Umsetzung auf die Werkzeuge des Verwalters umstellen (Master unter `G:\Meine Ablage\U104_Agents`, dann spiegeln)](phase-7-skills.md) | standard | 7 | M | pending |

Rein sequenziell: jede Phase 2–5 erweitert dieselben Dateien (`src-tauri/src/implementation/mod.rs`, `commands/implementation.rs`, `src/lib/implementation.ts`, `ImplementationView.tsx`); Phase 4 und 5 ließen sich nur mit einem Kontrakt-Split auf Dateiebene parallelisieren, der mehr kostet als eine Session Wartezeit. Phase 1 ist eine Fundament-Phase (Datenmodell + Werkzeuge), ihr sichtbarer Anteil ist die Zeile in der Sidebar und der Lagebericht als Text.

**Rahmen (work-Profil, Datei-Fall):** Feature-Branch `feature/umsetzung`, ein Commit je Phase, Scope `implementation` (Phase 7: kein Commit im Verwalter-Repo, Decision-Log im Master). `pnpm check` grün vor jedem Commit, `cargo` im PATH, Bindings mitcommitten. Keine neuen Abhängigkeiten (`uuid`, `serde_json`, `rusqlite`, `regex` sind da). **Keine automatisierten Tests und keine Smoke-Checkliste** — Projektregel (`STATE.md`: „getestet wird im laufenden Betrieb“), sie schlägt die Test-Pflicht des work-Profils. Prüfbarkeit stattdessen über das Prüfprogramm `src-tauri/examples/implementation-probe.rs` (Phase 1, erweitert in 3 und 4) und `pnpm tauri dev`. Review-Modus: **gebündelt** — ein Gesamt-Review auf dem Branch vor dem Merge, Merge durch den User. Planabschluss mit Versions-Tag (Minor-Version).

**Voraussetzung:** Phase 1 des Plans `2026-10-30_artefakte-dateitypen` (MCP-Zugang, `src-tauri/src/mcp_access/`, ADR 046) ist auf `main` gemergt — dieser Plan startet erst danach. Ist dort der `ToolSet`-Vertrag anders ausgefallen als in dessen README beschrieben, gilt der Code; Abweichungen in `FINDINGS.md` für Phase 1 festhalten und `tools.rs` daran anpassen.

**Nummern:** ADR 048 (Umsetzung, Phase 1), Migration 17 (Phase 1). Belegt bzw. reserviert und nicht zu verwenden: ADR 034, 037, 038 (geparkte Pläne), 045 (Notiz pro Option), 046 und 047 sowie Migration 16 (Artefakte aller Dateitypen).

**Nicht drin:** Kanban, Burn-up, Drag and Drop; Stationen in anderen Betriebsarten als Claude, Claude + LM Studio und Autark (Codex nur, wenn ADR 046 den Zugang dort belegt — sonst ist eine Codex-Station ein Fehler beim Start, Satz „In der Betriebsart Codex gibt es die Werkzeuge des Verwalters nicht.“); Haiku-Kurzfassung des Lageberichts (Backlog); Kopplung an den geparkten Plan `2026-10-22_parallele-sessions` (dessen Leiste wäre bei einer Welle sichtbar und richtig — keine Sonderbehandlung).

## Festgelegte Entscheidungen

- **Eine Umsetzung gehört einem Vorhaben**; je Vorhaben höchstens eine **unfertige** Umsetzung (`finished_at IS NULL`). `implementation_create` bei vorhandener unfertiger → Fehler „Im Vorhaben läuft schon die Umsetzung „<Name>“.“ Fertig ist eine Umsetzung, wenn jede Station fertig ist; dann setzt der Core `finished_at`.
- **Station = Session.** Jede Station, die startet, ist eine frische Session des Vorhabens (immer neu angelegt, nie eine wartende „Neu“-Session übernommen), Name `Phase <key> · <Stufe>` (von Haiku nicht umbenannt: `name_auto = false`). Der **Zustand einer Station ist der Zustand ihrer Session**, nicht gespeichert: ohne Session `pending` (bzw. `countdown`, wenn ein Countdown läuft, `ready`, wenn sie starten darf, aber die Automatik aus ist); mit Session und ohne Fertigmeldung → `starting`/`running` bei `Starting`/`Running`, `asking` bei `Waiting`, `waitsForYou` bei `Completed`/`Paused` (Antwort ohne Fertigmeldung), `error` bei `Error`/`Cancelled`; mit Fertigmeldung `done`. Eine Station `by_user` ist `waitsForYou`, sobald ihre Vorgänger fertig sind, und wird nur per Knopf „Erledigt“ fertig.
- **Werkzeugnamen mit einem Präfix:** Der `ToolSet`-Vertrag aus ADR 046 kennt genau einen Präfix je Satz, deshalb heißen die fünf Werkzeuge `implementation_create`, `implementation_get`, `implementation_set_phases`, `implementation_describe_phase`, `implementation_complete_station` (beim Agenten `mcp__verwalter__implementation_…`). Abweichung vom Entwurf (`phase_describe`, `station_complete`), bewusst.
- **Bindung an die aufrufende Session:** `implementation_complete_station` findet die Station über die Session-ID des Aufrufs (`ToolCall.session_id`); ruft eine Session, die keine Station ist → Fehler „Diese Session ist keine Station.“ `implementation_get`, `implementation_set_phases`, `implementation_describe_phase` wirken auf die Umsetzung, zu der die Session als Station gehört, sonst auf die neueste Umsetzung ihres Vorhabens; ohne → Fehler „In diesem Vorhaben gibt es keine Umsetzung.“ `implementation_create` legt sie im Vorhaben der aufrufenden Session an.
- **Reihenfolge und Wellen:** Phasen stehen in der Reihenfolge des Aufrufs (`position`). Aufeinanderfolgende Phasen mit derselben `wave` bilden eine **Gruppe**; eine Phase ohne `wave` ist eine eigene Gruppe. Gruppen laufen nacheinander; innerhalb einer Phase laufen die Stationen nacheinander. Eine Station **darf starten**, wenn alle Stationen aller vorigen Gruppen und alle vorigen Stationen ihrer Phase fertig sind. Fix-Runde und zweites Review werden hinter der meldenden Station in dieselbe Phase eingefügt und schieben die Positionen dahinter.
- **Start einer Station** (Phase 3): Darf sie starten und `auto_start` ist an und `stop_after_current` aus → Countdown 10 s (Konstante `START_DELAY`, Thread wie `sessions/auto_handoff.rs`, Marke je Station), dann anlegen, Modell setzen, erste Nachricht senden. Automatik aus → Zustand `ready`, Start per ▶ auf der Karte (sofort, ohne Countdown). „Anhalten nach dieser Session“ setzt `stop_after_current`; beim nächsten Fertigmelden setzt der Core `auto_start = false` und `stop_after_current = false` und startet nichts.
- **Erste Nachricht** (Konstante `FIRST_MESSAGE` in `implementation/start.rs`, Platzhalter ersetzt), Zeilen genau so: `/<skill>` (nur mit `skill`, als erste Zeile, weil die Kommandozeile einen Befehl nur am Anfang erkennt) · `Station: Phase <key> „<Titel>“ · <Stufe>` · `Umsetzung: <Name> — Quelle: <source>` (ohne Quelle: `Umsetzung: <Name>`) · `Phase: <description>` (nur mit Beschreibung) · `Auftrag: <brief>` (nur mit Auftrag) · `Vorgänger:` + je fertige Station derselben Umsetzung in Plan-Reihenfolge `- Phase <key> · <Stufe> (#<n>): <Ergebnis>` (nur, wenn es welche gibt) · Leerzeile · `Melde diese Station mit dem Werkzeug implementation_complete_station fertig, mit einer Zeile Ergebnis. Endet deine Antwort ohne diese Meldung, wartet die Umsetzung auf den Benutzer. Gib keine Einstiegszeile aus.`
- **Fertig heißt gemeldet.** Eine Antwort ohne `implementation_complete_station` lässt die Station in `waitsForYou`; nichts startet. Meldet der Agent später (nach einer Nachricht des Users), geht es weiter. **„Automatisch weiter“ (ADR 040) ist für Stationen aus:** `auto_handoff::consider` kehrt sofort zurück, wenn die Session eine Station ist; die Einstiegszeile wird in Stationen weder erkannt noch gezeigt (der `HandoffBar`-Knopf bleibt, startet aber eine reguläre Session).
- **Fehler einer Station** (Session `Error` oder `Cancelled`): Zustand `error`, die Kette hält an, kein Start von selbst. Der User öffnet den Chat; eine spätere Fertigmeldung aus derselben Session setzt die Kette fort. Kein Neustart der Station durch den Verwalter.
- **Fix-Runde:** `implementation_complete_station` mit `fix_needed: true` hängt hinter der meldenden Station zwei Stationen in dieselbe Phase: `Fix-Runde` (Modell und Skill der letzten Station mit Stufe `Umsetzung` dieser Phase, sonst der meldenden) und `<Stufe der meldenden Station>` (gleiches Modell wie die meldende). Höchstens zweimal je Phase; die dritte Meldung mit `fix_needed` wird als fertig ohne Anhang behandelt und liefert dem Agenten den Hinweis „Zwei Fix-Runden sind durch; frag den Benutzer (🔴).“
- **Plan-Änderung** (Phase 4): `implementation_set_phases` bringt die **vollständige Liste der kommenden Phasen** (alle Phasen ohne gestartete Station). Enthält sie den Schlüssel einer begonnenen Phase → Fehler „Phase <key> hat schon begonnen.“ Solange keine Station der Umsetzung je gestartet ist, gilt die Liste sofort. Sonst liegt sie als `pending_change` (JSON in der Spalte) mit Unterschieden: **neu** (Schlüssel bisher unbekannt), **vorgezogen** (Schlüssel bekannt, steht jetzt vor einer Phase, hinter der er vorher stand), **entfallen** (Schlüssel fehlt). „Übernehmen“ ersetzt die kommenden Phasen, markiert neue mit `mark = new`, vorgezogene mit `mark = moved` (die Marke verschwindet beim Start der ersten Station der Phase), schreibt einen Eintrag in `implementation_changes` und löscht `pending_change`; „Verwerfen“ löscht nur `pending_change`. Nummern (`key`) kommen aus dem Aufruf; der Verwalter nummeriert nie um. Ein zweiter Vorschlag ersetzt einen offenen.
- **Rückfragen** (Phase 5): eine reguläre Session im Vorhaben (Status „Neu“, über `create_in_project`), der Lagebericht als Datei `lagebericht-<HHMM>.md` im Zwischenordner der Anhänge (`attachments::add_bytes`) und als Anhang in den Entwurf (`src/stores/attachments.ts`), Fokus ins Eingabefeld. Beim Senden aus einer Rückfrage prüft der Core, ob sich der Lagebericht seit dem letzten Anhang geändert hat (Vergleich des Texts ohne die Zeile „Stand“); dann hängt er den neuen Bericht an und schreibt einen Hinweis in den Verlauf (`Notice`-Art `ReportUpdated`). Rückfragen sind Sessions mit `inquiry_of = <Umsetzungs-ID>` (Spalte an `sessions`, Migration 17).
- **Lagebericht** (programmatisch, `implementation/report.rs`), Zeilen in dieser Reihenfolge: `Lage: Umsetzung <Name> — Stand <HH:MM>` · in einer Rückfrage: `Diese Session ist eine Rückfrage: lesen, erklären, umplanen; keinen Code ändern.` · `Quelle: <source>` · `Ziel: <erste nicht leere Zeile ohne „#“ der README der Quelle, falls `source` ein Ordner mit README.md im Workspace oder einem Repository ist, sonst weggelassen>` · `Stand: Phase <k> von <n> · <fertig> von <gesamt> Sessions fertig` · `Jetzt:` je laufende/fragende Station `Phase <key> · <Stufe> (#<n>) <läuft|fragt dich>:` + eingerückt die erste offene Frage (Text der ersten `Question` der Session) · `Fertig:` je fertige Station `Phase <key> · <Stufe> #<n> — <Ergebnis>` · `Offen:` die kommenden Stationen gruppiert („Phase 2 Review · Phase 3–5 · Abschluss“) · `Plan-Änderungen:` `keine` oder je Eintrag `<Datum HH:MM> aus #<n>: neu <keys>, vorgezogen <keys>, entfallen <keys>` · `Geänderte Dateien: <Zahl der Dateien unter „Alle“ in der Reichweite Vorhaben>`. In Phase 1 ohne „Ziel“, „Plan-Änderungen“ und „Geänderte Dateien“ (kommen in 4 und 5).
- **Sidebar:** Eine Umsetzung ist eine Zeile unter ihrem Vorhaben (Zeilenart `implementation`), vor den Sessions, mit Name, Fortschritt „6/16“ und dem Status-Symbol der dringendsten Station. **Stationen erscheinen nicht als Session-Zeilen** (sie gehören zur Umsetzung; Chat über die Kartei), zählen aber in letzte Aktivität und Ungelesen des Vorhabens. Rückfragen sind normale Session-Zeilen.
- **Ansicht:** Klick auf die Zeile öffnet die Umsetzung in der Hauptfläche (Kopfzeile „<Vorhaben> › Umsetzung <Name>“, Reiter „Ablauf · Changes“; Changes = Reichweite Vorhaben wie in der Übersicht). Zeitleiste links, Kartei rechts, Punkte gleich groß, Linie durchgehend, Pfeiltasten ↑ ↓ blättern — alles nach Entwurf Abschnitt 1, 6 und 7.

## Kontrakt

### Werkzeuge (MCP, `src-tauri/src/implementation/tools.rs`, Satz `ImplementationTools`, Präfix `implementation_`)

| Werkzeug | Eingabe (`inputSchema`) | Ausgabe |
|---|---|---|
| `implementation_create` | `{ name: string (1–80), source?: string (≤ 300), phases: PhaseInput[] (≥ 1) }` | `text` = Lagebericht; `structuredContent` = `ImplementationView` |
| `implementation_get` | `{}` | wie oben |
| `implementation_set_phases` (Phase 4) | `{ phases: PhaseInput[] }` | `text` = „Vorgeschlagen: neu …, vorgezogen …, entfallen …; wartet auf Übernehmen.“ bzw. „Übernommen.“ (noch nicht gestartet); `structuredContent` = `ImplementationView` |
| `implementation_describe_phase` | `{ key: string, title?: string (1–80), description?: string (≤ 500) }` | `text` = „Phase <key> beschrieben.“ |
| `implementation_complete_station` (Phase 3) | `{ result: string (1–200), fix_needed?: boolean }` | `text` = „Station fertig gemeldet. Als Nächstes startet: …“ bzw. „… wartet auf den Benutzer.“ |

```ts
// PhaseInput / StationInput (Eingabe der Werkzeuge, serde Deserialize, camelCase; `model` als Text)
interface PhaseInput { key: string /* 1–12, eindeutig */; title: string /* 1–80 */; description?: string /* ≤ 500 */; wave?: number /* ≥ 1 */; stations: StationInput[] /* ≥ 1 */ }
interface StationInput { stage: string /* 1–40 */; model?: 'fable' | 'opusplan' | 'opus' | 'sonnet' | 'haiku'; by?: 'agent' | 'you'; skill?: string /* ohne „/“, 1–40 */; brief?: string /* ≤ 1000 */ }
```

Fehler der Werkzeuge gehen als `ToolResult::Err(<Satz>)` zurück (beim Agenten `isError: true`), nie als Panik; der Satz nennt das Feld („phases[2].stations: mindestens eine Station“).

### Typen über die Tauri-Grenze (`src-tauri/src/implementation/model.rs`, `derive(TS)`, camelCase)

```rust
pub enum StationState { Pending, Ready, Countdown, Starting, Running, Asking, WaitsForYou, Done, Error }
pub enum PhaseMark { New, Moved }
pub struct StationView {
    pub id: String, pub position: u32, pub stage: String,
    pub model: Option<ModelId>, pub by_user: bool, pub skill: Option<String>, pub brief: String,
    pub session_id: Option<String>, pub session_number: Option<u32>,
    pub state: StationState,
    #[ts(type = "number | null")] pub starts_at: Option<f64>,   // Countdown-Ende, ms
    #[ts(type = "number | null")] pub started_at: Option<f64>,
    #[ts(type = "number | null")] pub completed_at: Option<f64>,
    pub result: Option<String>,
    /// Text der ersten offenen Rückfrage der Session; nur bei `Asking`.
    pub question: Option<String>,
    /// Laufzeit der Session in ms (abgeschlossene Abschnitte + laufender), für „24 min“.
    #[ts(type = "number")] pub running_ms: f64,
}
pub struct PhaseView { pub id: String, pub key: String, pub title: String, pub description: String, pub wave: Option<u32>, pub mark: Option<PhaseMark>, pub stations: Vec<StationView> }
pub struct ChangeSummary { pub added: Vec<String>, pub moved: Vec<String>, pub removed: Vec<String> }  // Schlüssel
pub struct ChangeView { pub id: String, #[ts(type = "number")] pub at: f64, pub session_id: String, pub session_number: u32, pub summary: ChangeSummary, /// Station, nach der die Marke auf der Linie steht; `None` = ganz oben.
    pub after_station_id: Option<String>, pub stations_before: u32, pub stations_after: u32 }
pub struct PendingChange { pub session_id: String, pub session_number: u32, #[ts(type = "number")] pub at: f64, pub summary: ChangeSummary, pub phases: Vec<PhaseInput> }
pub struct ImplementationView {
    pub id: String, pub project_id: String, pub name: String, pub source: String,
    #[ts(type = "number")] pub created_at: f64, #[ts(type = "number | null")] pub finished_at: Option<f64>,
    pub auto_start: bool, pub stop_after_current: bool,
    pub phases: Vec<PhaseView>, pub changes: Vec<ChangeView>, pub pending_change: Option<PendingChange>,
    pub stations_done: u32, pub stations_total: u32,
    /// Phase der ersten nicht fertigen Station (1-basiert in Plan-Reihenfolge); `phases.len()` wenn alles fertig.
    pub current_phase: u32,
    /// Dringendster Zustand über alle Stationen: Asking > WaitsForYou > Error > Countdown > Running/Starting > Ready > Pending > Done.
    pub urgency: StationState,
    #[ts(type = "number")] pub last_activity_at: f64, pub unread: bool,
}
pub struct ImplementationChangedEvent { pub implementation: ImplementationView }
```

`SessionSummary` bekommt zwei Felder (Phase 1): `station_of: Option<String>` (Umsetzungs-ID, wenn die Session eine Station ist) und `inquiry_of: Option<String>` (Umsetzungs-ID einer Rückfrage; Phase 5 füllt sie). Die Oberfläche filtert damit Stationen aus den Session-Zeilen.

### Commands (`src-tauri/src/commands/implementation.rs`, Wrapper `src/lib/implementation.ts`)

| Command | Wrapper | Phase |
|---|---|---|
| `implementation_list() -> Vec<ImplementationView>` (alle nicht archivierten Vorhaben, neueste zuerst) | `listImplementations()` | 1 |
| `implementation_set_auto_start(id, enabled)` | `setImplementationAutoStart` | 3 |
| `implementation_set_stop_after_current(id, enabled)` | `setImplementationStopAfterCurrent` | 3 |
| `implementation_start_station(id, station_id)` (▶: startet sofort; mit laufendem Countdown ebenfalls sofort) | `startStation` | 3 |
| `implementation_cancel_countdown(id, station_id)` | `cancelStationCountdown` | 3 |
| `implementation_complete_station(id, station_id, result)` („Erledigt“ nur für `by_user`) | `completeStationByUser` | 3 |
| `implementation_apply_change(id, accept: bool)` | `applyImplementationChange` | 4 |
| `implementation_report(id) -> String` | `loadImplementationReport` | 5 |
| `implementation_new_inquiry(id) -> SessionSummary` | `createInquiry` | 5 |

Ereignis `implementation://changed` mit `ImplementationChangedEvent` nach jeder Änderung an Aufbau, Texten, Automatik, Countdown und nach jedem Start; Zustandswechsel der Stations-Sessions kommen weiter über `session://changed` — die Oberfläche lädt die Liste neu, wenn `session://changed` eine Session meldet, die als `session_id` in einer Station steht.

### Datenbank (Migration 17, `src-tauri/src/db/migrations/017_implementations.sql`, Zugriffe `src-tauri/src/db/implementations.rs`)

```sql
CREATE TABLE implementations (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  source TEXT NOT NULL DEFAULT '',
  created_by_session_id TEXT,
  created_at REAL NOT NULL,
  finished_at REAL,
  auto_start INTEGER NOT NULL DEFAULT 1,
  stop_after_current INTEGER NOT NULL DEFAULT 0,
  pending_change TEXT
);
CREATE TABLE implementation_phases (
  id TEXT PRIMARY KEY,
  implementation_id TEXT NOT NULL REFERENCES implementations(id) ON DELETE CASCADE,
  position INTEGER NOT NULL,
  key TEXT NOT NULL,
  title TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  wave INTEGER,
  mark TEXT
);
CREATE TABLE implementation_stations (
  id TEXT PRIMARY KEY,
  phase_id TEXT NOT NULL REFERENCES implementation_phases(id) ON DELETE CASCADE,
  position INTEGER NOT NULL,
  stage TEXT NOT NULL,
  model TEXT,
  by_user INTEGER NOT NULL DEFAULT 0,
  skill TEXT,
  brief TEXT NOT NULL DEFAULT '',
  session_id TEXT REFERENCES sessions(id) ON DELETE SET NULL,
  started_at REAL,
  completed_at REAL,
  result TEXT
);
CREATE TABLE implementation_changes (
  id TEXT PRIMARY KEY,
  implementation_id TEXT NOT NULL REFERENCES implementations(id) ON DELETE CASCADE,
  session_id TEXT,
  at REAL NOT NULL,
  summary TEXT NOT NULL,
  after_station_id TEXT,
  stations_before INTEGER NOT NULL,
  stations_after INTEGER NOT NULL
);
CREATE INDEX implementation_stations_session ON implementation_stations(session_id);
ALTER TABLE sessions ADD COLUMN inquiry_of TEXT;
```

`model` als Text wie `ModelId::cli_id()`-Kennung der Session-Tabelle (dieselbe Umwandlung `enum_to_text`/`enum_from_text` aus `db/mod.rs`); `mark` `'new'`/`'moved'`/`NULL`; `summary` und `pending_change` als JSON der Typen oben.

### Oberfläche (Phase 1 legt an, 2–5 füllen)

- Navigation in `src/stores/sessions.ts`: `activeImplementationId: string | null`, `showImplementation: boolean`, `implementationView: 'flow' | 'changes'`, `selectImplementation(id)` (setzt `showImplementation`, löscht `showProjectOverview`, `showSettings`, `showNewSession`), `showImplementationView(view)`.
- Flüchtiges in `src/stores/implementation.ts`: `focused: Record<string, string>` (Umsetzung → Station im Fokus; fehlt → Automatik „die Station, die läuft oder fragt“), `setFocused`, `clearFocused`.
- Feature `src/features/implementation/`: `useImplementations.ts` (Liste + Abos), `ImplementationView.tsx`, `ImplementationHeader.tsx`, `Timeline.tsx`, `StationDot.tsx`, `CardStack.tsx`, `StationCard.tsx`, `PlanChangeCard.tsx` (4), `ChangeMarker.tsx` (4), `ReportPanel.tsx` (1, erweitert 5), `implementationTexts.ts`, `stationState.ts` (Reihenfolge, Farben, Labels), `implementationOrder.ts` (flache Stationsfolge in Plan-Reihenfolge, Gruppen je Welle — reine Funktionen).
- Sidebar: `src/app/buildSidebarRows.ts` bekommt die Zeilenart `implementation` (`{ kind: 'implementation'; key; implementation: ImplementationView }`) direkt nach der Vorhaben-Zeile; `src/app/SidebarImplementation.tsx` rendert sie.

## Finale Abnahmekriterien

- [ ] Eine Planungs-Session im Verwalter legt per `mcp__verwalter__implementation_create` eine Umsetzung an; die Sidebar zeigt sie als Zeile unter dem Vorhaben mit „0/<n>“, die Ansicht zeigt alle Phasen und Stationen grau; der Lagebericht als Text ist über `implementation_get` dieselbe Information.
- [ ] Mit Automatik an startet die erste Station nach 10 s Countdown als frische Session mit der ersten Nachricht nach Kontrakt; „Abbrechen“ hält den Countdown an; ▶ startet sofort.
- [ ] Meldet eine Station per `implementation_complete_station`, startet die nächste Station; meldet sie nicht (Antwort endet mit Frage oder 🔴), zeigt die Karte „wartet auf dich“ und nichts startet; ein Review mit `fix_needed` hängt Fix-Runde und zweites Review an.
- [ ] Zwei Phasen derselben Welle starten zugleich (zwei Linien), die Phase danach erst, wenn beide fertig sind.
- [ ] `implementation_set_phases` aus einer laufenden Umsetzung zeigt die Karte „Plan ändern?“ mit neu/vorgezogen/entfallen; erst „Übernehmen“ ändert die Linie und setzt die Marke „Plan geändert“.
- [ ] „Neue Rückfrage“ legt eine Session mit dem Lagebericht als Anhang an; beim Senden mit geändertem Stand hängt der Verwalter den neuen Bericht an und vermerkt es im Verlauf.
- [ ] Ein im Terminal gestarteter Agent arbeitet mit den Skills wie heute (Einstiegszeile); ein im Verwalter gestarteter nutzt die Werkzeuge und gibt keine Einstiegszeile aus.
- [ ] `pnpm check` grün, Code-Map, Glossar, AGENTS.md (Prüfprogramm), ADR 048 nachgezogen, Versions-Tag gesetzt.

## Wackelstellen (Konfidenz-Ausweis)

- 🟡 **`ToolSet`-Vertrag aus ADR 046:** Dieser Plan setzt auf die README des Plans `artefakte-dateitypen` auf, nicht auf Code — Phase 1 liest zuerst `src-tauri/src/mcp_access/tools.rs` und gleicht `ToolCall`/`ToolResult` ab (Check: `grep -n "pub trait ToolSet\|pub struct ToolCall\|pub enum ToolResult" src-tauri/src/mcp_access/tools.rs`).
- 🟡 **Erste Nachricht beginnt mit `/implement`:** Die Kommandozeile erkennt einen Skill-Aufruf nur am Anfang; ob sie den Rest der Nachricht als Argument an den Skill reicht oder als Folgetext liest, ist im Projekt für mehrzeilige Nachrichten nicht gemessen (Check in Phase 3: eine Station mit `skill: "implement"` starten und im Verlauf prüfen, ob `Station:`-Zeilen im Kontext ankommen; sonst `/implement` ans Ende der Nachricht und `skills::match_invocation` prüfen).
- 🟡 **Sperren:** `implementation_complete_station` läuft im Thread des MCP-Servers und nimmt Datenbank, Vorhaben- und Session-Sperren; Phase 3 hält die Regel aus `registry.rs` ein (nie zwei Session-Sperren, nie `Database::with` unter einer Session-Sperre) und prüft sie im Code-Review gezielt — die Fehlerklasse ist ein Deadlock, der nur unter Last auffällt.

## Summary

## Files touched

## Commits

## Deviations from plan

## Follow-ups
