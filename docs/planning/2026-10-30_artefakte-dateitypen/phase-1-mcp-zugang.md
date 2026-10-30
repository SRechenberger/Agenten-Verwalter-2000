# Phase 1 — MCP-Zugang des Verwalters

**Rating:** heikel (Sicherheit, neuer Kontrakt über Feature-Grenzen) · **Wave:** 1 · **Aufwand:** L

Ein eigener MCP-Server im Verwalter, über den jeder Agent einer Session Werkzeuge des Verwalters aufruft. Erste Werkzeuge: `artifact_list`, `artifact_show`. Der Plan „Umsetzung“ hängt später seinen Werkzeug-Satz an dieselbe Stelle.

## Kontext — vor dem Start lesen

- `README.md` dieses Plans, Abschnitt **Kontrakt → MCP-Zugang** (verbindlich: Köpfe, Statuscodes, JSON-RPC-Antworten, `ToolSet`, Werkzeug-Schemas, Konfigurationsdatei).
- `docs/decisions/026-artefakte.md` — warum der Artefakt-Server auf `127.0.0.1` läuft, Token, `Host`-Prüfung.
- `src-tauri/src/artifacts/server.rs` — Vorbild für Listener, Thread je Verbindung, `read_head`, `write_reply`, `percent_*`. **Verstanden:** Der Server beantwortet nur `GET`, liest keinen Körper, prüft `Host` gegen DNS-Rebinding und setzt bei Datei-Antworten `Access-Control-Allow-Origin: *` und eine CSP je Antwort. Diese Datei wird in dieser Phase **nicht** geändert.
- `src-tauri/src/standalone/mcp/client.rs` — Gegenstück als Client: `PROTOCOL_VERSION` (Zeile 25, `pub`), `JSON_RPC_VERSION` (privat, Zeile 26 — für den Server wird sie `pub(crate)`), Köpfe `Mcp-Session-Id`/`MCP-Protocol-Version`, `notify` erwartet nur einen Erfolgsstatus, `send` liest `application/json` oder `text/event-stream`. Der autarke Agent spricht damit gegen den neuen Server.
- `src-tauri/src/standalone/mcp/config.rs` — liest `--mcp-config`, Typ `http` mit `url` und `headers` (Zeilen 183–207).
- `src-tauri/src/agents/claude/process.rs` — `SpawnOptions` (Zeilen 16–44), `common_args` hängt `--mcp-config` an (Zeilen 204–209), `project_mcp_configs` (Zeilen 264–270).
- `src-tauri/src/sessions/registry.rs` um Zeile 3749 — dort entsteht `SpawnOptions` für den Agenten einer Session; `src-tauri/src/sessions/registry/side.rs` Zeile 103 — Nebenfragen (bekommen keinen Zugang).
- `src-tauri/src/lib.rs` Zeilen 60–90 — `setup`: Registry und Artefakt-Server werden dort gestartet und mit `app.manage` verwaltet.
- `src-tauri/src/artifacts/mod.rs` — `dir`, `list`, `resolve`; `src-tauri/src/sessions/registry/artifacts.rs` — `artifacts_dir`, `artifact_scope`; `src-tauri/src/commands/artifacts.rs` — wie `artifacts_list` beide verbindet (Vorbild für `artifact_list`).
- `src/app/App.tsx` (Weiche der Ansichten), `src/stores/sessions.ts` (`activeView`), `src/stores/artifacts.ts` (`select`) — Ziel von `artifacts://show`.
- `src-tauri/src/codex/rpc.rs` Zeile 58 ff. (Start von `codex app-server`), `src-tauri/src/codex/session.rs` (wo `thread/start` gesendet wird).
- Projektregeln: `docs/conventions/rust.md`, `docs/conventions/react.md`, `docs/conventions/linting.md`.
- Fehlerklassen geprüft: im Vault keine Entities zu Rust, Tauri oder MCP — keine einschlägig.

## Entscheidungen (fallen hier, nicht beim Umsetzer)

- Neuer Core-Ordner `src-tauri/src/mcp_access/` (der Name `mcp` ist für den MCP-Dialog belegt): `mod.rs` (`McpAccess` State, `start`, `config_for`, `tool_sets`), `http.rs` (Anfrage mit Körper lesen, Antwort schreiben), `rpc.rs` (JSON-RPC-Weiche), `tools.rs` (`ToolSet`, `ToolSpec`, `ToolCall`, `ToolResult`).
- Eigener Port statt eines Pfads im Artefakt-Server: Antworten des Artefakt-Servers tragen `Access-Control-Allow-Origin: *`; ein getrennter Listener schließt aus, dass das je für MCP gilt, und die CSP der Artefakte (`connect-src <scope> https:`) erreicht einen anderen Port ohnehin nicht.
- Schlüssel nur im Speicher, neu je App-Start und Session; die Konfigurationsdatei wird bei **jedem** Agenten-Start neu geschrieben (Port wechselt je App-Start). Keine Datenbank-Spalte.
- Antworten nur als `application/json`, kein Ereignis-Strom, kein `GET`-Kanal für Server-Nachrichten.
- Codex: Ist im Protokoll von `codex app-server` ein Feld für Konfigurations-Überschreibungen beim Thread-Start vorhanden (`thread/start` → `config`, Schlüssel `mcp_servers.verwalter.url` und `mcp_servers.verwalter.http_headers`), wird es genutzt. Sonst bleibt Codex ohne Zugang, und ADR 046 hält das als bekannte Lücke fest. Prüfweg siehe Checkliste.

## Abnahmekriterien

- [ ] Beim App-Start lauscht der MCP-Zugang auf `127.0.0.1:<zufälliger Port>`; `McpAccess` ist als Tauri-State verwaltet.
- [ ] `POST /mcp` mit gültigem Schlüssel und `initialize` liefert `200`, `serverInfo.name = "verwalter"`, Kopf `Mcp-Session-Id`.
- [ ] Ohne `Authorization` oder mit falschem Schlüssel: `401`; falscher `Host`: `403`; `GET` oder `OPTIONS`: `405`; anderer `Content-Type`: `415`; Körper > 1 MiB: `413`. Keine Antwort trägt einen `Access-Control-*`-Kopf.
- [ ] `tools/list` liefert genau `artifact_list` und `artifact_show` mit den Schemas aus dem Kontrakt.
- [ ] `artifact_list` liefert die Artefakte des Vorhabens der Session, zu der der Schlüssel gehört — und nur diese.
- [ ] `artifact_show` mit vorhandener Datei wählt sie im Reiter aus; ist die Session sichtbar, wechselt die Ansicht auf „Artefakte“. Mit unbekannter Datei: `isError: true`, Text `Kein Artefakt: <file>`.
- [ ] Ein Agent in den Betriebsarten Claude und Autark bekommt `--mcp-config <Benutzerordner>\.verwalter\mcp\<session-id>.json` und ruft beide Werkzeuge erfolgreich auf; eine Nebenfrage bekommt die Datei nicht.
- [ ] Codex: Ergebnis der Prüfung umgesetzt (Zugang aktiv) oder als Lücke in ADR 046 festgehalten.
- [ ] Eine Artefakt-Seite, die per `fetch` an den MCP-Port schickt, scheitert.

## Checkliste

- [ ] `client.rs`: `JSON_RPC_VERSION` auf `pub(crate)` stellen.
- [ ] `mcp_access/http.rs`: Anfrage lesen — Kopf wie `read_head` in `artifacts/server.rs` (Kopf ≤ 16 KiB, 5 s Lese-Zeitlimit), danach genau `Content-Length` Bytes Körper (fehlend → 411, > 1 MiB → 413). Kopfzeilen als `Vec<(String, String)>`, Namen ohne Beachtung der Groß-/Kleinschreibung gesucht. Antwort schreiben mit `Content-Type`, `Content-Length`, `Cache-Control: no-store`, `Connection: close`, optional `Mcp-Session-Id`; nie `Access-Control-*`.
- [ ] `mcp_access/mod.rs`: `McpAccess { origin: String, keys: Mutex<HashMap<String, String>>, config_dir: PathBuf }`; `start(app: AppHandle) -> io::Result<Arc<McpAccess>>` bindet `127.0.0.1:0`, Thread `mcp-access`, je Verbindung Thread `mcp-request` (wie `artifacts::server::start`). Prüfreihenfolge je Anfrage: Host → Methode → Schlüssel → Content-Type → Körper → JSON-RPC.
- [ ] `McpAccess::config_for(&self, session_id: &str) -> Option<PathBuf>`: vorhandenen Schlüssel der Session wiederverwenden, sonst neu erzeugen; Datei `config_dir/<session-id>.json` mit dem Inhalt aus dem Kontrakt schreiben (`serde_json::to_vec_pretty`); Schreibfehler → `None` und `eprintln!`, der Agent startet dann ohne Zugang. `config_dir` = `filesystem::workspace::data_dir()` + `mcp` (Ordner anlegen).
- [ ] `McpAccess::forget(&self, session_id)`: Schlüssel entfernen und Datei löschen; aufrufen, wo eine Session gelöscht wird (`delete_session` in `sessions/registry.rs`).
- [ ] `mcp_access/rpc.rs`: Weiche nach Kontrakt (`initialize`, `ping`, `notifications/*`, `tools/list`, `tools/call`, Fehlercodes `-32700`, `-32601`, `-32602`). `tools/call` sucht den Satz über das Präfix des Namens; kein Satz → `-32602` mit `Unbekanntes Werkzeug: <name>`.
- [ ] `mcp_access/tools.rs`: Typen aus dem Kontrakt; `tool_sets() -> Vec<Box<dyn ToolSet>>` mit genau einem Eintrag `artifacts::tools::ArtifactTools`.
- [ ] `src-tauri/src/artifacts/tools.rs`: `ArtifactTools` mit `artifact_list` (gleiche Daten wie `commands::artifacts::artifacts_list`, über `SessionRegistry::artifact_scope` und `artifacts::list`) und `artifact_show` (Datei gegen `list` prüfen, `project_id` der Session aus der Registry, `app.emit("artifacts://show", …)`). Schemas: `{"type":"object","properties":{},"additionalProperties":false}` bzw. `{"type":"object","properties":{"file":{"type":"string","description":"Dateiname direkt im Ordner .artefakte"}},"required":["file"],"additionalProperties":false}`.
- [ ] `lib.rs` `setup`: `mcp_access::start` nach dem Artefakt-Server; Erfolg → `app.manage(access)`, Fehler → `eprintln!` wie beim Artefakt-Server (App läuft ohne Zugang).
- [ ] `SpawnOptions` um `verwalter_mcp: Option<PathBuf>` erweitern. In `common_args` hinter `project_mcp_configs` anhängen: die Liste für `--mcp-config` ist `project_mcp_configs(..)` plus `verwalter_mcp`, als **ein** `--mcp-config` mit allen Pfaden. In `registry.rs` (Zeile ~3749) `verwalter_mcp` aus `app.try_state::<Arc<McpAccess>>()` → `config_for(&session_id)` setzen; in `registry/side.rs` `None`.
- [ ] Prüfen, dass die Betriebsart Autark `--mcp-config` mit mehreren Pfaden liest (`standalone/args.rs`, `standalone/mcp/config.rs`); falls nur ein Pfad gelesen wird, dort auf mehrere erweitern.
- [ ] Oberfläche: Abo auf `artifacts://show` einmal in `src/app/App.tsx` (Wrapper in `src/lib/artifacts.ts`: `onArtifactShow(handler)`). Handler: `useArtifactsStore.getState().select(projectId, file)`; ist `sessionId` die aktive Session, `activeView` auf `'artifacts'` setzen.
- [ ] **Codex prüfen:** `codex app-server generate-json-schema --out <Scratchpad>` (oder `generate-ts`) ausführen und in `ThreadStartParams` nach einem Feld `config` suchen. Vorhanden → in `codex/session.rs` beim `thread/start` `config` mit `mcp_servers.verwalter.url` und `mcp_servers.verwalter.http_headers = { Authorization = "Bearer <schlüssel>" }` mitsenden (Werte aus der Konfigurationsdatei der Session, die der Übersetzer über `--mcp-config` bekommt). Nicht vorhanden oder Codex nicht installiert → kein Code, Lücke in ADR 046 und als Eintrag in `FINDINGS.md` (`→ Plan Umsetzung`).
- [ ] Prüfprogramm `src-tauri/examples/mcp-probe.rs`: `mcp-probe <url> <schlüssel> [list|call <name> <json>]` — sendet `initialize`, `notifications/initialized`, dann `tools/list` oder `tools/call`, druckt Status, Köpfe und Körper; mit `--negative` zusätzlich die sechs Fehlfälle (ohne Schlüssel, falscher Host, GET, OPTIONS, text/plain, 2 MiB). Aufruf in `AGENTS.md` eintragen. Verwendet `ureq` wie `client.rs` (keine neue Abhängigkeit).
- [ ] Probe-Seite `docs/planning/2026-10-30_artefakte-dateitypen/artifacts/mcp-sicherheitsprobe.html`: liest Port und Schlüssel aus Eingabefeldern, versucht `fetch` (POST, JSON, mit und ohne `Authorization`) an `http://127.0.0.1:<port>/mcp` und zeigt das Ergebnis. In `.artefakte` eines Test-Vorhabens kopieren und im Reiter öffnen.
- [ ] ADR `docs/decisions/046-mcp-zugang.md` (Kontext, Optionen: Pfad im Artefakt-Server / eigener Port / stdio-Hilfsprozess; Entscheidung; Konsequenzen inkl. Codex-Ergebnis).
- [ ] Code-Map: Zeile „MCP-Zugang“ (Oberfläche: Abo in `App.tsx`; Core: `src-tauri/src/mcp_access/`, `artifacts/tools.rs`, `verwalter_mcp` in `process.rs`, Start in `lib.rs`); Querschnitt-Liste um `mcp_access` ergänzen. Glossar: „MCP-Zugang“, „Werkzeug-Satz“.

## Definition of Done

- Alle Abnahmekriterien belegt durch die Ausgabe von `mcp-probe` (normal und `--negative`) gegen die laufende Dev-App, den Screenshot der Probe-Seite im Reiter und einen Chat-Ausschnitt, in dem ein Claude- und ein Autark-Agent beide Werkzeuge aufrufen.
- Sichtbares Ergebnis: „Agent, zeig mir Artefakt X“ → der Reiter springt auf X.
- Artefakte: `artifacts/phase-1-mcp-probe.txt`, `artifacts/phase-1-sicherheitsprobe.png`, `artifacts/phase-1-chat.png`.
- `pnpm check` grün (Ausgabe-Zusammenfassung im Report-Back), `pnpm bindings` falls Typen die Grenze queren.
- Seven-Goals-Check im Report-Back, Schwerpunkt Sicherheit und Simplicity.

## Report-Back
