import type { ChatEntry } from '@/lib/bindings/ChatEntry';
import type { Question } from '@/lib/bindings/Question';
import type { QuestionAnswer } from '@/lib/bindings/QuestionAnswer';
import type { QuestionOption } from '@/lib/bindings/QuestionOption';

export type QuestionEntry = Extract<ChatEntry, { kind: 'question' }>;

export interface QuestionDraft {
  /** Gewählte Options-Namen (ohne „Empfohlen“-Zusatz) je Frage, in Wahlreihenfolge. */
  readonly labels: readonly (readonly string[])[];
  /** Index der aufgeklappten Frage; Ziffern wirken hier. */
  readonly active: number;
  /** Je Frage: Name → Notiz. Ein vorhandener Schlüssel heißt „Feld offen“, auch mit leerem Text. */
  readonly notes: readonly Readonly<Record<string, string>>[];
  /** Je Frage: eigene Antwort, `null` = Feld zu. */
  readonly custom: readonly (string | null)[];
}

export type PickResult =
  { kind: 'draft'; draft: QuestionDraft } | { kind: 'answer'; answer: QuestionAnswer };

export interface OptionName {
  name: string;
  recommended: boolean;
}

const DENY_MESSAGE = 'Der Benutzer hat abgelehnt.';
const MULTI_SELECT_SEPARATOR = ', ';
/** Trennt die Antworten mehrerer Fragen im gespeicherten Antworttext (Gegenstück zu `ANSWER_SEPARATOR` im Core). */
const ANSWER_SEPARATOR = ' · ';
/** Ersatz für den Trenner in Notiz und eigener Antwort, damit er zwischen den Fragen eindeutig bleibt. */
const SEPARATOR_REPLACEMENT = ' - ';
const NOTE_PREFIX = ' (Notiz: ';
const NOTE_SUFFIX = ')';
const ALLOW_INDEX = 0;
/** Claude markiert eine empfohlene Option mit einem Zusatz am Ende des Labels. */
const RECOMMENDED_SUFFIX = /\s*\((recommended|empfohlen)\)\s*$/i;

export function emptyDraft(entry: QuestionEntry): QuestionDraft {
  return {
    labels: entry.questions.map((): readonly string[] => []),
    active: 0,
    notes: entry.questions.map((): Readonly<Record<string, string>> => ({})),
    custom: entry.questions.map((): string | null => null),
  };
}

export function optionName(label: string): OptionName {
  const name: string = label.replace(RECOMMENDED_SUFFIX, '');
  return { name, recommended: name !== label };
}

/** Claudes Kurzname der Frage; Rechte-Abfragen und ältere Verläufe haben keinen. */
export function questionHeader(question: Question, index: number): string {
  if (question.header !== '') {
    return question.header;
  }
  return `Frage ${String(index + 1)}`;
}

/** Getrimmter Text, in dem der Trenner zwischen den Fragen durch ` - ` ersetzt ist. */
function cleanText(text: string): string {
  return text.trim().split(ANSWER_SEPARATOR).join(SEPARATOR_REPLACEMENT);
}

/** Namen wie `constructor` dürfen keinen Treffer im Prototyp ergeben. */
export function noteFor(notes: Readonly<Record<string, string>>, name: string): string | undefined {
  return Object.prototype.hasOwnProperty.call(notes, name) ? notes[name] : undefined;
}

function hasCustomText(draft: QuestionDraft, questionIndex: number): boolean {
  return (draft.custom[questionIndex] ?? '').trim() !== '';
}

export function isAnswered(draft: QuestionDraft, questionIndex: number): boolean {
  return (draft.labels[questionIndex]?.length ?? 0) > 0 || hasCustomText(draft, questionIndex);
}

/** Ein offenes Notiz- oder Eigene-Antwort-Feld, auch ein leeres. */
export function hasOpenField(draft: QuestionDraft): boolean {
  return (
    draft.custom.some((text: string | null) => text !== null) ||
    draft.notes.some((notes: Readonly<Record<string, string>>) => Object.keys(notes).length > 0)
  );
}

export function isComplete(entry: QuestionEntry, draft: QuestionDraft): boolean {
  return (
    entry.questions.length > 0 &&
    entry.questions.every((_question: Question, index: number) => isAnswered(draft, index))
  );
}

/** Ab zwei Fragen, mit einer Mehrfachauswahl oder einem offenen Feld sendet erst „Antworten“ bzw. Enter, sonst der Klick. */
export function needsSubmit(entry: QuestionEntry, draft: QuestionDraft): boolean {
  return (
    entry.questions.length > 1 ||
    entry.questions.some((question: Question) => question.multiSelect) ||
    hasOpenField(draft)
  );
}

export function setActive(draft: QuestionDraft, questionIndex: number): QuestionDraft {
  return { ...draft, active: questionIndex };
}

/** Die nächste unbeantwortete Frage hinter `from`, ringsum gesucht; gibt es keine, bleibt es `from`. */
export function nextActive(entry: QuestionEntry, draft: QuestionDraft, from: number): number {
  const count: number = entry.questions.length;
  for (let step = 1; step < count; step += 1) {
    const candidate: number = (from + step) % count;
    if (!isAnswered(draft, candidate)) {
      return candidate;
    }
  }
  return from;
}

function replaceAt<T>(items: readonly T[], index: number, value: T): readonly T[] {
  return items.map((item: T, position: number) => (position === index ? value : item));
}

/** Öffnet das Notizfeld einer Option oder schließt es und verwirft den Text; die Wahl bleibt. */
export function toggleNote(
  draft: QuestionDraft,
  questionIndex: number,
  name: string,
): QuestionDraft {
  const current: Readonly<Record<string, string>> = draft.notes[questionIndex] ?? {};
  const next: Readonly<Record<string, string>> =
    noteFor(current, name) === undefined
      ? { ...current, [name]: '' }
      : Object.fromEntries(
          Object.entries(current).filter(([key]: [string, string]) => key !== name),
        );
  return { ...draft, notes: replaceAt(draft.notes, questionIndex, next) };
}

/** Schreibt die Notiz; nicht leerer Text wählt die Option (Einfachauswahl ersetzt, Mehrfachauswahl ergänzt). */
export function editNote(
  entry: QuestionEntry,
  draft: QuestionDraft,
  questionIndex: number,
  name: string,
  text: string,
): QuestionDraft {
  const notes: Readonly<Record<string, string>> = {
    ...(draft.notes[questionIndex] ?? {}),
    [name]: text,
  };
  const written: QuestionDraft = { ...draft, notes: replaceAt(draft.notes, questionIndex, notes) };
  const selected: readonly string[] = draft.labels[questionIndex] ?? [];
  if (text.trim() === '' || selected.includes(name)) {
    return written;
  }
  if (entry.questions[questionIndex]?.multiSelect === true) {
    return { ...written, labels: replaceAt(draft.labels, questionIndex, [...selected, name]) };
  }
  return {
    ...written,
    labels: replaceAt(draft.labels, questionIndex, [name]),
    custom: replaceAt(draft.custom, questionIndex, null),
  };
}

/** Öffnet das Feld „Eigene Antwort“ einer Frage oder schließt es und verwirft den Text. */
export function toggleCustom(draft: QuestionDraft, questionIndex: number): QuestionDraft {
  const isOpen: boolean = draft.custom[questionIndex] !== null;
  return { ...draft, custom: replaceAt(draft.custom, questionIndex, isOpen ? null : '') };
}

/** Schreibt die eigene Antwort; nicht leerer Text ersetzt in einer Einfachauswahl die Wahl. */
export function editCustom(
  entry: QuestionEntry,
  draft: QuestionDraft,
  questionIndex: number,
  text: string,
): QuestionDraft {
  const written: QuestionDraft = {
    ...draft,
    custom: replaceAt(draft.custom, questionIndex, text),
  };
  if (text.trim() === '' || entry.questions[questionIndex]?.multiSelect === true) {
    return written;
  }
  return { ...written, labels: replaceAt(draft.labels, questionIndex, []) };
}

/** Antworttext einer Frage: gewählte Optionen in Wahlreihenfolge, je mit Notiz, danach die eigene Antwort. */
export function answerText(
  labels: readonly string[],
  notes: Readonly<Record<string, string>>,
  custom: string | null,
): string {
  const pieces: string[] = labels.map((name: string) => {
    const note: string = cleanText(noteFor(notes, name) ?? '');
    return note === '' ? name : `${name}${NOTE_PREFIX}${note}${NOTE_SUFFIX}`;
  });
  const customText: string = cleanText(custom ?? '');
  if (customText !== '') {
    pieces.push(customText);
  }
  return pieces.join(MULTI_SELECT_SEPARATOR);
}

/** Antwortteil einer Frage, wie er gesendet und in der zugeklappten Zeile gezeigt wird. */
export function draftAnswerPart(draft: QuestionDraft, questionIndex: number): string {
  return answerText(
    draft.labels[questionIndex] ?? [],
    draft.notes[questionIndex] ?? {},
    draft.custom[questionIndex] ?? null,
  );
}

export function draftAnswer(draft: QuestionDraft): QuestionAnswer {
  return {
    kind: 'options',
    answers: draft.labels.map((_labels: readonly string[], index: number) =>
      draftAnswerPart(draft, index),
    ),
  };
}

/** Wählt eine Option. Eine Rechte-Abfrage ist mit dem ersten Klick beantwortet; eine Rückfrage sendet der Klick
 *  nur, wenn sie keinen Knopf braucht (`needsSubmit`) und danach jede Frage eine Antwort hat. */
export function pickOption(
  entry: QuestionEntry,
  draft: QuestionDraft,
  questionIndex: number,
  optionIndex: number,
): PickResult | null {
  if (entry.questionKind === 'permission') {
    if (optionIndex === ALLOW_INDEX) {
      return { kind: 'answer', answer: { kind: 'allow' } };
    }
    return { kind: 'answer', answer: { kind: 'deny', message: DENY_MESSAGE } };
  }
  const question: Question | undefined = entry.questions[questionIndex];
  const label: string | undefined = question?.options[optionIndex]?.label;
  if (question === undefined || label === undefined) {
    return null;
  }
  const name: string = optionName(label).name;
  const labels: readonly (readonly string[])[] = draft.labels.map(
    (selected: readonly string[], index: number) => {
      if (index !== questionIndex) {
        return selected;
      }
      if (!question.multiSelect) {
        return [name];
      }
      return selected.includes(name)
        ? selected.filter((candidate: string) => candidate !== name)
        : [...selected, name];
    },
  );
  // Die Wahl einer Option in einer Einfachauswahl verwirft eine eigene Antwort derselben Frage.
  const custom: readonly (string | null)[] = question.multiSelect
    ? draft.custom
    : replaceAt(draft.custom, questionIndex, null);
  const picked: QuestionDraft = { ...draft, labels, custom };
  const next: QuestionDraft = question.multiSelect
    ? picked
    : setActive(picked, nextActive(entry, picked, questionIndex));
  if (!needsSubmit(entry, next) && isComplete(entry, next)) {
    return { kind: 'answer', answer: draftAnswer(next) };
  }
  return { kind: 'draft', draft: next };
}

/** Der gespeicherte Antworttext je Frage; `null`, wenn er sich nicht den Fragen zuordnen lässt (z. B. Freitext). */
export function answerParts(entry: QuestionEntry): readonly string[] | null {
  if (entry.answer === null) {
    return null;
  }
  const parts: string[] = entry.answer.split(ANSWER_SEPARATOR);
  if (parts.length !== entry.questions.length) {
    return null;
  }
  return parts;
}

/** Die Options-Namen, die ein gespeicherter Antworttext für eine Frage gewählt hat. Freitext trifft keinen. */
export function answeredLabels(entry: QuestionEntry, questionIndex: number): readonly string[] {
  if (entry.answer === null) {
    return [];
  }
  const part: string | undefined = answerParts(entry)?.[questionIndex] ?? entry.answer;
  const pieces: readonly string[] = part.split(MULTI_SELECT_SEPARATOR);
  const chosen: readonly string[] = pieces.map((piece: string) => optionName(piece).name);
  const whole: string = optionName(part).name;
  const names: readonly string[] =
    entry.questions[questionIndex]?.options.map(
      (option: QuestionOption) => optionName(option.label).name,
    ) ?? [];
  // Eine Notiz darf selbst `, ` enthalten; ihr Stück beginnt mit `<Name> (Notiz: `.
  return names.filter(
    (name: string) =>
      name === whole ||
      chosen.includes(name) ||
      pieces.some((piece: string) => piece.startsWith(`${name}${NOTE_PREFIX}`)),
  );
}
