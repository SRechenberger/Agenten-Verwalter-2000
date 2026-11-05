# Claude-Kommandozeile im Stream-Modus

Belegt am 2026-09-28 mit Claude Code 2.1.220 unter Windows 11, per Probe-Skript gegen die installierte `claude.exe`. Grundlage für [ADR 003](../decisions/003-claude-anbindung.md). Was hier nicht steht, ist nicht geprüft.

## Programm

- Die npm-Installation liefert eine **native** `claude.exe` (rund 265 MB) unter `%APPDATA%\npm\node_modules\@anthropic-ai\claude-code\bin\claude.exe`. Das `claude`, das die Shell findet, ist nur ein `.ps1`/`.cmd`-Starter davor — der Core startet die `.exe` direkt, ohne Shell und ohne Node.
- Arbeitsspeicher eines ruhenden Prozesses nach dem Start: rund 390 MB (ein Messwert, `WorkingSet64`). Fünf offene Sessions kosten damit rund 2 GB.
- Schließt der Aufrufer die Standardeingabe, beendet sich der Prozess nach der laufenden Antwort von selbst.

## Aufruf

```text
claude.exe -p --input-format stream-json --output-format stream-json --verbose
           --permission-prompt-tool stdio
           --model <id> --effort <low|medium|high|xhigh|max>
           --permission-mode <default|acceptEdits|plan|auto>
           --add-dir <worktree> …       (je Repository der Session einmal)
           --session-id <uuid>          (neue Session)   bzw.   --resume <uuid>   (fortsetzen)
Umgebung:  CLAUDE_CODE_ADDITIONAL_DIRECTORIES_CLAUDE_MD=1
```

- Jede Zeile auf stdin und stdout ist ein JSON-Objekt.
- `--verbose` ist mit `stream-json` als Ausgabe nötig.
- Die Einstellungen des Benutzers (`~/.claude/settings.json`: Rechte-Regeln, Hooks, Skills, Ausgabestil) gelten auch für so gestartete Prozesse. Eine Probe im Modus `default` legte eine Datei ohne Rechte-Abfrage an — vermutlich greift eine Freigabe-Regel des Benutzers. Welche Werkzeuge in „Manuell“ tatsächlich nachfragen, hängt also von diesen Einstellungen ab. `--setting-sources` schränkt die Quellen ein (nicht geprüft).
- `--session-id` legt die Session-ID fest; geprüft am 2026-09-28: eine mit `--session-id` gestartete Session lässt sich nach hartem Beenden des Prozesses mit `--resume <dieselbe-id>` fortsetzen, der Agent zitiert die erste Nachricht. `--resume <id>` setzt eine Session mit vollem Verlauf fort: geprüft, der Agent kannte eine vorher per Rückfrage gewählte Antwort.

## Mehrere Repositories

Belegt am 2026-09-28 mit Claude Code 2.1.220. Der Agent startet im Session-Workspace, die Repositories liegen als Worktrees in Unterordnern darunter.

- **Skills** aus `<repo>\.claude\skills` findet Claude nur mit `--add-dir <worktree>`: ohne ist `skills` in `system/init` leer, mit stehen sie dort.
- **`CLAUDE.md`** eines mit `--add-dir` hinzugefügten Ordners lädt Claude nur, wenn die Umgebungsvariable `CLAUDE_CODE_ADDITIONAL_DIRECTORIES_CLAUDE_MD=1` gesetzt ist.
- Geprüft ist nur ein frischer Start (`--session-id`); ob `--add-dir` auf dem `--resume`-Weg genauso wirkt, prüft die Smoke-Abnahme von Meilenstein 3.

## Nachrichten an den Agenten (stdin)

```json
{"type":"user","message":{"role":"user","content":"Text der Nachricht"}}
{"type":"control_request","request_id":"app-1","request":{"subtype":"interrupt"}}
{"type":"control_request","request_id":"app-2","request":{"subtype":"set_model","model":"claude-sonnet-5-5"}}
{"type":"control_request","request_id":"app-3","request":{"subtype":"set_permission_mode","mode":"acceptEdits"}}
```

- `interrupt`, `set_model`, `set_permission_mode` wirken mitten in der Session ohne Neustart; jede Anfrage wird mit `{"type":"control_response","response":{"subtype":"success","request_id":…}}` bestätigt.
- `set_max_thinking_tokens` wird angenommen. **`set_effort` gibt es nicht** (`"Unsupported control request subtype: set_effort"`) — der Denkaufwand lässt sich nur beim Start setzen; ein Wechsel heißt Prozess beenden und mit `--resume <id> --effort <neu>` neu starten, der Verlauf bleibt.
- Nach `set_model` erscheint eine `user`-Zeile mit Text-Inhalt `<local-command-stdout>Set model to …</local-command-stdout>` — keine Nutzernachricht.

## Ereignisse vom Agenten (stdout)

| `type` / `subtype` | Bedeutung |
|---|---|
| `system` / `init` | Nach jedem Start und vor jeder Antwort. Felder u.a. `session_id`, `cwd`, `model`, `permissionMode`, `tools`, `slash_commands`, `skills`, `agents`, `claude_code_version`, `scratchpad_path` (siehe „Scratchpad“). |
| `system` / `thinking_tokens` | Laufender Zähler, solange das Modell nachdenkt. Viele Zeilen, kein Inhalt. |
| `system` / `status` | Z.B. nach Moduswechsel, Feld `permissionMode`. Während einer Zusammenfassung `status: "compacting"`, danach `status: null` mit `compact_result: "success"`. |
| `system` / `compact_boundary` | Grenze einer Zusammenfassung, mit `compact_metadata` (`trigger`, `pre_tokens`, `post_tokens`); siehe „Cache, Kontingent, Zusammenfassung, Kosten“. |
| `system` / `permission_denied` | Ein Werkzeugaufruf wurde abgelehnt: `tool_name`, `tool_use_id`, `message`. Kommt vor dem `tool_result` mit `is_error`. |
| `system` / `informational` | Allgemeiner Hinweis, Feld `content` (nur aus dem Code der VS-Code-Erweiterung 2.1.301, nicht gemessen). |
| `system` / `model_refusal_fallback` | Claude weicht auf ein anderes Modell aus: `original_model`, `fallback_model`, `direction` (`retry`, `revert`, `sticky`), `content` (nur aus dem Code der VS-Code-Erweiterung 2.1.301, nicht gemessen). |
| `system` / `background_tasks_changed`, `task_started`, `task_progress`, `task_updated`, `task_notification` | Hintergrundprozesse (`task_type: "local_bash"`) und Subagenten (`task_type: "local_agent"`), Felder siehe „Hintergrundprozesse und Subagenten“. |
| `rate_limit_event` | Kontingent nach jeder Antwort: `rate_limit_info` mit `status`, `rateLimitType`, `resetsAt`, `unifiedWindows.five_hour.utilization`, `unifiedWindows.seven_day.utilization`, `overageStatus`. |
| `assistant` | Eine Zeile pro Inhaltsblock: `message.content` ist ein Array mit genau einem Block `thinking` (Feld `thinking`), `text` (Feld `text`) oder `tool_use` (`id`, `name`, `input`). `message.usage` trägt `input_tokens`, `cache_creation_input_tokens`, `cache_read_input_tokens`, `output_tokens`. `parent_tool_use_id` ist bei Nachrichten eines Subagenten gesetzt, sonst `null`. |
| `user` | Werkzeug-Ergebnisse: `message.content` = Array mit `tool_result` (`tool_use_id`, `content`, `is_error`). Oder Text-Inhalt als String (lokale Befehlsausgabe) bzw. `[{"type":"text","text":"[Request interrupted by user]"}]` nach einer Unterbrechung. |
| `control_request` / `can_use_tool` | Der Agent will ein Werkzeug benutzen und fragt nach: `request.tool_name`, `request.input`, `request.tool_use_id`, `request_id`. Kommt auch für `AskUserQuestion`. |
| `control_response` | Antwort auf eine eigene Steueranfrage. |
| `result` | Ende einer Antwort. `subtype` `success` bzw. `error_during_execution`, `is_error`, `stop_reason`, `terminal_reason` (`completed`, nach Unterbrechung `aborted_streaming`), `result` (Text), `modelUsage.<modell>.contextWindow` (z.B. 200000), `usage`, `total_cost_usd`, `permission_denials`. |

## Cache, Kontingent, Zusammenfassung, Kosten

Gemessen am 2026-10-21 mit Claude Code 2.1.293, `-p --input-format stream-json --output-format stream-json --verbose --model claude-sonnet-5-5` (Grundlage für [ADR 033](../decisions/033-warnhinweise.md); Beispielzeilen im Plan „Warnhinweise“ unter `artifacts/beispielzeilen.jsonl`).

- **Cache:** jede `assistant`-Zeile trägt `message.id` (eine Antwort kommt in mehreren Zeilen mit derselben `id`) und `message.usage.cache_creation` mit `ephemeral_5m_input_tokens` und `ephemeral_1h_input_tokens`. Gemessen war nur `ephemeral_1h_input_tokens` > 0, die Kommandozeile schreibt also mit 1 Stunde Haltedauer. Grundlast einer frischen Session: rund 64 000 Token (17 509 geschrieben + 46 720 gelesen).
- **Kontingent:** `rate_limit_event` kommt nach jeder Antwort. Gemessen nur `status: "allowed"`, `rateLimitType: "five_hour"`, `resetsAt` in Sekunden seit 1970, `unifiedWindows.five_hour.utilization` und `unifiedWindows.seven_day.utilization` als Anteil 0–1 (gemessen 0.51 und 0.28). Nur aus dem Code der VS-Code-Erweiterung 2.1.301, nicht gemessen: `status` `allowed_warning` und `rejected`.
- **Kosten:** `result.total_cost_usd` ist **über den Prozess aufsummiert**, nicht je Antwort: zwei Antworten in einem Prozess meldeten 0.0794 und 0.159.
- **Zusammenfassung (`/compact` als Nachricht):** `system`/`status` mit `status: "compacting"`, danach `status: null` mit `compact_result: "success"`, dann `system`/`compact_boundary` mit `compact_metadata` (`trigger: "manual"`, `pre_tokens: 64301`, `post_tokens: 3888`), danach eine `user`-Zeile mit `isSynthetic: true`, deren Text-Inhalt die Zusammenfassung ist (beginnt mit „This session is being continued from a previous conversation“), dann eine `user`-Zeile `<local-command-stdout>Compacted </local-command-stdout>` und `result` mit `num_turns: 0`. Nur aus dem Code der Erweiterung: `trigger: "auto"` bei der automatischen Zusammenfassung. Der Agent „Autark“ schickt `compact_boundary` ohne `compact_metadata`.
- **Abgelehnte Aufrufe:** gemessen mit `--model claude-haiku-5-5 --strict-mcp-config --permission-mode default` ohne `--permission-prompt-tool` (dann lehnt die Kommandozeile jede Rechte-Abfrage selbst ab) und dem Auftrag, mit `Write` eine Datei anzulegen. Es kommen `system`/`permission_denied` (`tool_name`, `tool_use_id`, `message`), das `tool_result` mit `is_error: true` und demselben Text, und in `result` steht `permission_denials` als Liste mit je `tool_name`, `tool_use_id` und `tool_input` (der vollständigen Eingabe des Werkzeugs, hier `file_path` und `content`). Ob die Liste auch Aufrufe enthält, die der User über `can_use_tool` mit `deny` beantwortet hat, ist nicht gemessen.

## Rückfragen und Rechte-Abfragen beantworten

Auf `can_use_tool` antwortet der Aufrufer mit derselben `request_id`:

```json
{"type":"control_response","response":{"subtype":"success","request_id":"<id>","response":{"behavior":"allow","updatedInput":{ …input… }}}}
{"type":"control_response","response":{"subtype":"success","request_id":"<id>","response":{"behavior":"deny","message":"Grund für den Agenten"}}}
```

- **`AskUserQuestion`:** `input.questions[]` mit `question`, `header`, `options[]` (`label`, `description`), `multiSelect`. Antwort: `allow` mit `updatedInput` = Eingabe plus `answers: { "<question>": "<gewähltes label>" }`. Geprüft: der Agent erhielt „Rot oder Blau?"="Blau" und antwortete „Blau.“
- `deny` ist aus dem SDK-Protokoll übernommen, nicht einzeln geprüft.

## Unterbrechen

`interrupt` wird mit `{"still_queued":[]}` bestätigt, danach kommen `user` mit `[Request interrupted by user]` und `result` mit `subtype: "error_during_execution"`, `is_error: true`, `terminal_reason: "aborted_streaming"`. Der Prozess bleibt offen und nimmt die nächste Nachricht an.

## Anhänge

Belegt am 2026-09-29 mit Claude Code 2.1.284 (Modell Haiku 4.5), per Probe-Skript.

- Eine Nachricht darf statt eines Strings ein Array von Inhaltsblöcken tragen: `{"type":"user","message":{"role":"user","content":[{"type":"text","text":"…"},{"type":"image","source":{"type":"base64","media_type":"image/png","data":"…"}}]}}`. Geprüft: der Agent erkannte die Farbe eines 16×16-PNG.
- PDFs gehen als `{"type":"document","source":{"type":"base64","media_type":"application/pdf","data":"…"}}`. Geprüft: der Agent las das Wort aus einem einseitigen PDF.
- Nicht geprüft: Größengrenzen der Kommandozeile, sehr lange stdin-Zeilen (ein PDF mit mehreren MB), und ob Bilder im Verlauf nach `--resume` erhalten bleiben. Die API-Grenze für ein Bild liegt bei 5 MB.

## Skills per Nachricht

- Eine Nachricht, deren Text mit `/<skill-name>` beginnt, führt den Skill aus — als String-Inhalt und ebenso als erster Textblock eines Block-Arrays. Geprüft mit einem Skill aus `<repo>\.claude\skills` (per `--add-dir`): `/probe-skill APFEL` → Antwort nach Skill-Anweisung.
- Die Kommandozeile meldet das Laden eines Skills nicht als eigenes Ereignis; es folgen nur Gedankengang und Antwort.
- `system/init` → `skills` ist eine Liste von **Namen** ohne Herkunft und Beschreibung, gemischt aus Benutzer-, Repository-, Plugin- und eingebauten Skills (Plugin-Skills mit Präfix, z.B. `anthropic-skills:pdf`). `slash_commands` enthält zusätzlich die Befehle aus `~\.claude\commands` und eingebaute Befehle der Kommandozeile. Der Verwalter speichert die Liste und zeigt sie im `/`-Menü (ADR 034). Es fehlen die interaktiven Befehle `/permissions`, `/hooks`, `/resume`, `/memory`, `/status` und `/login`; `/permissions` antwortet „isn't available in this environment“. Örtliche Befehle antworten ohne Modell-Lauf mit einer `assistant`-Zeile (`model: "<synthetic>"`) und einem `result` mit `num_turns: 0`; `/doctor` startet dagegen einen Agenten-Lauf. Stand 2.1.293, 2026-10-21.

## Befehle im Vordergrund (Bash)

- Erfolgreicher Aufruf: `tool_result` mit der Ausgabe, `tool_use_result` = `{"stdout":…,"stderr":…,"interrupted":false,…}` an der `user`-Zeile.
- Exit-Code ungleich 0: `tool_result.is_error` ist gesetzt, der Inhalt beginnt mit `Exit code <n>`, danach stdout und stderr (geprüft: `"Exit code 3\neins\nzwei"`); `tool_use_result` ist dann ein String `"Error: Exit code 3…"`.

## Hintergrundprozesse und Subagenten

- Bash mit `run_in_background: true`: nach der Freigabe kommen `background_tasks_changed` (Liste der laufenden: `task_id`, `task_type`, `description`) und `task_started` (`task_id`, `tool_use_id`, `description`, `is_backgrounded: true`, `task_type: "local_bash"`). Das `tool_result` kommt sofort („Command running in background with ID: … Output is being written to: <Datei>“).
- Die Ausgabe schreibt die Kommandozeile laufend in `%TEMP%\claude\<cwd-slug>\<session-id>\tasks\<task_id>.output`; die Datei ist während des Laufs lesbar und endet nach dem Ende mit `[exited with code <n>]`. Geprüft: eine Zeile `Local: http://localhost:5173/` stand dort, während der Prozess lief.
- Ende: `task_updated` (`patch.status`: `completed`, `failed`, `killed`) und `task_notification` (`task_id`, `tool_use_id`, `status`: `completed`, `failed`, `stopped`; `output_file`; `summary`, bei Bash mit Fehler z.B. `Background command "…" failed with exit code 4`).
- Subagent (Werkzeug `Agent`): `task_started` mit `task_type: "local_agent"`, `subagent_type`, `prompt`, `is_backgrounded` (`true` bei asynchronem, `false` bei wartendem Aufruf). Das `tool_result` eines asynchronen Aufrufs kommt sofort; `tool_use_result` trägt `resolvedModel` (Modell-ID des Subagenten) und `agentId`. Laufend: `task_progress` mit `usage.tool_uses`, `usage.total_tokens`, `usage.duration_ms`, `last_tool_name`. Ende: `task_notification` wie oben, `summary` = Ergebnistext, dazu `usage`.
- Nachrichten des Subagenten erscheinen auch **ohne** `--forward-subagent-text` als `assistant`- und `user`-Zeilen mit `parent_tool_use_id` = `tool_use_id` des `Agent`-Aufrufs: Gedankengang, Text, `tool_use` und `tool_result` seiner eigenen Werkzeuge. Die erste `user`-Zeile ist der Auftrag als Text.
- **Selbstständiges Aufwachen:** Endet ein Hintergrundprozess oder ein asynchroner Subagent, nachdem die Antwort schon mit `result` abgeschlossen war, startet der Agent ohne neue Nachricht eine weitere Runde (`system/init`, Gedankengang, Text, `result`). Geprüft für beide Fälle.

## Steueranfragen

- `{"subtype":"stop_task","task_id":"<id>"}` beendet einen Hintergrundprozess **und** einen laufenden Subagenten (geprüft für beide, auch für einen wartenden Subagenten). Bestätigung `control_response` mit `subtype: "success"`, danach `task_updated` (`killed`) und `task_notification` (`stopped`). Ein wartender `Agent`-Aufruf endet mit dem `tool_result` `[Request interrupted by user for tool use]` (`is_error`).
- Ein unbekannter Subtyp wird mit `{"subtype":"error","error":"Unsupported control request subtype: …"}` beantwortet; der Prozess läuft weiter.

## Scratchpad

- `system/init` meldete `scratchpad_path`, einen eigenen Ordner der Kommandozeile je Session: `%TEMP%\claude\<cwd-slug>\<session-id>\scratchpad`. Nach `--resume` derselben Session ist der Pfad unverändert (geprüft). Die Kommandozeile legte den Ordner selbst an: nach jedem Probe-Lauf existierte er, auch wenn der Agent dort nichts abgelegt hatte. Daneben liegen `tasks\` (Ausgabedateien) und bei Bild-Anhängen `images\`.
- **Seit spätestens 2.1.287 fehlt das Feld im Betrieb mit `-p`** (geprüft am 2026-10-05 mit `-p --output-format stream-json --verbose`), und unter `%TEMP%\claude\<cwd-slug>\<session-id>\` liegt nur noch `tasks\`. Interaktiv gestartete Sessions bekommen den Ordner weiter. Die App gibt deshalb einen eigenen vor ([ADR 022](../decisions/022-scratchpad-im-workspace.md)).

## Kontext und Kontingent abfragen

Geprüft am 2026-09-29 mit Claude Code 2.1.284; beide Anfragen stammen aus dem Agent SDK (`getContextUsage`, `usage_EXPERIMENTAL_MAY_CHANGE_DO_NOT_RELY_ON_THIS_API_YET`), die VS-Code-Erweiterung benutzt sie für ihre Fenster „Context usage“ und „Account & Usage“.

- Beide funktionieren sofort nach dem Start, **ohne** vorher eine Nachricht zu senden und ohne `initialize`. Sie lösen keine Modellantwort aus; ob sie Kontingent kosten, ist nicht isoliert gemessen (während der Proben lief parallel eine andere Session).
- `{"subtype":"get_context_usage"}` → `response` mit `categories` (je `name`, `tokens`, `color` (Themenname der Kommandozeile, für uns wertlos), `kind`: `used`, `free` oder `deferred`; `deferred` = nachladbare Werkzeuge, die **nicht** im Kontext liegen), `totalTokens`, `maxTokens`, `rawMaxTokens`, `percentage`, `model`, `memoryFiles` (je `path`, `type`, `tokens`), `mcpTools`, `agents`, `skills` (`totalSkills`, `tokens`, …), `slashCommands`, `autoCompactThreshold` (z.B. 167000 bei 200000), `isAutoCompactEnabled`, `messageBreakdown`, `gridRows` (fertiges Kästchenraster der Kommandozeile, für uns wertlos), `apiUsage`. Beobachtete Kategorien: `System prompt`, `System tools`, `MCP tools`, `MCP server instructions`, `Memory files`, `Skills`, `Messages`, `Free space` (`free`), dazu `MCP tools (deferred)`, `System tools (deferred)` (`deferred`). Eine Kategorie „Autocompact buffer“ kam in der Probe nicht vor; `Free space` war genau `maxTokens − totalTokens`.
- `{"subtype":"get_usage"}` → `response` mit `session` (Kosten und Dauer **dieses** Prozesses), `subscription_type` (z.B. `pro`), `rate_limits_available`, `rate_limits` und `behaviors`.
  - `rate_limits.limits`: Liste mit je `kind` (beobachtet `session` = 5-Stunden-Fenster, `weekly_all` = Woche), `group`, `percent` (0–100), `severity` (beobachtet nur `normal`), `resets_at` (ISO-8601 mit Zeitzone), `scope`, `is_active`. Daneben dieselben Werte noch einmal als `five_hour`/`seven_day` (`utilization`, `resets_at`) und viele Felder mit Codenamen, die `null` sind.
  - `behaviors.day` und `behaviors.week`: `request_count`, `session_count`, `behaviors` (je `key`, `pct`, `count`; beobachtet `long_context`, `cron`, `high_parallel`), `skills` (je `name`, `pct`), `agents`, `plugins`, `mcp_servers`. Die VS-Code-Erweiterung zeigt `long_context` als „usage at >150k context“, `cron` als „sessions active for 8+ hours“, `high_parallel` als „while 4+ sessions ran in parallel“ und nennt das Ganze eine Näherung aus den lokalen Sessions dieses Rechners.
  - Der Name im SDK sagt ausdrücklich, dass sich das Format ändern kann — jedes Feld ist optional zu lesen.
- **Hilfsprozess nur für das Kontingent:** `claude.exe -p --input-format stream-json --output-format stream-json --verbose --strict-mcp-config --no-session-persistence` mit Arbeitsverzeichnis `<Benutzerordner>\.verwalter`, dann sofort `get_usage` senden: Antwort nach rund 1,4 s, danach Prozess beenden. `--strict-mcp-config` ohne `--mcp-config` startet keine MCP-Server. Es entsteht kein Transkript; die Kommandozeile legt nur einmalig einen leeren Ordner `~\.claude\projects\C--Users-<name>--verwalter\memory` an. `--bare` ist ungeeignet (meldet sich nur mit API-Schlüssel an, nicht mit dem Abo).

## MCP-Server steuern

Gemessen am 2026-10-01 mit der Claude-Kommandozeile im Druckmodus mit `--input-format stream-json`; das Format ist nicht als stabil dokumentiert, jedes Feld ist optional zu lesen.

- `{"subtype":"mcp_status"}` → `control_response` `success` mit `response.mcpServers`: je Server `name`, `status`, `scope`, `source`, `config` (`type` `stdio` mit `command`/`args`, `claudeai-proxy` mit `url`), ab `connected` zusätzlich `serverInfo` und `tools` (je `name`, `annotations`), bei `failed` zusätzlich `error` (gemessen `"Connection closed"`). Beobachtete `status`: `pending`, `connected`, `failed`, `disabled`; beobachtete `scope`: `user`, `claudeai`, `dynamic`.
- Direkt nach dem Start stehen claude.ai-Server auf `pending` und wechseln binnen ~10 s auf `connected` — **ohne eigenes Ereignis**; wer den Wechsel sehen will, fragt erneut.
- `{"subtype":"mcp_reconnect","serverName":"<name>"}` → bei Erfolg `success` **ohne** `response`; bei Fehlschlag `{"subtype":"error","error":"Connection closed"}`. Die Antwort kommt erst nach dem Verbindungsversuch.
- `{"subtype":"mcp_toggle","serverName":"<name>","enabled":false|true}` → `success` ohne `response`; unbekannter Name → `error` `"Server not found: <name>"`. Danach meldet `mcp_status` `disabled` bzw. `pending` → `connected`.
- **Ausschalten bleibt gespeichert, pro Arbeitsordner:** ein neuer Prozess im selben Arbeitsordner meldet den Server weiter `disabled`, ein Prozess in einem anderen Ordner `pending`.
- `needs-auth` (Anmeldung nötig) gemessen am 2026-10-05 mit Claude Code 2.1.287: claude.ai-Connectoren ohne Anmeldung (Gamma, Atlassian Rovo) melden `"status":"needs-auth"`, `scope` `claudeai`, `config.type` `claudeai-proxy`.
- `{"subtype":"mcp_authenticate","serverName":"<name>"}` (ohne `redirectUri`) → gemessen für claude.ai: `success` mit `response` `{"authUrl":"https://claude.ai/api/organizations/<org>/mcp/start-auth/<server>","requiresUserAction":true,"callbackExpected":false}`; danach verbindet die Kommandozeile **nicht** selbst neu. Unbekannter Name → `error` `"Server not found: <name>"`. Nur aus dem Programmcode gelesen, nicht gemessen: eigene HTTP-Server öffnen den Browser nicht selbst, die Kommandozeile lauscht auf einem lokalen Port und antwortet `{"authUrl":…,"requiresUserAction":true,"callbackExpected":true,"redirectScheme":"localhost","state":…,"callbackPort":…}` und verbindet nach dem Rücksprung selbst; mit gültigem Token `{"requiresUserAction":false,"callbackExpected":false}` ohne `authUrl` ([ADR 024](../decisions/024-mcp-anmeldung.md)).
- Die Antworten auf `mcp_reconnect`/`mcp_toggle` tragen nur die `request_id`; der Core ordnet sie darüber der Aktion zu ([ADR 013](../decisions/013-mcp-server-im-dialog.md)).

## Claude in Chrome

Gemessen am 2026-10-19 mit Claude Code 2.1.293 ([ADR 030](../decisions/030-browser-zugriff.md)):

- `claude --help` kennt `--chrome` („Enable Claude in Chrome integration“) und `--no-chrome`.
- `claude -p --chrome --output-format stream-json --verbose` meldet in `system/init` den MCP-Server `claude-in-chrome` (Status `connected`, Herkunft `dynamic`) samt 22 Werkzeugen `mcp__claude-in-chrome__*`. Der Druckmodus trägt die Option also.
- `/chrome` steht im Druckmodus **nicht** in `slash_commands`: der Befehl ist interaktiv und kommt beim Agenten nicht an.
- Nicht gemessen: `--resume` zusammen mit `--chrome`, die Freigabe der Browser-Werkzeuge über `--permission-prompt-tool stdio` und der Status des Servers ohne laufendes Chrome.

## Einmal-Aufruf im Druckmodus (TL;DR)

Für das TL;DR läuft ein eigener, kurzlebiger `claude.exe` ohne Stream-Protokoll, unabhängig vom Agenten der Session (`src-tauri/src/agents/claude/print.rs`):

```text
claude.exe -p --model claude-haiku-4-5-20251001 --tools "" --safe-mode --strict-mcp-config --no-session-persistence --system-prompt <Anweisung> --json-schema <Schema> --output-format json
```

- Eingabe über die Standardeingabe, danach wird sie geschlossen; Arbeitsordner `<Benutzerordner>\.verwalter`. `--tools` nimmt mehrere Werte: der leere Wert schaltet alle Werkzeuge ab, das folgende `--safe-mode` beendet die Liste.
- Ausgabe: **ein** JSON-Objekt. Die Antwort nach dem Schema steht in `structured_output`, dazu `is_error`, `result` (Text, bei Fehlern die Meldung), `duration_ms`, `usage`, `total_cost_usd`.
- Läuft mit der Abo-Anmeldung (`--bare` nicht, siehe oben).
- **Gemessen am 2026-09-30 mit Claude Code 2.1.284** (Planung): kurze Eingabe, 1 455 Eingabe-Tokens Grundlast samt Eingabe, 268 Ausgabe-Tokens, 0,38 Cent.
- **Gemessen am 2026-09-30 mit Claude Code 2.1.284** (Umsetzung; der erste Lauf über `run_print` mit der echten Anweisung, die übrigen mit demselben Schema und einer gekürzten Anweisung, weil nur sie `usage` zeigen): der Verlauf mit den meisten Chat-Einträgen (235 Einträge, als Transkript 5 844 Zeichen) → 29,6 s beim ersten Lauf, 14,1 s beim zweiten; 3 495 Eingabe-Tokens, 1 164 Ausgabe-Tokens (davon 616 Denk-Tokens — Haiku denkt im Druckmodus mit), 1,2 Cent. Ein künstliches Transkript an der Obergrenze (292 298 Zeichen) → 18,7 s und 17,0 s; 107 169 Eingabe-Tokens, die die Kommandozeile in den 1-Stunden-Cache schreibt (doppelter Eingabepreis), 1 340 Ausgabe-Tokens, 33 Cent. `structured_output` kam in allen Läufen. Daraus das Zeitlimit: 30 s × 3 = 90 s.

## Lokales Modell über `ANTHROPIC_BASE_URL`

Gemessen am 2026-10-04 mit Claude Code 2.1.284, LM Studio mit `gemma4-12b-qat-uncensored-hauhaucs-balanced` (Q4_K_M, `vlm`, Kontext 64 000), Umgebung wie in `docs/planning/2026-10-01_claude-code-lokal/artifacts/messung.ps1` (`ANTHROPIC_BASE_URL=http://localhost:1234`, `ANTHROPIC_AUTH_TOKEN=lmstudio`, `CLAUDE_CODE_ATTRIBUTION_HEADER=0`, alle `ANTHROPIC_DEFAULT_*_MODEL` und `CLAUDE_CODE_SUBAGENT_MODEL` auf das Modell, `CLAUDE_CODE_MAX_CONTEXT_TOKENS=64000`):

- **Netzverkehr:** ohne `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC` und `CLAUDE_CODE_DISABLE_OFFICIAL_MARKETPLACE_AUTOINSTALL` geht beim Start eine Verbindung an GitHub (`140.82.121.6:443`, `lb-140-82-121-6-fra.github.com`) und eine an `2607:6bc0::10:443` (kein Reverse-Eintrag; dieselbe Adresse zeigt sich bei einem normalen Claude-Lauf). Mit beiden Schaltern: in fünf Läufen (Kurzlauf, zwei Werkzeug-Läufe, TL;DR, Fortsetzen) keine einzige Verbindung außer Loopback. Abgetastet wird alle 200 ms — eine Verbindung unter 200 ms Dauer fiele durch, der Gegenlauf zeigt aber, dass die Messung kurze Verbindungen sieht.
- **Kontextfenster:** `CLAUDE_CODE_MAX_CONTEXT_TOKENS` wirkt, `modelUsage.<modell>.contextWindow` steht auf 64000 statt 200000. Beim Fortsetzen einer Claude-Session (`--resume`) enthält `modelUsage` zusätzlich den Eintrag des früheren Claude-Modells mit `contextWindow` 200000 — das ist die mitgeführte Verlaufs-Statistik, kein Aufruf an Anthropic (keine Verbindung nach außen im selben Lauf).
- **Eingabegröße je Anfrage:** 31 000 bis 38 000 Token schon für „Antworte nur mit OK“ (Systemanweisung, Werkzeug-Beschreibungen, Anweisungen des Benutzers). Bei 64 000 Kontext bleibt wenig Platz; eine Aufgabe mit mehreren Werkzeug-Aufrufen füllt ihn.
- **Tempo:** erstes Token nach 45 bis 73 s, auch bei der kürzesten Anfrage. Eine Aufgabe mit einem Werkzeug-Aufruf (2 Anfragen) dauerte 127 s. `cacheReadInputTokens` ist im Lauf mit fünf Anfragen größer als null (LM Studio verwendet den Prompt-Anfang wieder), im Lauf mit zwei Anfragen null.
- **Zuverlässigkeit:** dieselbe Aufgabe („Lies STATE.md, nenne den nächsten Schritt“, `--max-turns 4`) gelang einmal in 2 Turns und scheiterte einmal nach 13,6 min mit `error_max_turns` (das Modell rief weiter Werkzeuge auf, statt zu antworten).
- **TL;DR (`--json-schema`):** funktioniert, `structured_output` kommt mit `is_error: false` in 9 s, 273 Eingabe-Token (mit `--tools ""` und `--safe-mode` ist die Grundlast klein).
- **Fortsetzen (`--resume`):** eine mit Claude begonnene Session (`--session-id`) lässt sich mit dem lokalen Modell fortsetzen; das Modell kannte das gemerkte Wort. Der Lauf war langsam (erstes Token nach 224 s) und löste eine Anfrage mit `query_source: compact` aus.
- **Stderr bei jedem lokalen Lauf:** Hinweis, dass claude.ai-Connectoren abgeschaltet sind, `[claude-code:unrecognized_model]` für den unbekannten Modellnamen (nur Meldung, die Läufe liefen durch) und ein Hinweis zur Abrechnung der Auto-Modus-Prüfung, der für lokale Läufe nichts bewirkt.
- **Kosten:** `total_cost_usd` ist bei lokalen Läufen ein Rechenwert der Kommandozeile für ein unbekanntes Modell, kein echter Betrag.

## Nachbau im eigenen Agenten

Der eigene Agent der Betriebsart „Autark“ spricht genau den Ausschnitt dieses Zeilenprotokolls, den der Verwalter liest und schreibt (Kontrakt: `docs/archive/2026-10/2026-10-01_autarker-agent/README.md`; Code: `src-tauri/src/standalone/`, [ADR 017](../decisions/017-autarker-agent.md)). Wer `translate.rs` oder `protocol.rs` ändert, prüft dort mit: Ausgabezeilen in `standalone/output.rs`, Steueranfragen in `standalone/session.rs`.
