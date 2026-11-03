# Parallele Sessions im selben Ordner

Ziel: Schreiben zwei Sessions mit laufendem Agenten im selben Ordner — im selben Ticket-Worktree oder beide direkt im Haupt-Checkout —, zeigt der Verwalter das an: eine Leiste unter der Kopfzeile, eine Marke an beiden Sessions in der Sidebar, und beim nächsten Senden ein Satz an den Agenten. Auch über Vorhaben hinweg. Sessions auf getrennten Worktrees desselben Repositorys bleiben still — das ist der Normalfall auf dem Arbeitslaptop. Entwurf: [artifacts/entwurf-parallele-sessions.html](artifacts/entwurf-parallele-sessions.html), vom User am 2026-10-22 freigegeben (mit dem Auslöser „schreiben“, nicht „angehängt“).

Kontext für jeden Umsetzer: [AGENTS.md](../../../AGENTS.md), [docs/code-map.md](../../code-map.md) (Zeilen „Sessions“, „Repositories einer Session, Worktrees“, „Changes“, „App-Rahmen“), [docs/glossary.md](../../glossary.md), [docs/conventions/](../../conventions/), [ADR 010](../../decisions/010-worktrees-durch-den-agenten.md) (Ticket-Worktrees), [ADR 014](../../decisions/014-changes-je-session.md) (geschriebene Dateien je Session). Fehlerklassen geprüft (Vault: TypeScript, React): keine einschlägig; für Rust und Tauri gibt es keine Einträge.

| # | Phase | Rating | Status |
|---|---|---|---|
| 1 | Core: Schreib-Ordner je Session, Überschneidungen ermitteln, Command, Satz an den Agenten; ADR 037 | standard | complete |
| 2 | Oberfläche: Leiste, Sidebar-Marke, Abfrage; Doku-Abschluss | standard | complete |

**Rahmen:** strikt 1 → 2, direkt auf `main`, ein Commit pro Phase, Scope `sessions`. `pnpm check` grün vor jedem Commit, `cargo` im PATH, keine neuen Abhängigkeiten, Bindings mitcommitten. Keine automatisierten Tests, keine Smoke-Checkliste. Planabschluss mit Versions-Tag (Minor-Version).

## Festgelegte Entscheidungen

- **Schreib-Ordner einer Session** = (a) jeder Ticket-Worktree, den sie benutzt hat (`SessionState.ticket_worktrees`), als absoluter Pfad; (b) der Haupt-Checkout bzw. Ordner ohne Git eines angehängten Repositorys (`RepositoryCheckout::Main`/`Folder`, `repository_path`), **nur wenn** unter `session_files` dieser Session mindestens ein Pfad darin liegt, der nicht in einem ihrer Ticket-Worktrees liegt. App-Worktrees (`AppWorktree`) zählen nicht — sie gehören je einer Session.
- **Zählt nur bei laufendem Agenten:** Status `Starting`, `Running` oder `Waiting` (wie `busy_in_project`). Ruhende, pausierte, abgeschlossene Sessions zählen nicht.
- **Vergleich:** Pfade über `attribution::normalize_path` (klein, `\`, ohne abschließenden `\`), Gleichheit des ganzen Ordners.
- **Frische per Abfrage:** die Oberfläche fragt alle 5 s und 500 ms nach jedem `session://changed` (entprellt) den Command `session_overlaps` ab. Kein neues Ereignis zwischen Sessions — `session://changed` geht heute nur für die geänderte Session raus.
- **Sperren:** erst jede Session einzeln kurz sperren und Status, Repositories, `ticket_worktrees`, `ticket_roots` kopieren; danach ohne Session-Sperre die Datenbank lesen (`session_files::load_for`); dann rechnen. Nie zwei Sperren zugleich, nie `Database::with` unter einer Session-Sperre (Regel aus `db/mod.rs` und `send`).
- **Satz an den Agenten:** in `send`, vor `update` ermittelt; angehängt an `agent_text` zwischen `review::agent_text` und `message_line`, nur wenn sich der Schlüssel (andere Session-IDs + Ordner, sortiert) seit dem letzten Satz dieser Session geändert hat. Nicht im Chat-Eintrag.
- **Ausblenden** gilt je Session und Schlüssel, flüchtig (Zustand), wie die Kontingent-Leiste.

## Kontrakt

```rust
// src-tauri/src/sessions/model.rs, camelCase, derive(TS)
pub struct SessionOverlap {
    pub session_id: String,
    /// Gemeinsamer Ordner, wie er angezeigt wird (nicht normalisiert).
    pub folder: String,
    pub other_session_id: String,
    pub other_number: u32,
    pub other_name: String,
    pub other_project_name: String,
    /// Weitere Sessions im selben Ordner außer `other_*`.
    pub more: u32,
    /// Schlüssel fürs Ausblenden und für den Satz an den Agenten.
    pub key: String,
}
```

Command `session_overlaps() -> Result<Vec<SessionOverlap>, CommandError>` (ein Eintrag je betroffener Session; jede Session höchstens einmal, mit dem ersten gemeinsamen Ordner nach Pfad sortiert). Wrapper `loadSessionOverlaps(): Promise<SessionOverlap[]>` in `src/lib/sessions.ts`.

Satz an den Agenten (Konstante `OVERLAP_HINT` in `registry.rs`, `{folder}`/`{name}` ersetzt):

```text
Hinweis des Verwalters: Eine andere Session arbeitet gerade im selben Ordner ({folder}, Session „{name}“). Ihre uncommitteten Änderungen dort sind nicht deine.
```

## Phase 1 — Core

Lesen: `src-tauri/src/sessions/registry.rs` (`busy_in_project`, `summarize`, `send` Zeilen um `carried`/`agent_text`/`message_line`, `SessionState.ticket_worktrees`/`ticket_roots`), `src-tauri/src/worktrees/mod.rs` (`TicketRoot`, `ticket_roots`, `SessionRepository`, `RepositoryCheckout`), `src-tauri/src/db/session_files.rs` (`load_for`), `src-tauri/src/changes/attribution.rs` (`normalize_path`), [docs/conventions/rust.md](../../conventions/rust.md).

AK:
1. `session_overlaps` liefert für zwei laufende Sessions mit gemeinsamem Ticket-Worktree je einen Eintrag (gegenseitig), ebenso für zwei laufende Sessions, die beide im selben Haupt-Checkout Dateien geschrieben haben.
2. Zwei laufende Sessions mit demselben angehängten Repository, die nur in verschiedenen Ticket-Worktrees schreiben, liefern nichts.
3. `send` hängt den Satz bei neuer Lage genau einmal an; bei gleicher Lage nicht erneut; der Chat-Eintrag bleibt ohne Satz.

- [x] `sessions/model.rs`: `SessionOverlap` nach Kontrakt; in `src-tauri/examples/gen-bindings.rs` exportieren.
- [x] Neues Modul `src-tauri/src/sessions/registry/overlaps.rs` (Muster wie `registry/artifacts.rs`), Modul-Kommentar „Sessions, die gerade im selben Ordner schreiben (ADR 037).“:
  - `struct WriterSnapshot { id, number, name, project_name, repositories: Vec<SessionRepository>, ticket_folders: Vec<PathBuf> }`.
  - `fn ticket_folder_path(repositories: &[SessionRepository], roots: &[TicketRoot], position: u32, folder: &str) -> Option<PathBuf>`: Repository an `position`; passende Wurzel = `roots` mit gleicher `position`, deren `prefix` den kleingeschriebenen `folder` beginnt; `inner == None` → `repository_path.parent()?.join(folder)`, sonst `repository_path.join(folder)`.
  - `impl SessionRegistry { pub fn overlaps(&self) -> Result<Vec<SessionOverlap>, CommandError> }`: Schritt 1 Schnappschüsse (Sperre je Session einzeln, nur Sessions im Status `Starting | Running | Waiting`; Name des Vorhabens wie in `busy_in_project`/`summarize` beschafft); Schritt 2 `self.database.with(|c| session_files::load_for(c, &[id]))` je Session (Funktion gibt die Pfade einer Session; ist `load_for` nur über mehrere Sessions zusammengefasst, eine neue Funktion `paths_of(connection, session_id) -> Result<Vec<String>, CommandError>` in `db/session_files.rs` ergänzen); Schritt 3 Schreib-Ordner nach Entscheidung, normalisiert; Schritt 4 Paare suchen, je Session den ersten Ordner (nach Pfad sortiert) mit `other_*` = erste andere Session (nach `number`), `more` = Rest; `key` = `folder|` + sortierte IDs aller Sessions dieses Ordners, mit `,` verbunden.
  - `pub fn overlap_of(&self, session_id: &str) -> Option<SessionOverlap>` (filtert `overlaps()`; Fehler → `None`).
- [x] `commands/sessions.rs`: `session_overlaps(registry: State<SessionRegistry>)`; in `lib.rs` registrieren.
- [x] `registry.rs`: Feld `SessionState.last_overlap_key: Option<String>`. In `send` **vor** `update` `let overlap = self.overlap_of(session_id);`; in der Closure nach `review::agent_text`: ist `overlap` gesetzt und `Some(key) != state.last_overlap_key`, Satz anhängen (`\n\n` davor) und Schlüssel merken; ist `overlap` `None`, `last_overlap_key = None`.
- [x] `docs/decisions/037-parallele-sessions.md`: Optionen (a) Warnung, sobald zwei Sessions dasselbe Repository angehängt haben — verworfen, meldet den Normalfall mehrerer Worktrees dauernd; (b) Schreib-Ordner aus Ticket-Worktrees und geschriebenen Dateien — gewählt; (c) Sperre statt Warnung — verworfen, der Verwalter hält keinen Agenten an. Konsequenzen: Abfrage alle 5 s aus der Oberfläche; Dateien, die der Agent per Shell schreibt, zählen nur, soweit `session_files` sie kennt.
- [x] `pnpm check` grün, Commit `feat(sessions): Sessions im selben Schreib-Ordner erkennen`.

## Phase 2 — Oberfläche

Lesen: Entwurf (Leiste, Sidebar-Marke), `src/features/usage/UsageAlertBar.tsx` + `.css` (Muster der Leiste), `src/stores/usage.ts` (Muster Ausblenden), `src/app/App.tsx` (Stelle nach `UsageAlertBar` in `renderMain`), `src/app/SidebarItem.tsx` (Marke nach `sidebar-item__clock`), `src/stores/sessions.ts`, [docs/conventions/react.md](../../conventions/react.md), [tailwind.md](../../conventions/tailwind.md).

AK:
1. Unter der Kopfzeile einer betroffenen Session steht `OverlapBar` (`role="status"`, Hintergrund `waiting-subtle`, Symbol zwei versetzte Rechtecke in `--color-waiting`): fett „Gleicher Ordner:“, dann „#<n> „<Name>“ im Vorhaben „<Vorhaben>“ arbeitet gerade auch in <Ordner in Mono>.“, bei `more > 0` „ und <more> weitere“ vor dem Punkt; Knöpfe „Zur Session“ (öffnet die andere Session) und „Ausblenden“.
2. „Ausblenden“ blendet für diese Session und diesen `key` aus; ändert sich der `key`, erscheint die Leiste wieder.
3. In der Sidebar trägt jede betroffene Session nach Uhr/vor Ungelesen-Punkt das Symbol (14 px, `--color-waiting`, `role="img"`, `aria-label`/`title` „Arbeitet im selben Ordner wie eine andere Session“), unabhängig vom Ausblenden.

- [x] `src/stores/sessions.ts` (oder neuer Slice `src/stores/overlaps.ts`, Feature-Name `overlaps`): `overlaps: Record<string, SessionOverlap>`, `dismissed: Record<string, string>` (Session → Schlüssel), Aktionen `setOverlaps(list)`, `dismissOverlap(sessionId, key)`.
- [x] Hook `src/features/sessions/useSessionOverlaps.ts`, einmal in `App.tsx`: Intervall 5 s, dazu Abo auf `session://changed` mit 500 ms Entprellung; Fehler still verwerfen (nächster Lauf).
- [x] `src/features/sessions/OverlapBar.tsx` + `.css` (BEM `overlap-bar__*`), Symbol als `src/components/TwinFoldersIcon.tsx`; in `App.tsx` nach `UsageAlertBar`.
- [x] `SidebarItem.tsx`: Marke nach AK 3 (Klasse `sidebar-item__overlap`).
- [x] Doc: `docs/code-map.md` neue Zeile „Parallele Sessions“ (Oberfläche `OverlapBar`, `useSessionOverlaps`, Store; Core `sessions/registry/overlaps.rs`, `session_overlaps`, ADR 037); `docs/glossary.md` neuer Eintrag **Schreib-Ordner**: „Ordner, in dem eine Session tatsächlich schreibt: ihre benutzten Ticket-Worktrees und der Haupt-Checkout, wenn sie dort Dateien geändert hat (ADR 037).“
- [x] Plan-Abschluss: Bottom-Sektionen, Archiv (ganzer Plan-Ordner), STATE.md, Versions-Tag.
- [x] `pnpm check` grün, Commit `feat(sessions): Leiste und Sidebar-Marke bei gleichem Schreib-Ordner`.

## Wackelstellen (nur Plan, kein Prüfauftrag)

- 🟡 `session_files` kennt nur Dateien aus `Edit`/`Write`/`MultiEdit`/`NotebookEdit` und Pfade in Shell-Befehlen (`used_paths`). Schreibt ein Agent nur per Skript, erkennt der Verwalter den Haupt-Checkout als Schreib-Ordner erst, wenn ein Pfad im Befehl steht.
- 🟡 Relative Pfade in `session_files` (selten, wenn der Agent `file_path` relativ angibt) liegen in keinem Ordner und zählen nicht.

## Summary

Schreiben zwei laufende Sessions im selben Ordner, zeigt der Verwalter eine Leiste unter der Kopfzeile und eine Marke an beiden Sessions in der Sidebar; der nächste gesendete Text trägt einen Hinweis an den Agenten. Sessions auf getrennten Ticket-Worktrees bleiben still. Entscheidung: [ADR 037](../../decisions/037-parallele-sessions.md).

## Files touched

- Core: `src-tauri/src/sessions/registry/overlaps.rs`, `registry.rs`, `sessions/model.rs`, `db/session_files.rs`, `commands/sessions.rs`, `lib.rs`, `examples/gen-bindings.rs`
- Oberfläche: `src/features/sessions/OverlapBar.tsx` + `.css`, `useSessionOverlaps.ts`, `src/stores/overlaps.ts`, `src/components/TwinFoldersIcon.tsx`, `src/app/SidebarItem.tsx` + `.css`, `src/app/App.tsx`, `src/lib/sessions.ts`, `src/styles/theme.css`
- Doku: `docs/decisions/037-parallele-sessions.md`, `docs/code-map.md`, `docs/glossary.md`

## Commits

- `c1278ab` feat(sessions): Sessions im selben Schreib-Ordner erkennen
- `5e2bf4c` feat(sessions): Leiste und Sidebar-Marke bei gleichem Schreib-Ordner

## Deviations from plan

- Neues Farb-Token `--color-status-waiting-subtle` in `theme.css` (hell und beide Dunkel-Blöcke): der Plan nannte `waiting-subtle`, das Token gab es nicht.
- Store als eigener Slice `src/stores/overlaps.ts` statt in `sessions.ts`.

## Follow-ups

- Die Leiste hat keinen Live-Lauf mit zwei echten Sessions hinter sich; geprüft sind Typen, Lint, Format, Build und Clippy.
