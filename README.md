# Agenten Verwalter 2000

Eine ruhige, lokale Desktop-Oberfläche, um mehrere Coding-Agenten parallel zu führen. Ein **Vorhaben** steht für eine Aufgabe und umfasst beliebig viele Git-Repositories; darin laufen nacheinander eine oder mehrere **Sessions**, jede mit frischem Kontext, alle mit demselben Workspace.

Du arbeitest im Chat mit dem Agenten, beantwortest seine Rückfragen direkt dort und siehst in der Changes-Ansicht, was er über alle betroffenen Repositories geändert hat. Alles läuft lokal: Tauri, React, Rust und SQLite — kein Server, keine Cloud, keine Telemetrie.

Leitsatz: **Chat first, Changes second.** Was dem Agent-Workflow nicht aktiv hilft, gehört nicht dauerhaft auf den Bildschirm.

**Status:** Der Funktionsumfang des MVP steht, die App wird laufend weiterentwickelt (aktuelle Version in `package.json`). Zielplattform ist zuerst Windows 11; macOS- und Linux-Pakete baut die Release-Pipeline mit, geprüft werden sie noch nicht.

## Was die App kann

- **Vorhaben und Sessions:** Sidebar als Baum, das zuletzt aktive Vorhaben oben, Ungelesen-Punkt bei Neuem. Neues Vorhaben in drei Schritten (Aufgabe mit Anhängen, Repositories, Agent). Übersicht je Vorhaben, Umbenennen, Archivieren, Repositories nachträglich anhängen. Eine neue Session lässt sich mit einem Klick aus der Einstiegszeile der letzten weiterführen.
- **TL;DR:** Kurzfassung von Session und Vorhaben, auf Knopfdruck im Hintergrund erstellt.
- **Chat:** Markdown mit Syntaxfarben und Kopieren-Knopf, eingeklappte Tool-Aktivität und Gedankengang, Aufgabenliste, Rückfragen direkt beantworten, Unterbrechen und Fortsetzen. Pfade in Antworten öffnen sich per Klick.
- **Eingabe:** Bilder und Dateien anhängen (Knopf, Hineinziehen, Einfügen), Skills und Befehle über `/`, Modell, Denkaufwand und Modus während der Session wechseln, Diktat per Mikrofon (`Strg+M`) mit lokaler Whisper-Erkennung.
- **Changes:** Dateibaum und Unified Diff über alle Repositories, getrennt nach committed und uncommitted, je Session nur die eigenen Änderungen. Einzelne Diff-Zeilen lassen sich kommentieren; die Kommentare gehen gesammelt mit der nächsten Nachricht an den Agenten.
- **Git-Werkzeuge:** Branch wechseln und anlegen, Commit mit Nachrichtenvorschlag, Push, Pull, Stash, Verlauf als Graph — direkt in den Changes.
- **Workspace:** Der Agent arbeitet im Haupt-Checkout oder in Ticket-Worktrees daneben; auch Ordner ohne Git und Repositories innerhalb eines angehängten Ordners funktionieren.
- **Hintergrund:** laufende Dev-Server, Subagenten, Skripte samt Ausgabe und der Scratchpad-Ordner der Session in einem Seitenpanel.
- **Artefakte:** HTML-Seiten, die der Agent im Ordner `.artefakte` ablegt, erscheinen als Karte im Chat und im Reiter „Artefakte“.
- **Kontext, Kontingent, Konto:** Aufschlüsselung des Kontexts, Abo-Kontingent, Claude-Konto und -Version in der App sehen und wechseln, MCP-Server je Session verwalten.
- **Vorhaben-Retro:** sammelt die Befunde aller Sessions eines Vorhabens und legt daraus eine Retro-Session an.
- **Einstellungen:** Farbschema (Dunkel, Hell, System), Standardwerte für neue Vorhaben, Betriebsart, Sprache des Diktats.
- **Update des Verwalters:** Der Abschnitt „Agenten Verwalter“ in den Einstellungen zeigt die laufende Version und die neueren Releases von GitHub samt Release-Notes; ein Klick lädt die neue Datei, prüft ihre Prüfsumme und startet den Verwalter neu (die portable exe ersetzt sich selbst, der Installer wird gestartet). Ein Punkt am Knopf „Einstellungen“ weist auf ein Update hin.

### Betriebsarten

| Betriebsart | Was dahinter läuft |
|---|---|
| Claude Code | Claude-Kommandozeile im JSON-Stream-Modus, direkt vom Rust-Kern gestartet |
| Claude Code + LM Studio | dieselbe Kommandozeile, aber gegen ein lokales Modell in LM Studio |
| Autark | eigener Agent im Rust-Kern, ohne Claude-Kommandozeile und ohne Anthropic; spricht mit einem lokalen OpenAI-kompatiblen Server |

## Quickstart

### Voraussetzungen

- Windows 11
- [Rust](https://rustup.rs/) (stable, Version in `rust-toolchain.toml`)
- Visual Studio 2022 Build Tools mit dem Workload „Desktopentwicklung mit C++“ (MSVC x64 und Windows SDK); eine Visual-Studio-Installation ohne diesen Workload reicht nicht
- CMake im PATH — liegt den Build Tools bei (`…\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin`) oder `winget install Kitware.CMake`
- [LLVM](https://llvm.org/) per `winget install LLVM.LLVM`, dazu `C:\Program Files\LLVM\bin` im PATH (winget trägt es nicht ein); `whisper-rs-sys` erzeugt seine Bindings per bindgen
- Node.js ≥ 22 und [pnpm](https://pnpm.io/) — pnpm per `npm install -g pnpm` installieren, Node 25+ bringt kein Corepack mehr mit
- Git
- [Claude Code](https://docs.anthropic.com/en/docs/claude-code), angemeldet (nur für die Betriebsarten mit Claude)

Sind mehrere Visual-Studio-Instanzen installiert, etwa Community ohne C++ neben den Build Tools, wählt clang unter Umständen die Instanz ohne C++-Werkzeuge und findet dann weder MSVC-Header noch Windows SDK (`fatal error: 'stdio.h' file not found`). Abhilfe: Umgebungsvariable `VCToolsInstallDir` auf `<Build Tools>\VC\Tools\MSVC\<Version>\` setzen oder den Workload in jeder Instanz installieren. Einzelheiten: [docs/conventions/linting.md](docs/conventions/linting.md).

Optional: [LM Studio](https://lmstudio.ai/) für lokale Modelle.

### Installieren und starten

```sh
pnpm install
pnpm tauri dev      # App im Entwicklungsmodus
```

Unter Windows startet auch `starten.cmd` die App im Entwicklungsmodus; `starten.cmd build` baut die lose exe.

### Prüfen und bauen

```sh
pnpm check                  # gesamte Prüfkette: Lint, Typecheck, Format, Build, rustfmt, Clippy
pnpm tauri build            # NSIS-Installer unter src-tauri/target/release/bundle/nsis/
pnpm tauri build --no-bundle  # nur die lose exe
```

Es gibt keine automatisierten Tests; abgesichert wird über strenge Typen, Lint, Build und eine manuelle Abnahme-Checkliste pro Plan.

Fertige Installer liegen bei den [GitHub-Releases](https://github.com/Aalphones/Agenten-Verwalter-2000/releases). Ein Versions-Tag `vX.Y.Z` löst die Release-Pipeline aus (Windows-Installer, dazu `.dmg`, `.AppImage` und `.deb` als Zusatz).

### Umgebungsvariablen (optional)

| Variable | Wirkung |
|---|---|
| `VERWALTER_IDLE_SECONDS` | Sekunden, nach denen der Agent einer ruhenden Session beendet wird (Standard 1800); der nächste Klick auf Senden startet ihn neu |
| `VERWALTER_LMSTUDIO_URL` | Adresse des LM-Studio-Servers (Standard `http://localhost:1234`) |
| `VERWALTER_BASH_PATH` | Pfad zu `bash.exe` von Git für die Betriebsart „Autark“, falls Git nicht unter `C:\Program Files\Git` liegt |
| `VERWALTER_BRAVE_API_KEY` | Schlüssel der Brave Search API für die Websuche in der Betriebsart „Autark“; ohne ihn sucht der Agent nicht im Web |

Die Daten liegen unter `%USERPROFILE%\.verwalter\` (Datenbank `verwalter.db`, Workspaces der Vorhaben).

## Aufbau

Eine App, kein Monorepo: `src/` (React 19, TypeScript strict, Zustand, Tailwind v4 als Token-Pipeline mit BEM-CSS) und `src-tauri/` (Rust, SQLite über rusqlite, Git über die native Kommandozeile). Typen, die die Grenze überqueren, werden aus Rust nach TypeScript generiert (`pnpm bindings`). Persistentes liegt in SQLite, der UI-State ist flüchtig; Sessions überleben einen Fenster-Absturz.

Ein Feature heißt in jeder Schicht gleich (`src/features/<x>/`, `src-tauri/src/<x>/`, `src-tauri/src/commands/<x>.rs`) — die Karte dazu steht in [docs/code-map.md](docs/code-map.md).

## Weiterlesen

- [docs/PROJECT.md](docs/PROJECT.md) — Ziel, Umfang, Nicht-Ziele, Meilensteine
- [AGENTS.md](AGENTS.md) — Einstieg für Coding-Agenten: Stack, Konventionen, Befehle, Regeln
- [docs/decisions/](docs/decisions/) — Architektur-Entscheidungen
- [docs/glossary.md](docs/glossary.md) — verbindliches Vokabular

## Lizenz

[Apache License 2.0](LICENSE)
