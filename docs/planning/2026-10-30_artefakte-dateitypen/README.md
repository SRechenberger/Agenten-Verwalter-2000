# Plan: MCP-Zugang und Artefakte aller Dateitypen

Der Verwalter bekommt einen eigenen MCP-Zugang, über den Agenten Werkzeuge des Verwalters aufrufen (erste Werkzeuge: `artifact_list`, `artifact_show`; danach baut der Plan „Umsetzung“ seine Werkzeuge darauf). Danach wird der Reiter „Artefakte“ vom reinen HTML-Betrachter zur Ansicht für jedes Medium, das einen Sachverhalt veranschaulicht: Dokumente, Bilder, Grafiken, Diagramme, Videos, Ton, strukturierte Daten, Ordner. Die Liste gruppiert die Artefakte nach der Session, die sie angelegt hat.

**Artefakt (Definition):** eine Datei oder ein Ordner, den ein Agent für den Menschen im Vorhaben ablegt, um einen Sachverhalt zu veranschaulichen oder zum Ausprobieren anzubieten — Bericht, Dokument, Grafik, Diagramm, Bild, Video, Ton, strukturierte Daten. Ein Artefakt ist Anschauungsmaterial, kein Arbeitsergebnis im Repository. Prüfstein: Ist die Datei zum Anschauen gemacht oder für das Programm?

## Überblick

| Phase | Inhalt | Rating | Wave | Aufwand | Status |
|---|---|---|---|---|---|
| 1 | [MCP-Zugang des Verwalters](phase-1-mcp-zugang.md) | heikel | 1 | L | pending |
| 2 | [Typ-Erkennung, Gruppierung nach Session, Medien](phase-2-typen-und-gruppen.md) | heikel | 2 | L | pending |
| 3 | [Text, Code, Log, Diff, Markdown](phase-3-text-und-markdown.md) | standard | 3 | M | pending |
| 4 | [Strukturierte Daten: JSON, JSON Lines, YAML, TOML/INI/.env, XML, CSV](phase-4-daten.md) | standard | 4 | M | pending |
| 5 | [PDF, Mermaid, Ordner](phase-5-pdf-diagramme-ordner.md) | standard | 5 | L | pending |
| 6 | [Doku, Anweisung an den Agenten, Release](phase-6-doku-und-release.md) | mechanisch | 6 | S | pending |

Rein sequenziell, weil Phase 2 den Typ-Katalog und die Betrachter-Weiche festlegt, in die 3–5 je ihre Zeilen eintragen (dieselben Dateien `viewers/mod.rs` und `ArtifactKind`), und Phase 1 den MCP-Zugang liefert, dessen `artifact_list` in Phase 2 die neuen Felder bekommt.

Grobschätzung gesamt: 6–8 Umsetzungs-Sessions (L ≈ 2, M ≈ 1, S ≈ ½).

**Nummern:** ADR 046 (MCP-Zugang, Phase 1), ADR 047 (Artefakte aller Dateitypen, Phase 2), Migration 16 (Phase 2). Belegt bzw. reserviert und nicht zu verwenden: ADR 034, 037, 038 (geparkte Pläne), 043 (Unveränderte Zeilen), 044 (Nebenfragen), 045 (Plan „Notiz pro Option“). Der Plan „Umsetzung“ beginnt bei ADR 048 und Migration 17.

**Nicht drin (Backlog, eigene Pläne):** Excel, SQLite, Graphviz, Excalidraw, 3D-Modelle, Zip, Karte für Office-Dateien — diese Typen laufen bis dahin über die Rest-Karte. Kein MCP-Werkzeug für offene Kommentare: die gesammelten Kommentare leben nur im UI-Zustand (`src/stores/review.ts`), der Core kennt sie erst beim Senden.

## Kontrakt

### MCP-Zugang (Phase 1, ADR 046)

- **Ort:** eigener `TcpListener` auf `127.0.0.1`, Zufalls-Port, eigener Thread, `std::net` ohne neue Abhängigkeit. Pfad `/mcp`. Getrennt vom Artefakt-Server: anderer Port, damit keine Antwort des Artefakt-Servers (die `Access-Control-Allow-Origin: *` trägt) je für MCP gilt.
- **Pflicht je Anfrage:** `Host: 127.0.0.1:<port>` (sonst 403) · `Authorization: Bearer <schlüssel>` mit bekanntem Schlüssel (sonst 401) · Methode `POST` (sonst 405, auch für `OPTIONS` und `GET`) · `Content-Type: application/json` (sonst 415) · `Content-Length` ≤ 1 MiB (sonst 413). Nie `Access-Control-*`-Köpfe.
- **Schlüssel:** 32 Hex-Zeichen (`uuid::Uuid::new_v4().simple()`), je Session und App-Start, nur im Speicher (`McpAccess.keys: Mutex<HashMap<String, String>>` Schlüssel → Session-ID). Er bindet jeden Aufruf an diese Session und damit an ihr Vorhaben.
- **JSON-RPC 2.0:** `initialize` → `{ protocolVersion: <Version der Anfrage, sonst PROTOCOL_VERSION aus standalone/mcp/client.rs>, capabilities: { tools: {} }, serverInfo: { name: "verwalter", version: <App-Version> } }`; Antwortkopf `Mcp-Session-Id: <Session-ID des Verwalters>`. `notifications/*` (ohne `id`) → `202 Accepted`, leerer Körper. `ping` → `{}`. `tools/list` → `{ tools: [ { name, description, inputSchema } ] }` ohne Seiten. `tools/call` → `{ content: [ { type: "text", text } ], structuredContent?, isError }`. Unbekannte Methode → Fehler `-32601`; kaputtes JSON → `-32700`; fehlende Parameter → `-32602`. Antworten immer `Content-Type: application/json`, kein Ereignis-Strom.
- **Werkzeug-Sätze:** Trait `ToolSet` in `src-tauri/src/mcp_access/tools.rs`:
  ```rust
  pub trait ToolSet: Send + Sync {
      /// Gemeinsamer Präfix aller Werkzeuge, z. B. "artifact_".
      fn prefix(&self) -> &'static str;
      fn tools(&self) -> Vec<ToolSpec>;
      fn call(&self, call: &ToolCall) -> ToolResult;
  }
  pub struct ToolSpec { pub name: &'static str, pub description: &'static str, pub input_schema: serde_json::Value }
  pub struct ToolCall<'a> { pub app: &'a AppHandle, pub session_id: &'a str, pub name: &'a str, pub arguments: &'a serde_json::Value }
  pub enum ToolResult { Ok { text: String, structured: Option<serde_json::Value> }, Err(String) }
  ```
  Die Liste der Sätze steht an genau einer Stelle (`mcp_access::tool_sets()`); ein neues Feature hängt dort eine Zeile an. Ein `Err` wird zu `isError: true` mit dem Text — der Agent liest ihn, die Verbindung bleibt.
- **Werkzeuge der Artefakte** (`src-tauri/src/artifacts/tools.rs`):
  - `artifact_list` — Eingabe `{}`. Ausgabe: `structuredContent = { dir, items: [ { file, title, kind, modifiedAt, sessionNumber, sessionName, isOwnSession } ] }`, `text` = eine Zeile je Artefakt `„<title>“ (<file>, <kind>, #<n> <name>)`. Ab Phase 2 kommt `kind` aus `ArtifactKind`; in Phase 1 steht dort `"html"`.
  - `artifact_show` — Eingabe `{ "file": string }` (Name direkt in `.artefakte`). Prüft, dass die Datei ein Artefakt der Liste ist (sonst `Err("Kein Artefakt: <file>")`), sendet Ereignis `artifacts://show` mit `{ sessionId, projectId, file }` und antwortet `„<title>“ im Reiter Artefakte ausgewählt.`
- **Übergabe an den Agenten:** beim Start jedes Agenten schreibt der Verwalter `<Benutzerordner>\.verwalter\mcp\<session-id>.json` neu: `{ "mcpServers": { "verwalter": { "type": "http", "url": "http://127.0.0.1:<port>/mcp", "headers": { "Authorization": "Bearer <schlüssel>" } } } }` und hängt den Pfad an die Liste von `--mcp-config` (neben den `.mcp.json` der Repositories). Werkzeugnamen beim Agenten: `mcp__verwalter__artifact_list` usw. Nebenfragen (`registry/side.rs`) bekommen keinen Zugang.

### Artefakte (Phase 2, ADR 047)

- **`ArtifactKind`** (`src-tauri/src/artifacts/model.rs`, `derive(TS)`, `serde(rename_all = "kebab-case")`): `Html`, `Site`, `Folder`, `Markdown`, `Json`, `JsonLines`, `Yaml`, `Config` (toml, ini, env), `Xml`, `Csv`, `Code`, `Text`, `Log`, `Diff`, `Image`, `Video`, `Audio`, `Pdf`, `Mermaid`, `Other`. Zuordnung der Endungen steht einmal in `artifacts::kind_of(path)`.
- **`Artifact`** bekommt `kind: ArtifactKind`, `size: f64` (Bytes; bei Ordnern Summe der Dateien, höchstens 2 Ebenen), `entries: Option<u32>` (nur Ordner: Zahl der Dateien), `viewable: bool` (Html, Site oder ein Betrachter existiert; sonst Rest-Karte) und `last_session_number: Option<u32>`, `last_session_name: Option<String>` (gesetzt nur, wenn die zuletzt schreibende Session eine andere ist als die anlegende). `session_id/number/name` bedeuten ab jetzt **anlegende** Session.
- **Anlegende Session:** Migration 16 ergänzt `session_files.first_touched_at REAL` (Altbestand `NULL`). Anlegend ist die Session mit dem kleinsten `first_touched_at` für den Pfad; hat keine einen Wert, gilt die zuletzt schreibende (bisheriges `latest_owner`).
- **Was ist ein Artefakt:** jede Datei direkt in `.artefakte` außer versteckten (Name beginnt mit `.`), und jeder Unterordner, der **kein Zubehör** ist. Zubehör ist ein Unterordner, dessen Name mit `_` beginnt, der so heißt wie eine HTML-Datei daneben ohne Endung, oder auf den eine HTML-Datei direkt im Ordner mit `"<name>/` oder `'<name>/` verweist. Ein Unterordner mit `index.html` ist `Site`, sonst `Folder`.
- **Betrachter:** Für alles außer `Html` und `Site` lädt der Rahmen `<baseUrl><datei>?view=1`; der Artefakt-Server antwortet dann mit der Betrachter-Seite des Typs (HTML aus `src-tauri/src/artifacts/viewers/`, per `include_str!` eingebaut, mit Kommentar-Brücke). Die Seite holt die Rohdatei per `fetch("<datei>")` aus derselben Adresse (liegt in `connect-src`). Bibliotheken kommen über https von `cdn.jsdelivr.net` mit fester Version und `integrity`-Prüfsumme; lädt eine nicht, zeigt der Betrachter den Rohtext. Ohne `?view=1` liefert der Server wie bisher die Datei selbst.
- **Gruppierung:** `src/features/artifacts/groupArtifacts.ts` — reine Funktion `groupArtifacts(items, currentSessionId) → ArtifactGroup[]`; Gruppe je anlegender Session, eigene Session zuerst, dann nach neuestem Artefakt der Gruppe absteigend, „Ohne Zuordnung“ zuletzt. Klappzustand je Vorhaben und Gruppe in `src/stores/artifacts.ts` (flüchtig).

## Finale Abnahmekriterien

- [ ] Ein Agent in den Betriebsarten Claude, Claude + LM Studio und Autark sieht `mcp__verwalter__artifact_list` und `mcp__verwalter__artifact_show` und kann beide aufrufen; Codex laut Ergebnis der Prüfung in Phase 1 (eingebunden oder als bekannte Lücke in ADR 046 dokumentiert).
- [ ] Eine Artefakt-Seite kann den MCP-Zugang nicht benutzen: Anfrage aus einer Artefakt-Seite per `fetch` scheitert (CSP und fehlender Schlüssel), belegt mit der Probe-Seite aus Phase 1.
- [ ] Jede Datei und jeder Nicht-Zubehör-Ordner in `.artefakte` erscheint im Reiter, mit Typ-Abzeichen, gruppiert nach anlegender Session; unbekannte Typen zeigen die Rest-Karte statt eines Fehlers.
- [ ] Bild, Video, Audio, Text, Code, Log, Diff, Markdown, JSON, JSON Lines, YAML, TOML/INI/.env, XML, CSV, PDF, Mermaid, Ordner und Website-Ordner werden mit ihrem Betrachter dargestellt und lassen sich kommentieren.
- [ ] Keine Agenten-Datei wird im Fenster der App gerendert; alles läuft im abgeschotteten Rahmen.
- [ ] `pnpm check` grün, Code-Map, Glossar, ADR 046 und 047 nachgezogen, Versions-Tag gesetzt.

## Smoke-Checkliste (Abgleich durch Sascha am Ende)

Wackelstellen zuerst:

1. **Codex:** In einer Codex-Session fragen „Welche Artefakte gibt es?“ — erscheint ein Aufruf `mcp__verwalter__artifact_list` oder die in ADR 046 dokumentierte Lücke?
2. **Zubehör-Ordner:** `.artefakte` dieses Vorhabens öffnen — `entwurf-umsetzung/`, `ablauf-recherche/`, `phasen-und-rollen/` erscheinen **nicht** als eigene Artefakte (gleichnamige HTML daneben).
3. **Anlegende Session:** Eine Datei in Session A schreiben lassen, in Session B ändern — sie steht unter A, Meta-Zeile nennt B als letzten Bearbeiter.
4. Agent sagen „zeig mir das Artefakt X“ — der Reiter springt auf X.
5. Je eine Datei `.md` (mit Mermaid-Block), `.json`, `.yaml`, `.csv`, `.png`, `.mp4`, `.mp3`, `.pdf`, `.log`, `.xlsx` ablegen — passende Ansicht, `.xlsx` mit Rest-Karte.
6. Internet aus, Markdown öffnen — Rohtext statt leerer Fläche.

## Summary

## Files touched

## Commits

## Deviations from plan

## Follow-ups
