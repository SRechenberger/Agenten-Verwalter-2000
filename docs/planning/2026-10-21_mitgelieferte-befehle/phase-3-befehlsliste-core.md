# Phase 3 — Core: Liste der mitgelieferten Befehle je Betriebsart

## Kontext

Lesen: [README.md](README.md) (Entscheidungen „Quelle je Betriebsart“, „Die Betriebsart ist die aus den Einstellungen“, „Ausblenden“, „Beschreibungen“, „Typ über die Tauri-Grenze“, „Kein Skill-Abzeichen“; Kontrakt `SkillOrigin`, Startzeile), [ADR 007](../../decisions/007-anhaenge-skills-hintergrund.md), [docs/knowledge/claude-stream-json.md](../../knowledge/claude-stream-json.md) (Abschnitt zu `system/init`), [docs/conventions/rust.md](../../conventions/rust.md).

Code: `src-tauri/src/skills/` (`mod.rs` `collect`; `model.rs` `SkillOrigin`, `SkillInfo`, `SkillKind`), `src-tauri/src/commands/skills.rs`, `src-tauri/src/agents/claude/protocol.rs` (`SystemLine`, `lenient`), `src-tauri/src/agents/event.rs` (`AgentEvent::Ready`), `src-tauri/src/agents/claude/translate.rs` (`handle_system`, Zweig `"init"`), `src-tauri/src/sessions/registry.rs` (`Outbox`, `apply_event` Zweig `Ready`, `update`), `src-tauri/src/db/settings.rs` (`read_all`, `write`), `src-tauri/src/settings/mod.rs` (`load`), `src-tauri/src/settings/model.rs` (`OperatingMode`), `src-tauri/src/settings/mod.rs` `overview` (nutzt `SkillOrigin::User` in einem `matches!`; bleibt gültig), `src-tauri/examples/gen-bindings.rs`.

## AK der Phase

1. Jede `system/init`-Zeile der Claude-Kommandozeile mit `slash_commands` speichert die Liste in `settings` unter `claude_slash_commands` als JSON-Array. Geschrieben wird nur, wenn sich der Inhalt geändert hat, und außerhalb der Session-Sperre.
2. `skill_list_for_session` und `skill_list_for_repositories` liefern zuerst die Datei-Einträge wie bisher, dann die mitgelieferten Befehle der Betriebsart aus den Einstellungen, nach Name sortiert (Kleinschreibung), als `SkillInfo { kind: Command, origin: Builtin }`.
3. In „Claude“ und „Claude Code + LM Studio“ entsprechen die Namen der gespeicherten Liste. Ausgeblendet werden die Namen aus `HIDDEN`, jeder Name mit Präfix `__` und jeder Name, den ein Datei-Eintrag schon trägt. Fehlt die Liste oder ist sie unlesbar, gibt es keine mitgelieferten Befehle und keinen Fehler.
4. In „Autark“ ergeben sich genau `compact`, in „Codex“ genau `codex-review` und `compact`. Trägt ein Datei-Eintrag denselben Namen, fällt dort der **Datei-Eintrag** weg, weil Autark und Codex den mitgelieferten Befehl vorher abfangen (Phasen 1 und 2) und die Datei nie erreicht würde.
5. Beschreibung aus der Tabelle unten, sonst leer.
6. `pnpm bindings` erzeugt `SkillOrigin` mit `{ kind: "builtin" }`.
7. ADR 034 liegt unter `docs/decisions/034-mitgelieferte-befehle.md`.

## Checkliste

- [ ] `skills/model.rs`: Variante `Builtin` an `SkillOrigin` mit Doc-Kommentar aus dem Kontrakt der README.
- [ ] `agents/claude/protocol.rs` `SystemLine`: Feld `#[serde(default, deserialize_with = "lenient")] pub slash_commands: Option<Vec<String>>`; den Doc-Kommentar über dem Struct um `slash_commands` ergänzen („gehört zu `init`“).
- [ ] `agents/event.rs`: `AgentEvent::Ready { model: String, slash_commands: Option<Vec<String>> }` mit Doc-Kommentar „Liste der Kommandozeile; Autark und Codex schicken keine“.
- [ ] `agents/claude/translate.rs` Zweig `"init"`: `slash_commands: system.slash_commands` mitgeben.
- [ ] `sessions/registry.rs` `Outbox`: Feld `slash_commands: Option<Vec<String>>` mit Kommentar „aus `system/init`; gespeichert nach dem Freigeben der Sperre“. `apply_event` Zweig `AgentEvent::Ready { slash_commands, .. }`: `if slash_commands.is_some() { outbox.slash_commands = slash_commands; }`. In `update` direkt nach `outbox.emit(app, session)` gilt: `if let Some(names) = outbox.slash_commands.take()`, dann `skills::builtin::remember_claude_commands(&session.database, &names)`. Ein Fehler geht nach `session.log_line(format!("Befehlsliste nicht gespeichert: {error}"))`. Vorher per Grep auf `apply_event(` prüfen, ob es Aufrufer außerhalb von `update` gibt; dort dieselbe Behandlung nach dem Freigeben der Sperre.
- [ ] Neue Datei `skills/builtin.rs` (in `skills/mod.rs` als `pub mod builtin;`), Modul-Kommentar: „Mitgelieferte Befehle der Betriebsart fürs `/`-Menü (ADR 034); Quelle für Claude ist `system/init`, für Autark und Codex der Verwalter selbst.“ Inhalt:
  - `const SETTINGS_KEY: &str = "claude_slash_commands";`
  - `const STANDALONE_COMMANDS: [&str; 1] = ["compact"];`, `const CODEX_COMMANDS: [&str; 2] = ["codex-review", "compact"];`
  - `const HIDDEN: [&str; 11] = ["model", "effort", "mcp", "rename", "chrome", "clear", "agents", "heapdump", "color", "focus", "workflow-launch-exec"];` und `const HIDDEN_PREFIX: &str = "__";`, mit Kommentar je Gruppe wie in der README.
  - `const DESCRIPTIONS: [(&str, &str); N]` mit genau diesen Zeilen:

    | Name | Beschreibung |
    |---|---|
    | `compact` | Verlauf zusammenfassen, um Platz im Kontext zu schaffen |
    | `codex-review` | Codex prüft die offenen Änderungen der Repositories |
    | `context` | Belegung des Kontexts als Tabelle |
    | `usage` | Kontingent und Verbrauch des Abos |
    | `config` | Einstellung der Kommandozeile setzen: /config schlüssel=wert |
    | `output-style` | Ausgabestil anzeigen oder wechseln |
    | `advisor` | Berater-Modell ein- oder ausschalten |
    | `goal` | Ziel setzen, an dem der Agent weiterarbeitet |
    | `fast` | Schnellmodus umschalten |
    | `autocompact` | Automatisches Zusammenfassen umschalten |
    | `reload-skills` | Skills neu einlesen |
    | `reload-plugins` | Plugins neu einlesen |
    | `list-agents` | Andere laufende Claude-Sessions anzeigen |
    | `skill-doctor` | Geladene Skills mit Verbrauch prüfen |
    | `init` | CLAUDE.md für das Projekt anlegen |
    | `doctor` | Installation prüfen – startet einen Agenten-Lauf |
    | `insights` | Auswertung der eigenen Nutzung |
    | `recap` | Kurzfassung der Session |
    | `security-review` | Sicherheits-Review der offenen Änderungen |
    | `ultrareview` | Gründliches Review mit mehreren Agenten in der Cloud, kostenpflichtig |
    | `code-review` | Änderungen auf Fehler prüfen |
    | `simplify` | Geänderten Code vereinfachen und aufräumen |
    | `loop` | Befehl in Abständen wiederholen: /loop 5m /befehl |
    | `schedule` | Geplante Agenten in der Cloud anlegen und verwalten |
    | `run` | App starten und eine Änderung live ansehen |
    | `claude-api` | Nachschlagen zur Claude-API und den SDKs |
    | `update-config` | Einstellungen von Claude Code ändern |
    | `fewer-permission-prompts` | Häufige Rechte-Abfragen als Freigaben eintragen |
    | `dataviz` | Diagramme und Dashboards gestalten |
    | `anthropic-skills:pdf` | PDF-Dateien lesen und erzeugen |
    | `anthropic-skills:docx` | Word-Dateien lesen und erzeugen |
    | `anthropic-skills:xlsx` | Excel-Dateien lesen und erzeugen |
    | `anthropic-skills:pptx` | PowerPoint-Dateien lesen und erzeugen |
    | `anthropic-skills:deep-research` | Recherche über viele Quellen mit Bericht |

  - `pub fn remember_claude_commands(database: &Database, names: &[String]) -> Result<(), CommandError>`: `serde_json::to_string(names)`; den gespeicherten Wert über `database.with(|connection| db::settings::read_all(connection))` holen, gleich → nichts tun; sonst `db::settings::write(connection, SETTINGS_KEY, &json)`.
  - `fn names_for(database: &Database, mode: OperatingMode) -> Vec<String>`: `Claude | ClaudeCodeLocal` → gespeicherten Wert lesen, `serde_json::from_str::<Vec<String>>`; fehlt der Wert oder ist er unlesbar, gilt eine leere Liste. Danach `HIDDEN` und `HIDDEN_PREFIX` herausfiltern. `Standalone` → `STANDALONE_COMMANDS`, `Codex` → `CODEX_COMMANDS`.
  - `pub fn menu_entries(database: &Database, home: &Path, roots: &[(String, PathBuf)]) -> Result<Vec<SkillInfo>, CommandError>`: `mode = settings::load(database)?.operating_mode`; `files = super::collect(home, roots)`; `builtins = names_for(database, mode)`. Bei `Claude | ClaudeCodeLocal` fallen aus `builtins` die Namen weg, die in `files` vorkommen. Bei `Standalone | Codex` fallen aus `files` die Einträge weg, deren Name in `builtins` steht. `builtins` werden nach `to_lowercase()` sortiert, zu `SkillInfo { name, description: describe(&name), kind: SkillKind::Command, origin: SkillOrigin::Builtin }` gemacht und hinten an `files` gehängt.
  - `fn describe(name: &str) -> String` über `DESCRIPTIONS`, sonst `String::new()`.
- [ ] `commands/skills.rs`: beide Commands bekommen `database: tauri::State<'_, Arc<Database>>` (bei `skill_list_for_repositories` schon da) und geben `skills::builtin::menu_entries(&database, &home_dir(&app)?, &roots)` zurück.
- [ ] `pnpm bindings`; `src/lib/bindings/SkillOrigin.ts` mitcommitten. In der Oberfläche stehen drei Stellen, die `origin.kind === 'user'` sonst als Repository lesen: `commandMenuRows.ts` `rowKey`, `CommandMenu.tsx` `rowOrigin`, `UserMessage.tsx`. `rowKey` und `rowOrigin` passt Phase 4 an. `UserMessage.tsx` bekommt nie `builtin` (kein Skill-Abzeichen), braucht aber für `pnpm check` einen erschöpfenden Zweig. Dieser zeigt `geladen · mitgeliefert`.
- [ ] `docs/decisions/034-mitgelieferte-befehle.md` schreiben (≤ 15 Zeilen, Muster der übrigen ADRs). Kontext: das Menü zeigte nur Dateien, `/permissions` und Co. fehlen im Druckmodus. Optionen: feste Liste, Hilfsprozess beim Start, gespeicherte Liste aus `system/init`. Entscheidung: die Punkte der README-Abschnitte „Quelle je Betriebsart“ bis „Ausgabe nach dem Zusammenfassen“. Konsequenzen: vor dem ersten Claude-Start fehlt der Abschnitt; neue Befehle erscheinen ohne Beschreibung; löst den entsprechenden Punkt aus ADR 007 ab. In ADR 007 bei „Plugin- und eingebaute Skills stehen nicht im Menü“ den Zusatz „— abgelöst durch ADR 034“.
- [ ] Doc: `docs/knowledge/claude-stream-json.md`, Absatz zu `slash_commands`: belegte Liste der fehlenden interaktiven Befehle (`/permissions`, `/hooks`, `/resume`, `/memory`, `/status`, `/login`). Dazu die Antwortform örtlicher Befehle (`assistant` mit `model: "<synthetic>"`, `num_turns: 0`) und dass `/doctor` einen Agenten-Lauf startet. Stand 2.1.293, 2026-10-21.
- [ ] Doc: `docs/code-map.md` Zeile „Skills und Befehle“: Core um `builtin.rs` (`menu_entries`, `remember_claude_commands`, Schlüssel `claude_slash_commands` in `settings`) ergänzen; Oberfläche unverändert bis Phase 4.
- [ ] `pnpm check` grün; Commit `feat(skills): mitgelieferte Befehle je Betriebsart in der Skill-Liste`.

## Report-Back
