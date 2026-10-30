# Phase 4 — Wellen (parallele Phasen) und Plan-Änderung mit Karte „Plan ändern?“

Rating **heikel** · Wave 4 · Aufwand L · Status: pending

## Kontext

- [README.md](README.md): Entscheidungen „Reihenfolge und Wellen“, „Plan-Änderung“, „Lagebericht“ (Zeile „Plan-Änderungen“); Kontrakt `implementation_set_phases`, `implementation_apply_change`, Typen `ChangeSummary`, `ChangeView`, `PendingChange`, `PhaseMark`, Tabelle `implementation_changes`, Spalte `pending_change`.
- Entwurf [artifacts/entwurf-umsetzung.html](artifacts/entwurf-umsetzung.html), Abschnitte **2** (zwei Linien, 12 px versetzt, Phasen untereinander, Zusammenlauf vor der nächsten Phase) und **3** (Marke „Plan geändert“ auf der Linie, Karte „Plan geändert“, Karte „Plan ändern?“ mit Übernehmen).
- Phase-1/3-Dateien: `implementation/schedule.rs` (`groups`, `may_start`, `ready_stations`), `implementation/mod.rs` (`complete_station`), `db/implementations.rs` (`replace_upcoming_phases`, `insert_change`, `set_pending_change`), `report.rs`; Oberfläche `Timeline.tsx`, `CardStack.tsx`, `StationCard.tsx`, `implementationOrder.ts`.
- [docs/conventions/rust.md](../../conventions/rust.md), [react.md](../../conventions/react.md). Fehlerklassen (Vault): keine einschlägig.

## Abnahmekriterien

1. Zwei aufeinanderfolgende Phasen mit derselben `wave` starten ihre ersten Stationen zugleich, sobald die Gruppe davor fertig ist; die Phase nach der Welle startet erst, wenn beide fertig sind; innerhalb jeder Phase bleibt es sequenziell. Phasen mit gleicher `wave`, die nicht aufeinanderfolgen, sind getrennte Gruppen.
2. Die Zeitleiste zeigt je Phase einer Welle eine eigene Linie, 12 px versetzt (`timeline__lane--2`, `--3`; ab der vierten Phase einer Welle teilt sie sich die dritte Linie), über der Welle die Zeile „Welle <n> · Phase <a> und <b> gleichzeitig“; die Inhalte stehen untereinander; vor der nächsten Phase laufen die Linien zusammen (ein Segment je Linie zur gemeinsamen Linie).
3. Die Kartei zeigt bei zwei aktiven Spuren vorn die Station, die den Benutzer braucht, die andere aktive direkt darüber; sonst Fokus-Automatik wie Phase 2. Die Kopfzeile ergänzt „2 Spuren aktiv“.
4. `implementation_set_phases` vor dem ersten Start ersetzt die Phasen sofort (`text` „Übernommen.“); während die Umsetzung läuft, legt es `pending_change` ab und `text` nennt die Unterschiede; ein Schlüssel einer begonnenen Phase → `Err`; eine Liste mit doppelten Schlüsseln → `Err`.
5. Die Ansicht zeigt bei `pending_change` die Karte „Plan ändern?“ vorn in der Kartei (vor der Fokus-Automatik): „aus #<n> · <Datum HH:MM>“, Listen „Neu: …“, „Vorgezogen: …“, „Entfallen: …“ („nichts“ bei leer), „Stationen <vorher> → <nachher>“, Knöpfe „Übernehmen“ (Akzent) und „Verwerfen“; nach „Übernehmen“ sind die kommenden Phasen ersetzt, neue tragen die Marke „neu“, vorgezogene „vorgezogen“ an der Phasen-Überschrift bis zum Start ihrer ersten Station, entfallene sind weg; auf der Linie steht an der Stelle (nach der letzten fertigen Station zum Zeitpunkt der Übernahme) die Marke „Plan geändert · aus #<n> · von dir übernommen“, anklickbar wie eine Station; die Kartei zeigt dann die Karte „Plan geändert“ (Entwurf Abschnitt 3) mit Knopf „Chat öffnen“ (Session, die den Vorschlag machte).
6. Ein `implementation_set_phases` aus einer Session, die keine Station ist (Rückfrage, Planungs-Session), wirkt auf die neueste Umsetzung des Vorhabens.
7. Der Lagebericht trägt die Zeile „Plan-Änderungen: …“.
8. `implementation-probe … set-phases <Umsetzungs-ID> <JSON-Datei>` druckt die Unterschiede, ohne zu schreiben.

## Checkliste

- [ ] `schedule.rs`: `groups` nach Entscheidung (aufeinanderfolgende gleiche `wave` → eine Gruppe), `ready_stations` berücksichtigt Gruppen (erste Station jeder Phase einer Gruppe darf starten, wenn alle Stationen aller vorigen Gruppen fertig sind).
- [ ] `implementation/changes.rs`: `pub fn diff(current_upcoming: &[PhaseView], proposed: &[PhaseInput]) -> Result<ChangeSummary, String>` (neu/vorgezogen/entfallen nach Entscheidung; „vorgezogen“ = Schlüssel, der jetzt vor einem Schlüssel steht, hinter dem er vorher stand), `pub fn apply(...)` (Datenbank: `replace_upcoming_phases` ersetzt alle Phasen ohne gestartete Station, behält `id` bekannter Schlüssel, vergibt neue, setzt `mark`; `insert_change` mit `after_station_id` = letzte fertige Station in Plan-Reihenfolge, `stations_before/after`; `set_pending_change(None)`).
- [ ] `tools.rs`: `implementation_set_phases` nach Kontrakt (Prüfung über `model::validate`, Schlüssel begonnener Phasen prüfen, dann sofort anwenden oder `pending_change` schreiben); `commands/implementation.rs`: `implementation_apply_change(id, accept)`; Wrapper `applyImplementationChange`.
- [ ] `mark` löschen beim Start der ersten Station einer Phase (`start::start_now`).
- [ ] `report.rs`: Zeile „Plan-Änderungen“.
- [ ] Oberfläche: `Timeline.tsx` Linien je Welle (Spur = Index der Phase in der Gruppe, `min(index, 2)`), Zusammenlauf-Segment, Zeile „Welle …“, Marken „neu“/„vorgezogen“ an der Phasen-Überschrift (`timeline__phase-mark`), `ChangeMarker.tsx` auf der Linie (Zeile wie eine Station, Symbol ⟳, Klick → Fokus auf `change:<id>`); `CardStack.tsx`: Fokus-Ziel kann Station oder Änderung sein (`focused` im Store als `station:<id>` | `change:<id>`), `PlanChangeCard.tsx` für beide Karten (Vorschlag mit Knöpfen, übernommene ohne); `implementationOrder.ts`: `autoFocus` bevorzugt `pending_change`, dann Spuren-Regel aus AK 3; Kopfzeile „<n> Spuren aktiv“.
- [ ] `implementation-probe.rs`: Aufruf `set-phases` (AK 8, nur `changes::diff`).
- [ ] `pnpm check` grün, Commit `feat(implementation): Wellen und Plan-Änderung mit Übernehmen`.

## Definition of Done

- AK 1–8 erfüllt; **Reviewable Artifact:** Screenshot der Zeitleiste mit zwei Linien (`artifacts/phase-4-welle.png`) und der Karte „Plan ändern?“ (`artifacts/phase-4-plan-aendern.png`), Ausgabe von `implementation-probe set-phases`; **User-visible outcome:** Ein Vorschlag aus einer Session erscheint als Karte, „Übernehmen“ baut den grauen Teil der Linie um.
- Commands run: `pnpm check`, Probe-Aufruf.
- Seven-Goals-Check (je ein Satz); Carry-Overs höchstens 2.

## Report-Back
