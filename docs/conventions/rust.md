# Rust Conventions — verwalter

> **Stack** (für dieses Projekt festgelegt):
> | Layer | Choice |
> |---|---|
> | Rust | stable, Edition 2024, Toolchain per `rust-toolchain.toml` gepinnt |
> | Desktop | Tauri 2 |
> | Datenbank | SQLite über rusqlite |
> | Git | native `git`-Kommandozeile |
> | Lint | `cargo clippy -- -D warnings`, `cargo fmt --check` |
>
> Projektentscheidungen in dieser Datei haben Vorrang vor allgemeinen Gewohnheiten.

## Project Layout

```text
src-tauri/src/
  main.rs, lib.rs      App-Start, Registrierung der Commands
  commands/<feature>.rs  Tauri Commands (dünn: Eingabe prüfen, Fachlogik aufrufen, Ergebnis zurück)
  <feature>/           Fachlogik (sessions, chat, changes, repositories, settings)
  agents/              ein Modul pro Provider, gemeinsamer Provider-Trait
  git/                 einziger Ort, der `git` aufruft
  worktrees/           Anlegen/Aufräumen von Worktrees
  processes/           Start, Überwachung, Wiederfinden von Agent-Prozessen
  db/<table>.rs        SQLite-Zugriffe, Migrationen
  filesystem/          Pfad-Logik, Workspace-Grenzen
```

## Fehlerbehandlung

- Kein `unwrap()` / `expect()` außerhalb von Programmstart und echten Invarianten; `expect` nennt dann die Invariante.
- Eigener Fehlertyp pro Modul (`thiserror`), am Command-Rand in eine serialisierbare Fehlermeldung für die UI übersetzt.
- Fehler, die der Nutzer sehen soll (Repo fehlt, Branch existiert, Agent abgestürzt), sind eigene Varianten, damit die UI eine passende Aktion anbieten kann — keine generische Textmeldung.

## Git

- Git läuft ausschließlich über das Modul `git/`, das die Kommandozeile mit festen Argumenten aufruft und die Ausgabe in Typen parst (`--porcelain`, `-z`, `--numstat`).
- Nie Nutzereingaben ungeprüft in Git-Argumente — Branch-Namen validieren, Pfade als eigene Argumente übergeben, nie über eine Shell.
- Der Git-Zustand, den die UI zeigt, stammt aus diesem Modul, nie aus Aussagen des Agenten.
- Lesen im Worktree des Agenten nur mit Plumbing (`diff-index`, `diff-tree`, `ls-files`, `rev-list`), nie mit `git diff` oder `git status` — die nehmen kurz `index.lock` und lassen einen gleichzeitigen Commit des Agenten scheitern ([ADR 006](../decisions/006-changes-und-diff.md)). `-z`-Ausgaben über `run_raw` lesen, das nichts abschneidet.

## Prozesse

- Sessions überleben die UI (Datenbank, `--resume`); der Agent-Prozess endet mit der App und wird bei Bedarf neu gestartet — nicht der Prozess, sondern die Session ist die Einheit, die einen Absturz überlebt.
- Windows zuerst: Beenden ohne POSIX-Signale, Pfade mit Laufwerksbuchstaben, Pfadlänge beachten. Plattformunterschiede gekapselt in `processes/` und `filesystem/`.
- Kindprozesse starten nur über `processes::command` (das Pseudo-Terminal über `processes::pty_command`), nie mit `Command::new` oder `CommandBuilder::new`; Clippy sperrt beide (`src-tauri/clippy.toml`). Der Helfer nimmt im AppImage alle Einträge unter `$APPDIR` aus den Umgebungsvariablen des Kindprozesses (siehe „Umgebung des AppImage“ im [Glossar](../glossary.md)), sonst laden `node`, `python3`, `curl` und `git` die älteren Bibliotheken des Bündels und scheitern. Er ändert nur die betroffenen Variablen, nie mit `env_clear`: der Schlüssel des MCP-Zugangs und die Variablen der Betriebsarten stehen nur in der Umgebung; ein späteres `.env(…)` des Aufrufers gewinnt. Ein Programm aus dem Bündel selbst (die eigene Exe in den Betriebsarten Autark und Codex) behält die Umgebung.
- Ein Pfad, der mit `/` aus Git-Ausgabe oder URL kommt, wird mit `filesystem::native_path` zum Dateisystempfad — nie mit einem eigenen `replace('/', "\\")`. Unter Linux und macOS ist `\` ein gültiges Zeichen im Dateinamen. Reine Vergleichsschlüssel (`normalized_dir`, `attribution::normalize_path`) bleiben davon ausgenommen: sie werden nie geöffnet.

## Datenbank

- Schema-Änderungen nur über nummerierte Migrationen in `db/` (`db/migrations/NNN_*.sql`, in `db/migrations.rs` an die Liste hängen, nie eine bestehende ändern).
- Datei `<Benutzerordner>\.verwalter\verwalter.db`, eine Verbindung hinter einem Mutex (`Database::with`), WAL-Modus. Wer die Datenbank unter einer Session-Sperre benutzt, nimmt erst die Session-Sperre, dann die der Datenbank — nie umgekehrt; unter `Database::with` wird keine Session gesperrt.
- rusqlite ist synchron: längere Zugriffe laufen nicht auf dem Tauri-Haupt-Thread.
- Chat und Events werden seitenweise gelesen (Cursor), nie vollständig in den Speicher geladen.

## Typen für die UI

Typen, die über die Tauri-Grenze gehen, werden im Core definiert und nach `src/lib/bindings/` generiert — mit `ts-rs`, ausgelöst über `pnpm bindings`. Der Export läuft über das Hilfsprogramm `src-tauri/examples/gen-bindings.rs` (neuer Typ → `derive(TS)` und Eintrag dort), nicht über `#[ts(export)]`, weil das an `cargo test` hängt und `pnpm check` keinen Testlauf fährt (Tests gibt es nur in `implementation/tests.rs`, von Hand mit `cargo test`, und unter `remote/` und `processes/`, dort auch in CI). Weil `examples/` ein zweites Programm neben der App ist, steht in `Cargo.toml` `default-run = "verwalter"`. Begründung: [ADR 002](../decisions/002-typgenerierung-und-listen.md).

## Critical Rules

1. **Git nur über `git/`** — eine zweite Stelle, die Git aufruft, zerstört die deterministische Sicht auf den Repository-Zustand.
2. **Kein Shell-Aufruf mit zusammengesetzten Strings** — Agent-Namen, Branch-Namen und Pfade kommen teils aus Nutzereingaben.
3. **Sessions überleben die UI** — Session-Verlust nach einem Fenster-Absturz ist der Fehler, den die App verhindern soll; der Agent-Prozess selbst darf enden.
4. **Kein `unwrap()` in Command-Pfaden** — ein Panic im Core reißt die ganze App mit.
