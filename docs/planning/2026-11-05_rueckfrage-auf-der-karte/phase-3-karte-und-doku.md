# Phase 3 — Karte beantwortet Rückfragen; ADR 055, Doku

Rating **standard** · Status: pending

## Kontext

- [README.md](README.md): alle Entscheidungen (Quelle für ADR 055), Kontrakt, finale Abnahmekriterien.
- Ergebnis von Phase 1 und 2: `src/lib/bindings/OpenQuestion.ts`, `StationView.openQuestion`, `src/features/chat/useQuestionAnswer.ts`, `useQuestionKeys.ts`, `QuestionBlock` mit `showsComposerHint`.
- `src/features/implementation/StationCard.tsx` + `StationCard.css` (`renderQuestion`, `&__question`, gemeinsamer Selektor `&__phase, &__state, &__question, &__result`), `CardStack.tsx` (rendert `StationCard` für die Station im Fokus; die Kartei-Spalte ist `minmax(0, 1fr)` breit und rollt, `ImplementationView.css`).
- `src/app/SessionActionError.tsx` — Fehlerzeile einer Session mit Schließen-Knopf (Vorbild für einen Import aus `@/app/` in einem Feature: `src/features/projects/ProjectSessionCard.tsx`).
- Entwurf `docs/archive/2026-11/2026-10-30_umsetzung/artifacts/entwurf-umsetzung.html`, Abschnitt 1, große Karte im Zustand „fragt dich“: Frage, Antwortmöglichkeiten, „Eigene Antwort …“, darunter die Knöpfe. Abweichung, bewusst: statt der einfachen Knopfreihe des Entwurfs der Rückfrage-Kasten des Chats (ADR 045); die Faktenzeile des Entwurfs bleibt draußen (Nicht-Ziel).
- [ADR 045](../../decisions/045-rueckfrage-kasten.md), [ADR 048](../../decisions/048-umsetzung.md) (Format eines ADR), [docs/code-map.md](../../code-map.md), [docs/glossary.md](../../glossary.md), [docs/conventions/react.md](../../conventions/react.md), [tailwind.md](../../conventions/tailwind.md) (BEM je Komponente).

## Abnahmekriterien (Struktur der Karte)

1. Station `asking` mit offener Rückfrage: die große Karte zeigt von oben nach unten Kopf „Phase <key> · <Stufe>“ mit `#<n>`, Phasentitel, Zustand „fragt dich“, den Rückfrage-Kasten (`QuestionBlock`, Aussehen wie im Chat: Kopf „Claude wartet auf deine Entscheidung“, ab zwei Fragen Ziehharmonika mit „<n> Fragen“, Optionen mit Ziffer, Plakette „Empfohlen“, Notiz je Option, „+ Eigene Antwort …“, ab zwei Fragen oder bei Mehrfachauswahl der Fuß „<x> von <n> beantwortet“ mit „Antworten“), darunter die Fehlerzeile der Session (nur wenn eine da ist) und zuletzt die Knopfreihe „Chat öffnen“ · „Changes · <n> Dateien“.
2. Rechte-Abfrage: dieselbe Stelle zeigt Frage und Knöpfe der Rechte-Abfrage; ein Klick beantwortet sie.
3. Auf der Karte steht nirgends „Oder schreib unten eine eigene Antwort.“; im Chat weiter.
4. Ziffern 1–9 wählen in der aufgeklappten Frage, Enter sendet bei vollständiger Antwort — solange der Ablauf mit dieser Karte sichtbar ist; nicht in Textfeldern, Enter nicht auf Knöpfen; ↑/↓ blättern weiter durch die Stationen.
5. Nach dem Senden bleibt der Kasten gesperrt, bis die Ansicht nachzieht; dann zeigt die Karte die nächste offene Anfrage oder den neuen Zustand („läuft“).
6. Eine auf der Karte angefangene Notiz steht nach „Chat öffnen“ im Kasten des Chats und umgekehrt.
7. Scheitert das Senden, steht „Antwort nicht gesendet: <Grund>“ unter dem Kasten, schließbar per ×; dieselbe Zeile zeigt der Chat der Session.
8. Die Karte wird so hoch wie ihr Inhalt; die Kartei rollt; die Breite der Kartei bleibt.

## Checkliste

- [ ] `src/features/implementation/StationQuestion.tsx` anlegen, Props `{ sessionId: string; openQuestion: OpenQuestion }`, Doc-Kommentar „Rückfrage-Kasten auf der Karte einer Station; antwortet über denselben Baustein wie der Chat (ADR 055).“ Inhalt: `const entry: QuestionEntry = useMemo(() => ({ kind: 'question', seq: openQuestion.seq, requestId: openQuestion.requestId, questionKind: openQuestion.questionKind, questions: openQuestion.questions, answer: null }), [openQuestion]);`, `const answering: QuestionAnswering = useQuestionAnswer(sessionId);`, `useQuestionKeys(entry, answering);`; Rückgabe als Fragment: `QuestionBlock` mit `entry`, `draft={answering.draftFor(entry)}`, `isSending={answering.isSending(entry)}`, `onPick`/`onSubmit`/`onDraft` über `answering`, `showsComposerHint={false}`, danach `<SessionActionError sessionId={sessionId} />`.
- [ ] `StationCard.tsx`: `renderQuestion` gibt `null` zurück bei `station.state !== 'asking' || station.openQuestion === null || station.sessionId === null`, sonst `<StationQuestion key={station.openQuestion.requestId} sessionId={station.sessionId} openQuestion={station.openQuestion} />` (der `key` setzt Sende-Sperre und Tasten je Anfrage neu auf; Entwürfe liegen im Store und bleiben). Doc-Kommentar der Komponente: „Die große Karte der Station im Fokus; eine offene Rückfrage wird hier beantwortet wie im Chat (ADR 055).“
- [ ] `StationCard.css`: Regel `&__question` und `&__question` im gemeinsamen Selektor entfernen.
- [ ] `docs/decisions/055-rueckfrage-auf-der-karte.md` im Format von ADR 045/048: Kontext (Karte zeigte nur Text und schickte in den Chat; Abweichung vom Entwurf in Phase 2 des Plans Umsetzung), Optionen (Ansicht trägt die Anfrage — gewählt; Karte lädt den Verlauf — verworfen mit Grund aus der README; eigene einfache Knopfreihe wie im Entwurf — verworfen, dritte Antwort-Oberfläche), Entscheidung (die Punkte der README), Konsequenzen (Entwürfe überleben den Wechsel der Ansicht; `implementation_get` liefert dem Agenten im Zustand `asking` die ganze Anfrage statt nur ihres Texts; nach jeder Antwort geht eine Zusammenfassung raus).
- [ ] `docs/code-map.md`: Zeile „Chat“ um `useQuestionAnswer` (Entwürfe im Chat-Store `questionDrafts`, Senden, Fehlerzeile) und `useQuestionKeys` (Ziffern und Enter) ergänzen, `src/lib/keyTargets.ts` nennen; Zeile „Umsetzung“ um `StationQuestion` (Rückfrage-Kasten auf der Karte, ADR 055) und `OpenQuestion` in `implementation/model.rs` ergänzen. Kopfzeile „Stand:“ nachziehen.
- [ ] `docs/glossary.md`: Eintrag „Rückfrage“ um „beantwortbar im Chat und auf der Karte ihrer Station ([ADR 055](decisions/055-rueckfrage-auf-der-karte.md))“ ergänzen; Eintrag „Waiting“ sagt „wartet auf eine Antwort im Chat“ → „im Chat oder auf der Karte der Station“.
- [ ] `pnpm check` grün, Commit `feat(implementation): Rückfragen auf der Karte der Station beantworten`.
- [ ] Plan abschließen nach `mode-implementing`: archivieren, `STATE.md` nachziehen, Version und Tag nach [releases.md](../../conventions/releases.md).

## Report-Back
