# Notiz pro Option in Rückfragen

Ziel: In einer Rückfrage des Agenten (Kasten „Claude wartet auf deine Entscheidung“) kann man zu jeder Option eine kurze Notiz schreiben, etwa „ja, aber erst nach dem Labortest“. Die Notiz geht zusammen mit der gewählten Option an den Agenten und steht danach im Verlauf in der Antwortzeile. Rechte-Abfragen (Erlauben/Ablehnen) bleiben unverändert. Freitext im Eingabefeld beantwortet weiterhin die ganze Rückfrage.

Kontext für den Umsetzer: [AGENTS.md](../../AGENTS.md), [docs/code-map.md](../code-map.md) (Zeile „Chat“), [docs/glossary.md](../glossary.md), [docs/conventions/react.md](../conventions/react.md), [typescript.md](../conventions/typescript.md), [tailwind.md](../conventions/tailwind.md). Fehlerklassen geprüft (Vault: TypeScript, React): keine einschlägig.

| # | Phase | Rating | Status |
|---|---|---|---|
| 1 | Notizfeld je Option, Antworttext mit Notiz, Anzeige im Verlauf; ADR 045; Doku-Abschluss | standard | pending |

**Rahmen:** eine Phase, direkt auf `main`, ein Commit, Scope `chat`. **Voraussetzung:** Phase 3 von `2026-10-29_nebenfragen` ist committet — sie ändert `src/features/chat/ChatTimeline.tsx`, das diese Phase auch anfasst. Nur Oberfläche: kein Core, keine Bindings, keine Migration. `pnpm check` grün vor dem Commit. Keine automatisierten Tests, keine Smoke-Checkliste. Planabschluss mit Versions-Tag (Minor-Version). Schätzung: eine Session, rund 1–2 Stunden.

## Festgelegte Entscheidungen

- **Transport:** Die Notiz reist im Antworttext der Frage, nicht in einem eigenen Feld. Aus der gewählten Option `Gerätetausch` mit Notiz `erst nach Labortest` wird der Text `Gerätetausch (Notiz: erst nach Labortest)`. Mehrfachauswahl wie bisher mit `, ` getrennt; nur Optionen mit nicht leerer Notiz bekommen den Zusatz. Der Core bleibt unverändert: `QuestionAnswer::Options { answers }` schreibt den Text in `answers[<Frage>]` und speichert `answers.join(" · ")` als Antwort im Verlauf (`answer_line` in `src-tauri/src/sessions/registry.rs`).
- **Notiz säubern:** `trim()`, Zeilenumbrüche gibt es nicht (einzeiliges Feld), jedes Vorkommen von ` · ` (Trenner zwischen Fragen) wird durch ` - ` ersetzt, damit die Anzeige im Verlauf die Fragen weiter trennen kann. Leere Notiz = keine Notiz.
- **Bedienung:** Jede Option einer Rückfrage (`questionKind === 'askUser'`, offen, nicht gesperrt) trägt rechts einen kleinen Knopf ✎ (`aria-label` und `title`: `Notiz zu „<Label>“`). Klick öffnet unter der Option ein einzeiliges Textfeld (Platzhalter „Notiz für Claude …“, `maxLength` 500) und setzt den Fokus hinein; ein zweiter Klick auf ✎ schließt das Feld und verwirft die Notiz. Der Knopf wählt die Option nicht aus.
- **Tippen wählt aus:** Wird eine Notiz nicht leer, ist ihre Option gewählt (Einfachauswahl: ersetzt die bisherige Wahl; Mehrfachauswahl: kommt dazu). Leeren der Notiz wählt nichts ab.
- **Kein Sofort-Senden mit offener Notiz:** Bisher sendet bei reiner Einfachauswahl der letzte fehlende Klick sofort. Sobald im Kasten ein Notizfeld offen ist, zeigt der Kasten stattdessen den Knopf „Antworten“, und Klicks auf Optionen wählen nur aus. Schließen alle Felder, gilt wieder das alte Verhalten.
- **Tasten im Notizfeld:** Enter = „Antworten“, sofern jede Frage eine Wahl hat; sonst nichts. Escape schließt das Feld und verwirft die Notiz. Die Zifferntasten wirken im Feld nicht (bereits so: `isTextTarget` in `ChatTimeline.tsx` übergeht `input`).
- **Notiz an abgewählter Option:** bleibt im offenen Feld stehen, wird aber nicht gesendet — gesendet werden nur Notizen gewählter Optionen.
- **Nach dem Senden:** Felder verschwinden (Kasten gesperrt bzw. beantwortet). Die Antwortzeile „Antwort: …“ zeigt den vollen Text samt Notizen; die gewählten Optionen bleiben hervorgehoben (`answeredLabels` erkennt `<Label> (Notiz: …)`).

## Kontrakt

```ts
// src/features/chat/questionDraft.ts
/** Wahl und Notizen je Frage, in Reihenfolge von `questions`. */
export interface QuestionDraft {
  readonly labels: readonly (readonly string[])[];
  /** Je Frage: Label → Notiz. Ein vorhandener Schlüssel heißt „Feld offen“, auch mit leerem Text. */
  readonly notes: readonly Readonly<Record<string, string>>[];
}
export function hasOpenNote(draft: QuestionDraft): boolean;
export function needsSubmit(entry: QuestionEntry, draft: QuestionDraft): boolean; // Mehrfachauswahl ODER hasOpenNote
export function toggleNote(draft: QuestionDraft, questionIndex: number, label: string): QuestionDraft;
export function editNote(entry: QuestionEntry, draft: QuestionDraft, questionIndex: number, label: string, text: string): QuestionDraft;
export function answerText(labels: readonly string[], notes: Readonly<Record<string, string>>): string;
```

`QuestionBlock` bekommt zusätzlich `onToggleNote(questionIndex, label)` und `onEditNote(questionIndex, label, text)`; `ChatTimeline` reicht sie an `setDrafts` durch wie heute `pick`.

## Phase 1 — Notiz pro Option

Lesen: `src/features/chat/questionDraft.ts` (ganz), `src/features/chat/QuestionBlock.tsx` + `QuestionBlock.css`, `src/features/chat/ChatTimeline.tsx` (`handleDigit`, `draftFor`, `submit`, `pick`, Fall `'question'` in `renderEntry`, `isTextTarget`), `src-tauri/src/sessions/registry.rs` (`answer_line`, `answer_oldest_with_text`, nur lesen), `docs/decisions/043-unveraenderte-zeilen.md` (ADR-Format).

AK:
1. Jede Option einer offenen Rückfrage zeigt rechts ✎ mit `aria-label` `Notiz zu „<Label>“`; Rechte-Abfragen und beantwortete Rückfragen zeigen keinen.
2. Klick auf ✎ öffnet unter genau dieser Option ein Textfeld mit Fokus; erneuter Klick schließt es und verwirft den Text; die Wahl ändert sich durch ✎ nicht.
3. Tippen in das Feld wählt die Option (Einfachauswahl ersetzt, Mehrfachauswahl ergänzt).
4. Mit offenem Feld sendet ein Klick auf eine Option nicht, der Knopf „Antworten“ erscheint; Enter im Feld sendet, sobald jede Frage eine Wahl hat; Escape schließt das Feld.
5. Zifferntasten im Feld tippen Ziffern und wählen keine Option.
6. Gesendet wird je Frage `<Label> (Notiz: <Text>)` für gewählte Optionen mit Notiz, sonst `<Label>`; ` · ` in einer Notiz kommt als ` - ` an. Belegt im Verlauf: Zeile „Antwort: …“ zeigt genau diesen Text, durch ` · ` je Frage getrennt.
7. Nach dem Senden sind die gewählten Optionen im beantworteten Kasten hervorgehoben, auch die mit Notiz.
8. Rückfragen ohne Notiz verhalten sich wie vorher (Einfachauswahl sendet beim letzten Klick, Mehrfachauswahl über „Antworten“, Freitext im Eingabefeld beantwortet alle Fragen).

- [ ] `questionDraft.ts`: `QuestionDraft` nach Kontrakt; `emptyDraft` liefert `labels` und `notes` (je Frage `{}`); `isComplete`, `digitQuestionIndex`, `pickOption` auf `draft.labels` umstellen; `pickOption` nimmt `needsSubmit(entry, next)` statt `needsSubmit(entry)`; `draftAnswer` baut `answers` aus `answerText(labels, notes)` je Frage. Konstanten `NOTE_PREFIX = ' (Notiz: '`, `NOTE_SUFFIX = ')'`, `NOTE_SEPARATOR_REPLACEMENT = ' - '`. `editNote` setzt den Text und wählt die Option bei nicht leerem Text wie `pickOption` es täte (Einfachauswahl ersetzt, Mehrfachauswahl ergänzt, ohne abzuwählen). `answeredLabels`: ein Label gilt als gewählt, wenn `part === label` oder ein mit `MULTI_SELECT_SEPARATOR` getrenntes Stück gleich `label` ist oder mit `label + NOTE_PREFIX` beginnt.
- [ ] `QuestionBlock.tsx`: Optionszeile als `<div className="question-block__option-row">` mit bisherigem Options-Knopf und neuem Knopf `question-block__note-toggle` (✎) daneben — nicht verschachtelt, ein `<button>` darf keinen Knopf enthalten. Darunter bei offenem Feld `<input type="text" className="question-block__note">` mit `autoFocus`, `maxLength={500}`, Platzhalter „Notiz für Claude …“, `aria-label` `Notiz zu „<Label>“`, Enter/Escape nach Entscheidung. `renderFooter` nutzt `needsSubmit(entry, draft)`. ✎ und Feld nur, wenn `entry.questionKind === 'askUser'` und nicht `isLocked`.
- [ ] `QuestionBlock.css`: `__option-row` (Flex, `gap: var(--space-sm)`, Options-Knopf `flex: 1`), `__note-toggle` (quadratisch 28px, Rahmen und Farben wie `__option`, Hover wie dort, `--open` mit `border-color: var(--color-accent)`), `__note` (volle Breite, `height: 28px`, Rahmen `var(--color-border)`, Fokus-Umriss wie `__submit`, Schrift `var(--font-size-xs)`). Nur vorhandene Token.
- [ ] `ChatTimeline.tsx`: `onToggleNote`/`onEditNote` an `QuestionBlock`, beide schreiben über `setDrafts` wie `pick`; `handleDigit` liest `draft.labels` über die angepassten Funktionen (keine eigene Logik).
- [ ] `docs/decisions/045-notiz-pro-option.md` im Format von ADR 043: Kontext (Rückfragen kennen nur Optionen oder Freitext für alles); Optionen (a) Notiz im Antworttext — gewählt: erreicht den Agenten sicher, braucht keinen Core-Umbau, steht im Verlauf; (b) eigenes Feld `notes` im Kontrakt und Weitergabe als `annotations` an Claude Code — verworfen: ob Claude Code diese Notizen dem Modell zeigt, ist nicht belegt, und der gespeicherte Antworttext verlöre sie; (c) Notiz nur je Frage statt je Option — verworfen, entspricht nicht dem Wunsch. Konsequenzen: der Antworttext ist für den Agenten Freitext mit festem Muster; eine Notiz mit `, ` kann die Hervorhebung bei Mehrfachauswahl nicht stören, weil das erste Stück mit dem Label beginnt.
- [ ] Doku: `docs/glossary.md` neuer Eintrag **Notiz (Rückfrage)**: „Kurzer Text zu einer Option einer Rückfrage; geht als `<Option> (Notiz: <Text>)` an den Agenten (ADR 045).“ `docs/code-map.md` Zeile „Chat“ um „Notiz pro Option (ADR 045)“ ergänzen, falls die Zeile ADRs aufzählt.
- [ ] Selbstprüfung in `pnpm tauri dev` mit einer echten Rückfrage (Agenten bitten, per AskUserQuestion zwei Fragen mit je drei Optionen zu stellen, eine davon Mehrfachauswahl): AK 1–8 durchgehen; der Agent wiederholt in seiner Antwort die Notiz. Screenshot des Kastens mit offenem Notizfeld und des beantworteten Kastens; Pfad (Scratchpad genügt) unter „Report-Back“ eintragen.
- [ ] Plan-Abschluss: Bottom-Sektionen, Archiv nach `docs/archive/2026-10/`, STATE.md, Versions-Tag.
- [ ] `pnpm check` grün, Commit `feat(chat): Notiz pro Option in Rückfragen`.

## Report-Back

## Wackelstellen

- 🟡 `autoFocus` in einem virtualisierten Verlauf: rendert der Virtualisierer den Kasten neu (Höhe ändert sich durch das Feld), kann der Fokus verloren gehen. Prüfen: Feld öffnen und sofort tippen; geht der Fokus verloren, statt `autoFocus` einen `ref` mit `focus()` im Klick-Handler nutzen und in FINDINGS festhalten.
- 🟡 Der Agent sieht die Notiz nur als Teil des Antworttexts. Prüfen über die Selbstprüfung (Agent wiederholt die Notiz); wiederholt er sie nicht, ist Entscheidung (a) falsch und der Plan geht zurück.

## Summary

## Files touched

## Commits

## Deviations from plan

## Follow-ups
