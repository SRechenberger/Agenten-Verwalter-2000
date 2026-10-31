# Rückfrage-Kasten: Ziehharmonika und eigene Ergänzungen

Ziel: Der Kasten „Claude wartet auf deine Entscheidung“ wird lesbar und sicher bedienbar. Mehrere Fragen stehen nicht mehr als Wand untereinander: Nur die aktive Frage ist aufgeklappt, beantwortete klappen zu einer Zeile mit ihrer Antwort zusammen (Ziehharmonika). Zu jeder Option lässt sich eine Notiz schreiben, und jede Frage hat eine eigene Freitext-Antwort. Nach dem Beantworten steht der Kasten kompakt im Verlauf. Rechte-Abfragen (Erlauben/Ablehnen) bleiben unverändert. Entwurf: [artifacts/entwurf-rueckfrage-kasten.html](artifacts/entwurf-rueckfrage-kasten.html), Variante „Ziehharmonika“, vom User am 2026-10-30 freigegeben.

Kontext für jeden Umsetzer: [AGENTS.md](../../../AGENTS.md), [docs/code-map.md](../../code-map.md) (Zeile „Chat“), [docs/glossary.md](../../glossary.md) (Rechte-Abfrage, Session-Status `waiting`), [docs/conventions/](../../conventions/) (react, typescript, tailwind, rust), [artifacts/README.md](artifacts/README.md). Fehlerklassen geprüft (Vault: TypeScript, React): keine einschlägig; für Rust, Tauri und den Virtualisierer gibt es keine Einträge.

| # | Phase | Rating | Status |
|---|---|---|---|
| 1 | Kasten neu: Kurzname aus dem Core, Ziehharmonika, Auswahl-Marker, aktive Frage, Senden mit Absicht, „Empfohlen“, kompakt beantwortet; ADR 045 | standard | complete |
| 2 | Eigene Ergänzungen: Notiz je Option, eigene Antwort je Frage, Antworttext; Doku-Abschluss | standard | complete |

**Rahmen:** strikt 1 → 2 (beide Phasen ändern dieselben Dateien, nichts läuft parallel), direkt auf `main`, ein Commit pro Phase, Scope `chat`. `pnpm check` grün vor jedem Commit, `cargo` im PATH, keine neuen Abhängigkeiten, Bindings mitcommitten. Keine Migration: die Rückfragen liegen als JSON in `chat_entries`, das neue Feld bekommt `#[serde(default)]`. Keine automatisierten Tests, keine Smoke-Checkliste; jede Phase endet mit einer Selbstprüfung in `pnpm tauri dev` samt Screenshot. Planabschluss mit Versions-Tag (Minor-Version). Schätzung: Phase 1 rund 3 Stunden, Phase 2 rund 2 Stunden.

## Festgelegte Entscheidungen

- **Geltung:** Alles Neue gilt nur für Rückfragen (`questionKind === 'askUser'`). Rechte-Abfragen (`'permission'`) behalten Aufbau, Texte und Verhalten von heute (Klick sendet sofort, keine Notiz, keine eigene Antwort).
- **Kurzname:** Claude schickt je Frage `header` (höchstens 12 Zeichen, z. B. „Einstieg“). Der Core liest ihn als `Question.header` ein; fehlt er (Rechte-Abfrage, ältere Verläufe), zeigt die Oberfläche „Frage <n>“.
- **Ziehharmonika:** Bei zwei und mehr Fragen ist genau eine Frage aufgeklappt, die **aktive**. Jede andere ist eine Zeile: Haken (beantwortet) oder leerer Kreis, Kurzname, dann die Antwort bzw. die Frage einzeilig gekürzt, rechts „ändern“ bzw. „öffnen“. Klick auf die Zeile macht sie aktiv. Eine Rückfrage mit einer Frage hat keine Zeilen.
- **Aktive Frage:** Start ist Frage 1. Nach einer Wahl in einer Einfachauswahl (Klick oder Ziffer) wird die nächste unbeantwortete Frage dahinter aktiv, ringsum gesucht; gibt es keine, bleibt die Frage aktiv. Mehrfachauswahl, Notiz und eigene Antwort ändern die aktive Frage nicht.
- **Ziffern:** stehen nur an der aktiven Frage (rechts in der Option) und wirken nur dort. Die Kopfzeile der aktiven Frage zeigt „Tasten 1–<k>“.
- **Senden mit Absicht:** Ab zwei Fragen, mit einer Mehrfachauswahl oder (Phase 2) mit einem offenen Notiz- oder Eigene-Antwort-Feld sendet nur der Knopf „Antworten“ oder Enter. Sonst (eine Frage, Einfachauswahl) sendet der Klick wie heute sofort.
- **Enter:** sendet die älteste offene Rückfrage, wenn sie per Knopf gesendet wird und jede Frage beantwortet ist, und der Fokus weder in einem Textfeld noch auf einem Knopf oder Link liegt. In einem Notiz- oder Eigene-Antwort-Feld (Phase 2) sendet Enter unter derselben Bedingung.
- **„Empfohlen“:** Endet ein Options-Label auf `(Recommended)` oder `(Empfohlen)` (Regex `/\s*\((recommended|empfohlen)\)\s*$/i`), zeigt die Oberfläche den Namen ohne Zusatz plus Plakette „Empfohlen“. Wahl, Antworttext und Vergleich arbeiten mit dem Namen ohne Zusatz; `answeredLabels` entfernt den Zusatz auf beiden Seiten, damit auch ältere Antworten mit Zusatz markiert bleiben.
- **Kompakt beantwortet:** Eine beantwortete Rückfrage zeigt eine Zeile je Frage: Kurzname, Frage einzeilig gekürzt (`title` = ganze Frage), darunter der Antwortteil. Die Teile kommen aus `entry.answer.split(' · ')`; passt die Zahl nicht zur Zahl der Fragen, steht eine einzige Zeile mit der ganzen Antwort ohne Kurzname. Der Link „Ganze Rückfrage zeigen“ / „Fragen ausblenden“ klappt alle Fragen schreibgeschützt mit markierter Wahl auf (Zustand nur in der Komponente).
- **Lesebreite:** Kasten höchstens 760 px breit, Frage- und Erklärtexte höchstens 72 Zeichen (`max-width: 72ch`).
- **Notiz (Phase 2):** ✎-Knopf rechts neben jeder Option öffnet darunter ein einzeiliges Feld. Tippen wählt die Option (Einfachauswahl ersetzt, Mehrfachauswahl ergänzt), Leeren wählt nichts ab. Erneuter Klick auf ✎ oder Escape schließt und verwirft. Nur Notizen gewählter Optionen werden gesendet.
- **Eigene Antwort (Phase 2):** Letzte Zeile jeder Frage „Eigene Antwort …“ öffnet ein Feld plus Link „Verwerfen“. Nicht leerer Text zählt als Antwort der Frage: in der Einfachauswahl ersetzt er die Wahl, in der Mehrfachauswahl kommt er dazu. Wählt man danach in einer Einfachauswahl eine Option, schließt das Feld und der Text ist verworfen. Escape oder „Verwerfen“ schließt und verwirft. Das Eingabefeld unten im Chat beantwortet die Rückfrage weiter als Ganzes.
- **Antworttext (Phase 2):** je Frage: gewählte Optionen in Wahlreihenfolge, mit Notiz als `<Label> (Notiz: <Text>)`, danach die eigene Antwort, alles mit `, ` verbunden. Notiz und eigene Antwort werden getrimmt, ` · ` wird zu ` - ` (der Trenner zwischen Fragen muss eindeutig bleiben). Der Core bleibt dafür unverändert: `QuestionAnswer::Options { answers }` setzt den Text in `answers[<Frage>]` und speichert `answers.join(" · ")` als Antwort (`answer_line` in `src-tauri/src/sessions/registry.rs`).

## Kontrakt

```rust
// src-tauri/src/agents/event.rs
pub struct Question {
    pub question: String,
    /// Kurzname der Frage aus Claudes AskUserQuestion (`header`); leer, wenn keiner kam (ADR 045).
    #[serde(default)]
    pub header: String,
    pub options: Vec<QuestionOption>,
    pub multi_select: bool,
}
```

```ts
// src/features/chat/questionDraft.ts
export interface QuestionDraft {
  /** Gewählte Options-Namen (ohne „Empfohlen“-Zusatz) je Frage, in Wahlreihenfolge. */
  readonly labels: readonly (readonly string[])[];
  /** Index der aufgeklappten Frage; Ziffern wirken hier. */
  readonly active: number;
  /** Phase 2 — je Frage: Name → Notiz. Ein vorhandener Schlüssel heißt „Feld offen“, auch mit leerem Text. */
  readonly notes: readonly Readonly<Record<string, string>>[];
  /** Phase 2 — je Frage: eigene Antwort, `null` = Feld zu. */
  readonly custom: readonly (string | null)[];
}
```

`QuestionBlock` bekommt neben `onPick` und `onSubmit` genau eine weitere Rückgabe: `onDraft(next: QuestionDraft)`. Alle Änderungen außer Wahl und Senden (aktive Frage, Notiz, eigene Antwort) berechnet `QuestionBlock` mit den reinen Funktionen aus `questionDraft.ts` und reicht den neuen Entwurf über `onDraft` hoch; `ChatTimeline` legt ihn in `drafts` ab wie heute in `pick`.

## Phase 1 — Kasten neu

Lesen: Entwurf (Variante „Ziehharmonika“, Zustände „Offen“ und „Beantwortet“), `src/features/chat/questionDraft.ts` (ganz), `src/features/chat/QuestionBlock.tsx` + `QuestionBlock.css`, `src/features/chat/ChatTimeline.tsx` (`handleDigit`, `draftFor`, `submit`, `pick`, Fall `'question'` in `renderEntry`, `isTextTarget`, `virtualizer.measureElement`), `src-tauri/src/agents/event.rs` (`Question`), `src-tauri/src/agents/claude/translate.rs` (`ask_user_questions`, `permission_question`, `string_field`), `src-tauri/examples/gen-bindings.rs`, `src/styles/theme.css` (Tokens), `docs/decisions/043-unveraenderte-zeilen.md` (ADR-Format).

AK:
1. Eine offene Rückfrage mit zwei und mehr Fragen zeigt genau eine Frage aufgeklappt (anfangs Frage 1), jede andere als Zeile mit Kurzname und Frage bzw. Antwort. Klick auf eine Zeile klappt sie auf und die bisher aktive zu.
2. Kopf jeder aufgeklappten Frage: Plakette mit Claudes Kurznamen (ohne Kurzname „Frage <n>“), „Frage <n> von <m>“, bei Mehrfachauswahl „· mehrere möglich“, an der aktiven Frage rechts „Tasten 1–<k>“.
3. Einfachauswahl zeigt Kreise, Mehrfachauswahl Kästchen; eine gewählte Option hat gefüllten Marker, Rahmen in Akzentfarbe und leicht getönten Hintergrund.
4. Ziffern stehen nur in den Optionen der aktiven Frage und wählen dort. Nach einer Einfachauswahl klappt die Frage zu und die nächste unbeantwortete auf.
5. Ab zwei Fragen oder mit Mehrfachauswahl sendet nur „Antworten“ oder Enter (Fokus nicht in Textfeld, Knopf oder Link). Der Fuß zeigt je Frage einen Punkt (gefüllt = beantwortet), „<n> von <m> beantwortet“ und rechts „Ziffern wählen in der markierten Frage“ bzw. „Enter sendet“. Eine Rückfrage mit einer Frage in Einfachauswahl sendet beim Klick wie heute.
6. Ein Label mit „(Recommended)“ oder „(Empfohlen)“ am Ende zeigt den Namen ohne Zusatz plus Plakette „Empfohlen“; die Antwort enthält den Namen ohne Zusatz.
7. Der Kasten ist höchstens 760 px breit; Frage- und Erklärtexte brechen spätestens nach 72 Zeichen um.
8. Eine beantwortete Rückfrage zeigt eine Zeile je Frage (Kurzname, gekürzte Frage, Antwort). „Ganze Rückfrage zeigen“ klappt alle Fragen schreibgeschützt auf, die gewählten Optionen sind markiert. Ein älterer Verlauf ohne Kurzname zeigt „Frage <n>“.
9. Rechte-Abfragen sehen aus und verhalten sich wie vor der Phase.
10. Freitext im Eingabefeld beantwortet eine offene Rückfrage wie vor der Phase.

- [x] Core: `Question.header` nach Kontrakt; `ask_user_questions` setzt `header: string_field(question, "header")`, `permission_question` setzt `header: String::new()`. `pnpm bindings` (erzeugt `src/lib/bindings/Question.ts` mit `header: string`).
- [x] `questionDraft.ts`: `QuestionDraft` nach Kontrakt, in dieser Phase mit `notes` je Frage `{}` und `custom` je Frage `null` (noch ohne Bedienung). Neue bzw. geänderte Funktionen: `emptyDraft(entry)`; `optionName(label: string): { name: string; recommended: boolean }` mit der Regex aus den Entscheidungen; `questionHeader(question: Question, index: number): string`; `isAnswered(draft, questionIndex): boolean` (Phase 1: `labels[i].length > 0`); `isComplete(entry, draft)`; `needsSubmit(entry, draft)` = mehr als eine Frage oder eine Mehrfachauswahl; `setActive(draft, questionIndex): QuestionDraft`; `nextActive(entry, draft, from): number` (nächste unbeantwortete nach `from`, ringsum, sonst `from`); `pickOption` arbeitet mit `optionName(...).name`, setzt bei Einfachauswahl `active = nextActive(...)` und sendet nur, wenn `!needsSubmit(entry, next) && isComplete(entry, next)`; `draftAnswer(draft)` wie heute aus `labels`; `answerParts(entry): readonly string[] | null` (Split an `ANSWER_SEPARATOR`, `null` bei falscher Anzahl); `answeredLabels` vergleicht nach `optionName(...).name` auf beiden Seiten. `digitQuestionIndex` entfällt: Ziffern gehen an `draft.active` (gleiche Idee wie bisher, „erste offene Frage“, nur ausdrücklich gespeichert).
- [x] Komponenten-Schnitt, Styles alle in `QuestionBlock.css` (BEM `question-block__*`): `QuestionBlock.tsx` = Kasten mit drei Zweigen: Rechte-Abfrage (heutiger Aufbau unverändert, in eine Funktion `renderPermission` gezogen), offen, beantwortet; Kopf, Ziehharmonika-Zeilen (`question-block__row`, `__row-icon`, `__row-text`, `__row-action`) und Fuß (`__progress`, `__dots`, `__dot`/`__dot--done`, `__submit-hint`, `__submit`). Neu `QuestionItem.tsx` = eine aufgeklappte Frage (`__question`/`__question--active`, `__question-head`, `__chip`, `__meta`, `__keys`, Optionen mit `__marker--radio`/`__marker--check`, `__badge`, `__key` rechts als `<kbd>`), Prop `isLocked` für die schreibgeschützte Ansicht. Neu `QuestionAnswered.tsx` = kompakte Ansicht (`__answers`, `__answer`, `__answer-question`, `__answer-text`, `__toggle-full`).
- [x] Barrierefreiheit: Optionen einer Einfachauswahl in einem Container mit `role="radiogroup"`, je Option `role="radio"` + `aria-checked`; Mehrfachauswahl `role="group"`, Optionen `role="checkbox"` + `aria-checked`; Ziehharmonika-Zeile ist ein `<button>` mit `aria-label` „<Kurzname>: <Antwort bzw. Frage> — öffnen“.
- [x] `QuestionBlock.css`: Maße und Farben wie im Entwurf, nur Tokens aus `theme.css`: Haken der Zeile `--color-status-completed`, Plakette der aktiven Frage `--color-accent-subtle`/`--color-accent-text`, Plakette „Empfohlen“ Rahmen und Schrift `--color-status-completed`, gefüllter Punkt `--color-accent`.
- [x] `ChatTimeline.tsx`: `onDraft` an `QuestionBlock` (schreibt in `drafts` wie `pick`); `handleDigit` nimmt `draft.active`; Enter nach Entscheidung im selben Handler (Ausschluss per `isTextTarget` und `target.closest('button, a, [role="button"]')`), sendet über `submit(entry, draftAnswer(draft))`.
- [x] `docs/decisions/045-rueckfrage-kasten.md` im Format von ADR 043. Kontext: Rückfragen mit mehreren Fragen sind eine Wand, Freitext gilt nur für alle Fragen gemeinsam. Optionen Darstellung: (a) alle Fragen offen als Liste — verworfen, bleibt bei vier Fragen lang; (b) Ziehharmonika — gewählt (Entwurf vom 2026-10-30); (c) Reiter — verworfen, versteckt die anderen Antworten. Optionen Transport der Ergänzungen: (d) im Antworttext je Frage — gewählt, erreicht den Agenten sicher und steht im Verlauf, ohne Core-Umbau; (e) eigenes Feld und Weitergabe als `annotations` an Claude Code — verworfen, ob Claude Code diese Notizen dem Modell zeigt, ist nicht belegt, und der gespeicherte Antworttext verlöre sie. Konsequenzen: `Question.header` mit `serde(default)`, keine Migration; der Antworttext ist für den Agenten Freitext mit festem Muster.
- [x] Doc: `docs/code-map.md` Zeile „Chat“ um `QuestionItem.tsx`, `QuestionAnswered.tsx` und ADR 045 ergänzen.
- [ ] Selbstprüfung in `pnpm tauri dev`: den Agenten bitten, per AskUserQuestion vier Fragen mit Kurznamen zu stellen (drei Einfachauswahl, eine Mehrfachauswahl, eine Option mit „(Recommended)“); AK 1–10 durchgehen; eine Rechte-Abfrage auslösen (AK 9). Screenshots offen und beantwortet, Pfade unter „Report-Back“.
- [x] `pnpm check` grün, Commit `feat(chat): Rückfragen als Ziehharmonika mit Kurznamen`.

## Phase 2 — Eigene Ergänzungen

Lesen: Entwurf (✎ und „Eigene Antwort“ ausprobieren), Stand nach Phase 1 von `questionDraft.ts`, `QuestionBlock.tsx`, `QuestionItem.tsx`, `QuestionBlock.css`, `ChatTimeline.tsx` (`handleDigit`, `isTextTarget`), `docs/decisions/045-rueckfrage-kasten.md`, `src-tauri/src/sessions/registry.rs` (`answer_line`, `answer_oldest_with_text`, nur lesen).

AK:
1. Jede Option einer offenen Rückfrage trägt rechts ✎ mit `aria-label` „Notiz zu „<Name>““; Rechte-Abfragen und beantwortete Rückfragen haben keinen.
2. Klick auf ✎ öffnet unter genau dieser Option ein Feld mit Fokus (Platzhalter „Notiz für Claude …“, höchstens 500 Zeichen); erneuter Klick schließt es und verwirft den Text; ✎ ändert die Wahl nicht.
3. Tippen in die Notiz wählt die Option (Einfachauswahl ersetzt, Mehrfachauswahl ergänzt).
4. Die letzte Zeile jeder Frage „Eigene Antwort …“ öffnet ein Feld mit Fokus (Platzhalter „Eigene Antwort auf diese Frage …“, höchstens 2000 Zeichen) und den Link „Verwerfen“. Text darin beantwortet die Frage: in der Einfachauswahl ersetzt er die Wahl, in der Mehrfachauswahl kommt er dazu. Die Wahl einer Option in einer Einfachauswahl schließt das Feld und verwirft den Text.
5. Mit einem offenen Notiz- oder Eigene-Antwort-Feld sendet kein Klick, auch bei einer einzelnen Frage; „Antworten“ erscheint. Enter im Feld sendet, sobald jede Frage beantwortet ist. Escape schließt das Feld und verwirft seinen Text.
6. Zifferntasten in einem Feld tippen Ziffern und wählen nichts.
7. Gesendet wird je Frage der Antworttext nach Entscheidung; ` · ` in Notiz oder eigener Antwort kommt als ` - ` an. Belegt in der kompakten Ansicht nach dem Senden.
8. Eine zugeklappte Zeile zeigt denselben Antworttext.
9. Nach dem Senden sind in „Ganze Rückfrage zeigen“ die gewählten Optionen markiert, auch die mit Notiz.
10. In der Selbstprüfung gibt der Agent Notiz und eigene Antwort in seiner nächsten Nachricht sinngemäß wieder.

- [x] `questionDraft.ts`: Konstanten `NOTE_PREFIX = ' (Notiz: '`, `NOTE_SUFFIX = ')'`, `SEPARATOR_REPLACEMENT = ' - '`. Neue Funktionen `toggleNote(draft, questionIndex, name)`, `editNote(entry, draft, questionIndex, name, text)`, `toggleCustom(draft, questionIndex)`, `editCustom(entry, draft, questionIndex, text)`, `hasOpenField(draft): boolean`, `answerText(labels, notes, custom): string`. `isAnswered` zählt eine nicht leere eigene Antwort mit; `needsSubmit` zusätzlich `hasOpenField(draft)`; `pickOption` schließt bei Einfachauswahl `custom[i]` (`null`); `draftAnswer` baut je Frage `answerText(...)`; `answeredLabels` erkennt ein Stück, das mit `<Name> (Notiz: ` beginnt.
- [x] `QuestionItem.tsx`: Optionszeile `<div className="question-block__option-row">` mit dem Options-Knopf und daneben dem Knopf `question-block__note-toggle` (✎, `aria-expanded`), nicht verschachtelt. Bei offenem Feld darunter `<input type="text" className="question-block__note">`; Fokus über einen `ref` und `focus()` direkt nach dem Öffnen (kein `autoFocus`, siehe Wackelstellen). Letzte Zeile `question-block__custom-open` („+ Eigene Antwort …“, gestrichelter Rahmen) bzw. `question-block__custom` mit `__custom-input` und Link `__custom-discard`. Enter und Escape im Feld nach Entscheidung (Enter ruft `onSubmit(draftAnswer(draft))`, wenn `isComplete`).
- [x] `QuestionBlock.tsx`: Ziehharmonika-Zeile zeigt `answerText(...)` der Frage.
- [x] `QuestionBlock.css`: `__option-row` (Flex, Abstand `--space-sm`, Options-Knopf `flex: 1`), `__note-toggle` (34 px breit, Rahmen und Hintergrund wie `__option`, gedimmt bis Hover oder Fokus der Zeile, offen mit Rahmen `--color-accent`), `__note` und `__custom-input` (Höhe 30 px, Schrift `--font-size-xs`, Fokus-Umriss wie `__submit`), `__custom-open` (gestrichelter Rahmen, Schrift `--color-fg-muted`).
- [x] Doc: `docs/glossary.md` neue Einträge **Notiz (Rückfrage)**: „Kurzer Text zu einer Option einer Rückfrage; geht als `<Option> (Notiz: <Text>)` an den Agenten (ADR 045).“ und **Eigene Antwort (Rückfrage)**: „Freitext zu genau einer Frage einer Rückfrage; ersetzt bei Einfachauswahl die Wahl (ADR 045).“ Neuer Eintrag **Rückfrage** (das Glossar hat bisher keinen eigenen) vor **Rechte-Abfrage**: „Frage des Agenten mit Optionen (AskUserQuestion); mehrere Fragen erscheinen als Ziehharmonika, nur die aktive ist aufgeklappt (ADR 045).“
- [ ] (nicht gefahren, siehe Report-Back) Selbstprüfung in `pnpm tauri dev`: Rückfrage wie in Phase 1, eine Notiz an einer Einfachauswahl, eine eigene Antwort an einer anderen Frage, eine Notiz mit ` · ` darin; AK 1–10 durchgehen. Screenshots mit offenem Notizfeld und beantwortet, Pfade unter „Report-Back“.
- [x] Plan-Abschluss: Bottom-Sektionen, Archiv nach `docs/archive/2026-10/`, STATE.md, Versions-Tag.
- [x] `pnpm check` grün, Commit `feat(chat): Notiz je Option und eigene Antwort je Frage`.

## Report-Back

- Phase 1: Selbstprüfung in `pnpm tauri dev` hat die umsetzende Session nicht gefahren (keine Bedienung der Oberfläche möglich); Commit vom User freigegeben. Ob der Verlauf beim Zuklappen springt (Wackelstelle 1), ist damit offen.

- Archiviert und getaggt (`v0.39.0`) auf Zuruf des Users ohne Selbstprüfung durch die umsetzende Session; ob AK 10 trägt, zeigt der laufende Betrieb.
- Phase 2: Umsetzung und `pnpm check` grün (inkl. Rust); die Entwurfslogik (Notiz, eigene Antwort, Antworttext mit ` · `, `answeredLabels`) ist per Wegwerf-Skript gegen die AK 3, 4, 7, 9 geprüft. Die Selbstprüfung in `pnpm tauri dev` (AK 1, 2, 5, 6, 8, 10 samt Fokus nach ✎ und Sprung des Verlaufs) steht aus: die umsetzende Session kann die Oberfläche nicht bedienen. Plan-Abschluss (Archiv, Tag) wartet auf diese Selbstprüfung, vor allem auf AK 10 — gibt der Agent Notiz und eigene Antwort nicht wieder, ist Entscheidung (d) falsch.

## Wackelstellen

- 🟡 Der Verlauf ist virtualisiert (`virtualizer.measureElement`). Klappt eine Frage zu, ändert sich die Höhe des Kastens; der Verlauf darf dabei nicht springen. Prüfen in der Selbstprüfung von Phase 1: Kasten ganz unten, Frage 1 beantworten, Lage des Kastens beobachten. Springt er, in FINDINGS (neu anlegen) festhalten und die Höhe vor dem Zuklappen über `virtualizer.measure()` nachziehen.
- 🟡 Ein `autoFocus` kann in einem virtualisierten Verlauf beim Neumessen verloren gehen, deshalb `ref` + `focus()` im Klick-Handler. Prüfen: ✎ klicken und sofort tippen.
- 🟡 Der Agent sieht Notiz und eigene Antwort nur als Teil des Antworttexts. Prüfen über AK 10 in Phase 2; gibt er sie nicht wieder, ist Entscheidung (d) aus ADR 045 falsch und der Plan geht zurück.

## Nicht in diesem Plan

- Hinweis über dem Eingabefeld, wenn der Kasten aus dem Bild gescrollt ist (Entwurf: Baustein „Hinweis am Eingabefeld“).
- Claudes Vorschau je Option anzeigen (braucht weiteren Core-Umbau).

## Summary

Der Rückfrage-Kasten zeigt mehrere Fragen als Ziehharmonika (nur die aktive aufgeklappt, Kurznamen von Claude, „Empfohlen“-Plakette, kompakt im Verlauf) und nimmt zu jeder Option eine Notiz und zu jeder Frage eine eigene Antwort an; beides geht im Antworttext je Frage an den Agenten (ADR 045). Rechte-Abfragen unverändert.

## Files touched

`src/features/chat/` (`questionDraft.ts`, `QuestionBlock.tsx`/`.css`, `QuestionItem.tsx`, `QuestionAnswered.tsx`, `ChatTimeline.tsx`), `src-tauri/src/agents/event.rs` und `agents/claude/translate.rs` (`Question.header`), Bindings, `docs/decisions/045-rueckfrage-kasten.md`, `docs/glossary.md`, `docs/code-map.md`.

## Commits

`4c50713` Phase 1, `35d0773` Phase 2, dazu der Archiv- und der Release-Commit `v0.39.0`.

## Deviations from plan

- Phase 1: Die Optionen einer Rückfrage heißen `question-block__choice` (mit `__choice-text`, `__choice-name`), die Ziffer `question-block__kbd` statt `__option`/`__key` — die alten Klassen tragen weiter den unveränderten Aufbau der Rechte-Abfrage. `needsSubmit` nimmt in Phase 1 nur `entry`; Phase 2 ergänzt `draft` für `hasOpenField`. Der Antwortteil je Frage kommt aus `draftAnswerPart(draft, i)`; Phase 2 lässt ihn `answerText(...)` liefern.
- Phase 2: `QuestionItem` bekommt Notiz und eigene Antwort gebündelt als optionales Prop `extras` (ohne `extras` reine Anzeige, so in `QuestionAnswered`); Enter im Feld läuft über `extras.onEnter`. Die Optionszeile heißt `question-block__option-row`, der Options-Knopf bleibt `question-block__choice`; `noteFor` ist exportiert und schützt vor Prototyp-Namen wie `constructor`.

## Follow-ups

- Selbstprüfung in `pnpm tauri dev` steht aus: Verlauf springt beim Zuklappen nicht (Wackelstelle 1), Fokus nach ✎ (Wackelstelle 2), AK 10 (Agent gibt Notiz und eigene Antwort wieder; sonst ist Entscheidung (d) aus ADR 045 falsch). Springt der Verlauf, FINDINGS anlegen und `virtualizer.measure()` vor dem Zuklappen nachziehen.
- Nicht in diesem Plan: Hinweis über dem Eingabefeld bei weggescrolltem Kasten; Claudes Vorschau je Option.
