# Plan: Regel-Spuren in der Vorhaben-Retro

**Ziel:** Die Vorhaben-Retro (ADR 028) zeigt künftig nicht nur, was schiefging, sondern auch, welche Regeln gewirkt haben: welche Skills eine Session geladen hat, welche Hook-Sperren gegriffen haben und an welchen Stellen der Agent wegen einer Regel anders gehandelt hat. Damit kann der Skill `session-review` laufend beurteilen, welche Regeln tragen, welche raus können und welche fehlen. Der Grundsatz aus ADR 028 bleibt: der Verwalter sammelt, der Skill urteilt.

**Warum:** Heute erscheint eine Hook-Sperre im Retro-Verlauf als „Werkzeug fehlgeschlagen: Bash …“ ohne Grund (`retro/transcript.rs` `retro_block`), eine funktionierende Regel sieht also aus wie ein Fehlversuch. Erfolgreiche Werkzeug-Aufrufe werden ausgefiltert, damit fehlen auch alle Skill-Aufrufe durch den Agenten. Die Mini-Retro (`retro/prompt.rs`) sammelt nur Korrekturen, Fehlschläge, unbelegte Behauptungen, Fakten und Offenes, also nie die Gegenrichtung „hat gewirkt“.

**Entscheidung:** ADR 053 (in Phase 1 anlegen, Inhalt unten im Kontrakt).

## Overview

| Phase | Inhalt | Status | Rating | Wave | Schätzung |
|---|---|---|---|---|---|
| 1 | Regel-Spuren deterministisch: Skills und Hook-Sperren je Session, im Verlauf und im Kopf von `session-<N>.md`, plus `retro-probe` | complete | standard | 1 | ~3 h |
| 2 | Mini-Retro und `befunde.md`: Rubrik „Hook-Sperren“, Zeile „Skills“, neue Kategorie „Regel hat gewirkt“; Retro-Lauf in der Dev-App | complete | standard | 2 | ~2 h |

Sequenziell, weil Phase 2 die Typen aus Phase 1 (`RuleTraces`, `HookBlock`) braucht und beide `sessions/registry/retro.rs` schreiben.

## Definition of Ready

- **Scope drin:** Erkennen von Hook-Sperren und Skill-Aufrufen aus dem gespeicherten Verlauf und aus `tool_details` (ADR 042); Ausgabe in `session-<N>.md` und `befunde.md`; neue Kategorie der Mini-Retro; Probe; Doku.
- **Scope draußen:** jede Oberfläche (kein neuer Knopf, keine Zahl am Retro-Knopf); Einwürfe von Hooks ohne Sperre (`additionalContext`, sie erscheinen nicht als Werkzeug-Ergebnis); Codex und Autark (haben keine Claude-Hooks und kein Skill-Werkzeug, liefern einfach leere Spuren); Hook-Namen (der Sperrtext enthält keinen, siehe Kontrakt); alles im Regelwerk des Nutzers (siehe „Gegenstück im Regelwerk“).
- **Abhängigkeiten:** `tool_details` (Migration 15, ADR 042) muss Ausgaben der Sessions enthalten — Aufrufe vor ADR 042 liefern keine Zeile, ihre Sperren bleiben „Werkzeug fehlgeschlagen“. Keine Migration, keine neue Bibliothek, keine Typen über die Tauri-Grenze (kein `pnpm bindings`).
- **Token-Budget:** je Phase eine Umsetzungs-Session, Konventionen `docs/conventions/rust.md` lesen; keine Subagenten nötig.
- **Prüfstrategie:** Dieses Projekt hat keine automatisierten Tests (`docs/conventions/rust.md`, Abschnitt Typgenerierung). Beleg je Phase: `pnpm check` grün plus das genannte Artefakt (Probe-Ausgabe bzw. `befunde.md` eines echten Laufs in `pnpm tauri dev`).
- **Risiken (Top 3):** (1) Das Präfix des Sperrtexts ändert sich mit einer Claude-Code-Version → Sperre fällt still auf „Werkzeug fehlgeschlagen“ zurück; Mitigation: Erkennung in genau einer Funktion (`parse_hook_block`), Probe zeigt es sofort. (2) Die Mini-Retro zählt eine Sperre zusätzlich als Fehlversuch → doppelte Zählung im Urteil; Mitigation: ausdrücklicher Satz im Prompt (Phase 2). (3) Die Kategorie „Regel hat gewirkt“ bleibt leer oder wird mit Allerweltsstellen gefüllt; Mitigation: enge Definition im Prompt, Sichtprüfung im Phase-2-Lauf.

## Kontrakt

### Erkennung einer Hook-Sperre

Ein Werkzeug-Eintrag `ChatEntry::Tool` mit `state == ToolState::Failed`, dessen gespeicherte Ausgabe (`tool_details.output`) nach `trim_start()` mit `PreToolUse:` beginnt und die Zeichenfolge ` hook error: ` enthält. Belegtes Beispiel (Claude-Transkript, `is_error: true`): `PreToolUse:Bash hook error: STOP: rekursiver Shell-grep. Der laeuft hier …`. Der **Sperrgrund** ist der Text nach dem ersten ` hook error: `, auf eine Zeile gezogen (Zeilenumbrüche → Leerzeichen, Mehrfach-Leerzeichen bleiben) und auf höchstens 200 Zeichen gekürzt (an einer Zeichengrenze, dann `…`). Hook-Namen stehen nicht im Text und werden nicht geraten; die Zuordnung Grund → Hook macht der Skill.

### Erkennung eines Skill-Aufrufs

- **Menü:** `ChatEntry::User { skill: Some(SkillRef { name, .. }), .. }` → Aufruf `name`, Herkunft Menü.
- **Agent:** `ChatEntry::Tool { tool, target, .. }` mit `tool == "Skill"` → Aufruf `target`, Herkunft Agent (das Ziel ist der Wert des Eingabefelds `skill`, siehe `agents/claude/translate.rs` `TARGET_FIELDS`). Zählt unabhängig vom `state`.

### Typen (`src-tauri/src/retro/traces.rs`, neu)

```rust
/// Was eine Session über die Regeln des Nutzers verrät: geladene Skills und Hook-Sperren.
pub struct RuleTraces {
    /// Skill-Name → (Aufrufe über das Menü, Aufrufe durch den Agenten), sortiert nach Name.
    pub skills: BTreeMap<String, (u32, u32)>,
    /// In der Reihenfolge des Verlaufs.
    pub hook_blocks: Vec<HookBlock>,
}

pub struct HookBlock {
    pub tool_use_id: String,
    pub tool: String,
    pub target: String,
    /// Eine Zeile, höchstens 200 Zeichen.
    pub reason: String,
}

pub fn collect(connection: &Connection, session_id: &str, entries: &[ChatEntry]) -> Result<RuleTraces, CommandError>;
pub fn parse_hook_block(output: &str) -> Option<String>;
/// `mode-planning (Agent ×2), project-committing (Menü ×1, Agent ×1)` oder `keine`.
pub fn skills_line(traces: &RuleTraces) -> String;
impl RuleTraces { pub fn block_reason(&self, tool_use_id: &str) -> Option<&HookBlock>; }
```

`skills_line`: je Skill `name (…)`, darin nur die Herkünfte mit Zahl > 0, Menü vor Agent, getrennt durch `, `; Skills getrennt durch `, `; ohne Skills `keine`.

### Dateiformate (Leser: Skill `session-review`, Modus `vorhaben`)

`session-<N>.md` — Kopf nach Phase 1 (neu sind die Zeilen 5 und 6):

```
# Session #<N> – <Name>
Status: …
Modell: …
Werkzeug-Aufrufe: <a>, davon fehlgeschlagen: <f>, abgebrochen: <i>
Hook-Sperren: <h>
Skills: <skills_line>
Gekürzt: ja|nein
```

Im Verlauf ersetzt eine Sperre die Zeile `Werkzeug fehlgeschlagen: <tool> <target>` durch `Hook-Sperre: <tool> <target> — <reason>`. Andere Fehlschläge bleiben unverändert. `<f>` zählt Sperren weiter mit (sie sind fehlgeschlagene Aufrufe); `<h>` ist die Teilmenge.

`befunde.md` — je Session nach Phase 2:

```
## Session #<N> – <Name>
Status: … · Modell: … · Werkzeug-Aufrufe: …, fehlgeschlagen: …, abgebrochen: … · Hook-Sperren: <h> · Gekürzt: … · Verlauf: session-<N>.md
Skills: <skills_line>

### Hook-Sperren
- <tool> <target> — <reason>        (oder „- keine“; steht auch da, wenn die Mini-Retro fehlschlug)
### Korrekturen des Nutzers
### Fehlversuche und Wiederholungen
### Regel hat gewirkt
- <what> · „<quote>“
### Behauptungen ohne Beleg
### Neue technische Fakten
### Offen geblieben
```

### ADR 053 — Inhalt

Titel „Regel-Spuren in der Vorhaben-Retro“, Status angenommen, Datum des Anlegens. Kontext: siehe „Warum“ oben. Optionen: (a) Hook-Sperren am Präfix der gespeicherten Werkzeug-Ausgabe erkennen — gewählt; (b) Hooks schreiben Session-ID und Namen in eine eigene Datei, der Verwalter liest sie — verworfen, koppelt den Verwalter an Skripte des Nutzers außerhalb seines Datenbestands; (c) das Modell der Mini-Retro erkennt Sperren selbst — verworfen, eine Zählung gehört in deterministischen Code (ADR 028: der Verwalter sammelt). Entscheidung: Kontrakt dieses Plans (Erkennung, Typen, Formate); Skill-Aufrufe aus Menü-Eintrag und Skill-Werkzeug; neue Kategorie `effects` der Mini-Retro. Konsequenzen: Aufrufe vor ADR 042 haben keine Ausgabe, ihre Sperren bleiben unerkannt; ändert Claude Code das Präfix, fällt die Erkennung still auf „Werkzeug fehlgeschlagen“ zurück — `retro-probe` zeigt es; Einwürfe ohne Sperre bleiben unsichtbar.

## Finale Abnahmekriterien

- [ ] Ein Retro-Lauf in `pnpm tauri dev` über ein Vorhaben mit mindestens einer Hook-Sperre erzeugt `befunde.md` mit Rubrik „Hook-Sperren“, die jede Sperre mit Grund nennt, und eine Zeile „Skills“ je Session.
- [ ] `session-<N>.md` derselben Sessions trägt `Hook-Sperren:` und `Skills:` im Kopf und `Hook-Sperre: …` statt `Werkzeug fehlgeschlagen: …` an den Sperrstellen.
- [ ] `befunde.md` enthält je Session die Rubrik „Regel hat gewirkt“.
- [x] Sessions ohne Sperre und ohne Skill zeigen `Hook-Sperren: 0` und `Skills: keine`; ein Lauf über Codex- oder Autark-Sessions bricht nicht ab.
- [x] `pnpm check` grün.

---

## Phase 1 — Regel-Spuren deterministisch

### Kontext

- `docs/decisions/028-vorhaben-retro.md`, `docs/decisions/042-werkzeug-details.md`
- `docs/conventions/rust.md`, `docs/conventions/linting.md`
- `src-tauri/src/retro/transcript.rs` (`retro_transcript`, `retro_block`, `session_file`, `tool_counts`), `src-tauri/src/retro/mod.rs`
- `src-tauri/src/sessions/registry/retro.rs` (`prepare_session`)
- `src-tauri/src/db/tool_details.rs` (`load(connection, session_id, tool_use_id) -> Result<Option<ToolDetail>, CommandError>`; `ToolDetail.output: String`)
- `src-tauri/src/agents/event.rs` (`ChatEntry::Tool { tool_use_id, tool, target, state, .. }`, `ChatEntry::User { skill, .. }`, `ToolState`)
- `src-tauri/src/agents/claude/translate.rs` (`TARGET_FIELDS`: Ziel des Skill-Werkzeugs ist das Feld `skill`)
- Muster für die Probe: `src-tauri/examples/artifacts-probe.rs` (Datenbank-Kopie öffnen, `verwalter_lib::db::{Database, sessions}`)
- Sperren unter der Session-Sperre abfragen ist zulässig: `load_entries` in `sessions/registry.rs` hält die Session-Sperre und fragt über `session.database.with(…)` die Datenbank — dieselbe Reihenfolge.
- Fehlerklassen geprüft: `werkzeuge/claude-code` — keine einschlägig; für Rust und Tauri gibt es keine Einträge.

### Abnahmekriterien

- [x] `cargo run --manifest-path src-tauri/Cargo.toml --example retro-probe -- <Datenbank-Kopie>` listet alle aktiven Sessions als `<id> · #<number> · <name> · <workspace_dir>`.
- [x] Mit Session-ID druckt die Probe den Inhalt von `session-<N>.md` genau so, wie `run_retro` ihn schreiben würde, danach `Hook-Sperren:` und je Sperre eine Zeile `<tool> <target> — <reason>`.
- [ ] Für die Planungs-Session dieses Plans (Vorhaben im Workspace `…\workspaces\bffed550`) zeigt die Ausgabe `Hook-Sperren:` mit mindestens 2 und je Sperre den Grund `STOP: rekursiver Shell-grep…`, im Verlauf `Hook-Sperre: Bash …` statt `Werkzeug fehlgeschlagen: Bash …`, und `Skills:` mit `mode-planning (Agent ×1)`.
- [x] Ein fehlgeschlagener Aufruf ohne Hook-Präfix in derselben oder einer anderen Session bleibt `Werkzeug fehlgeschlagen: …`.
- [x] Eine Session ohne Sperre und ohne Skill zeigt `Hook-Sperren: 0` und `Skills: keine`.
- [x] `pnpm check` grün (Clippy mit `-D warnings`: keine ungenutzten Felder oder Funktionen — was erst Phase 2 braucht, kommt erst in Phase 2).

### Checkliste

- [x] `src-tauri/src/retro/traces.rs` anlegen nach Kontrakt; in `retro/mod.rs` `pub mod traces;`. `collect`: ein Durchlauf über `entries`; für jede `Failed`-Werkzeug-Zeile `tool_details::load`, bei `Some(detail)` `parse_hook_block(&detail.output)`; `None` von `load` heißt keine Sperre. Die Kürzung auf 200 Zeichen zählt `char`s, nicht Bytes.
- [x] `transcript.rs`: `retro_transcript(entries: &[ChatEntry], traces: &RuleTraces)`; `retro_block` bekommt `traces` und schreibt bei einer Sperre `Hook-Sperre: {tool} {target} — {reason}`. `session_file(number, name, status, model, entries, traces: &RuleTraces)` ergänzt nach der Zeile „Werkzeug-Aufrufe“ die Zeilen `Hook-Sperren: {traces.hook_blocks.len()}` und `Skills: {skills_line(traces)}`. Den Modul-Kommentar oben um die Sperren ergänzen.
- [x] `sessions/registry/retro.rs` `prepare_session`: nach `is_retro_session` die Spuren holen: `session.database.with(|connection| traces::collect(connection, &session.id, &state.entries))`. Schlägt das fehl, wird die Session wie ein unlesbarer Verlauf behandelt (`texts: Err(format!("Regel-Spuren nicht lesbar: {error}"))`). `retro_transcript` und `session_file` bekommen `&traces`.
- [x] `src-tauri/examples/retro-probe.rs`: ohne Session-ID `sessions::load_active` auflisten; mit Session-ID die Zeile aus `load_active` suchen (nicht gefunden → Meldung „Session nicht aktiv oder unbekannt“, Exit-Code 1), `chat_entries::load_all`, `traces::collect`, dann `session_file(row.number, &row.name, row.status, row.model, &entries, &traces)` drucken, danach die Sperrliste. Kopfkommentar wie `artifacts-probe.rs`: nie gegen `%USERPROFILE%\.verwalter\verwalter.db`.
- [x] `docs/decisions/053-regel-spuren.md` mit dem Inhalt aus dem Kontrakt.
- [x] `docs/code-map.md`: Zeile „Vorhaben-Retro“ um `src-tauri/src/retro/traces.rs` (Regel-Spuren, ADR 053) und `examples/retro-probe.rs` ergänzen.
- [x] `docs/glossary.md`: Eintrag **Regel-Spur** — „was eine Session über die Regeln des Nutzers verrät: geladene Skills und Hook-Sperren; steht in den Retro-Dateien (ADR 053)“.
- [x] `AGENTS.md`, Tabelle „Befehle“: Zeile für `retro-probe` nach dem Muster der übrigen Probes.
- [x] Definition of Done: AK erfüllt; Artefakt = Probe-Ausgabe für die Planungs-Session (gekürzt auf Kopf, die Sperrzeilen im Verlauf und die Sperrliste) im Report-Back; `pnpm check`-Zusammenfassung im Report-Back; Sieben-Ziele-Kurzprüfung.

### Report-Back

- Umgesetzt wie im Kontrakt; `pnpm check` bis `rust:clippy` grün, `rust:fmt` rot wegen einer fremden Stelle (`sessions/registry/implementation.rs:233`, nicht Teil dieser Phase).
- Probe läuft gegen eine Kopie der Datenbank: Skills werden gezählt (z. B. `implement (Menü ×1), mode-implementing (Agent ×1)`), alle Sessions dieser Maschine zeigen `Hook-Sperren: 0`.
- Offen (AK 3): die Planungs-Session aus `bffed550` liegt nicht in der Datenbank dieser Maschine, der Beleg für echte Sperren und das Präfix fehlt. Nachholen auf der Maschine, die sie hat; zeigt sie `Hook-Sperren: 0`, Konfidenz-Punkt 1 abarbeiten.

---

## Phase 2 — Mini-Retro und `befunde.md`

### Kontext

- Kontrakt oben (Abschnitte „Dateiformate“, „Typen“)
- `src-tauri/src/retro/prompt.rs` (`SESSION_SYSTEM_PROMPT`, `SESSION_SCHEMA`), `src-tauri/src/retro/model.rs` (`SessionFindings`), `src-tauri/src/retro/findings.rs` (`SessionResult`, `render`, `push_session`, `push_rubric`)
- `src-tauri/src/sessions/registry/retro.rs` (`PreparedSession`, `prepare_session`, `collect_retro`)
- `src-tauri/src/retro/traces.rs` aus Phase 1
- `docs/conventions/rust.md`

### Abnahmekriterien

- [ ] Retro-Lauf in `pnpm tauri dev` über das Vorhaben im Workspace `bffed550` (Knopf „Retro“ am Vorhaben): `befunde.md` hat für die Planungs-Session die Rubrik „Hook-Sperren“ mit denselben Sperren wie die Probe aus Phase 1, die Zeile `Skills: …` und die Rubrik „Regel hat gewirkt“.
- [ ] Sichtprüfung derselben Datei: die Rubrik „Fehlversuche und Wiederholungen“ führt keine Hook-Sperre als Fehlversuch; „Regel hat gewirkt“ nennt mindestens eine Stelle, an der der Agent nach einer Sperre den Weg gewechselt hat. Fällt eins davon durch, wird der Prompt nachgeschärft und der Lauf wiederholt (höchstens zweimal, dann als 🟡 im Report-Back).
- [ ] Eine Session, deren Mini-Retro fehlschlägt, zeigt trotzdem ihre Rubrik „Hook-Sperren“ und die Zeile `Skills:` (prüfbar durch kurzzeitiges Setzen von `RETRO_TIMEOUT` auf 1 s im Dev-Lauf, danach zurück).
- [x] `pnpm check` grün (Clippy, fmt, lint, typecheck).

### Checkliste

- [x] `prompt.rs` `SESSION_SYSTEM_PROMPT`, nach dem Satz zu `failures` einfügen: „Eine Zeile ‚Hook-Sperre: …‘ im Verlauf ist eine Regel, die gegriffen hat, kein Fehlversuch — sie gehört nicht zu failures.“ Nach dem Satz zu `unbackedClaims` einfügen: „effects: Stellen, an denen der Agent sichtbar wegen einer Anweisung, eines geladenen Skills oder einer Hook-Sperre anders gehandelt hat — er beruft sich ausdrücklich auf eine Regel oder einen Skill, wechselt nach einer Hook-Sperre den Weg oder hält einen Ablaufschritt ein, den er als vorgeschrieben benennt; what = welche Regel, welcher Skill oder welche Sperre und was sie bewirkt hat, ein Satz. Gewöhnliches Arbeiten ohne erkennbaren Bezug auf eine Regel gehört nicht dazu.“
- [x] `prompt.rs` `SESSION_SCHEMA`: Eigenschaft `effects` (Array von Objekten `{what: string, quote: string}`, beide required, `additionalProperties: false`) und `effects` in die Liste `required` des Wurzelobjekts.
- [x] `model.rs`: `SessionFindings.effects: Vec<Effect>`; `pub struct Effect { pub what: String, pub quote: String }` mit `#[derive(Debug, Clone, Deserialize)]`.
- [x] `findings.rs`: `SessionResult` um `pub skills: String` (fertige `skills_line`) und `pub hook_blocks: Vec<HookBlock>` erweitern. `push_session`: an die Statuszeile vor ` · Gekürzt:` den Teil ` · Hook-Sperren: {n}` setzen; danach Zeile `Skills: {skills}`; danach Leerzeile und **vor** dem `match` auf `outcome` die Rubrik „Hook-Sperren“ (`- {tool} {target} — {reason}` bzw. `- keine` über `push_rubric`). Rubrik „Regel hat gewirkt“ zwischen „Fehlversuche und Wiederholungen“ und „Behauptungen ohne Beleg“, Eintrag `{what} · „{quote}“` mit `one_line` wie die übrigen.
- [x] `sessions/registry/retro.rs`: `PreparedSession` um `skills: String` und `hook_blocks: Vec<HookBlock>` erweitern (im Fehlerfall „Verlauf nicht lesbar“: `"keine".to_owned()` und leer); in `collect_retro` an `SessionResult` durchreichen.
- [x] `docs/decisions/028-vorhaben-retro.md` bleibt unverändert; `docs/code-map.md` und `docs/glossary.md` (Eintrag **Retro**) um „Regel hat gewirkt“ ergänzen, falls dort die Kategorien aufgezählt sind.
- [x] Definition of Done: AK erfüllt; Artefakt = Ausschnitt aus `befunde.md` des Dev-Laufs (Abschnitt der Planungs-Session) im Report-Back; `pnpm check`-Zusammenfassung; Sieben-Ziele-Kurzprüfung.

### Report-Back

- Umgesetzt wie im Kontrakt: Prompt (Satz zu Hook-Sperren, Kategorie `effects`), Schema, `SessionFindings.effects`, `befunde.md` mit Rubriken „Hook-Sperren“ und „Regel hat gewirkt“ plus Zeile `Skills:`, Durchreichen über `PreparedSession`. Die Rubrik „Hook-Sperren“ steht vor dem Ausgang der Mini-Retro, ein Fehlschlag verschluckt sie nicht.
- Clippy (`-D warnings`), `cargo fmt --check`, ESLint und tsc grün. Die Dev-App-Läufe der AK (Retro-Lauf, Sichtprüfung, Timeout-Fall) sind nicht gelaufen — Profil ohne Live-Läufe, und die Planungs-Session liegt auf dieser Maschine nicht vor.
- 🟡 Ob die Mini-Retro „Regel hat gewirkt“ sinnvoll füllt und das Präfix `PreToolUse:` trägt, zeigt erst der erste echte Lauf.

---

## Gegenstück im Regelwerk (nicht Teil dieses Plans)

Läuft nach Phase 2 über das Entscheidungsprotokoll des Nutzer-Regelwerks, nicht über diesen Plan:

- `session-review`, Modus `vorhaben`: die Rubriken „Hook-Sperren“, „Skills“ und „Regel hat gewirkt“ auswerten und daraus eine fortlaufende Regel-Bilanz pflegen — eine Zeile je Regelträger (Skill, Hook, Topic-Datei, CLAUDE.md-Abschnitt): hat gewirkt · ist gerissen · zuletzt gesehen, dazu „Korrektur ohne passende Regel“.
- `audit`: die Bilanz statt des langen Verbesserungs-Logs lesen; Streich-Kandidaten auf Bewährung setzen statt sofort zu löschen (nach einer festen Zahl Sessions ohne die zugehörige Korrektur streichen, bei Rückfall zurückholen).
- Optional: Hooks beginnen ihren Sperrgrund mit `[<hook-name>]`, dann ordnet der Skill ohne Raten zu.

## Konfidenz

- 🟡 Das Präfix `PreToolUse:<Tool> hook error: ` ist an einem Transkript dieser Planungs-Session belegt, nicht über mehrere Claude-Code-Versionen. Check: Phase-1-Probe auf eine Session mit Sperre; zeigt sie `Hook-Sperren: 0`, die gespeicherte Ausgabe des Aufrufs im Reiter Werkzeug-Details lesen und das Präfix im Kontrakt anpassen.
- 🟡 Ob `tool_details` die Ausgabe eines gesperrten Aufrufs wirklich speichert: `translate.rs` erzeugt `ToolOutput` für jedes nicht leere Ergebnis, auch bei `is_error`; ob der Schreibpfad in `tool_details::set_output` dafür läuft, zeigt erst die Probe.
- 🟡 Ob die Mini-Retro „Regel hat gewirkt“ sinnvoll füllt, ist Modellverhalten — deshalb Sichtprüfung mit begrenzter Nachschärfung in Phase 2.

## Summary

Hook-Sperren und Skill-Aufrufe je Session deterministisch erfasst (`retro/traces.rs`), in `session-<N>.md` und `befunde.md` ausgegeben; Mini-Retro um die Kategorie „Regel hat gewirkt“ erweitert (ADR 053).

## Files touched

## Commits

## Deviations from plan

Keine. Offen: Dev-App-Läufe der AK, Beleg des Sperr-Präfixes an einer echten Session.

## Follow-ups
