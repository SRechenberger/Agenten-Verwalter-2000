import type { ChatEntry } from '@/lib/bindings/ChatEntry';
import type { HandoffHalt } from '@/lib/bindings/HandoffHalt';
import type { HandoffLine } from '@/lib/bindings/HandoffLine';
import type { KnownRepository } from '@/lib/bindings/KnownRepository';

const SESSION_PREFIX = 'Weiter:';
const PROJECT_PREFIX = 'Neues Vorhaben';
const BACK_PREFIX = 'Zurück an';
const REPOSITORY_NAME_CHARACTER = /[a-z0-9_-]/;

/** Erklärung am Info-Symbol des Schalters „Automatisch weiter“ (ADR 040). */
export const AUTO_HANDOFF_TITLE =
  'Startet nach jeder fertigen Antwort mit genau einer „Weiter:“-Zeile die nächste Session dieses Vorhabens selbst — nach 10 Sekunden Countdown, mit dem Modell aus der Zeile und der Zeile als erster Nachricht. Hält an, wenn die Antwort 🔴 enthält, mit einer Frage endet, „Merge“ nennt oder mehrere „Weiter:“-Zeilen hat, und nach 6 automatischen Starts in Folge ohne Nachricht von dir — oder sendet die Zeile „Zurück an …“ an die Session, aus der diese gestartet wurde (nur wenn sie ruht).';

/** Grund unter der Antwort, warum „Automatisch weiter“ nicht startet (ADR 040). */
export const HALT_TEXT: Record<HandoffHalt, string> = {
  red: 'Nicht automatisch weiter: die Antwort enthält 🔴.',
  question: 'Nicht automatisch weiter: die Antwort endet mit einer Frage.',
  merge: 'Nicht automatisch weiter: die Antwort nennt den Merge.',
  severalLines: 'Nicht automatisch weiter: mehrere Einstiegszeilen — wähle selbst.',
  pendingSession: 'Nicht automatisch weiter: im Vorhaben wartet schon eine neue Session.',
  chainLimit:
    'Nicht automatisch weiter: 6 automatische Starts in Folge — schick eine Nachricht oder starte von Hand.',
  parentBusy:
    'Nicht automatisch weiter: die Session, an die zurückgemeldet wird, arbeitet oder wartet auf dich.',
  cancelled: 'Automatisch weiter abgebrochen.',
};

/** `seq` der letzten Textantwort, wenn nach ihr keine Nutzernachricht steht; sonst `null`. */
export function lastAnswerSeq(entries: readonly ChatEntry[]): number | null {
  for (let index = entries.length - 1; index >= 0; index -= 1) {
    const entry: ChatEntry | undefined = entries[index];
    if (entry?.kind === 'user') {
      return null;
    }
    if (entry?.kind === 'text') {
      return entry.seq;
    }
  }
  return null;
}

/** Der Auftrag einer Zeile „Neues Vorhaben … : <Auftrag>“ — alles hinter dem ersten Doppelpunkt. */
export function taskOfProjectLine(text: string): string {
  return text.slice(text.indexOf(':') + 1).trim();
}

/** Die Zeilen, zu denen die Leiste eine Reihe zeigt: „Zurück an“ nur mit gemerkter Eltern-Session. */
export function visibleHandoffLines(
  lines: readonly HandoffLine[],
  parentSessionId: string | null,
): HandoffLine[] {
  return lines.filter((line: HandoffLine) => line.kind !== 'back' || parentSessionId !== null);
}

/** Ob die Reihe einer Zeile gezeigt wird; während des Countdowns ersetzt er „Weiter“ und „Zurück“ (Text wie Leiste). */
export function isRowShown(line: HandoffLine, isCountdown: boolean): boolean {
  return !isCountdown || line.kind === 'project';
}

/** Wie `clean` im Core (`sessions/handoff.rs`): Zitatzeichen vorn, `**` überall, Backticks außen weg. */
function cleanLine(raw: string): string {
  const trimmed: string = raw.trim();
  const unquoted: string = trimmed.startsWith('>') ? trimmed.slice(1).trimStart() : trimmed;
  return unquoted
    .replace(/\*\*/g, '')
    .trim()
    .replace(/^`+|`+$/g, '')
    .trim();
}

/** Ein Stück des Antworttexts; `lineIndex` ist die Einstiegszeile (Index in `lines`), die dieses Stück ist, sonst `null`. */
export interface HandoffSegment {
  text: string;
  lineIndex: number | null;
}

/**
 * Teilt den Antworttext vor und hinter jeder Zeile, die er im Text findet, sodass jede Einstiegszeile ein
 * eigenes Stück (eigener Absatz) ist — auch wenn der Agent sie ohne Leerzeile unter anderen Text oder unter
 * eine andere Einstiegszeile geschrieben hat. Alles andere bleibt, wie es ist. `lines` sind die sichtbaren
 * Zeilen in Antwort-Reihenfolge; `placed[i]` sagt, ob Zeile `i` gefunden wurde. Leere Stücke entfallen.
 *
 * Der Core speichert nur den Text der Zeilen, keine Position — deshalb von hinten zuordnen: steht derselbe
 * Text weiter oben noch einmal, ist die letzte Fundstelle die Einstiegszeile. Eine Rohzeile in einem offenen
 * Code-Zaun zählt nicht (der Core liest dort keine).
 */
export function splitAtHandoffLines(
  text: string,
  lines: readonly HandoffLine[],
): { segments: HandoffSegment[]; placed: boolean[] } {
  const rawLines: string[] = text.split('\n');
  const insideFence: boolean[] = [];
  let open = false;
  for (const raw of rawLines) {
    if (raw.trim().startsWith('```')) {
      open = !open;
      insideFence.push(true);
    } else {
      insideFence.push(open);
    }
  }
  const placed: boolean[] = lines.map(() => false);
  const found: { rawIndex: number; lineIndex: number }[] = [];
  let next: number = lines.length - 1;
  for (let index = rawLines.length - 1; index >= 0 && next >= 0; index -= 1) {
    if (insideFence[index] ?? false) {
      continue;
    }
    // Eine Zeile, die nicht im Text steht, soll die davor nicht mitreißen: weiter nach vorn suchen.
    const cleaned: string = cleanLine(rawLines[index] ?? '');
    let match: number = next;
    while (match >= 0 && lines[match]?.text !== cleaned) {
      match -= 1;
    }
    if (match >= 0) {
      found.push({ rawIndex: index, lineIndex: match });
      placed[match] = true;
      next = match - 1;
    }
  }
  found.reverse();
  const segments: HandoffSegment[] = [];
  let from = 0;
  for (const { rawIndex, lineIndex } of found) {
    segments.push({ text: rawLines.slice(from, rawIndex).join('\n'), lineIndex: null });
    segments.push({ text: rawLines[rawIndex] ?? '', lineIndex });
    from = rawIndex + 1;
  }
  segments.push({ text: rawLines.slice(from).join('\n'), lineIndex: null });
  return {
    segments: segments.filter((segment: HandoffSegment) => segment.text.trim() !== ''),
    placed,
  };
}

/** Der Text einer Zeile „Zurück an <Name>: <Text>“ — alles hinter dem ersten Doppelpunkt. */
export function handoffBackText(line: HandoffLine): string {
  return line.text.slice(line.text.indexOf(':') + 1).trim();
}

/** Der Name aus einer Zeile „Zurück an <Name>: <Text>“ — für den Knopf, wenn die Eltern-Session nicht im Speicher steht. */
export function handoffBackName(line: HandoffLine): string {
  return line.text.slice(BACK_PREFIX.length, line.text.indexOf(':')).trim();
}

const PREFIX_OF_KIND: Record<HandoffLine['kind'], string> = {
  session: SESSION_PREFIX,
  project: PROJECT_PREFIX,
  back: BACK_PREFIX,
};

/** Die Zeile ohne ihr Kennwort („Weiter:“, „Neues Vorhaben“ bzw. „Zurück an“) — für die Anzeige in einer Reihe. */
export function handoffExcerpt(line: HandoffLine): string {
  return line.text.slice(PREFIX_OF_KIND[line.kind].length).trim();
}

/** IDs der vorhandenen Repositories, deren Name in `text` als ganzes Wort steht (ohne Groß-/Kleinschreibung). */
export function matchingRepositoryIds(
  text: string,
  repositories: readonly KnownRepository[],
): string[] {
  const haystack: string = text.toLowerCase();
  return repositories
    .filter(
      (repository: KnownRepository) =>
        !repository.isMissing && containsWholeWord(haystack, repository.name.toLowerCase()),
    )
    .map((repository: KnownRepository) => repository.id);
}

function containsWholeWord(haystack: string, word: string): boolean {
  if (word === '') {
    return false;
  }
  let from = 0;
  for (;;) {
    const index: number = haystack.indexOf(word, from);
    if (index === -1) {
      return false;
    }
    const before: string = haystack.charAt(index - 1);
    const after: string = haystack.charAt(index + word.length);
    const isBounded: boolean =
      (before === '' || !REPOSITORY_NAME_CHARACTER.test(before)) &&
      (after === '' || !REPOSITORY_NAME_CHARACTER.test(after));
    if (isBounded) {
      return true;
    }
    from = index + 1;
  }
}
