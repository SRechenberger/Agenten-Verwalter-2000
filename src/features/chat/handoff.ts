import type { ChatEntry } from '@/lib/bindings/ChatEntry';
import type { HandoffLine } from '@/lib/bindings/HandoffLine';
import type { KnownRepository } from '@/lib/bindings/KnownRepository';

const PROJECT_PREFIX = 'Neues Vorhaben';
const REPOSITORY_NAME_CHARACTER = /[a-z0-9_-]/;

/** `seq` der letzten Textantwort, wenn nach ihr keine Nutzer- oder Session-Nachricht steht; sonst `null`. */
export function lastAnswerSeq(entries: readonly ChatEntry[]): number | null {
  for (let index = entries.length - 1; index >= 0; index -= 1) {
    const entry: ChatEntry | undefined = entries[index];
    if (entry?.kind === 'user' || entry?.kind === 'sessionMessage') {
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
 * eine andere Einstiegszeile geschrieben hat. Alles andere bleibt, wie es ist. `lines` sind die Zeilen in
 * Antwort-Reihenfolge; `placed[i]` sagt, ob Zeile `i` gefunden wurde. Leere Stücke entfallen.
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

/** Die Zeile ohne ihr Kennwort „Neues Vorhaben“ — für die Anzeige in einer Reihe. */
export function handoffExcerpt(line: HandoffLine): string {
  return line.text.slice(PROJECT_PREFIX.length).trim();
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
