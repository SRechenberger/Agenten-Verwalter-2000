# 074 — Agentenprozesse samt Kindern beenden, Annahme gibt den Agenten frei, Neustart aus dem Transkript

**Status:** angenommen und umgesetzt im Kern (Taiga-Issues 71, 72, 50 und 98; keine Migration).

## Kontext

- Beim Beenden einer Session traf `Child::kill` nur die Claude-Kommandozeile. MCP-Server, Shells und Builds, die ihr Agent gestartet hatte, liefen weiter: ein `tail -F` und ein `ugrep` aus einer Review 16 Stunden lang, `agda` hält 2 bis 5 GB (71).
- Eine angenommene Station behielt ihren Agenten, bis der Leerlauf-Reaper ihn nach 30 Minuten beendete (72).
- Eine Terminal-Shell, die SIGHUP ignoriert, überlebte Archivieren und App-Ende (50).
- Kam `system/init` der Kommandozeile nie an (Absturz vor der ersten Zeile), war `has_agent_history` aus, und der Neustart nahm `--session-id`. Die Kommandozeile fand ihr eigenes Transkript und endete mit „Error: Session ID … is already in use.“, in einer Schleife (98).

## Entscheidung

### Prozessgruppen (71)

- **Eigene Gruppe.** Unter Unix startet jeder Agentenprozess in einer eigenen Prozessgruppe, deren Nummer seine PID ist (`processes::own_group`, `process_group(0)`): die Claude-Kommandozeile (und mit ihr Übersetzer und eigener Agent, `agents::claude::process::start`), Codex (`codex/rpc.rs`), die Shell-Aufgaben und die MCP-Clients des eigenen Agenten. Kurze Befehle wie `git` und `gh` bleiben in der Gruppe des Verwalters. Unter Windows ändert der Helfer nichts.
- **Gruppe beenden.** `processes::signal_group(pgid, Terminate | Kill)` schickt `kill(-pgid, …)`. `ESRCH` (keine solche Gruppe) ist kein Fehler. `ClaudeProcess::kill` und `standalone::tasks::kill_process_tree` nutzen ihn; unter Windows bleibt `Child::kill` bzw. `taskkill /T /F`.
- **Drei Sicherheitsregeln.**
    1. Nie für `pgid <= 1`: `kill(0, …)` träfe die eigene Gruppe, `kill(-1, …)` jeden Prozess des Benutzers.
    2. Nie für die eigene Gruppe des Verwalters (`getpgrp()`).
    3. Nie für eine Gruppe, deren Anführer der Verwalter schon eingesammelt hat: Danach kann die Nummer frei sein und einem fremden Prozess gehören. Signalisiert wird nur vor dem Einsammeln, und Einsammeln und Signal stehen unter derselben Sperre: `Leader` mit `reaped` in `agents/claude/process.rs`, `running` je Aufgabe in `standalone/tasks.rs` (`pid: None` nach dem Einsammeln), `TerminalInfo::running` im Terminal. Die Überwacher fragen deshalb mit `try_wait` in Abständen von 50 ms, statt in `wait` zu blockieren.
- Regeln 1 und 2 prüft der Helfer selbst (`group_target`, ohne zu signalisieren); Regel 3 ist Pflicht des Aufrufers.
- **Session beenden.** `retire_process` schließt stdin und wartet `KILL_GRACE` (5 s) wie bisher; danach geht SIGKILL an die ganze Gruppe. Endet die Kommandozeile vorher selbst, beendet ihr Überwacher den Rest der Gruppe, bevor er sie einsammelt (`Leader::try_reap`, mit `waitid(…, WNOWAIT)`). Hält ein Kind ihre Ausgabe offen, sammelt der Überwacher sie nicht ein, und der Abschuss nach der Frist trifft die Gruppe. Kinder sterben also mit der Kommandozeile, wie auch immer sie endet.
- **App-Ende.** `RunEvent::Exit` beendet zuerst alle Agentenprozesse, die noch nicht eingesammelt sind, auch einen in der Frist von `retire_process` (`process::stop_all`, Liste schwacher Verweise): SIGTERM an alle Gruppen, höchstens 2 s Warten für alle zusammen, dann SIGKILL an die übrigen. Ab dem SIGTERM meldet ein so beendeter Prozess nichts mehr, weder Zeilen noch sein Ende: Das App-Ende ist kein Absturz, und die Session behält ihren Status, wie vor dieser Entscheidung; die letzten Zeilen stehen im Transkript, und `--resume` findet sie. Danach die Terminals (höchstens 1 s) und die Befehls-Schritte (ADR 073).

### Terminal (50)

`remove` schickt weiter SIGHUP (portable-pty). Ist die Shell 1 s danach nicht eingesammelt, geht SIGKILL an ihre Gruppe; portable-pty startet sie mit `setsid`, ihre Gruppe ist ihre PID. `close_all` beim App-Ende wartet höchstens 1 s für alle Terminals zusammen.

### Annahme gibt den Agenten frei (72)

Nach dem Speichern der Annahme (`complete_station_with_handoff`, nicht „Als fertig markieren“) beendet der Kern den Agenten der Session der Station, wenn sie ruht (`is_resting`), auf dem Weg des Reapers (`retire_idle_process`). Mit laufender Hintergrundarbeit behält sie ihn. Die nächste Nachricht startet ihn mit `--resume` neu. Der Reaper bleibt, wie er ist.

### Neustart aus dem Transkript (98)

- Die Kommandozeile prüft `--session-id` nur im Ordner ihres Arbeitsverzeichnisses: `<Basis>/projects/<Ordner>/<id>.jsonl`, Basis `CLAUDE_CONFIG_DIR` oder `~/.claude`.
- Der Ordnername (`locate::project_folder`, nachgebaut aus der Kommandozeile 2.1.296): Jede UTF-16-Einheit außer `a-zA-Z0-9` wird `-`. Über 200 Zeichen bleiben die ersten 200, dahinter `-` und der Betrag des 32-Bit-Hashes `h = h * 31 + Einheit` über den ganzen Pfad, zur Basis 36. Unter Unix gilt der Pfad ohne symbolische Links, wie `getcwd` ihn liefert.
- **Regel** (`start_choice`, reine Funktion): Im Claude-Betrieb entscheidet allein, ob dieser Ordner `<id>.jsonl` enthält: dann `--resume`, sonst `--session-id`. `has_agent_history` entscheidet dort nicht mehr. Ist `projects` nicht lesbar, gilt weiter `has_agent_history`. Codex und der eigene Agent bleiben bei `has_agent_history` und eigenem Transkript.
- Der Hinweis „Verlauf nicht übernommen“ erscheint weiter, wenn die Session Verlauf hat, der Start aber nicht fortsetzt.

## Grenzen

- **Kinder mit eigener Gruppe oder Sitzung** (`setsid`, `setpgid`, Job-Steuerung einer interaktiven Shell) verlassen die Gruppe und überleben. Im Terminal betrifft das Vordergrund-Jobs einer interaktiven Shell, die SIGHUP ignorieren.
- **Verschachtelte Gruppen.** Codex, die Shell-Aufgaben und die MCP-Clients des eigenen Agenten haben eigene Gruppen innerhalb des Übersetzers bzw. eigenen Agenten. Beendet der sich geordnet (stdin zu), räumt er sie selbst ab. Trifft ihn SIGKILL nach der Frist oder SIGTERM beim App-Ende, erreicht das ihre Gruppen nicht.
- **Kurze Aufrufe der Kommandozeile** (Nebenfrage, `-p`, Helfer) sind keine Agentenprozesse in diesem Sinn und laufen ohne eigene Gruppe.
- **Windows:** keine Prozessgruppen und keine Job Objects. `Child::kill` beendet nur die Kommandozeile, `taskkill /T /F` die Bäume der Aufgaben und MCP-Clients. Ein eigener systemd-Scope oder cgroups sind nicht Teil dieser Entscheidung.

## Tests

`processes::tests::groups` (Wegwerf-Baum `sh` mit `sleep`, Ablehnung von `0`, `1` und der eigenen Gruppe, `ESRCH`, `has_exited` ohne Einsammeln), `sessions::registry::tests` (Frist von `retire_process` im Test einstellbar: Agent ignoriert stdin, Agent endet selbst; `stop` mit einem Agenten, der SIGTERM ignoriert; Annahme ruhend und mit Hintergrundarbeit; `start_choice` mit allen Fällen; `project_folder` mit langem Pfad; `transcript_in`), `terminal::tests` (Shell mit `trap '' HUP` nach `remove` in 2 s weg). Nur unter Unix.
