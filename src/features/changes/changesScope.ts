import type { ChangeScope } from '@/lib/bindings/ChangeScope';
import type { ChangesReach } from '@/lib/bindings/ChangesReach';
import type { FileChange } from '@/lib/bindings/FileChange';
import type { LineStat } from '@/lib/bindings/LineStat';
import type { RepositoryChanges } from '@/lib/bindings/RepositoryChanges';
import type { SessionChanges } from '@/lib/bindings/SessionChanges';

export const NUMBER_FORMAT = new Intl.NumberFormat('de-DE');

export const EMPTY_SCOPE_TEXT: Record<ChangeScope, string> = {
  all: 'Noch keine Änderungen gegen die Basis.',
  uncommitted: 'Keine uncommitted Änderungen.',
  committed: 'Noch nichts committed.',
};

export const REACH_TEXT: Record<ChangesReach, string> = {
  session: 'Nur, was diese Session geändert hat.',
  implementation: 'Was die Sessions dieser Umsetzung geändert haben.',
  project: 'Was die Sessions dieses Vorhabens geändert haben.',
};

export const FOREIGN_TEXT: Record<ChangesReach, string> = {
  session: 'Dieser Diff enthält auch Änderungen von außerhalb dieser Session.',
  implementation: 'Dieser Diff enthält auch Änderungen von außerhalb dieser Umsetzung.',
  project: 'Dieser Diff enthält auch Änderungen von außerhalb dieses Vorhabens.',
};

/** Sessions aus der Zeit vor der Aufzeichnung (ADR 014). */
export function trackedSinceText(milliseconds: number): string {
  const since: string = new Date(milliseconds).toLocaleString('de-DE', {
    day: '2-digit',
    month: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  });
  return `Erfasst seit ${since} — Änderungen davor fehlen hier.`;
}

export interface LineSums {
  added: number;
  deleted: number;
}

export function statOf(file: FileChange, scope: ChangeScope): LineStat | null {
  return file[scope];
}

/** Ändert sich der Stempel, hat sich die Datei im Blickwinkel geändert und ihr Diff ist veraltet. */
export function statStamp(stat: LineStat | null): string {
  if (stat === null) {
    return 'none';
  }
  return `${stat.kind}:${String(stat.added)}:${String(stat.deleted)}`;
}

export function compareLabel(scope: ChangeScope, baseRef: string, branch: string): string {
  switch (scope) {
    case 'all':
      return `${baseRef} → Arbeitsverzeichnis`;
    case 'committed':
      return `${baseRef} → ${branch}`;
    case 'uncommitted':
      return `${branch} → Arbeitsverzeichnis`;
  }
}

/** Dateien, die unter „Alle“ geändert sind, über alle Repositories. */
export function countChangedFiles(changes: SessionChanges): number {
  let total = 0;
  for (const repository of changes.repositories) {
    total += countFilesInScope(repository, 'all');
  }
  return total;
}

export function countFilesInScope(repository: RepositoryChanges, scope: ChangeScope): number {
  return repository.files.filter((file: FileChange) => statOf(file, scope) !== null).length;
}

/** Summe der Zeilen der Dateien im Blickwinkel; Binärdateien tragen 0 bei. */
export function sumLines(repository: RepositoryChanges, scope: ChangeScope): LineSums {
  const sums: LineSums = { added: 0, deleted: 0 };
  for (const file of repository.files) {
    const stat: LineStat | null = statOf(file, scope);
    if (stat !== null) {
      sums.added += stat.added;
      sums.deleted += stat.deleted;
    }
  }
  return sums;
}

export function formatCount(value: number): string {
  return NUMBER_FORMAT.format(value);
}

export function fileCountLabel(count: number): string {
  return count === 1 ? '1 Datei' : `${formatCount(count)} Dateien`;
}
