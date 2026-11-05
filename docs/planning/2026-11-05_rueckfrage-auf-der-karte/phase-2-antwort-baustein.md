# Phase 2 — Antworten auf Rückfragen als gemeinsamer Baustein

Rating **standard** · Status: pending

Reiner Umbau ohne sichtbare Änderung im Chat, bis auf eine: ein halber Entwurf überlebt den Wechsel der Session oder Ansicht.

## Kontext

- [README.md](README.md): Entscheidungen „Ein Antwortweg, zwei Orte“, „Nach dem Senden“, „Fehler“, „Tasten“, „Satz …“; Kontrakt (TypeScript-Teil).
- `src/features/chat/ChatTimeline.tsx` — heute Sitz der Logik: Zustände `drafts` und `sendingIds`, Funktionen `draftFor`, `submit`, `pick`, `storeDraft`, Tastenbehandlung `handleDigit` (über `useEffectEvent`) samt `window`-Abo, Konstanten `DIGIT_KEYS`, `ENTER_KEY`, Hilfen `isTextTarget`, `isControlTarget`, `isLinkTarget`, `findOldestOpenQuestion`; der `case 'question'` in `renderEntry`. Der Kommentar über `expandedKeys` erklärt, warum Entwürfe nicht im Block leben: ein Block verlässt beim Scrollen das DOM (virtualisierte Liste). Das gilt weiter — der Store erfüllt es ebenso.
- `src/features/chat/questionDraft.ts` — `QuestionEntry`, `QuestionDraft`, `emptyDraft`, `pickOption`, `draftAnswer`, `isComplete`, `needsSubmit`.
- `src/features/chat/QuestionBlock.tsx` — `renderPermission` (Fuß „Oder schreib unten eine eigene Antwort.“ bzw. „Antwort: …“), `renderFooter` (gleicher Satz, wenn kein „Antworten“ nötig ist).
- `src/stores/chat.ts` (Muster für Setzen/Entfernen), `src/stores/sessionErrors.ts`, `src/lib/chat.ts` (`answerQuestion`), `src/lib/errors.ts` (`commandErrorText`).
- `src/features/implementation/ImplementationView.tsx` — eigene Fassung von `isTextTarget` (mit `select`).
- [docs/conventions/react.md](../../conventions/react.md) (Reihenfolge der Hooks, Zustand nur flüchtig), [typescript.md](../../conventions/typescript.md).
- Zustand v5 (`package.json`): ein Selektor, der je Aufruf ein neues Objekt liefert, löst eine Render-Endlosschleife aus.

## Abnahmekriterien

1. Im Chat unverändert: Ziffern wählen in der aufgeklappten Frage der ältesten offenen Rückfrage, Enter sendet, wenn vollständig (nicht mit Shift, nicht auf Knopf/Link, nicht in Textfeldern); eine einzelne Frage in Einfachauswahl sendet beim Klick; Notiz je Option, eigene Antwort je Frage, Rechte-Abfragen; während des Sendens gesperrt; Fehler ergibt „Antwort nicht gesendet: <Grund>“ in der Fehlerzeile der Session, Erfolg löscht die Zeile; der Satz „Oder schreib unten eine eigene Antwort.“ steht wie bisher.
2. Ein angefangener Entwurf (gewählte Option einer Mehrfachfrage, Notiz, eigene Antwort) ist nach Wechsel in eine andere Session und zurück noch da; nach erfolgreichem Senden ist er aus dem Store entfernt.
3. `isTextTarget` und `isControlTarget` gibt es nur noch in `src/lib/keyTargets.ts`; Pfeiltasten in der Umsetzung blättern wie bisher.
4. Kein Selektor auf `useChatStore` liefert einen Rückfallwert (`?? {}`) im Selektor selbst.
5. `pnpm check` grün.

## Checkliste

- [ ] `src/lib/keyTargets.ts` anlegen: `isTextTarget` mit dem Selektor `'input, textarea, select, [contenteditable]:not([contenteditable="false"])'` (Vereinigung beider bisherigen Fassungen) und `isControlTarget` mit `'button, a, [role="button"]'`, Code 1:1 aus `ChatTimeline.tsx`. Die lokalen Fassungen in `ChatTimeline.tsx` (`isTextTarget`, `isControlTarget`) und `ImplementationView.tsx` (`isTextTarget`) löschen und importieren; `isLinkTarget` bleibt in `ChatTimeline.tsx`.
- [ ] `src/stores/chat.ts`: `questionDrafts`, `setQuestionDraft`, `clearQuestionDraft` nach Kontrakt, Doc-Kommentar „Halbe Antworten auf Rückfragen je Session und Anfrage-ID; Chat und Karte der Umsetzung teilen sie.“ `clearQuestionDraft` entfernt den Eintrag und die Session, wenn danach leer (Muster `clearDraft`). `import type { QuestionDraft } from '@/features/chat/questionDraft';` (Vorbild für einen Typ-Import aus `features`: `src/stores/settingsNav.ts`).
- [ ] `src/features/chat/useQuestionAnswer.ts` anlegen, Kontrakt `QuestionAnswering`/`useQuestionAnswer(sessionId)`:
  - `const drafts = useChatStore((state) => state.questionDrafts[sessionId]);` — Rückfall erst außerhalb: `drafts?.[entry.requestId] ?? emptyDraft(entry)` in `draftFor`.
  - `sending` und `sent` als `useState<ReadonlySet<string>>` (Anfrage-IDs); `isSending(entry)` = in einer der beiden. `sent` hält Anfragen, die hier erfolgreich beantwortet wurden, bis die Komponente verschwindet — so bleibt der Kasten gesperrt, bis die Ansicht die Anfrage nicht mehr meldet.
  - `submit`: Ablauf 1:1 aus `ChatTimeline.submit` (in `sending` aufnehmen, `answerQuestion(sessionId, requestId, answer)`, Erfolg: `clearSessionError(sessionId)`, Fehler: `console.error('Rückfrage nicht beantwortbar', reason)` und `reportSessionError(sessionId, \`Antwort nicht gesendet: ${commandErrorText(reason)}\`)`, `finally` aus `sending` entfernen); zusätzlich bei Erfolg in `sent` aufnehmen und `clearQuestionDraft(sessionId, requestId)`.
  - `pick` 1:1 aus `ChatTimeline.pick`, `storeDraft` = `setQuestionDraft(sessionId, entry.requestId, draft)`.
- [ ] `src/features/chat/useQuestionKeys.ts` anlegen: `useQuestionKeys(entry, answering)` übernimmt `handleDigit` und das `window`-Abo 1:1 aus `ChatTimeline.tsx` (`useEffectEvent`, `useEffect` mit leerem Abhängigkeits-Array), samt `DIGIT_KEYS` und `ENTER_KEY`; statt `oldestOpenQuestion` der Parameter `entry`, statt der lokalen Funktionen `answering.draftFor`/`isSending`/`submit`/`pick`. Doc-Kommentar „Ziffern wählen in der aufgeklappten Frage, Enter sendet — für die eine offene Rückfrage, die die Ansicht gerade bedient.“
- [ ] `ChatTimeline.tsx`: Zustände `drafts`, `sendingIds` und Funktionen `draftFor`, `submit`, `pick`, `storeDraft`, `handleDigit` samt Abo und die beiden Konstanten entfernen; `const answering: QuestionAnswering = useQuestionAnswer(session.id);` bei den eigenen Hooks, `useQuestionKeys(oldestOpenQuestion, answering);` direkt vor dem ersten `useEffect`. Der `case 'question'` reicht `answering.draftFor(entry)`, `answering.isSending(entry)`, `answering.pick(entry, …)`, `answering.submit(entry, …)`, `answering.storeDraft(entry, …)` und `showsComposerHint` durch. Den Kommentar über `expandedKeys` anpassen: „Auf-/Zugeklappt lebt hier, nicht im Block: ein Block verlässt beim Scrollen das DOM. Halbe Antworten liegen aus demselben Grund im Chat-Store (`questionDrafts`).“ Unbenutzte Importe entfernen (ESLint meldet sie).
- [ ] `QuestionBlock.tsx`: Pflicht-Prop `showsComposerHint: boolean` mit Kommentar „Der Satz „Oder schreib unten eine eigene Antwort.“ steht nur, wo darunter eine Eingabeleiste ist.“ In `renderPermission` den Fuß bei unbeantworteter Anfrage nur mit `showsComposerHint` rendern (beantwortet: „Antwort: …“ wie bisher); in `renderFooter` den Zweig ohne „Antworten“ zu `showsComposerHint ? <div …>…</div> : null`, Rückgabetyp `ReactElement | null`.
- [ ] Doc: noch nichts (Phase 3).
- [ ] `pnpm check` grün, Commit `refactor(chat): Antworten auf Rückfragen als gemeinsamer Baustein`.

## Report-Back
