import { formatClock } from '@/features/context/formatTokens';
import type { EnvironmentStatus } from '@/lib/bindings/EnvironmentStatus';
import type { LinkCheck } from '@/lib/bindings/LinkCheck';
import type { MirrorDiff } from '@/lib/bindings/MirrorDiff';
import type { MirrorKind } from '@/lib/bindings/MirrorKind';
import type { MirrorRun } from '@/lib/bindings/MirrorRun';

/** Ein Eintrag, den „Spiegeln“ kopieren würde. */
export function isPending(diff: MirrorDiff): boolean {
  return diff.state === 'onlyMaster' || diff.state === 'masterNewer';
}

/** Ein Link ist kaputt oder ein Eintrag im Master ist neuer bzw. fehlt lokal. */
export function needsAttention(status: EnvironmentStatus | null): boolean {
  if (status === null) {
    return false;
  }
  return status.links.broken.length > 0 || status.diffs.some(isPending);
}

export function linksSummary(links: LinkCheck): string {
  if (links.total === 0) {
    return 'Keine Verknüpfungen';
  }
  if (links.broken.length === 0) {
    return `Alle ${String(links.total)} erreichbar`;
  }
  return `⚠ ${String(links.broken.length)} von ${String(links.total)} zeigt ins Leere`;
}

function countText(count: number, singular: string, plural: string): string {
  return count === 1 ? `1 ${singular}` : `${String(count)} ${plural}`;
}

function countOfKind(diffs: readonly MirrorDiff[], kind: MirrorKind): number {
  return diffs.filter((diff: MirrorDiff) => diff.kind === kind).length;
}

function joinParts(parts: readonly string[]): string {
  if (parts.length < 2) {
    return parts.join('');
  }
  return `${parts.slice(0, -1).join(', ')} und ${parts[parts.length - 1] ?? ''}`;
}

/** Der Satz über die abweichenden Einträge; `null`, wenn nichts abweicht. */
export function mirrorSummary(diffs: readonly MirrorDiff[]): string | null {
  const pending: MirrorDiff[] = diffs.filter(isPending);
  if (pending.length === 0) {
    return null;
  }
  const parts: string[] = [];
  const skills: number = countOfKind(pending, 'skill');
  const commands: number = countOfKind(pending, 'command');
  const outputStyles: number = countOfKind(pending, 'outputStyle');
  if (skills > 0) {
    parts.push(countText(skills, 'Skill', 'Skills'));
  }
  if (commands > 0) {
    parts.push(countText(commands, 'Befehl', 'Befehle'));
  }
  if (outputStyles > 0) {
    parts.push(countText(outputStyles, 'Ausgabestil', 'Ausgabestile'));
  }
  if (countOfKind(pending, 'rules') > 0) {
    parts.push('die Regeldatei');
  }
  const verb: string = pending.length === 1 ? 'weicht ab' : 'weichen ab';
  return `${joinParts(parts)} ${verb}`;
}

export function mirroredText(run: MirrorRun): string {
  return `zuletzt gespiegelt ${formatClock(run.at)} (${countText(run.copied, 'Datei', 'Dateien')})`;
}

export function kindLabel(kind: MirrorKind): string {
  switch (kind) {
    case 'skill':
      return 'Skill';
    case 'command':
      return 'Befehl';
    case 'outputStyle':
      return 'Ausgabestil';
    case 'rules':
      return 'Regeldatei';
  }
}

function dateText(milliseconds: number): string {
  return new Date(milliseconds)
    .toLocaleString('de-DE', {
      day: '2-digit',
      month: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
    })
    .replace(',', '');
}

export function stateText(diff: MirrorDiff): string {
  switch (diff.state) {
    case 'onlyMaster':
      return 'fehlt lokal';
    case 'masterNewer':
      if (diff.masterModifiedAt === null) {
        return 'im Master neuer';
      }
      return `im Master neuer · ${dateText(diff.masterModifiedAt)}`;
    case 'localNewer':
      return 'lokal neuer · bleibt';
    case 'onlyLocal':
      return 'nur lokal · bleibt';
  }
}

export function hintCount(count: number): string {
  return countText(count, 'Hinweis', 'Hinweise');
}
