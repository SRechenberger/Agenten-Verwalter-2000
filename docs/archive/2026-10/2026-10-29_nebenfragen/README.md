# Nebenfragen (`/btw`)

Ziel: Der Nutzer stellt eine Frage zur laufenden Session, die Claude aus dem bisherigen Verlauf beantwortet, **ohne dass Frage oder Antwort in den Chat-Verlauf der Session wandern** und ohne den laufenden Agenten zu stören. Die Antwort erscheint in einem Fenster über der Eingabeleiste und wird zusätzlich in einem vierten Reiter „Nebenfragen“ des Hintergrund-Panels gesammelt. Vier Einstiege: `/btw` im `/`-Menü, Zeile im `+`-Menü, `Strg+;`, Text im Chat markieren → Rechtsklick. Vorbild: `/btw` bzw. „Side chat“ in Claude Code (Doku: [Interactive mode → Side questions](https://code.claude.com/docs/en/interactive-mode), [Desktop](https://code.claude.com/docs/en/desktop)). Entwurf: [artifacts/entwurf-nebenfragen.html](artifacts/entwurf-nebenfragen.html) (Abschnitt A für das Fenster, Abschnitt F für Einstiege und Reiter), vom User am 2026-10-29 freigegeben.

Kontext für jeden Umsetzer: [AGENTS.md](../../../AGENTS.md), [docs/code-map.md](../../code-map.md), [docs/glossary.md](../../glossary.md), [docs/conventions/](../../conventions/) (Rust, TypeScript, React, Tailwind, Linting), [ADR 003](../../decisions/003-claude-anbindung.md) (Claude-Anbindung), [ADR 033](../../decisions/033-warnhinweise.md) (Prompt-Cache), [docs/knowledge/claude-stream-json.md](../../knowledge/claude-stream-json.md) (Abschnitt Einmal-Aufruf im Druckmodus, `--resume`). Fehlerklassen geprüft: im Vault liefert ein Glob über `sprachen`, `frameworks`, `werkzeuge` keine einschlägigen Dateien für Rust/Tauri/React (Lücke, kein Negativbefund) — keine Fehlerklasse belegt, keine geprüft.

| # | Phase | Rating | Wave | Status |
|---|---|---|---|---|
| 1 | Core: Einmal-Aufruf als Fork, Zustand, Befehle, Prüfprogramm `side-probe`, ADR 044 | heikel | 1 | complete |
| 2 | Oberfläche: Fenster, Reiter, `/btw`, `+`-Menü, `Strg+;`, Doku | standard | 2 | complete |
| 3 | Markieren → Rechtsklick → „Nebenfrage zur Auswahl“ | standard | 3 | complete |

**Rahmen:** strikt 1 → 2 → 3, keine Parallelspur (drei Phasen; Phase 2 braucht die erzeugten Bindings aus Phase 1, Phase 3 den Store aus Phase 2). Direkt auf `main`, ein Commit pro Phase, Scope `side`. `pnpm check` grün vor jedem Commit, Prüfkette auf diesem Rechner siehe [STATE.md](../../../STATE.md) („Nächster Schritt“), keine neuen Abhängigkeiten, Bindings mitcommitten. Keine automatisierten Tests (Projektregel, [linting.md](../../conventions/linting.md)), keine Smoke-Checkliste; geprüft wird mit dem Prüfprogramm (Phase 1) und in der laufenden App (Phase 2, 3). Planabschluss mit Versions-Tag (Minor-Version). **Reihenfolge zu anderen Plänen:** unabhängig von `unveraenderte-zeilen`, `parallele-sessions`, `umgebung` (andere Dateien); `parallele-sessions` und `umgebung` fassen `send` in `registry.rs` an, dieser Plan nicht — der neue Code liegt in `sessions/registry/side.rs`.

## Festgelegte Entscheidungen

- **Nebenfrage = eigener Einmal-Aufruf auf einem Fork der Session.** `claude -p --resume <Session-ID> --fork-session --no-session-persistence` im Arbeitsordner der Session (`session.workspace`), Ausgabe `--output-format json`. Der Hauptprozess der Session wird nicht berührt, ihr Verlauf bekommt keinen Eintrag. Die Session-ID ist die, die der Verwalter selbst vergibt (`--session-id <session.id>`, `build_command` in `agents/claude/process.rs`).
- **Nur im Speicher.** Keine Datenbank, keine Migration. Die Nebenfragen leben im `SessionState` und sind nach dem Beenden der App weg (wie bei Claude: der Side Chat wird nicht gespeichert). Höchstens **20** je Session, die 21. verdrängt die älteste. Höchstens **3** laufen gleichzeitig je Session, die 4. wird abgelehnt.
- **Keine Werkzeuge für die Nebenfrage, aber dieselbe Werkzeugliste wie der Hauptprozess** (gemessen in Phase 1, [ADR 044](../../decisions/044-nebenfragen.md)): Werkzeugliste, Systemprompt-Zusatz und Modell wie der Hauptprozess (`common_args` in `agents/claude/process.rs`), Werkzeuge nur durch `--permission-mode dontAsk` gesperrt. So liest schon die erste Nebenfrage aus dem Prompt-Cache der Session (0,045 $ statt 0,92 $ bei ~114k Token Verlauf auf Opus). Die vorab festgelegte Regel „zweite Frage entscheidet, bei Gleichstand `bare`“ ist nach der Messung verworfen (User, 2026-10-29); eine Variante `bare` gibt es im Code nicht.
- **Frage im Prompt, nicht im Systemprompt.** Ein Zitat und die Anweisung „Beantworte nur aus dem bisherigen Verlauf, knapp, ohne Werkzeuge“ stehen im Text der Frage, damit der Anfang des Prompts (und damit der Cache) unverändert bleibt.
- **Betriebsarten (Version 1):** Claude und Claude + LM Studio (beide starten die Claude-Kommandozeile). **Autark und Codex nicht:** `side_ask` liefert dort `Internal("Nebenfragen gibt es in dieser Betriebsart noch nicht.")`; die Oberfläche zeigt denselben Satz als Hinweis, wenn man den Einstieg wählt (Muster `LOCAL_MODEL_PICK_HINT` in `Composer.tsx`). Zeitlimit: **120 s** in Claude, **300 s** mit LM Studio (das erste Token kam beim Fortsetzen einer Session mit dem lokalen Modell nach 224 s, [claude-stream-json.md](../../knowledge/claude-stream-json.md)).
- **Anzeige:** Fenster über der Eingabeleiste (Entwurf A) mit Frage, Antwort (als Markdown), Pille für den Cache, „Kopieren“, Schließen mit `Esc`; dazu der Reiter „Nebenfragen“ im Hintergrund-Panel (Liste, Klick öffnet die Antwort wieder im Fenster, „Entfernen“ je Zeile).
- **Nicht in Version 1:** „Als Fork weiterführen“ (im Entwurf E, nicht freigegeben), Nachfragen im selben Fenster, Speichern in der Datenbank, Nebenfragen für Autark und Codex.
- **Namen:** Feature `side` (Oberfläche `src/features/side/`, Store `src/stores/side.ts`, Wrapper `src/lib/side.ts`, Core `src-tauri/src/side/`, Befehle `src-tauri/src/commands/side.rs`, Registry `src-tauri/src/sessions/registry/side.rs`, Claude-Aufruf `src-tauri/src/agents/claude/side.rs`); Wort in der Oberfläche: **Nebenfrage**. Kein `db/side.rs`.
- **ADR:** 044 (034, 037, 038, 043 sind in anderen Plänen reserviert; 042 ist die höchste vergebene).

## Kontrakt

```rust
// src-tauri/src/side/model.rs — camelCase, derive(TS)
pub enum SideState { Running, Done, Failed }

pub struct SideQuestion {
    pub id: String,                 // UUID, vom Core vergeben
    pub session_id: String,
    pub question: String,
    pub quote: Option<String>,      // markierter Text aus dem Chat, höchstens 1000 Zeichen
    pub state: SideState,
    pub answer: Option<String>,     // gesetzt bei Done
    pub error: Option<String>,      // gesetzt bei Failed
    pub asked_at: f64,              // Millisekunden seit 1970 (wie `now_ms`, TS `number`)
    pub answered_at: Option<f64>,
    pub usage: Option<SideUsage>,   // aus der Antwort der Kommandozeile
}

pub struct SideUsage { pub input_tokens: u64, pub cache_read_tokens: u64 }  // input_tokens = alle Eingabe-Token samt Cache; TS `number`
pub struct SideChangedEvent { pub session_id: String }
```

Befehle (`src-tauri/src/commands/side.rs`, `async`, Muster `commands/tldr.rs`):

```rust
side_ask(app, registry, session_id: String, question: String, quote: Option<String>) -> Result<String, CommandError>  // liefert die ID, kehrt sofort zurück
side_load(registry, session_id: String) -> Result<Vec<SideQuestion>, CommandError>                                       // neueste zuletzt
side_dismiss(registry, session_id: String, id: String) -> Result<(), CommandError>
```

Ereignis `side://changed`, Nutzlast `{ sessionId: string }` (Muster `tldr://changed`, `emit_tldr_changed`); die Oberfläche lädt dann mit `side_load` neu. Fehler: leere Frage (nach `trim`) → `Internal("Die Frage ist leer.")`; vierte gleichzeitige → `Internal("Es laufen schon drei Nebenfragen. Warte auf eine Antwort.")`; Session unbekannt → wie die anderen Befehle der Registry; Session ohne Transkript der Kommandozeile → `Internal("Diese Session hat noch keinen Verlauf.")`. `side_dismiss` meldet kein Ereignis, die Oberfläche lädt selbst neu. Wrapper `src/lib/side.ts`: `askSide(sessionId, question, quote)`, `loadSide(sessionId)`, `dismissSide(sessionId, id)`, `subscribeSide(handler)`.

Store (`src/stores/side.ts`, Phase 2; Phase 3 ruft ihn nur auf):

```ts
interface SideState {
  /** Das offene Fenster je Session; fehlt der Schlüssel, ist es zu. */
  windows: Record<string, { questionId: string | null; quote: string | null }>;
  open: (sessionId: string, options?: { questionId?: string; quote?: string }) => void; // questionId null = leeres Eingabefeld
  close: (sessionId: string) => void;
}
```

## Finale AK (fürs Ganze)

1. `/btw` im `/`-Menü, die Zeile „Nebenfrage stellen …“ im `+`-Menü, `Strg+;` und „Nebenfrage zur Auswahl …“ im Rechtsklick-Menü öffnen dasselbe Fenster.
2. Eine gestellte Nebenfrage erscheint **nicht** im Chat-Verlauf der Session und nicht im Verlauf nach einem Neustart der App; der laufende Zug des Agenten läuft ungestört weiter.
3. Jede Nebenfrage steht im Reiter „Nebenfragen“ der Session mit Zähler; Klick öffnet die Antwort, „Entfernen“ löscht sie.
4. In Autark und Codex sagt jeder Einstieg in einem Satz, dass es dort noch nicht geht.
5. Die Kosten-Aussage der Oberfläche (Cache-Pille bzw. fester Hinweis) stimmt mit der Messung aus ADR 044 überein.
6. `pnpm check` grün; Code-Map, Glossar, AGENTS.md und ADR 044 nachgezogen.

## Risiken

- **Cache entwertet.** Siehe Entscheidung oben; Phase 1 misst, bevor die Oberfläche gebaut wird.
- **`--resume` auf eine Session, die gerade geschrieben wird.** Der Fork liest die Transkript-Datei, während der Hauptprozess anhängt. Gegenmaßnahme: Phase 1 prüft das ausdrücklich während eines laufenden Zugs; die letzte, noch nicht geschriebene Antwort fehlt dem Fork (wie beim Original).
- ~~**Fork-Dateien.**~~ Erledigt in Phase 1: ohne Schalter hinterlässt jeder Fork eine Datei, mit `--no-session-persistence` keine.
- **`Strg+;` auf deutscher Tastatur.** Das Semikolon liegt auf Umschalt+Komma, `event.shiftKey` ist dort wahr. Die Abfrage darf `shift` deshalb nicht ausschließen.
- **Kosten bei vielen Nebenfragen.** Jede liest den ganzen Verlauf; bei kaltem Cache voller Preis. Gegenmaßnahme: Pille zeigt den Zustand vor dem Senden.

## Konfidenz

Unsicherste Stellen: (1) trägt der Prompt-Cache bei einem Fork mit `--tools ""`? — Check: `side-probe` zweimal hintereinander, `cache_read_input_tokens` ablesen (Phase 1, AK 3); (2) geht `--resume --fork-session` auf eine Session, die der Hauptprozess gerade beschreibt? — Check: Probe während eines laufenden Zugs (Phase 1, AK 4); (3) hinterlässt der Fork Dateien, und liefert `--output-format json` im Druckmodus `usage` mit den Cache-Zahlen? — Check: Probe-Ausgabe und Blick in den Projektordner (Phase 1, AK 5). Billige Vorprüfung hier gelaufen: `claude --help` führt `--fork-session`, `--resume`, `--tools`, `--no-session-persistence`, `--output-format` (belegt).

## Bottom-Sektionen (beim Archivieren füllen)

### Summary

Nebenfragen sind umgesetzt: eine Frage zur laufenden Session läuft als Einmal-Aufruf auf einem Fork (gleiche Werkzeugliste wie der Hauptprozess, daher Prompt-Cache nutzbar, Messung in ADR 044), erscheint nicht im Chat-Verlauf und liegt nur im Speicher. Vier Einstiege öffnen dasselbe Fenster: `/btw`, `+`-Menü, `Strg+;`, Rechtsklick auf markierten Text. Der Reiter „Nebenfragen“ sammelt sie je Session. In Autark und Codex sagt jeder Einstieg, dass es dort noch nicht geht.

### Files touched

Core `src-tauri/src/side/`, `sessions/registry/side.rs`, `agents/claude/side.rs`, `commands/side.rs`, Prüfprogramm `side-probe`; Oberfläche `src/features/side/` (Fenster, Reiter, `openSide`, `SelectionMenu`, `selectedChatText`), `src/stores/side.ts`, `src/lib/side.ts`; Anschluss in `Composer`, `commandMenuRows`, `BackgroundPanel`, `App`, `ChatTimeline`, `composerDraft`; Doku `docs/code-map.md`, ADR 044.

### Commits

`5632fbe` Core · `bc68a37` Oberfläche · `551cc66` Rechtsklick-Menü (jeweils Scope `side`).

### Deviations from plan

- Die vorab festgelegte Regel „zweite Frage entscheidet, bei Gleichstand `bare`“ entfiel nach der Messung in Phase 1; es gibt nur eine Variante ([ADR 044](../../decisions/044-nebenfragen.md)).
- Phase 3: Die Zeile „Nebenfrage zur Auswahl …“ trägt kein `Strg ;`, weil `Strg+;` das Fenster ohne Zitat öffnet.
- Phase 3 wurde auf Wunsch des Users ohne Selbstprüfung der AK 1–7 in der laufenden App committet.

### Follow-ups

- Selbstprüfung der Oberfläche in der echten App (Phase 2 und AK 1–7 aus Phase 3), vor allem: bleibt die Auswahl erhalten, wenn die erste Menüzeile den Fokus bekommt.
- Nicht in Version 1: „Als Fork weiterführen“, Nachfragen im selben Fenster, Speichern in der Datenbank, Nebenfragen für Autark und Codex.
