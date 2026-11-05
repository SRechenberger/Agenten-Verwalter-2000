# Mitgelieferte Befehle

Ziel: Das `/`-Menü zeigt neben den Skills und Befehlen aus den Dateien auch die **mitgelieferten Befehle** der laufenden Betriebsart, also die, die der Agent selbst versteht. Es zeigt nur, was in dieser Betriebsart wirklich funktioniert. „Claude“ und „Claude Code + LM Studio“ übernehmen die Liste, die die Claude-Kommandozeile bei jedem Start meldet. „Autark“ und „Codex“ bekommen die Befehle, die der Verwalter für sie selbst umsetzt: `/compact` in beiden, dazu `/codex-review` in Codex. Nebenbei funktioniert damit der Compact-Knopf im Kontext-Fenster auch in „Autark“ und „Codex“. Heute schickt er dort „/compact“ als gewöhnlichen Text an das Modell.

**Belegt am 2026-10-21:**

- Claude Code 2.1.293 im Druckmodus: `system/init` → `slash_commands` listet Benutzer- und Repository-Skills, Befehlsdateien, mitgelieferte Skills (`code-review`, `simplify`, …), Plugin-Skills (`anthropic-skills:pdf`, …) und eingebaute Befehle (`compact`, `context`, `usage`, `config`, …). `/permissions`, `/hooks`, `/resume`, `/memory`, `/status`, `/login` fehlen in der Liste. `/permissions` antwortet „/permissions isn't available in this environment.“
- Örtliche Befehle antworten ohne Modell-Lauf mit einer `assistant`-Zeile (`model: "<synthetic>"`) und `result` mit `num_turns: 0`. Gemessen mit `/context`, `/usage`, `/config`, `/advisor`, `/goal`, `/output-style`, `/reload-skills`, `/list-agents`, `/skill-doctor`, `/mcp`, `/model`, `/effort` und `/rename`. `/agents` antwortet „The /agents wizard has been removed.“ `/doctor` startet einen Agenten-Lauf (2 Turns, 0,71 $ Rechenwert).
- Codex `0.162.0-alpha.17.2`, live gegen `codex app-server`. Protokoll der Probe: [artifacts/codex-probe.log](artifacts/codex-probe.log).
  - `thread/compact/start {threadId}` antwortet `{}`. Danach kommen `turn/started` (neue Turn-ID), `item/started` und `item/completed` mit `type: "contextCompaction"`, `thread/tokenUsage/updated` (`last.totalTokens` = Stand nach dem Zusammenfassen) und `turn/completed` mit derselben Turn-ID.
  - `review/start {threadId, delivery: "inline", target: {type: "custom", instructions}}` antwortet mit `turn.id` = **A**. Danach kommen `enteredReviewMode`, `turn/started` mit einer **anderen** ID **B**, ein `agentMessage`, das nur startet und nie endet, dann `exitedReviewMode` (`review` = Ergebnis-Text) und ein vollständiges `agentMessage` mit demselben Text. Den Abschluss bildet `turn/completed` mit ID **A**.
- `/compact` im Druckmodus der Claude-Kommandozeile: `system/compact_boundary` mit `compact_metadata` (`trigger`, `pre_tokens`, `post_tokens`), danach `result`. Eine `assistant`-Zeile kommt nicht; der Verwalter zeigt deshalb heute nach `/compact` in „Claude“ nichts außer der eigenen Nachricht. Die Trennlinie im Verlauf bringt der geparkte Plan [Warnhinweise](../2026-10-21_warnhinweise/README.md), Phase 4, und zwar für jede Betriebsart, die `compact_boundary` schickt.

Kontext für jeden Umsetzer: [AGENTS.md](../../../AGENTS.md), [docs/code-map.md](../../code-map.md), [docs/glossary.md](../../glossary.md), die Konventionen unter [docs/conventions/](../../conventions/), [ADR 007](../../decisions/007-anhaenge-skills-hintergrund.md) (Skills im Menü), [ADR 017](../../decisions/017-autarker-agent.md) (Autark), [ADR 031](../../decisions/031-codex.md) (Codex), [docs/knowledge/claude-stream-json.md](../../knowledge/claude-stream-json.md), [docs/knowledge/codex-app-server.md](../../knowledge/codex-app-server.md). Fehlerklassen geprüft (Vault: TypeScript, React, Claude Code): keine einschlägig; für Rust, Tauri und Codex gibt es keine Einträge.

## Phasen

| # | Phase | Datei | Rating | Status |
|---|---|---|---|---|
| 1 | Autark: `/compact` von Hand, `compact_boundary` mit Tokenständen | [phase-1-autark-compact.md](phase-1-autark-compact.md) | standard | pending |
| 2 | Codex: `/compact` und `/codex-review` im Übersetzer | [phase-2-codex-befehle.md](phase-2-codex-befehle.md) | standard | pending |
| 3 | Core: Liste der mitgelieferten Befehle je Betriebsart (Claude-Liste merken, feste Listen, Ausblenden, Beschreibungen); ADR 034 | [phase-3-befehlsliste-core.md](phase-3-befehlsliste-core.md) | standard | pending |
| 4 | Oberfläche: Abschnitt „Mitgeliefert“ im `/`-Menü; Doku-Abschluss | [phase-4-menue.md](phase-4-menue.md) | standard | pending |

**Reihenfolge:** strikt 1 → 4. Erst müssen die Befehle in „Autark“ und „Codex“ funktionieren, dann bietet das Menü sie an. Umsetzung direkt auf `main`, ein Commit pro Phase; Scopes: Phase 1 `standalone`, Phase 2 `codex`, Phase 3 und 4 `skills`. Vor jedem Commit `pnpm check` grün; `cargo` im PATH (`$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"`). Keine neuen Abhängigkeiten. Bindings nach Änderungen an Typen über die Tauri-Grenze neu erzeugen (`pnpm bindings`) und mitcommitten. Keine automatisierten Tests (Profil des Projekts). Erkenntnisse nach [FINDINGS.md](FINDINGS.md). Planabschluss mit Versions-Tag (Minor-Version), Ablauf nach [releases.md](../../conventions/releases.md).

**Verhältnis zum Plan „Warnhinweise“:** Beide Pläne sind unabhängig voneinander umsetzbar, in beliebiger Reihenfolge. Dieser Plan liefert `compact_boundary` mit `compact_metadata` jetzt auch aus „Autark“ und „Codex“. Phase 4 der Warnhinweise zeigt diese Zeile dann als Trennlinie im Verlauf. Ihre ADR-Nummer 033 und ihre Migration 12 bleiben reserviert. Dieser Plan nimmt ADR 034 und braucht keine Migration.

## Festgelegte Entscheidungen

Phase 3 schreibt daraus [ADR 034](../../decisions/034-mitgelieferte-befehle.md) „Mitgelieferte Befehle im `/`-Menü“ (Kontext / Optionen / Entscheidung / Konsequenzen). Damit wird der Punkt „Plugin- und eingebaute Skills stehen nicht im Menü“ aus ADR 007 abgelöst.

- **Quelle je Betriebsart.** „Claude“ und „Claude Code + LM Studio“: die zuletzt gesehene `slash_commands`-Liste aus `system/init`, gespeichert in der Tabelle `settings` unter dem Schlüssel `claude_slash_commands` (JSON-Array von Namen). Sie überlebt den Neustart und steht damit auch in „Neue Session“ bereit, bevor ein Agent läuft. Vor dem allerersten Start einer Claude-Session fehlt der Abschnitt. „Autark“: fest `compact`. „Codex“: fest `compact` und `codex-review`. Verworfen: eine feste Liste auch für Claude. Sie veraltet mit jeder Version der Kommandozeile und kennt die Plugin-Skills des Benutzers nicht. Ebenfalls verworfen: ein Hilfsprozess beim App-Start, der die Liste abfragt. Er kostet bei jedem Start einen Prozess, und die gespeicherte Liste reicht.
- **Die Betriebsart ist die aus den Einstellungen** (`settings::load(...).operating_mode`), wie bei den übrigen Einträgen des Menüs (`hasBrowser`, `hasMcp`). Läuft eine Session noch mit der vorigen Betriebsart, bis ihr Agent neu startet, zeigt das Menü schon die neue. So verhalten sich die übrigen Einträge heute auch.
- **Ausblenden in „Claude“ und „Claude Code + LM Studio“** — Namen, die das Menü nie zeigt: `model`, `effort`, `mcp`, `rename`, `chrome` (der Verwalter hat dafür eigene Einträge), `clear` (setzt den Kontext des Agenten zurück, während der Verlauf im Verwalter stehen bleibt; dafür gibt es „Neue Session“), `agents` (antwortet nur „wizard has been removed“), `heapdump`, `color`, `focus`, `workflow-launch-exec` (Bedienung des Terminals bzw. intern), dazu jeder Name, der mit `__` beginnt. Ebenfalls weg fallen Namen, die schon als Skill oder Befehl aus den Dateien im Menü stehen (die Datei-Einträge haben Beschreibung und Herkunft). Alles Übrige erscheint, auch Unbekanntes aus künftigen Versionen. Ein neuer Befehl taucht lieber ohne Beschreibung auf, als zu fehlen.
- **Beschreibungen:** eine feste deutsche Tabelle im Core (`skills/builtin.rs`) für die Namen, die heute bekannt sind; für alle anderen bleibt die Beschreibung leer. Die Tabelle steht wörtlich in Phase 3.
- **Typ über die Tauri-Grenze:** `SkillOrigin` bekommt die Variante `Builtin`. Mitgelieferte Befehle kommen als `SkillInfo { kind: Command, origin: Builtin }` aus denselben Commands `skill_list_for_session` und `skill_list_for_repositories`, hinter den Datei-Einträgen, nach Name sortiert. Kein neuer Command. Die Oberfläche teilt nach `origin.kind` in zwei Abschnitte auf.
- **Kein Skill-Abzeichen an der Nachricht** für mitgelieferte Befehle: `skills::collect`, `find_file` und `match_invocation` bleiben bei den Dateien. Die neue Liste ist nur für das Menü da.
- **Der Codex-Review heißt `/codex-review`, nicht `/review`.** Der Benutzer hat eine eigene Befehlsdatei `review.md`; ein Name `/review` würde sie in Codex verdecken oder von ihr verdeckt. Er prüft immer per `target: custom`, nie per `uncommittedChanges`: der Arbeitsordner einer Codex-Session ist der Workspace des Vorhabens, kein Git-Repository, und `uncommittedChanges` liest Git im Arbeitsordner. Die Anweisung nennt die Repository-Ordner der Session (`--add-dir`); Text nach dem Befehl hängt als Zusatz an. Ohne Repository-Ordner antwortet der Befehl mit einem Fehler und startet nichts.
- **`/compact` in „Autark“ und „Codex“** wird abgefangen, **bevor** Skills und Befehlsdateien gesucht werden. Die Nachricht selbst geht nicht in den Verlauf des Modells (so macht es die Claude-Kommandozeile auch). Text nach `/compact` ist in „Autark“ ein Schwerpunkt für die Zusammenfassung. In „Codex“ wird er ignoriert, weil `thread/compact/start` keinen Text annimmt.
- **Ausgabe nach dem Zusammenfassen wie bei der Claude-Kommandozeile:** `system/compact_boundary` mit `compact_metadata {trigger: "manual", pre_tokens, post_tokens}`, danach `result` mit Erfolg. Keine `assistant`-Zeile mit Text: sie stünde nach Phase 4 der Warnhinweise doppelt neben der Trennlinie. Bis dahin sieht der Benutzer den Erfolg am kleineren Kontext-Donut. Das automatische Zusammenfassen in „Autark“ schickt dieselbe Zeile mit `trigger: "auto"`.

## Kontrakt

### Core → Oberfläche (`src-tauri/src/skills/model.rs`, Phase 3)

```rust
pub enum SkillOrigin {
    User,
    Repository { name: String },
    /// Mitgelieferter Befehl der Betriebsart (ADR 034): kommt aus dem Agenten, nicht aus einer Datei.
    Builtin,
}
```

TypeScript nach `pnpm bindings`: `SkillOrigin = { kind: "user" } | { kind: "repository", name: string } | { kind: "builtin" }`.

### Ausgabezeile der Agenten „Autark“ und „Codex“ (`src-tauri/src/standalone/output.rs`, Phase 1)

```rust
/// `system/compact_boundary` wie bei der Claude-Kommandozeile; unbekannte Tokenstände als `null`.
pub fn compact_boundary(trigger: &str, pre_tokens: Option<u32>, post_tokens: Option<u32>) -> Value
// → {"type":"system","subtype":"compact_boundary","compact_metadata":{"trigger":…,"pre_tokens":…,"post_tokens":…}}
```

### Neues Feld der Startzeile (`src-tauri/src/agents/claude/protocol.rs`, Phase 3)

`SystemLine.slash_commands: Option<Vec<String>>` (nachsichtig gelesen wie die übrigen Felder) → `AgentEvent::Ready { model, slash_commands: Option<Vec<String>> }`.

## Finale Abnahmekriterien

1. In „Claude“ zeigt das `/`-Menü nach dem ersten Agent-Start einen Abschnitt „Mitgeliefert“ mit u. a. `/compact`, `/context`, `/usage`, `/code-review`, `/simplify` und den Plugin-Skills `anthropic-skills:…`; bekannte Einträge mit deutscher Beschreibung, rechts die Marke „mitgeliefert“. `/model`, `/effort`, `/mcp`, `/rename`, `/clear`, `/agents` stehen dort nicht; Skills aus den Dateien stehen nicht doppelt.
2. Nach einem App-Neustart steht derselbe Abschnitt sofort, auch in „Neue Session“, bevor ein Agent läuft.
3. In „Autark“ zeigt der Abschnitt nur `/compact`, in „Codex“ nur `/compact` und `/codex-review`.
4. In „Autark“ und „Codex“ fasst `/compact` (getippt oder über den Compact-Knopf im Kontext-Fenster) den Verlauf zusammen: der Kontext-Donut wird kleiner, der Agent kennt danach noch den Inhalt des Gesprächs, und die Nachricht „/compact“ landet nicht als Frage beim Modell.
5. In „Codex“ liefert `/codex-review` eine Prüfung der offenen Änderungen in den Repositories der Session als Antwort im Verlauf; Text nach dem Befehl fließt als Zusatz ein. Ohne Repository erscheint ein Fehlersatz, kein Lauf.
6. In „Claude“ verhält sich `/compact` wie vorher.

## Smoke-Checkliste

Wackelstellen zuerst:

1. **Esc während `/codex-review`.** Codex meldet den Review-Turn unter zwei IDs (Antwort A, `turn/started` B). Der Übersetzer bricht mit der ID aus `turn/started` ab. Ob Codex das annimmt, ist nicht belegt. Prüfen: in „Codex“ `/codex-review` schicken, nach 2–3 Sekunden Esc. Erwartet: die Session steht sofort auf „abgebrochen“, und eine danach geschickte Nachricht bekommt eine normale Antwort. Bleibt Codex hängen (die nächste Nachricht wartet ewig), das stderr-Protokoll der Session nach FINDINGS kopieren.
2. **Review sieht die Repositories.** Die Prüfung läuft in der Sandbox der Session; ob Codex dort `git diff` in den Repository-Ordnern lesen darf, ist nicht belegt. Prüfen: Session in „Codex“ mit einem Repository, eine Datei darin ändern (nicht committen), Modus „Manuell“, `/codex-review` schicken. Erwartet: die Antwort nennt die geänderte Datei. Fragt Codex nach Freigaben für Lesebefehle oder meldet „kein Zugriff“, den Fall nach FINDINGS.
3. **Autark: `/compact` in einer kurzen Session.** Mit nur einem früheren Turn gibt es fast nichts zusammenzufassen. Das Zusammenfassen behält den letzten Turn wörtlich und kürzt nur die Werkzeug-Ergebnisse darin. Prüfen: neue Session in „Autark“, eine Nachricht, dann `/compact`. Erwartet: eine Erfolgsmeldung ohne Fehler, der Donut bleibt gleich oder wird kleiner. Danach eine Session mit 3–4 Turns, `/compact`, dann fragen „Worum ging es bisher?“. Die Antwort muss den Inhalt kennen.
4. „Claude“: `/` tippen → Abschnitt „Mitgeliefert“ wie AK 1; `/context` wählen, senden → Tabelle der Kontext-Belegung als Antwort.
5. App beenden, neu starten, „Neue Session“ → `/` → Abschnitt wie AK 2.
6. Betriebsart „Autark“ → Menü wie AK 3; Kontext-Fenster öffnen → „Compact“ → Donut kleiner.
7. Betriebsart „Codex“ → Menü wie AK 3; Compact-Knopf → Donut kleiner; danach „Worum ging es bisher?“ → Antwort kennt den Inhalt.
8. „Codex“ in einem Vorhaben ohne Repository: `/codex-review` → Fehlersatz.
9. Gegenprobe „Claude“: Compact-Knopf wie vorher.

## Summary

## Files touched

## Commits

## Deviations from plan

## Follow-ups
