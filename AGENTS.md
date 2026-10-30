# AGENTS.md — Agenten Verwalter 2000

🚧 Aktive Arbeit → [STATE.md](STATE.md)

Lokale Desktop-Kommandozentrale für Coding-Agenten: ein Vorhaben = eine Aufgabe über mehrere Git-Repositories; darin nacheinander eine oder mehrere Sessions mit frischem Kontext; ob der Agent im Haupt-Checkout oder in einem Ticket-Worktree arbeitet, bestimmen seine Anweisungen; der Chat ist die Hauptarbeitsfläche, Changes/Diffs die Prüfebene. Kontext, Scope, Meilensteine und offene Fragen: **[docs/PROJECT.md](docs/PROJECT.md)**. Ausführliches Ursprungskonzept: [docs/konzept.md](docs/konzept.md).

## Code finden — erst hier, dann suchen

**Bevor du im Code suchst, lies [docs/code-map.md](docs/code-map.md).** Ein Feature heißt in jeder Schicht gleich (`src/features/<x>/`, `src/stores/<x>.ts`, `src-tauri/src/commands/<x>.rs`, `src-tauri/src/<x>/`, `src-tauri/src/db/<x>.rs`) — den Pfad kannst du meist raten statt ihn zu suchen. Begriffe: [docs/glossary.md](docs/glossary.md).

## Stack

| Layer | Choice |
|---|---|
| Desktop | Tauri 2 |
| UI | React 19 + TypeScript (strict) |
| Styling | Tailwind v4 als Token-Pipeline + BEM-CSS pro Komponente |
| UI-State | Zustand (nur flüchtig) |
| Core | Rust |
| Datenbank | SQLite (rusqlite) |
| Git | native `git`-Kommandozeile aus dem Core |
| Diff | eigene Unified-Diff-Ansicht (virtualisiert) |
| Paketmanager | pnpm |
| Zielplattform | Windows zuerst |

Begründungen: [docs/decisions/001-stack-und-plattform.md](docs/decisions/001-stack-und-plattform.md).

## Konventionen

| Thema | Datei |
|---|---|
| TypeScript | [docs/conventions/typescript.md](docs/conventions/typescript.md) |
| React | [docs/conventions/react.md](docs/conventions/react.md) |
| Styling / Tailwind | [docs/conventions/tailwind.md](docs/conventions/tailwind.md) |
| Rust | [docs/conventions/rust.md](docs/conventions/rust.md) |
| Linting / Prüfkette | [docs/conventions/linting.md](docs/conventions/linting.md) |
| Commits | [docs/conventions/commits.md](docs/conventions/commits.md) |
| Releases / Versions-Tags | [docs/conventions/releases.md](docs/conventions/releases.md) |

## Befehle

| Befehl | Wirkung |
|---|---|
| `pnpm tauri dev` | App im Entwicklungsmodus |
| `pnpm check` | gesamte Prüfkette (Lint, Typecheck, Format, Build, rustfmt, Clippy) — vor jedem Commit grün |
| `pnpm bindings` | TS-Typen aus Rust neu erzeugen — nach jeder Änderung an Typen, die die Tauri-Grenze überqueren |
| `pnpm tauri icon src-tauri/icons/icon.svg -o <Ordner>` | erzeugt aus der SVG-Quelle des App-Icons alle Größen in `<Ordner>`; von dort die Desktop-Dateien (`32x32.png`, `128x128.png`, `128x128@2x.png`, `Square*.png`, `StoreLogo.png`, `icon.png`, `icon.icns`, `icon.ico`) nach `src-tauri/icons/` kopieren, `android/` und `ios/` verwerfen; danach `src-tauri/build.rs` anfassen (oder `cargo clean -p verwalter`), sonst behält die exe das alte Icon |
| `pnpm format` | Prettier schreibt `src/` neu |
| `cargo run --manifest-path src-tauri/Cargo.toml --example changes-probe -- <Datenbank-Kopie> <Session-ID> [session\|project] [<Schlüssel> <Pfad> [<von> <bis>]]` | zeigt, was die Changes einer Session aus einer Kopie der Datenbank ermitteln (Einträge, Diff mit Zeilenzahl der neuen Fassung, mit `<von> <bis>` zusätzlich diese Zeilen, Suchordner der Commit-Suche; `PROBE_SCOPE=committed\|uncommitted` wählt einen anderen Blickwinkel als „alles“); nie gegen `%USERPROFILE%\.verwalter\verwalter.db` |
| `cargo run --manifest-path src-tauri/Cargo.toml --example git-probe -- <Datenbank-Kopie> <Session-ID> [<Schlüssel>]` | zeigt den Git-Zustand der Changes einer Session aus einer Kopie der Datenbank (Branch, ↓/↑, fremde Änderungen, eigene Commits), mit Schlüssel die Branch-Liste des Eintrags; nie gegen `%USERPROFILE%\.verwalter\verwalter.db` |
| `cargo run --manifest-path src-tauri/Cargo.toml --example file-link-probe -- --base <Ordner> [--base …] [--allow <Ordner> …] <Pfad>` | zeigt, wohin ein Dateiverweis aus dem Chat aufgelöst wird (`OK <Pfad>` oder `FEHLER <Variante>: <Text>`), ohne etwas zu öffnen |
| `cargo run --manifest-path src-tauri/Cargo.toml --example app-update-probe -- check [--current <Version>] \| download <Ordner> [--current <Version>] \| swap \| pick \| kind <Ordner>` | prüft die Bausteine des App-Updates ohne Datenbank und ohne laufende App: Status gegen die echten Releases, Download mit Prüfsumme, Tausch einer laufenden exe samt Rücknahme und Neustart, Wahl der Release-Datei, Art der Installation |
| `cargo run --manifest-path src-tauri/Cargo.toml --example side-probe -- <Datenbank-Kopie> <Session-ID> "<Frage>" [--twice]` | stellt eine Nebenfrage an eine Session mit denselben Argumenten wie die App und druckt Antwort, Züge, Eingabe-Token samt Anteil aus dem Cache, Zeilen und Größe des Session-Transkripts vorher und nachher und neue Dateien im Projektordner der Kommandozeile; `--twice` fragt zweimal; nur Betriebsart Claude; nie gegen `%USERPROFILE%\.verwaltererwalter.db` |
| `VERWALTER_APP_VERSION` (Umgebungsvariable, nur `pnpm tauri dev`) | ersetzt die laufende Version für den Vergleich mit den GitHub-Releases (z. B. `0.0.1`), damit „Update verfügbar“ im Dev-Lauf erscheint; in einem Release wirkungslos |
| `pnpm tauri build` | NSIS-Installer unter `src-tauri/target/release/bundle/nsis/`, lose exe unter `src-tauri/target/release/verwalter.exe` |
| `pnpm tauri build --no-bundle` | nur die lose exe, ohne Installer |
| `VERWALTER_IDLE_SECONDS` (Umgebungsvariable) | Sekunden, nach denen der Agent einer ruhenden Session beendet wird (Standard 1800); der nächste Klick auf Senden startet ihn neu; eine Session mit laufendem Hintergrundprozess oder Subagent gilt nicht als ruhend |
| `VERWALTER_LMSTUDIO_URL` (Umgebungsvariable) | Adresse des lokalen Servers von LM Studio für die Betriebsart „Claude Code + LM Studio“ (Standard `http://localhost:1234`) |
| `VERWALTER_BASH_PATH` (Umgebungsvariable) | Pfad zu `bash.exe` von Git für den Agenten der Betriebsart „Autark“, falls Git nicht unter `C:\Program Files\Git` liegt |
| `VERWALTER_BRAVE_API_KEY` (Umgebungsvariable) | Schlüssel der Brave Search API für die Websuche des Agenten in der Betriebsart „Autark“; ohne ihn sucht der Agent nicht im Web. Muss beim Start des Verwalters gesetzt sein. |
| `VERWALTER_CODEX_PATH` (Umgebungsvariable) | Pfad zu `codex.exe` für die Betriebsart „Codex“, falls sie weder im `PATH` noch in der jüngsten VS-Code-Erweiterung von Codex liegt |
| `VITE_VERWALTER_CHAT_PAGE_SIZE` (Umgebungsvariable, nur `pnpm tauri dev`) | Seitengröße beim Nachladen des Verlaufs (1–500, Standard 200); klein gesetzt lässt sich das Nachladen beim Hochscrollen mit kurzen Verläufen prüfen |

Die Prüfkette im Einzelnen und ihre Reihenfolge: [docs/conventions/linting.md](docs/conventions/linting.md).

## Dokumentation

| Datei | Inhalt |
|---|---|
| [docs/PROJECT.md](docs/PROJECT.md) | Ziel, Scope, Nicht-Ziele, Constraints, Meilensteine, offene Fragen |
| [docs/code-map.md](docs/code-map.md) | Feature → Ordner |
| [docs/glossary.md](docs/glossary.md) | Verbindliches Vokabular |
| [docs/decisions/](docs/decisions/) | Architektur-Entscheidungen (ADRs) |
| [docs/planning/](docs/planning/) | Aktive Pläne |
| [docs/archive/](docs/archive/) | Abgeschlossene Pläne |
| [docs/design/](docs/design/) | UI-Entwürfe und Mockups |
| [docs/knowledge/](docs/knowledge/) | Projektwissen (Fakten, Referenzen), offene Recherchefragen |
| [docs/memory/](docs/memory/) | Projekt-Notizen für Agenten |

Nach Code-Änderungen: Code-Map, Glossar und betroffene Konventionen im selben Commit nachziehen.

## Critical Rules

1. **Chat first, Changes second** — keine UI-Elemente, die nicht aktiv beim Agent-Workflow helfen, dauerhaft auf den Bildschirm.
2. **Git-Zustand ist deterministisch** — Git läuft nur über `src-tauri/src/git/`; die UI zeigt den Zustand aus Git, nie aus Aussagen des Agenten.
3. **Persistentes gehört in SQLite, nicht in den UI-State** — der Speicherbedarf der UI wächst mit dem Sichtbaren, nicht mit der Zahl der Sessions oder Events.
4. **Sessions überleben die UI** — ein Fenster-Absturz verliert keine Session und keinen Worktree; der Agent wird mit der nächsten Nachricht wieder gestartet.
5. **Der Workspace des Vorhabens ist die Sicherheitsgrenze** — Agenten bekommen den Ordner des Vorhabens (alle seine Sessions teilen ihn) als Arbeitsverzeichnis, je Repository den Haupt-Checkout bzw. den Ordner ohne Git per `--add-dir` und Lese-/Schreibfreigabe für dessen Ticket-Worktrees `<repo>-wt-*` (Sessions vor ADR 010: ihre App-Worktrees), nicht das Benutzerverzeichnis.
6. **Ein vollständig abgeschlossener Plan endet mit einem Versions-Tag** — beim Archivieren Version in den drei Dateien anheben, `chore(release)`-Commit, Tag `vX.Y.Z` setzen und pushen; der Tag baut das Release. Ablauf und Versionsregel: [docs/conventions/releases.md](docs/conventions/releases.md).
