# Phase 1 — Core (heikel, Wave 1) — complete

Ergebnis: ein Tauri-Befehl `side_ask`, der eine Nebenfrage als Fork der Session beantwortet, plus Zustand, Ereignis und das Prüfprogramm `side-probe`, das die Cache-Frage **misst**. Kontrakt (Typen, Befehle, Ereignis, Grenzen) und Entscheidungen stehen in der [README](README.md) — dort nachlesen, hier nicht wiederholt.

## Kontext (lesen)

- `src-tauri/src/agents/claude/print.rs` (`print_command`, `exchange`, `start_reader`, `start_writer`, `run_print`, `print_program`): das Muster für einen kurzlebigen Aufruf. Die Hilfsfunktionen `exchange`/`start_reader`/`start_writer` sind privat und werden `pub(super)`, damit `side.rs` sie nutzt.
- `src-tauri/src/agents/claude/process.rs` (`build_command`, `SpawnOptions`): Optionen des Hauptprozesses; Session-ID per `--session-id <id>`, `cwd` = `session.workspace`.
- `src-tauri/src/sessions/registry.rs` um Zeile 3690–3790 (Programm-Wahl `find_claude`, `locate::has_transcript`, `is_standalone`, `backend`, Betriebsart) und `src-tauri/src/sessions/registry/tldr.rs` (Muster: eigener Thread, Ereignis `tldr://changed`, `emit_tldr_changed`, Laufzustand je Session).
- `src-tauri/src/commands/tldr.rs` (Muster der Befehle), `src-tauri/src/lib.rs` (Eintragen der Befehle), `src-tauri/examples/gen-bindings.rs` (Export-Liste), `src-tauri/examples/changes-probe.rs` und `git-probe.rs` (Muster eines Prüfprogramms: Kopie der Datenbank als Argument).
- `src-tauri/src/agents/claude/locate.rs` (`find_claude`, `has_transcript`, Pfad der Transkript-Datei), `src-tauri/src/settings/model.rs` (`OperatingMode`), `src-tauri/src/agents/claude/local.rs` (`LocalBackend`, `LocalProgram`, `apply`).
- [docs/conventions/rust.md](../../conventions/rust.md), [docs/knowledge/claude-stream-json.md](../../knowledge/claude-stream-json.md).

## Neue und geänderte Dateien

- `src-tauri/src/side/model.rs` — `SideState`, `SideQuestion`, `SideUsage` nach Kontrakt (`#[derive(Serialize, Clone, TS)]`, `rename_all = "camelCase"`, wie die Typen in `tldr/model.rs`).
- `src-tauri/src/side/mod.rs` — Konstanten `MAX_QUESTIONS = 20`, `MAX_RUNNING = 3`, `TIMEOUT_CLAUDE = 120 s`, `TIMEOUT_LOCAL = 300 s`, `MAX_QUOTE_CHARS = 1000`; `fn prompt(question: &str, quote: Option<&str>) -> String` baut den Text: optional `Bezug (Auszug aus deiner Antwort):\n> <Zitat, Zeilen mit "> " eingerückt>\n\n`, dann die Frage, dann die Zeile `Beantworte das nur aus dem bisherigen Verlauf, knapp und ohne Werkzeuge.`; `fn clip_quote(&str) -> String` kürzt auf `MAX_QUOTE_CHARS` Zeichen (an Zeichen-, nicht Byte-Grenze) und hängt `…` an.
- `src-tauri/src/agents/claude/side.rs` — `pub enum SideVariant { Bare, Same }`; `pub struct SideRequest { exe, leading_args, cwd, session_id, model, local: Option<LocalBackend>, variant, prompt, timeout }`; `pub struct SideAnswer { text, input_tokens, cache_read_tokens }`; `pub fn run_side(request: &SideRequest) -> Result<SideAnswer, String>`. Argumente: immer `-p --model <…> --resume <session_id> --fork-session --output-format json`. `Bare` zusätzlich `--tools "" --safe-mode --strict-mcp-config`. `Same` zusätzlich `--permission-mode dontAsk` und die Argumente des Hauptprozesses, die Systemprompt und Werkzeugliste bestimmen (`--append-system-prompt`, `--add-dir`, `--mcp-config`, Chrome-Schalter): dafür die gemeinsamen Argumente aus `build_command` in eine Funktion `common_args(command, opts)` ziehen, die `build_command` und `side_command` beide aufrufen (Verhalten von `build_command` darf sich nicht ändern; Diff gegen die Argumentliste vorher/nachher prüfen). Kein `--allowedTools` in `Same`. Ausgabe lesen: JSON mit `result` (Text), `is_error`, `usage.input_tokens`, `usage.cache_read_input_tokens`; `is_error: true` → `Err(result)`. Prozess immer beenden (`kill` + `wait`) wie `run_print`. `hide_console` setzen.
- `src-tauri/src/sessions/registry/side.rs` — `impl SessionRegistry { pub fn ask_side(&self, app, session_id, question, quote) -> Result<String, CommandError>; pub fn side_view(&self, session_id) -> Result<Vec<SideQuestion>, CommandError>; pub fn dismiss_side(&self, session_id, id) -> Result<(), CommandError> }`. Zustand: neues Feld `side: Vec<SideQuestion>` in `SessionState` (nicht persistiert, nicht in `SessionSummary`). `ask_side`: Betriebsart prüfen (Autark, Codex → Fehlersatz aus README), Frage `trim` (leer → Fehler), Zähler der laufenden prüfen, Eintrag `Running` anlegen, bei mehr als 20 den ältesten entfernen, Programm bestimmen wie im Block um Zeile 3690 (`find_claude`, `backend`), Thread starten (`thread::Builder`, Name `side-ask`), dort `run_side`, Ergebnis in den Eintrag schreiben (`Done` mit `answer`, `usage`, `answered_at` bzw. `Failed` mit `error`), danach `side://changed` mit `{ sessionId }`. Beginn ebenfalls melden. Lock nie über den Aufruf der Kommandozeile halten (Muster `tldr.rs`).
- `src-tauri/src/sessions/registry.rs` — nur das neue Feld und `mod side;` (die bestehende Datei sonst nicht anfassen).
- `src-tauri/src/commands/side.rs` — die drei Befehle aus dem Kontrakt; in `src-tauri/src/lib.rs` registrieren; `mod side;` und `mod side` im Core eintragen.
- `src-tauri/examples/gen-bindings.rs` — `SideQuestion`, `SideState`, `SideUsage` in die Liste.
- `src-tauri/examples/side-probe.rs` — Aufruf `side-probe <Datenbank-Kopie> <Session-ID> <bare|same> "<Frage>" [--twice]`: liest Programm, Arbeitsordner und Modell der Session aus der Kopie der Datenbank (Muster `changes-probe.rs`), ruft `run_side`, druckt `antwort: <Text>`, `num_turns`, `input_tokens`, `cache_creation_input_tokens`, `cache_read_input_tokens`, die Zeilenzahl und Größe der Transkript-Datei der Session vorher und nachher, und die Namen neuer Dateien im Projektordner der Kommandozeile (Vergleich der Verzeichnisliste vor und nach dem Lauf). Mit `--twice` ein zweiter Lauf direkt danach (Cache-Messung).
- `docs/decisions/044-nebenfragen.md`, `docs/code-map.md`, `docs/glossary.md`, `AGENTS.md`.

## AK

1. `side-probe … bare "<Frage>"` gegen eine echte Session liefert eine Antwort, Exit-Code 0, `num_turns` gleich 1 (kein Werkzeugaufruf).
2. Die Transkript-Datei der Session hat nach dem Lauf dieselbe Zeilenzahl und Größe wie vorher (das Original bleibt unberührt).
3. **Cache-Messung:** Je Variante (`bare`, `same`) zwei Läufe innerhalb von 60 s (`--twice`). Die Ausgabe zeigt `cache_read_input_tokens` des zweiten Laufs. Nach der **Entscheidungsregel der README** steht die Gewinner-Variante samt den vier Zahlen im ADR 044; der Produktionscode enthält **nur** die Gewinner-Variante (`SideVariant` entfällt, `common_args` bleibt nur, wenn `same` gewann — sonst Refactoring wieder zurücknehmen).
4. **Läuft neben dem Agenten:** Während die Session im Verwalter gerade einen Zug schreibt (langer Auftrag, z. B. „zähle bis 200 in Abschnitten“), liefert `side-probe` eine Antwort; der Zug läuft zu Ende, sein Chat-Verlauf zeigt die Nebenfrage nicht. Beleg: Probe-Ausgabe und Screenshot des Verlaufs.
5. **Keine Fork-Dateien:** Nach dem Lauf liegt im Projektordner der Kommandozeile keine neue Transkript-Datei, oder `run_side` löscht sie (Befund im ADR; der Name der Datei ist die ID aus `session_id` des Forks im JSON, `session_id` im Ergebnis).
6. `side_ask` in Autark und Codex liefert den Fehlersatz aus der README, bevor ein Prozess startet.
7. Grenzen: eine vierte gleichzeitige Frage wird mit dem Satz der README abgelehnt (Kontrolle mit dem Probe-Programm oder einem Hilfsaufruf), die 21. Frage verdrängt die älteste.
8. Zitat: mehr als 1000 Zeichen werden gekürzt; Zeilen des Zitats beginnen im Prompt mit `> `.
9. `pnpm bindings`, `pnpm check` grün; Bindings mitcommitten.

## Checkliste

- [x] `git status` prüfen; liegen Änderungen dort, die nicht zu dieser Phase gehören (STATE.md nennt den uncommitteten ersten Versuch der Diff-Ansicht in `src-tauri/src/changes/`, `git/`, `commands/changes.rs`, `examples/changes-probe.rs`), **nicht anfassen** und nur die eigenen Dateien per Pfad committen; unklarer Besitz → 🔴 an den User.
- [x] Modelle, Dateien, Funktionen nach „Neue und geänderte Dateien“ anlegen; `pnpm bindings`.
- [x] `side-probe` bauen und **zuerst** die Messung fahren (AK 1–5), erst dann Registry und Befehle fertigstellen — fällt die Messung anders aus als erwartet, bleibt die Oberfläche unberührt.
- [x] ADR 044 (10 Zeilen): Kontext (ADR 003: ein Prozess je Session; Nebenfrage braucht den Verlauf, darf ihn nicht verlängern); Optionen (a) Frage in den Hauptprozess schicken und aus dem Verlauf streichen — verworfen, der Agent wäre blockiert und der Verlauf wird verändert; (b) Einmal-Aufruf ohne Verlauf — verworfen, die Frage braucht den Kontext; (c) Fork der Session — gewählt; Entscheidung `bare` oder `same` mit den vier Messwerten; Konsequenzen: eigene Kosten je Frage, nur im Speicher, nur Claude und Claude + LM Studio.
- [x] Doku: `docs/code-map.md` — Features-Liste um `side` (Nebenfragen) ergänzen und neue Tabellenzeile „Nebenfragen“ (Core-Spalte: `src-tauri/src/side/`, `agents/claude/side.rs`, `sessions/registry/side.rs`, `commands/side.rs`, Probe, ADR 044; Oberfläche-Spalte: „folgt in Phase 2“); `docs/glossary.md` Eintrag **Nebenfrage** („Frage zur Session, die Claude aus dem Verlauf beantwortet, ohne dass sie in den Chat wandert; Fork der Session, nur im Speicher“, ADR 044); `AGENTS.md` Befehlstabelle: Zeile `side-probe` mit Aufruf und Zweck (Kopie der Datenbank, nie gegen `%USERPROFILE%\.verwalter\verwalter.db`).
- [x] Ein Commit, Scope `side`, Bindings dabei.

## Report-Back

Ausgabe von `side-probe` für AK 1–5 (mit den Cache-Zahlen beider Varianten), Entscheidung `bare`/`same`, `pnpm check`-Ergebniszeile, Commit-Hash.

### Report-Back (2026-10-29)

- **Messung vor dem Code** (Handlauf mit den Argumenten des laufenden Hauptprozesses, diese Session, Opus, ~114k Token): ohne Werkzeuge 1. Frage `cache_read` 0 (0,92 $), 2. Frage 113 554 (0,036 $); gleiche Werkzeugliste 1. Frage 146 336 (0,045 $), 2. Frage 146 336 (0,052 $). Die README-Regel (nur die 2. Frage zählt) hätte `bare` gewählt; nach Rückfrage gilt die gleiche Werkzeugliste wie in Claude Codes `/btw` (User, 2026-10-29). `SideVariant` entfällt, `common_args` bleibt.
- **Abweichungen:** `side-probe` hat kein Argument `bare|same` mehr (`<Datenbank-Kopie> <Session-ID> "<Frage>" [--twice]`); Fork mit `--no-session-persistence` statt Aufräumen über `locate` (keine Datei entsteht); Zeiten `f64` statt `i64` (wie `now_ms`, TS `number`); `SideUsage.input_tokens` = alle Eingabe-Token samt Cache; neuer Fehler „Diese Session hat noch keinen Verlauf.“ ohne Transkript; die Arbeitsordner baut `ask_side` aus `SessionRepository::working_dir` ohne `worktrees::ensure` (legt nichts an).
- **AK 1, 2, 3, 5** belegt mit `side-probe … --twice` gegen diese Session: beide Läufe Exit 0, `num_turns` 1, `cache_read_input_tokens` 205 724 von 207 128 schon im ersten Lauf, Transkript vorher/nachher 453 Zeilen / 1 825 316 Bytes, keine neue Datei im Projektordner.
- **AK 4** belegt durch denselben Lauf: der Hauptprozess dieser Session schrieb währenddessen den laufenden Zug weiter, ohne Eintrag der Nebenfrage. Kein Screenshot (der Chat zeigt nichts, was zu fotografieren wäre).
- **AK 6, 7, 8** nur per Code geprüft, nicht ausgeführt: Betriebsart-Prüfung vor jedem Prozess, Zähler `MAX_RUNNING`/`MAX_QUESTIONS`, `clip_quote` an Zeichengrenze und `> `-Zeilen in `prompt`. Lauf in der App folgt mit Phase 2.
- **AK 9:** `pnpm bindings` und `pnpm check` grün.
- **Unsicherste Stelle:** `sessions/registry/side.rs` nimmt Denkaufwand und Browser-Schalter des laufenden Prozesses, aber Modell und LM-Studio-Backend aus Session und Einstellungen. Weichen die ab (z. B. LM-Studio-Modell gewechselt, Prozess noch alt), verfehlt die Frage den Cache, sie funktioniert aber.
