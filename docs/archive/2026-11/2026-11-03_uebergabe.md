# Plan: Übergabe bei kaltem Cache

**Ziel:** Der Knopf „In neuer Session weiter“ im Hinweis über dem Eingabefeld (kalter Prompt-Cache, ADR 033) legt nicht mehr nur eine leere Session an, sondern lässt Haiku den Gesprächstext der alten Session zu einer Übergabe zusammenfassen und setzt sie in den Entwurf der neuen Session. Damit lässt sich nach Feierabend, Wochenende, leerem Kontingent, Netzausfall oder zugeklapptem Laptop weiterarbeiten, ohne den ganzen Verlauf der alten Session neu in den Cache zu schreiben.

**Warum:** Der Cache einer langen Session besteht vor allem aus Werkzeug-Ausgaben; eine Nachricht nach Ablauf schreibt ihn ganz neu (im Hinweis z. B. 384k Token auf Opus). Das TL;DR (ADR 011) schickt Haiku nur den Gesprächstext ohne Werkzeuge und Gedankengang; gemessen: Transkript an der Obergrenze 292 298 Zeichen → 107 169 Eingabe-Token, 18,7 s (`docs/knowledge/claude-stream-json.md`, „Einmal-Aufruf im Druckmodus“), echte Verläufe Median 16k Zeichen (`tldr/transcript.rs`). Heute setzt der Knopf keinen Entwurf (ADR 033: „ohne Entwurf“), die neue Session startet ohne jeden Stand.

**Entscheidung:** ADR 054 (in Phase 1 anlegen; ADR 053 ist von `2026-11-03_regel-spuren.md` reserviert). Keine Migration, nichts wird gespeichert.

## Overview

| Phase | Inhalt | Status | Rating |
|---|---|---|---|
| 1 | Core: Command `tldr_handoff_brief` mit Haiku-Übergabe und Rückfall ohne Modell, ADR 054 | complete | standard |
| 2 | Oberfläche: Knopf im Cache-Hinweis verdrahten, Doku, Release | complete | standard |

Sequenziell: Phase 2 braucht den Typ `HandoffBrief` aus Phase 1 (`pnpm bindings`).

## Finale Abnahmekriterien

1. Klick auf „In neuer Session weiter“ im Hinweis bei kaltem Cache: Knopf zeigt „Fasse zusammen …“, danach „Lege Session an …“; die neue Session ist ausgewählt, ihr Eingabefeld enthält die Übergabe im Format aus dem Kontrakt und hat den Fokus. Gesendet wird nichts.
2. Die Übergabe nennt Nummer und Namen der alten Session und enthält nur nicht leere Felder.
3. Trägt die alte Session Einstiegszeilen der Art Session, stehen sie wörtlich in der Übergabe; nennt die erste davon ein Modell, läuft die neue Session auf diesem Modell (wie `HandoffButton`).
4. Scheitert Haiku (Kontingent leer, kein Netz, Zeitlimit, unlesbare Antwort), entsteht trotzdem eine Session; ihr Entwurf ist die Rückfall-Übergabe mit dem Grund im Kopf. Kein zweiter Knopf, keine Fehlermeldung, die beim Wechsel der Session verschwindet.
5. Scheitert schon das Lesen der alten Session (unbekannt, ohne Verlauf), entsteht keine Session; der Fehler steht im Hinweis unter dem Knopf.
6. Während des Laufs ist der Knopf gesperrt; ein Doppelklick legt keine zweite Session an.
7. Der Tooltip des Knopfs erklärt, was passiert (Text im Kontrakt).
8. Der Knopf „In neuer Session weiter“ unter einer Einstiegszeile (`HandoffButton`) verhält sich unverändert.
9. `pnpm check` grün.

## Kontrakt

### Command (`src-tauri/src/commands/tldr.rs`, registriert in `src-tauri/src/lib.rs` neben `tldr_session_create`)

```rust
#[tauri::command]
pub async fn tldr_handoff_brief(
    app: tauri::AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    session_id: String,
) -> Result<HandoffBrief, CommandError>;
```

Ablauf: erst `registry.handoff_brief_input(&session_id)?` (hält die Session-Sperre nur dafür), dann in `tauri::async_runtime::spawn_blocking` `print_program(&app)` und `ask_haiku` mit `HANDOFF_SYSTEM_PROMPT`, `HANDOFF_SCHEMA`, Eingabe `input.transcript`, Zeitlimit `TLDR_TIMEOUT` — Muster: `git_suggest_message` in `src-tauri/src/commands/git.rs`. Ein Fehler von `print_program` oder `ask_haiku` wird **nicht** zurückgegeben, sondern führt zum Rückfall (`summarized: false`, `failure: Some(<Fehlertext>)`). Nur `handoff_brief_input` und ein `JoinError` geben `Err` zurück.

### Typen (`src-tauri/src/tldr/model.rs`, mit `#[derive(Debug, Clone, Serialize, TS)]` und `#[serde(rename_all = "camelCase")]` wie die übrigen Typen dort; wie `pnpm bindings` ihn aufnimmt, steht in `docs/conventions/rust.md`, Abschnitt Typgenerierung)

```rust
/// Text für den Entwurf der Folgesession (ADR 054); nur im Speicher.
pub struct HandoffBrief {
    pub text: String,
    /// `true`, wenn Haiku zusammengefasst hat; `false` beim Rückfall.
    pub summarized: bool,
    /// Grund des Rückfalls; `None`, wenn Haiku geantwortet hat.
    pub failure: Option<String>,
}
```

Nicht exportiert (`src-tauri/src/tldr/brief.rs`, neu):

```rust
pub struct BriefInput {
    pub number: u32,
    pub name: String,
    /// `session_transcript(&entries)`.
    pub transcript: String,
    /// `fallback_body(&entries)`.
    pub fallback: String,
    /// Texte der Einstiegszeilen der Art Session aus `state.handoff_lines`, in Reihenfolge.
    pub handoff_texts: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HandoffSummary { pub goal: String, pub done: String, pub decisions: String, pub current: String, pub open: String, pub next: String, pub references: String }

pub fn summarized_text(input: &BriefInput, summary: &HandoffSummary) -> String;
pub fn fallback_text(input: &BriefInput, failure: &str) -> String;
/// Erster Nutzer-Beitrag (höchstens 2 000 Zeichen) und die letzten Beiträge (zusammen höchstens 6 000 Zeichen).
pub fn fallback_body(entries: &[ChatEntry]) -> String;
```

Registry (`src-tauri/src/sessions/registry/tldr.rs`): `pub fn handoff_brief_input(&self, session_id: &str) -> Result<BriefInput, CommandError>` — `self.get(session_id)?`, `session.lock_loaded()?`; bei `SessionStatus::New` oder leeren `entries` `Err(CommandError::Internal(NO_SESSION_HISTORY.to_owned()))` wie `begin_tldr`; `number` aus `session.number`, `name` aus `state.name`, `handoff_texts` aus `state.handoff_lines` mit `kind == HandoffKind::Session`.

### Anweisung und Schema (`src-tauri/src/tldr/prompt.rs`)

```rust
pub const HANDOFF_SYSTEM_PROMPT: &str = "Du schreibst die Übergabe einer Arbeitssitzung zwischen einem Entwickler (Nutzer) und einem Coding-Agenten (Claude) an eine neue Sitzung, die ohne diesen Verlauf weitermacht. Du bekommst nur den Gesprächstext, keine Werkzeug-Ausgaben. Antworte auf Deutsch, sachlich, ohne Einleitung; jedes Feld ist leer, wenn der Verlauf nichts dazu enthält. goal: worum es geht, höchstens zwei Sätze. done: was erledigt ist, als Stichpunkte mit „- “ je Zeile. decisions: getroffene Entscheidungen und verworfene Wege samt Grund, als Stichpunkte. current: woran zuletzt gearbeitet wurde und wie weit es ist, höchstens drei Sätze. open: offene Punkte und unbeantwortete Fragen, als Stichpunkte; beginnt mit „Deine Entscheidung:“, wenn der Agent auf den Nutzer wartet. next: der nächste Schritt, ein Satz. references: im Verlauf genannte Dateien, Pläne, Branches, Tickets und Befehle, die die neue Sitzung braucht, als Stichpunkte, wörtlich wie im Verlauf. Erfinde nichts, was nicht im Verlauf steht.";

pub const HANDOFF_SCHEMA: &str = r#"{"type":"object","properties":{"goal":{"type":"string"},"done":{"type":"string"},"decisions":{"type":"string"},"current":{"type":"string"},"open":{"type":"string"},"next":{"type":"string"},"references":{"type":"string"}},"required":["goal","done","decisions","current","open","next","references"],"additionalProperties":false}"#;
```

### Format der Übergabe (Leser: der Agent der neuen Session)

Mit Zusammenfassung (`summarized_text`); Felder mit leerem `trim()` entfallen samt Bezeichnung, Mehrzeiliges steht unter der Bezeichnung:

```
Übergabe aus Session #<N> „<Name>“ — von Haiku aus dem Gesprächstext zusammengefasst, ohne Werkzeug-Ausgaben.

Ziel: <goal>
Erledigt:
<done>
Entscheidungen:
<decisions>
Stand: <current>
Offen:
<open>
Nächster Schritt: <next>
Fundstellen:
<references>
Einstiegszeile: <handoff_texts[i]>      ← je Eintrag eine Zeile, nur wenn vorhanden

Prüfe vor dem ersten Schritt den Stand im Repository (git status, STATE.md, falls vorhanden); was hier fehlt, steht dort oder in den genannten Dateien.
```

Regel für die Bezeichnungen: einzeiliger Wert → `<Bezeichnung>: <Wert>` in einer Zeile; Wert mit Zeilenumbruch → `<Bezeichnung>:` und darunter der Wert. Zwischen Kopf, Feldblock und Schlusssatz je eine Leerzeile, im Feldblock keine.

Rückfall (`fallback_text`):

```
Übergabe aus Session #<N> „<Name>“ — ohne Zusammenfassung (<failure>), nur Auszüge aus dem Gesprächstext.

<fallback>
Einstiegszeile: <handoff_texts[i]>      ← wie oben

Prüfe vor dem ersten Schritt den Stand im Repository (git status, STATE.md, falls vorhanden); der Auszug ist unvollständig.
```

`fallback_body`: Blöcke aus `entry_block` (alle Einträge). `Erste Nachricht:` + Zeilenumbruch + erster Block mit `is_user`, mit `truncated` auf 2 000 Zeichen; Leerzeile; `Letzte Beiträge:` + Zeilenumbruch + die letzten Blöcke von hinten gesammelt, solange die Summe ihrer Zeichen 6 000 nicht überschreitet, in Verlaufsreihenfolge mit `BLOCK_SEPARATOR` verbunden; ist schon der letzte Block länger, steht er allein, auf 6 000 Zeichen gekürzt. Der erste Nutzer-Block erscheint nicht doppelt, wenn er unter den letzten ist. `failure` wird auf eine Zeile gezogen und auf 200 Zeichen gekürzt.

### Oberfläche (`src/features/promptcache/CacheColdHint.tsx`, Wrapper in `src/lib/tldr.ts`)

```ts
/** Übergabe der Session für den Entwurf einer Folgesession (ADR 054); scheitert Haiku, kommt der Rückfall.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `internal` (unbekannte Session, ohne Verlauf) */
export function createHandoffBrief(sessionId: string): Promise<HandoffBrief> {
  return invoke<HandoffBrief>('tldr_handoff_brief', { sessionId });
}
```

Tooltip (`CONTINUE_TITLE`): „Haiku fasst den Gesprächstext dieser Session zusammen (wenige Sekunden, kostet wenig), legt im Vorhaben eine neue Session an und setzt die Zusammenfassung in ihr Eingabefeld. Gesendet wird erst, wenn du auf Senden klickst.“

## Phase 1 — Core

### Kontext

- `docs/conventions/rust.md`
- `src-tauri/src/tldr/transcript.rs` (`session_transcript`, `entry_block`, `Block`, `truncated`, `labelled_lines`, `BLOCK_SEPARATOR`)
- `src-tauri/src/tldr/prompt.rs`, `src-tauri/src/tldr/model.rs`, `src-tauri/src/tldr/mod.rs`
- `src-tauri/src/sessions/registry/tldr.rs` (`begin_tldr`, `NO_SESSION_HISTORY`)
- `src-tauri/src/commands/git.rs` (`git_suggest_message`: Muster für `spawn_blocking` + `ask_haiku`), `src-tauri/src/commands/tldr.rs`, `src-tauri/src/lib.rs` (Command-Liste)
- `src-tauri/src/agents/claude/print.rs` (`HaikuRequest`, `ask_haiku`, `print_program`)
- `src-tauri/src/sessions/model.rs` (`HandoffLine`, `HandoffKind`)
- Vorlage für das ADR: `docs/decisions/044-nebenfragen.md` (Gliederung Kontext / Optionen / Entscheidung / Konsequenzen)
- Fehlerklassen im Vault geprüft: keine Entity zu Rust, TypeScript, React oder Tauri vorhanden.

### Abnahmekriterien

- `tldr_handoff_brief` liefert für eine Session mit Verlauf ein `HandoffBrief` im Format des Kontrakts, auch wenn `claude.exe` fehlt oder scheitert (dann `summarized: false`).
- Für eine unbekannte Session oder eine ohne Verlauf liefert es `Err`.
- Die Session-Sperre ist während des Haiku-Aufrufs nicht gehalten (Eingabe vor `spawn_blocking` gebaut).
- `pnpm bindings` erzeugt `src/lib/bindings/HandoffBrief.ts`; `pnpm check` grün.

### Checkliste

- [x] `transcript.rs`: `truncated` und `labelled_lines` auf `pub(crate)` stellen (nichts sonst ändern).
- [x] `tldr/brief.rs` neu: `BriefInput`, `HandoffSummary`, `summarized_text`, `fallback_text`, `fallback_body` laut Kontrakt; Schlusssätze und Kopfzeilen als `const`. In `tldr/mod.rs` `pub mod brief;` ergänzen und den Modul-Kommentar um die Übergabe erweitern.
- [x] `tldr/prompt.rs`: `HANDOFF_SYSTEM_PROMPT`, `HANDOFF_SCHEMA` wörtlich aus dem Kontrakt.
- [x] `tldr/model.rs`: `HandoffBrief`.
- [x] `sessions/registry/tldr.rs`: `handoff_brief_input`.
- [x] `commands/tldr.rs`: `tldr_handoff_brief`; in `lib.rs` registrieren.
- [x] `pnpm bindings`, `pnpm check`.
- [x] `docs/decisions/054-uebergabe-bei-kaltem-cache.md`: Kontext (Hinweis aus ADR 033 legte leere Session an; Cache-Neuschreiben teuer), Optionen (a) Nebenfrage auf einer Kopie der Session (ADR 044) — günstig nur bei warmem Cache, bei kaltem schreibt sie den ganzen Verlauf; (b) Haiku über den Gesprächstext wie das TL;DR — unabhängig vom Cache; (c) nur Auszüge ohne Modell — Gewählt (b) mit (c) als Rückfall; Entscheidung: nichts gespeichert, Entwurf statt Senden (wie ADR 027), Rückfall statt Fehler, damit Kontingent-Ende und Netzausfall eine Session ergeben; Konsequenzen: Haiku kennt keine Werkzeug-Ausgaben (die neue Session liest Dateien selbst nach), jeder Klick ist ein Haiku-Aufruf, bei leerem Kontingent wartet der Klick bis zu `TLDR_TIMEOUT` auf den Rückfall, wenn die Kommandozeile nicht sofort ablehnt.
- [x] `docs/decisions/033-warnhinweise.md`: im Kopf „teilweise abgelöst durch ADR 054 (Knopf im Cache-Hinweis)“ ergänzen, wie in ADR 027 vorgemacht.

## Phase 2 — Oberfläche, Doku, Release

### Kontext

- `docs/conventions/react.md`, `docs/conventions/typescript.md`
- `src/features/promptcache/CacheColdHint.tsx` (heutiger Knopf: `continueInNewSession`, `CONTINUE_TITLE`)
- `src/features/chat/HandoffButton.tsx` (Muster: Modell der Einstiegszeile mit `setSessionModel` übernehmen, `setHandoffDraft`, `focusComposer`)
- `src/features/chat/composerDraft.ts`, `src/lib/tldr.ts`, `src/lib/sessions.ts`
- `docs/code-map.md` (Zeilen „TL;DR“ und „Prompt-Cache“), `docs/glossary.md`, `docs/conventions/releases.md`

### Abnahmekriterien

- AK 1–8 der finalen Abnahmekriterien.

### Checkliste

- [x] `src/lib/tldr.ts`: `createHandoffBrief` laut Kontrakt.
- [x] `CacheColdHint.tsx`: Zustand `phase: 'idle' | 'summarizing' | 'creating'` ersetzt `isCreating`. `continueInNewSession`: `phase = 'summarizing'`, `createHandoffBrief(session.id)` — bei Fehler `errorMessage` setzen, `phase = 'idle'`, Ende; dann `phase = 'creating'`, `createSessionInProject(session.projectId)` — bei Fehler wie bisher; `onSessionCreated(created)`; Modell: erste Zeile aus `session.handoffLines` mit `kind === 'session'`, ist ihr `model` nicht `null` und ungleich `created.model`, `setSessionModel(created.id, model)` (Fehlertext wie in `HandoffButton`, Ablauf geht weiter); `setHandoffDraft(created.id, brief.text)`; `focusComposer()`; `phase = 'idle'`. Knopftext: `idle` „In neuer Session weiter“, `summarizing` „Fasse zusammen …“, `creating` „Lege Session an …“; `disabled` bei `phase !== 'idle'`. `CONTINUE_TITLE` laut Kontrakt.
- [x] `pnpm check`.
- [x] `docs/code-map.md`: Zeile „TL;DR“ um `tldr/brief.rs` (Übergabe für die Folgesession) und den Command `tldr_handoff_brief` ergänzen; Zeile „Prompt-Cache“: `CacheColdHint` legt die Folgesession mit Übergabe im Entwurf an (ADR 054).
- [x] `docs/glossary.md`: Begriff **Übergabe** — „Von Haiku aus dem Gesprächstext einer Session geschriebene Zusammenfassung, die der Knopf im Hinweis bei kaltem Cache in den Entwurf der Folgesession setzt; ohne Haiku ein Auszug aus erster Nachricht und letzten Beiträgen (ADR 054). Nicht zu verwechseln mit der Einstiegszeile, die der Agent selbst schreibt.“
- [x] Plan archivieren und Release nach `docs/conventions/releases.md` (Minor-Version).

## Konfidenz-Ausweis (Wackelstellen)

- 🟡 Wie schnell `claude -p` bei leerem Kontingent ablehnt, ist nicht gemessen; im schlechtesten Fall wartet der Klick 90 s (`TLDR_TIMEOUT`) auf den Rückfall. Check: einmal bei erschöpftem 5-Stunden-Fenster klicken und die Dauer bis „Lege Session an …“ ablesen.
- 🟡 Ob Haiku aus reinem Gesprächstext brauchbare „Entscheidungen“ und „Fundstellen“ zieht, ist nicht belegt. Check: in `pnpm tauri dev` an einer langen echten Session klicken und die Übergabe gegen den Verlauf lesen.
- Haiku-Kontext reicht: das Transkript ist auf 300 000 Zeichen gedeckelt, gemessen 107 169 Token bei 292 298 Zeichen; Haiku hat 200 000 (`agents/event.rs`).

## Summary

Der Knopf „In neuer Session weiter“ im Hinweis bei kaltem Cache lässt Haiku den Gesprächstext zusammenfassen und setzt die Übergabe in den Entwurf der neuen Session; scheitert Haiku, kommt ein Auszug als Rückfall (ADR 054).

## Files touched

`src-tauri/src/tldr/` (`brief.rs`, `prompt.rs`, `model.rs`), `commands/tldr.rs`, `sessions/registry/tldr.rs`, `src/lib/tldr.ts`, `src/features/promptcache/CacheColdHint.tsx`, `docs/` (ADR 054, Code-Map, Glossar).

## Commits

Phase 1 `a568d5f`; Phase 2 und Release siehe Log.

## Deviations from plan

Phase 1: `labelled_lines` blieb privat (`brief.rs` formatiert die Felder selbst, wegen der Mehrzeilen-Regel); stattdessen `BLOCK_SEPARATOR` auf `pub(crate)`.

## Follow-ups

Wackelstellen aus dem Konfidenz-Ausweis (Dauer bei leerem Kontingent, Qualität der Übergabe aus reinem Gesprächstext) stehen für den Betrieb aus.
