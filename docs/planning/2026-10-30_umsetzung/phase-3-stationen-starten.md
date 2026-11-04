# Phase 3 — Stationen starten: Fertigmeldung, Nachfolger, erste Nachricht, Countdown, Fix-Runde, Fehler

Rating **heikel** · Wave 3 · Aufwand L · Status: complete

## Kontext

- [README.md](README.md): Entscheidungen „Station = Session“, „Reihenfolge und Wellen“ (hier nur ohne Wellen: jede Phase eine Gruppe; Wellen aktiviert Phase 4), „Start einer Station“, „Erste Nachricht“, „Fertig heißt gemeldet“, „Fehler einer Station“, „Fix-Runde“; Kontrakt `implementation_complete_station`, Commands der Phase 3.
- `src-tauri/src/sessions/registry/session_tools.rs` (`schedule_inbox_delivery`: eigener Thread am Haken `turn_completed`, Fehler über `log_line`) — das Muster für `implementation/start.rs`, die Marke je Countdown kommt dazu; `sessions/registry.rs`: `create_in_project` (Zeilen ab „let number = latest.number + 1“ — daraus `create_fresh_in_project` ausgliedern), `rename`, `set_model`, `send(app, session_id, text, &[], &[])`, `update` (Stelle `if outbox.turn_completed { session_tools::schedule_inbox_delivery(app, session); }`), `SessionState.pending` (erste offene Rückfrage), `set_status`-Zweige für `Error`.
- `src-tauri/src/sessions/model.rs` (`SessionStatus`), `agents/event.rs` (`ModelId`).
- `src-tauri/src/mcp_access/tools.rs` (`ToolCall.app`, `ToolCall.session_id`), Phase-1-Dateien `implementation/mod.rs`, `tools.rs`, `schedule.rs`, `db/implementations.rs`, `registry/implementation.rs`.
- Oberfläche: `src/features/chat/useSecondsUntil.ts` (neu anlegen: Sekunden der Countdown-Anzeige; die Zeile mit „Abbrechen“ entsteht ebenfalls neu), Phase-2-Dateien `StationCard.tsx`, `ImplementationHeader.tsx`, `src/lib/implementation.ts`.
- Sperrregeln: Kopf von `src-tauri/src/db/mod.rs`, Kommentare an `SessionRegistry` und `update` in `registry.rs` — **die Werkzeuge laufen im Thread des MCP-Servers, nicht im Tauri-Thread.**
- [docs/conventions/rust.md](../../conventions/rust.md). Fehlerklassen (Vault): keine einschlägig.

## Abnahmekriterien

1. Nach `implementation_create` mit `auto_start` startet die erste Station nach 10 s als neue Session des Vorhabens (immer neu, auch wenn eine Session „Neu“ im Vorhaben wartet), Name `Phase <key> · <Stufe>`, Modell aus der Station (ohne Angabe das der jüngsten Session des Vorhabens), erste Nachricht nach Kontrakt; `StationView.session_id` gesetzt, Zustand `starting`/`running`.
2. `implementation_complete_station` aus der Stations-Session setzt `completed_at` und `result`; alle Stationen, die danach starten dürfen, bekommen einen Countdown (Automatik an) oder den Zustand `ready` (Automatik aus); das `text` nennt die nächste Station oder „wartet auf den Benutzer“. Aus einer Nicht-Station → `Err`.
3. `fix_needed: true` aus einer Review-Station hängt „Fix-Runde“ und ein zweites Review nach Entscheidung an und startet die Fix-Runde; die dritte Meldung mit `fix_needed` in derselben Phase hängt nichts an und liefert den Hinweis.
4. Endet die Antwort einer Station ohne Fertigmeldung (Status `Completed`), zeigt die Karte „wartet auf dich“, nichts startet; eine Zeile „Weiter: …“ in der Antwort ist normaler Text (ADR 052) und startet nichts.
5. Eine Station mit Session `Error` zeigt „Fehler“; die Kette hält; eine spätere Fertigmeldung aus derselben Session setzt sie fort.
6. Countdown: Karte zeigt „Startet in <s> s“ mit „Abbrechen“ (→ `ready`, ▶); ▶ startet sofort; Schalter „Automatisch weiter“ und „Anhalten nach dieser Session“ wirken nach Entscheidung; `by_user`-Stationen zeigen „Erledigt“ und blockieren bis dahin.
7. Sind alle Stationen fertig, setzt der Core `finished_at`; die Sidebar-Zeile zeigt den grünen Haken, die Kopfzeile „fertig“.
8. Nach einem App-Neustart läuft kein Countdown weiter (wie ADR 040); Stationen ohne Session zeigen `ready`/`pending`, mit Session den Session-Zustand.
9. `implementation-probe … complete <Umsetzungs-ID> <Station-ID> "<Ergebnis>" [--fix]` wendet die Fertigmeldung auf die Datenbank-Kopie an (ohne Start) und druckt, welche Stationen danach starten dürfen.

## Checkliste

- [x] `registry.rs`: `pub fn create_fresh_in_project(&self, project_id) -> Result<SessionSummary, CommandError>` (der Teil von `create_in_project` ab `latest`; `create_in_project` prüft weiter zuerst auf eine Session „Neu“ und ruft dann die neue Funktion); `pub fn set_name_fixed(&self, app, session_id, name)` = `rename` + `name_auto = false` (prüfen, ob `rename` das schon tut — dann nur benutzen); `SessionState.station_of: Option<String>` beim Start setzen und beim App-Start aus `db::implementations::station_of_session` laden.
- [x] `implementation/start.rs`: `pub fn first_message(view, station) -> String` (Konstante `FIRST_MESSAGE`, Zeilen nach Kontrakt); `pub fn schedule(app, service, implementation_id, station_id)` (Marke erhöhen, `starts_at` merken, Thread `implementation-start` mit `thread::sleep(START_DELAY)`, dann `fire`); `fn fire(...)` prüft Marke, `auto_start`, `stop_after_current`, dann `start_now`; `pub fn start_now(app, implementation_id, station_id) -> Result<(), CommandError>`: Station darf starten (`schedule::may_start`) und ist ohne Session, sonst `Err`; Reihenfolge: `create_fresh_in_project` → `set_name_fixed` → `set_model` (nur mit Angabe) → `db::set_station_session` + `started_at` → `send` → Ereignis. Scheitert `send`, bleibt die Session (Zustand `error` über die Session) und der Fehler steht im Log der Session (`log_line`).
- [x] `implementation/mod.rs`: `complete_station(app, implementation_id, station_id, result, fix_needed, from_tool: bool) -> Result<CompleteOutcome, CommandError>` (`CompleteOutcome { next: Vec<StationRef>, waits_for_user: bool, hint: Option<String> }`): Datenbank-Schreibzugriffe **ohne gehaltene Session-Sperre**, Fix-Runden-Logik nach Entscheidung (`fix_rounds(phase) < 2`), danach `schedule::ready_stations(view)` → je nach Flags `start::schedule` oder Zustand `ready`; `stop_after_current` → Flags zurücksetzen, nichts planen; alles fertig → `finished_at`. Konstante `START_DELAY = Duration::from_secs(10)`.
- [x] `tools.rs`: `implementation_complete_station` nach Kontrakt (Station über `station_of_session(ToolCall.session_id)`), `text` aus `CompleteOutcome`.
- [x] `commands/implementation.rs`: `implementation_set_auto_start`, `implementation_set_stop_after_current`, `implementation_start_station` (laufenden Countdown entwerten, `start_now`), `implementation_cancel_countdown`, `implementation_complete_station` (nur `by_user`, sonst `Err("Diese Station meldet der Agent selbst.")`); registrieren; Wrapper in `src/lib/implementation.ts`.
- [x] `StationView.state` in `registry/implementation.rs` um `Countdown` (Marke im `ImplementationService`, `starts_at`) und `Ready` (`schedule::may_start` und keine Session) erweitern.
- [x] Oberfläche: `StationCard.tsx` — `useSecondsUntil.ts` neu anlegen; Countdown-Zeile mit `useSecondsUntil(startsAt)` und „Abbrechen“, ▶ „Jetzt starten“ bei `ready`, „Erledigt“ bei `by_user`; `ImplementationHeader.tsx` — Schalter und Knopf verdrahten; Fehler als Satz über `commandErrorText` in die Fehlerleiste der Ansicht (eigene Zeile `implementation-view__error`, Muster `SessionActionError`).
- [x] `implementation-probe.rs`: Aufruf `complete` (AK 9), nutzt `db::implementations` + `schedule::ready_stations`.
- [x] `pnpm check` grün, Commit `feat(implementation): Stationen starten, Fertigmeldung und Fix-Runden`.

## Definition of Done

- AK 1–9 erfüllt; **Reviewable Artifact:** (a) Ausgabe von `implementation-probe complete` vor/nach einer Fertigmeldung, (b) Auszug des Session-Logs der automatisch gestarteten Station mit der ersten Nachricht (die ersten 12 Zeilen), (c) Screenshot der Karte mit Countdown (`artifacts/phase-3-countdown.png`); **User-visible outcome:** Nach der Fertigmeldung einer Station erscheint die nächste nach 10 s als laufende Session in der Kartei.
- Commands run: `pnpm check`, Probe-Aufrufe.
- **Gezielt geprüft (Sperren):** jede Stelle in `implementation/mod.rs` und `start.rs`, die `Database::with`, `lock_projects` oder eine Session-Sperre nimmt, trägt einen Kommentar, welche Sperre gerade gehalten wird (Regel aus `db/mod.rs`); keine zwei Session-Sperren zugleich.
- Seven-Goals-Check (je ein Satz); Carry-Overs höchstens 2.

## Report-Back

- **Umgesetzt:** `implementation/start.rs` (erste Nachricht, Countdown, `start_now`), `complete_station`/`plan_completion` in `implementation/mod.rs`, fünf Commands, Werkzeug `implementation_complete_station`, `Countdowns` in der Registry (ersetzt `ImplementationService`), Oberfläche (Schalter, ▶, Abbrechen, Erledigt, Fehlerzeile), Probe `complete`.
- **Abweichungen:** `set_name_fixed` entfällt (`rename` setzt `name_auto = false` schon). `Countdowns` liegt in der `SessionRegistry` statt in einem eigenen Tauri-State, weil `build_view` die Countdown-Enden braucht und die Registry die Ansicht baut. Eine startbare Station ohne Countdown ist immer `ready` (auch bei Automatik an) — so zeigt sie nach einem App-Neustart ▶, statt hängen zu bleiben. Ein Start im Zeitgeber, der scheitert, bevor eine Session existiert, steht nur auf stderr; die Station bleibt `ready`.
- **Belegt:** `pnpm check` grün; Probe `complete`: Test → Umsetzung → Review `--fix` hängt Fix-Runde und zweites Review an, die dritte Meldung mit `--fix` hängt nichts an und liefert den Hinweis.
- **Nicht belegt (Laufzeit in der echten App):** Countdown-Anzeige, Start der Session mit erster Nachricht, ob `/<skill>` vor mehrzeiligem Text den Rest als Argument erhält (Wackelstelle der README), Verhalten bei zwei Stationen zugleich.
