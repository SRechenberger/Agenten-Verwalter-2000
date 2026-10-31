# Agenten Verwalter 2000 — Kontext

Ausführliches Produkt- und Technikkonzept: [konzept.md](konzept.md). Diese Datei ist die verdichtete, verbindliche Fassung; wo sie vom Konzept abweicht, gilt diese Datei (Abweichungen stehen in [decisions/001-stack-und-plattform.md](decisions/001-stack-und-plattform.md)).

## Ziel & Vision

Agenten Verwalter 2000 ist eine minimalistische lokale Desktop-Kommandozentrale für Coding-Agenten. Ein Vorhaben steht für eine Aufgabe und kann beliebig viele Git-Repositories umfassen, wahlweise direkt im Haupt-Checkout oder in einem Ticket-Worktree, den der Agent nach seinen Anweisungen anlegt. Darin läuft nacheinander eine oder mehrere Sessions; jede ist eine eigene Claude-Session mit frischem Kontext, alle teilen Workspace, Repositories und Changes. Der Chat mit dem Agenten ist die Hauptarbeitsfläche, die Changes-Ansicht (Dateien, Diffs) die Prüfebene. Zielgruppe sind Entwickler, die mehrere Agenten parallel über mehrere Repositories laufen lassen und dabei den Überblick verlieren — der Engpass ist der Mensch, nicht die Rechenleistung.

Leitsatz: **Chat first. Changes second.** Was nicht aktiv beim Agent-Workflow hilft, gehört nicht dauerhaft auf den Bildschirm.

## Scope

MVP (Version 1):

Die Bedienung orientiert sich an der Claude-Erweiterung für VS Code, damit sich die Arbeit mit dem Agenten gleich anfühlt. Verbindlicher Entwurf: [design/2026-09-28_hauptansichten/](design/2026-09-28_hauptansichten/README.md).

MVP (Version 1):

- **Navigation:** Vorhaben-Baum in der Sidebar (das zuletzt aktive Vorhaben oben, Ungelesen-Punkt bei Neuem, darunter aufklappbar die Sessions), neues Vorhaben (drei Schritte: Aufgabe mit Anhängen → Repositories → Agent), neue Session im Vorhaben, Übersicht je Vorhaben, Vorhaben und Sessions umbenennen, Vorhaben archivieren und im Archiv suchen und wiederherstellen
- **TL;DR:** Kurzfassung von drei, vier Zeilen an jeder Session und jedem Vorhaben, nur per Knopf von Haiku im Hintergrund erstellt; zeigt, wie viele Einträge seither dazukamen, und geht auf Wunsch mit der ersten Nachricht einer neuen Session mit
- **Chat:** Nachrichten von User und Agent, Antworten als Markdown mit Code-Blöcken (Syntaxfarben, Kopieren-Knopf), eingeklappte Tool-Aktivität und Gedankengang, Aufgabenliste, Agent-Status, Unterbrechen, Fortsetzen, Rückfragen direkt im Chat beantworten
- **Eingabe:** Bilder und Dateien anhängen (Knopf, Hineinziehen, Einfügen); Skills und Befehle über `/`-Knopf und `/` im Eingabefeld; Modell und Denkaufwand während der Session wechseln; Modus (Manuell, Automatisch bearbeiten, Planen, Auto)
- **Artefakte:** von einem Agenten als HTML-Seite im Ordner `.artefakte` des Vorhabens abgelegte Artefakte als Karte im Chat und im Reiter „Artefakte“ von Session und Übersicht ([ADR 026](decisions/026-artefakte.md))
- **Hintergrund:** laufende Dev-Server und Subagenten der Session, ausgeführte Skripte samt Ausgabe und der Scratchpad-Ordner der Session sind in einem Seitenpanel der Session sichtbar und einsehbar
- **Workspace:** mehrere Repositories pro Session, auch Ordner ohne Git (ohne Changes, [ADR 018](decisions/018-ordner-ohne-git.md)); der Agent arbeitet im Haupt-Checkout oder in Ticket-Worktrees daneben, die App zeigt beides in den Changes, Ticket-Worktrees gegen den Standard-Branch ([ADR 010](decisions/010-worktrees-durch-den-agenten.md)); Skills der beteiligten Repositories stehen in der Session zur Verfügung
- **Changes:** Repository-Filter, geänderte Dateien, Unified Diff, Trennung committed/uncommitted gegen eine konfigurierbare Basis; je Session nur ihre eigenen Änderungen, in der Übersicht des Vorhabens die Summe ([ADR 014](decisions/014-changes-je-session.md)); der Diff zeigt Syntaxfarben, und einzelne Zeilen lassen sich kommentieren — die Kommentare werden in der Eingabeleiste gesammelt und gehen mit der nächsten Nachricht an den Agenten ([ADR 015](decisions/015-changes-review.md))
- **Agent:** Claude als einziger Provider; Betriebsart Claude Code + LM Studio für ein lokales Modell ([ADR 016](decisions/016-betriebsarten-und-lokales-modell.md)); Betriebsart Autark mit eigenem Agenten ohne Claude-Kommandozeile und ohne Anthropic ([ADR 017](decisions/017-autarker-agent.md))
- **Persistenz:** SQLite, Session-Wiederherstellung nach App-Neustart
- **Desktop:** Tauri-App für Windows

Danach (Reihenfolge laut Konzept, Abschnitt 66): OpenCode-Provider (der LM-Studio-Provider ist durch ADR 016 und ADR 017 erledigt), feinere Rechte-Oberfläche über die Modi hinaus, Suche, app-weite Command Palette → Multi-Agent, „Ask about this change“, Commit-Automatik → PR-Workflow, Agent-Pipelines, Session-Vorlagen.

## Nicht-Ziele

Keine IDE, kein VS-Code-Ersatz, kein vollständiger Git-Client, kein Ticket-System, kein CI/CD-Dashboard, kein Cloud-Dienst, kein LSP-Host, kein Debugger, kein Dev-Container-Manager, kein allgemeiner LLM-Chat-Client, keine Workflow-DSL. Kein permanenter Code-Editor, keine KPI-Dashboards, keine Toast-Flut bei Statuswechseln.

## Stack

| Bereich | Wahl | Begründung |
|---|---|---|
| Desktop-Shell | Tauri 2 | Kleine Shell, native Prozesse und Dateisystem direkt — passt zu „leichte Shell, schwerer Worker“ |
| UI | React + TypeScript (strict) | |
| Styling | Tailwind v4 als Token-Pipeline (`@theme`), Komponenten mit BEM-Klassen und eigener CSS-Datei | Einheitliche Tokens für Hell/Dunkel ohne Utility-Klassen-Wildwuchs, siehe [conventions/tailwind.md](conventions/tailwind.md) |
| UI-State | Zustand, nur für flüchtigen UI-Zustand | Persistente Daten liegen in SQLite, nicht im React-State |
| Native Core | Rust | |
| Datenbank | SQLite über rusqlite | Synchron, schlank, keine Datenbank beim Kompilieren nötig |
| Gemeinsame Typen | aus Rust nach TypeScript generiert (`ts-rs`, siehe [ADR 002](decisions/002-typgenerierung-und-listen.md)) | Eine Quelle für Session-, Event- und Status-Typen |
| Git | native Git-Kommandozeile, vom Rust-Kern aufgerufen | Deterministisch, unabhängig von der KI-Logik |
| Diff | eigene Unified-Diff-Ansicht; der Core zerlegt den Diff, die Oberfläche zeigt nur sichtbare Zeilen ([ADR 006](decisions/006-changes-und-diff.md)) | Entwurf verlangt Nummern- und Vorzeichenspalten, die Monaco nicht abbildet; kein Editor-Paket von mehreren MB |
| Agent | Claude-Kommandozeile im JSON-Stream-Modus, direkt vom Rust-Kern gestartet ([ADR 003](decisions/003-claude-anbindung.md)) | Das SDK startet intern dieselbe Kommandozeile; ein Node-Hilfsprozess pro Agent brächte nur eine Schicht mehr |
| IPC | Tauri Commands + Events | |
| Paketmanager | pnpm | |
| Projektform | eine App (`src/` + `src-tauri/`), kein Monorepo | Monorepo erst, wenn ein zweites Paket echten Bedarf hat |

## Constraints

- **Plattform:** Windows 11 zuerst — dort wird entwickelt und ausprobiert. macOS und Linux bleiben Ziele, werden vor dem MVP aber nicht geprüft.
- **Ressourcen:** Zielgerät ist ein Laptop mit rund 16 GB RAM, auf dem die Agenten selbst den Großteil verbrauchen. Der UI-Speicher wächst mit dem sichtbaren Inhalt, nicht mit der Zahl der Sessions oder Events.
- **Lokal:** kein Server, keine Cloud, kein Docker, kein separater Datenbankdienst. Keine Telemetrie, außer später als Opt-in.
- **Sicherheit:** Der Session-Workspace ist die Sicherheitsgrenze; Agenten bekommen nicht standardmäßig das ganze Benutzerverzeichnis.
- **Absturzfestigkeit:** Ein UI-Absturz darf keine Session und keinen Worktree verlieren; der Agent-Prozess wird danach neu gestartet.
- **Team:** Einzelentwickler, keine Deadline.
- **Qualitätssicherung:** keine automatisierten Tests; abgesichert wird über strenge Typen, Lint und Build; getestet wird im laufenden Betrieb, ohne Abnahme-Checkliste.

## Meilensteine

Reihenfolge nach Entwicklungsrisiko (Konzept, Abschnitt 67) — das Riskanteste zuerst, UI-Politur zuletzt.

1. **Gerüst:** Rust-Toolchain, Tauri-2-App mit React, Lint/Typecheck/Build lokal und als GitHub-Actions-Prüfung.
1b. **Design-Entwurf:** klickbarer Entwurf der Hauptansichten, abgelegt in [design/2026-09-28_hauptansichten/](design/2026-09-28_hauptansichten/README.md) — abgenommen am 2026-09-28. Jeder folgende Meilenstein baut die ihm dort zugeordneten Tafeln gleich nach Entwurf, keine Wegwerf-Oberfläche.
2a. **Durchstich & Chat:** Claude starten, Events empfangen, Rückfragen beantworten, unterbrechen, fortsetzen, Status erkennen, Modell, Modus und Denkaufwand wechseln; gebaut werden App-Rahmen, Leerzustand, Neue Session (ohne Repositories), Chat-Verlauf mit Markdown und kopierbaren Code-Blöcken sowie die Eingabeleiste nach Entwurf. Der Agent arbeitet in einem leeren Ordner pro Session. Anbindungsweg: [ADR 003](decisions/003-claude-anbindung.md).
2b. **Anhänge, Skills, Hintergrund:** Anhänge, `/`-Menü mit Skills, Hintergrund-Panel mit Prozessen, Subagenten und Scratchpad (Fragen in [knowledge/GAPS.md](knowledge/GAPS.md)) — gebaut am 2026-09-29.
4. **Persistenz & Wiederherstellung** (vor 3, siehe [ADR 004](decisions/004-persistenz-und-wiederherstellung.md)): Sessions und Chat-Einträge in SQLite, Wiederherstellung mit `--resume` nach einem Neustart, Umbenennen, Archivieren, ruhende Agenten beenden.
3. **Worktree-Orchestrierung** (auf der Datenbank aus Meilenstein 4): mehrere Repositories als eine Session anlegen, aufräumen, Fehlerfälle (Branch existiert, Repo fehlt).
5. **Changes & Diff:** Diffs über mehrere Repositories zusammenfassen, committed/uncommitted, lazy Diff-Ansicht.
3b. **Vorhaben und Sessions, mit TL;DR:** das Vorhaben als zentrale Einheit, mehrere Sessions darin, Übersicht, Repository nachträglich anhängen, TL;DR von Sessions und Vorhaben ([ADR 011](decisions/011-vorhaben-und-sessions.md)) — gebaut am 2026-09-30.
6. **UI auf Zielbild:** restliche Tafeln des Entwurfs (Einstellungen, Hellmodus), virtuelle Listen, Zustände konsistent über alle Ansichten → MVP — gebaut am 2026-10-01. Die Einstellungsseite speichert Farbschema (Dunkel, Hell, System; wirkt sofort und ohne Aufblitzen beim Start) sowie Standardmodell und Standard-Modus samt Denkaufwand für neue Vorhaben ([ADR 012](decisions/012-einstellungen-farbschema-und-listen.md)); Sidebar, Hintergrund-Listen und Scratchpad-Baum rendern nur das Sichtbare; Lade- und Aktionsfehler erscheinen als Satz statt nur in der Konsole.
6b. **Sprachdiktat:** Mikrofon-Knopf in der Eingabeleiste (Session und „Neues Vorhaben“), Erkennung lokal mit Parakeet (bis v0.38.0 Whisper), Text erscheint satzweise schon während des Sprechens; das Sprachmodell (rund 670 MB) wird einmalig auf Klick geladen ([ADR 009](decisions/009-sprachdiktat-lokal.md), Erkennung seit [ADR 049](decisions/049-sprachdiktat-parakeet.md)) — gebaut am 2026-10-01.

## Offene Fragen

- **Wie wird die Rechtegrenze pro Session auf Windows durchgesetzt?** Über die Rechte-Einstellungen des Agenten selbst, über das Arbeitsverzeichnis, oder mehr? Für das MVP reicht voraussichtlich die Agent-eigene Konfiguration.
- **Windows-Pfadlänge:** Worktrees liegen unter `~\.verwalter\workspaces\<8 Zeichen>\<Repository>` ([ADR 005](decisions/005-repositories-und-worktrees.md)). Die App setzt `core.longpaths` nicht; tiefe Pfade in einem Repository können beim Anlegen scheitern — dann bleibt nichts zurück, und die Meldung nennt das Repository.
