import { SETTINGS_CATEGORIES } from '@/features/settings/settingsCatalog';
import type { SettingEntry, SettingsCategoryId } from '@/features/settings/settingsCatalog';

// Reine Suchfunktionen der Einstellungsseite (ADR 057); kein React.

export interface TextPart {
  text: string;
  isHit: boolean;
}

/** Zeichen vor und nach dem ersten Treffer, wenn ein Satz als Auszug zu lang ist. */
const EXCERPT_CONTEXT = 60;
const EXCERPT_MAX_LENGTH = 140;

/** Die Suchwörter in Kleinschrift; leer bei leerem oder nur aus Leerzeichen bestehendem Text. */
export function searchTerms(query: string): string[] {
  return query
    .trim()
    .toLowerCase()
    .split(/\s+/)
    .filter((word: string) => word !== '');
}

/** Einträge, in deren Zeilenname oder ⓘ-Text jedes Wort vorkommt; ohne Wörter alle. */
export function searchEntries(
  entries: readonly SettingEntry[],
  terms: readonly string[],
): SettingEntry[] {
  return entries.filter((entry: SettingEntry) => {
    const haystack: string = `${entry.label} ${entry.info}`.toLowerCase();
    return terms.every((term: string) => haystack.includes(term));
  });
}

export function countByCategory(
  entries: readonly SettingEntry[],
): Record<SettingsCategoryId, number> {
  const counts = {} as Record<SettingsCategoryId, number>;
  for (const category of SETTINGS_CATEGORIES) {
    counts[category.id] = 0;
  }
  for (const entry of entries) {
    counts[entry.category] += 1;
  }
  return counts;
}

function escapeRegExp(text: string): string {
  return text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

/** Zerlegt den Text so, dass jeder Fundort der Wörter ein eigener Teil mit `isHit` ist. */
export function markParts(text: string, terms: readonly string[]): TextPart[] {
  if (terms.length === 0) {
    return [{ text, isHit: false }];
  }
  const pattern: string = [...terms]
    .sort((first: string, second: string) => second.length - first.length)
    .map((term: string) => escapeRegExp(term))
    .join('|');
  return text
    .split(new RegExp(`(${pattern})`, 'gi'))
    .map((part: string, index: number): TextPart => ({ text: part, isHit: index % 2 === 1 }))
    .filter((part: TextPart) => part.text !== '');
}

function cutExcerpt(sentence: string, missing: readonly string[]): string {
  const lower: string = sentence.toLowerCase();
  const hitStart: number = Math.min(
    ...missing.map((term: string) => lower.indexOf(term)).filter((index: number) => index >= 0),
  );
  const hitTerm: string =
    missing.find((term: string) => lower.indexOf(term) === hitStart) ?? missing[0] ?? '';
  const hitEnd: number = hitStart + hitTerm.length;
  let start: number = Math.max(0, hitStart - EXCERPT_CONTEXT);
  let end: number = Math.min(sentence.length, hitEnd + EXCERPT_CONTEXT);
  if (start > 0) {
    const space: number = sentence.indexOf(' ', start);
    start = space >= 0 && space < hitStart ? space + 1 : hitStart;
  }
  if (end < sentence.length) {
    const space: number = sentence.lastIndexOf(' ', end);
    end = space > hitEnd ? space : hitEnd;
  }
  const prefix: string = start > 0 ? '…' : '';
  const suffix: string = end < sentence.length ? '…' : '';
  return `${prefix}${sentence.slice(start, end)}${suffix}`;
}

/**
 * Die Fundstelle im ⓘ-Text für Wörter, die im Zeilennamen fehlen; `null`, wenn der Name
 * alle Wörter enthält oder kein Satz ein fehlendes Wort trägt.
 */
export function foundInInfo(
  label: string,
  info: string,
  terms: readonly string[],
): TextPart[] | null {
  const lowerLabel: string = label.toLowerCase();
  const missing: string[] = terms.filter((term: string) => !lowerLabel.includes(term));
  if (missing.length === 0) {
    return null;
  }
  const sentence: string | undefined = info
    .split(/(?<=\.)\s+/)
    .find((candidate: string) =>
      missing.some((term: string) => candidate.toLowerCase().includes(term)),
    );
  if (sentence === undefined) {
    return null;
  }
  const excerpt: string =
    sentence.length <= EXCERPT_MAX_LENGTH ? sentence : cutExcerpt(sentence, missing);
  return markParts(excerpt, terms);
}
