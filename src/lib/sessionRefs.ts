// Verweise auf Sessions im Chattext (Taiga 108, ADR 023). Importiert nur Typen, damit
// `node src/lib/sessionRefs.selftest.ts` die Datei laden kann (`pnpm selftest`).

/** Ein Stück Text: gewöhnlich, oder ein Verweis auf eine Session nach Nummer („Session 3“) oder ID. */
export type SessionRefPart =
  | { kind: 'text'; text: string }
  | { kind: 'number'; text: string; number: number }
  | { kind: 'id'; text: string; id: string };

/** Was die Auflösung von einer Session braucht. */
export interface RefSession {
  id: string;
  number: number;
  name: string;
  projectId: string;
}

const UUID = '[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}';
// „Session“ als ganzes Wort, ein Leerzeichen, eine Zahl ab 1 als ganzes Wort; die UUID in Kleinbuchstaben.
const REFERENCE = new RegExp(
  `(?<![\\w-])(?:Session ([1-9]\\d*)(?![\\w-])|(${UUID})(?![\\w-]))`,
  'g',
);
const WHOLE_UUID = new RegExp(`^${UUID}$`);

/** Teilt freien Text in gewöhnliche Stücke und Verweise; Codeblöcke und Links schließt der Aufrufer aus. */
export function splitSessionRefs(text: string): SessionRefPart[] {
  const parts: SessionRefPart[] = [];
  let last = 0;
  for (const match of text.matchAll(REFERENCE)) {
    if (match.index > last) {
      parts.push({ kind: 'text', text: text.slice(last, match.index) });
    }
    const number: string | undefined = match[1];
    const id: string | undefined = match[2];
    if (number !== undefined) {
      parts.push({ kind: 'number', text: match[0], number: Number(number) });
    } else if (id !== undefined) {
      parts.push({ kind: 'id', text: match[0], id });
    }
    last = match.index + match[0].length;
  }
  if (last < text.length) {
    parts.push({ kind: 'text', text: text.slice(last) });
  }
  return parts;
}

/** Die ID, wenn `text` als Ganzes eine Session-ID ist (Inline-Code), sonst `null`. */
export function sessionIdOf(text: string): string | null {
  return WHOLE_UUID.test(text) ? text : null;
}

/** Die Session zu einem Verweis: eine Nummer gilt im Vorhaben `projectId`, eine ID überall; sonst `null`. */
export function resolveSessionRef(
  ref: { number: number } | { id: string },
  sessions: readonly RefSession[],
  projectId: string,
): RefSession | null {
  const found: RefSession | undefined =
    'id' in ref
      ? sessions.find((candidate: RefSession) => candidate.id === ref.id)
      : sessions.find(
          (candidate: RefSession) =>
            candidate.projectId === projectId && candidate.number === ref.number,
        );
  return found ?? null;
}

/** Tooltip des Links; bei einer Session eines anderen Vorhabens mit dessen Namen. */
export function sessionRefTitle(session: RefSession, foreignProjectName: string | null): string {
  const base = `Session ${String(session.number)} „${session.name}“`;
  return foreignProjectName === null
    ? `${base} öffnen`
    : `${base} im Vorhaben „${foreignProjectName}“ öffnen`;
}
