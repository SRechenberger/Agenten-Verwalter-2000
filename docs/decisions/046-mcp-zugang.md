# 046 — MCP-Zugang: eigener MCP-Server des Verwalters auf `127.0.0.1`

**Status:** angenommen · **Datum:** 2026-10-30

## Kontext

Agenten sollen Werkzeuge des Verwalters aufrufen können: zuerst die Artefakte des Vorhabens auflisten und eines im Reiter „Artefakte“ zeigen, später die Werkzeuge des Plans „Umsetzung“. Alle Betriebsarten sprechen MCP; Claude und der autarke Agent lesen Server aus `--mcp-config` ([ADR 017](017-autarker-agent.md)). Der Artefakt-Server ([ADR 026](026-artefakte.md)) läuft schon auf `127.0.0.1`, liefert aber Seiten des Agenten aus, die beliebiges JavaScript enthalten.

## Optionen

- **(a) Pfad `/mcp` im Artefakt-Server** — verworfen: dessen Datei-Antworten tragen `Access-Control-Allow-Origin: *`, und die CSP der Artefakt-Seiten erlaubt `connect-src` auf genau diesen Ursprung. Eine Artefakt-Seite stünde damit vor der Tür des MCP-Zugangs; nur der Schlüssel hielte sie draußen.
- **(b) Eigener Listener auf eigenem Port** — gewählt: `std::net`, keine neue Abhängigkeit, eigener Thread wie beim Artefakt-Server.
- **(c) stdio-Hilfsprozess, den der Agent startet und der zur App durchreicht** — verworfen: braucht trotzdem einen Kanal in die laufende App, also denselben Listener plus einen weiteren Prozess je Agent.

## Entscheidung

- **Ort:** `127.0.0.1:<Zufalls-Port>/mcp`, gestartet in `setup` nach dem Artefakt-Server (`src-tauri/src/mcp_access/`). Startet er nicht, laufen die Agenten ohne die Werkzeuge des Verwalters.
- **Prüfung je Anfrage in dieser Reihenfolge:** `Host` genau `127.0.0.1:<port>` (403, gegen DNS-Rebinding) · Methode `POST` (405, auch `GET` und `OPTIONS`) · Pfad `/mcp` (404) · `Authorization: Bearer <schlüssel>` mit bekanntem Schlüssel (401) · `Content-Type: application/json` (415) · `Content-Length` vorhanden (411) und höchstens 1 MiB (413). Keine Antwort trägt `Access-Control-*`; ein Browser lässt eine Seite die Antwort also nie lesen, und die Vorab-Anfrage für `Authorization` scheitert mit 405.
- **Schlüssel:** 32 Hex-Zeichen je Session und App-Start, nur im Speicher. Er bindet jeden Aufruf an die Session und damit an ihr Vorhaben; ein Werkzeug sieht nie ein anderes Vorhaben. Beim Löschen der Session werden Schlüssel und Datei entfernt.
- **Übergabe:** Bei jedem Agenten-Start schreibt der Verwalter `<Benutzerordner>\.verwalter\mcp\<session-id>.json` (Server `verwalter`, Typ `http`, Kopf `Authorization`) neu und hängt den Pfad an die Liste von `--mcp-config`. Dazu kommt die Freigabe `mcp__verwalter` in `--allowedTools`: die Werkzeuge lesen nur oder wählen in der eigenen Oberfläche aus, eine Rückfrage je Aufruf wäre Lärm.
- **Protokoll:** JSON-RPC 2.0 mit `initialize` (Version der Anfrage, sonst die des eigenen Clients), `ping`, `notifications/*` (202), `tools/list`, `tools/call`. Antworten nur `application/json`, kein Ereignis-Strom.
- **Werkzeug-Sätze:** Trait `ToolSet` mit Präfix; die Liste steht einmal in `mcp_access::tool_sets()`. Ein Fehler eines Werkzeugs wird `isError: true` mit Text, die Verbindung bleibt.
- **Nebenfragen** bekommen dieselbe Konfigurationsdatei: ihre Werkzeugliste muss der des Hauptprozesses gleichen, sonst verfehlen sie dessen Prompt-Cache ([ADR 044](044-nebenfragen.md)). Aufrufen können sie die Werkzeuge nicht, `--permission-mode dontAsk` lehnt sie ab.
- **Codex:** Auf dem Entwicklungsrechner ist keine `codex.exe` installiert; ob `thread/start` von `codex app-server` Konfigurations-Überschreibungen für MCP-Server annimmt, ist ungeprüft. Codex bleibt vorerst **ohne Zugang** (bekannte Lücke). Der Übersetzer bekommt `--mcp-config` zwar mitgeliefert, reicht es aber nicht an Codex weiter.

## Konsequenzen

- Ein neues Feature mit Werkzeugen für den Agenten legt einen `ToolSet` an und hängt ihn in `tool_sets()` an — kein neuer Server, keine neue Übergabe.
- Die Konfigurationsdatei enthält den Schlüssel im Klartext im Benutzerordner. Er gilt nur, solange die App läuft, und nur für die eine Session.
- Prüfen ohne Agent: `cargo run --example mcp-probe -- <konfigurationsdatei> [list | call <name> <json>] [--negative]`.
- Für Codex braucht es eine eigene Prüfung gegen das Protokoll von `codex app-server`, sobald eine `codex.exe` verfügbar ist.

## Nachtrag 2026-11-03: Session-Werkzeuge

Die Session-Werkzeuge kommen als Werkzeug-Satz `verwalter_` dazu, kein zweiter Server. Dafür ändert sich gegenüber der Entscheidung oben:

- **Ereignis-Strom für blockierende Werkzeuge:** `tools/call` eines Satzes mit `blocks()` antwortet als `text/event-stream` und schickt alle 30 s `notifications/progress` mit dem `progressToken` aus `_meta`, zuletzt `event: message` mit dem Ergebnis. Gemessen: reines JSON bricht die Kommandozeile nach rund 60 s ab, Kommentarzeilen nach 300 s, `notifications/progress` hielt 3000 s. Alle anderen Antworten bleiben `application/json`. `notifications/cancelled` und eine abgerissene Verbindung erreichen den Satz über `cancel`.
- **Schlüssel in der Umgebung im Claude-Betrieb:** Die Datei unter `.verwalter\mcp\` trägt dort nur `Bearer ${VERWALTER_MCP_TOKEN}`, den Schlüssel bekommt der Prozess in seiner Umgebung. Sonst könnte jeder Agent aus dem Ordner den Schlüssel einer anderen Session lesen und als sie sprechen. Außerhalb des Claude-Betriebs bleibt der Schlüssel in der Datei: Ob der eigene Agent `${…}` auflöst, ist ungeprüft, und dort gibt es die Session-Werkzeuge nicht. Ein Wechsel der Betriebsart ersetzt den Schlüssel.
- **Eigener Schlüssel der Nebenfrage:** Im Claude-Betrieb gehört zu jeder Session ein Schlüssel für den Hauptprozess und einer für Nebenfragen. Beide lesen dieselbe Datei, ihre Argumente bleiben gleich (ADR 044). Der Schlüssel der Nebenfrage darf von den Session-Werkzeugen nur `verwalter_get_sessions`, und sein `notifications/cancelled` wirkt nicht: Ihre JSON-RPC-IDs zählen unabhängig und träfen sonst den Kasten des Hauptprozesses.
- **Sichtbarkeit:** Ein Satz mit `requires_claude` erscheint nur für einen Schlüssel aus dem Claude-Betrieb; sonst sind seine Werkzeuge unbekannt.
- **Plan-Modus:** Trotz Freigabe fragt die Kommandozeile im Modus `plan` nach (`can_use_tool`); der Core erlaubt alles mit Präfix `mcp__verwalter__` ohne Kasten. Das gilt damit auch für `artifact_*`, die nur lesen und auswählen.

**Korrektur:** Oben steht, `--permission-mode dontAsk` lehne die Werkzeuge für Nebenfragen ab. Das stimmt nicht: `dontAsk` lehnt nicht ab, was `--allowedTools` freigibt (gemessen). Eine Nebenfrage kann `artifact_*` also aufrufen; das ist hingenommen, weil sie nur lesen und auswählen. Die Session-Werkzeuge sperrt ihr eigener Schlüssel.
