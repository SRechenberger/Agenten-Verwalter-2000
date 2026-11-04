import { mostUrgent, URGENCY, visibleSessions } from '@/features/projects/projectStatus';
import { displayStatus, type DisplayStatus } from '@/features/sessions/sessionStatus';
import type { ImplementationView } from '@/lib/bindings/ImplementationView';
import type { ProjectGroup } from '@/lib/bindings/ProjectGroup';
import type { ProjectSummary } from '@/lib/bindings/ProjectSummary';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';

/** Ab so vielen Sessions kürzt die Sidebar ein aufgeklapptes Vorhaben. */
export const COLLAPSE_FROM = 5;
/** So viele der zuletzt aktiven Sessions bleiben im gekürzten Vorhaben sichtbar. */
export const VISIBLE_COUNT = 3;

export type SidebarRow =
  | {
      kind: 'group';
      key: string;
      group: ProjectGroup;
      /** Anzahl der Vorhaben in der Gruppe, auch bei eingeklapptem Kopf. */
      count: number;
      /** Dringendster Status aller Vorhaben der Gruppe; `null` ohne Sessions. */
      urgentStatus: DisplayStatus | null;
    }
  | {
      kind: 'project';
      key: string;
      project: ProjectSummary;
      sessions: readonly SessionSummary[];
      isExpanded: boolean;
    }
  | { kind: 'implementation'; key: string; implementation: ImplementationView; isLast: boolean }
  | { kind: 'session'; key: string; session: SessionSummary; isLast: boolean }
  | {
      kind: 'more';
      key: string;
      projectId: string;
      /** Anzahl der ausgeblendeten Sessions; im aufgeklappten Zustand die der eingeklappten Ansicht. */
      hiddenCount: number;
      isShowingAll: boolean;
    };

export interface SidebarProjectEntry {
  project: ProjectSummary;
  /** Sessions des Vorhabens samt Stationen, zuletzt aktive zuerst; Stationen bekommen keine Zeile. */
  sessions: readonly SessionSummary[];
  /** Umsetzungen des Vorhabens, neueste zuerst (ADR 048). */
  implementations: readonly ImplementationView[];
  isExpanded: boolean;
  /** `true`: alle Sessions zeigen, auch bei langer Liste. */
  showAll: boolean;
  /** Sessions, die auch in der gekürzten Liste stehen bleiben (die offene und die, die gerade umbenannt wird). */
  pinnedIds: ReadonlySet<string>;
}

export interface SidebarGroupEntry {
  group: ProjectGroup;
  /** Vorhaben der Gruppe in der Reihenfolge, in der sie stehen sollen. */
  projects: readonly SidebarProjectEntry[];
}

/** Die flache Zeilenliste der Sidebar: erst die Vorhaben ohne Gruppe, dann je Gruppe ihr Kopf und, solange er
 *  nicht eingeklappt ist, ihre Vorhaben. Je Vorhaben seine Zeile, bei aufgeklappten darunter ihre Umsetzungen
 *  und Sessions. */
export function buildSidebarRows(
  ungrouped: readonly SidebarProjectEntry[],
  groups: readonly SidebarGroupEntry[],
): SidebarRow[] {
  const rows: SidebarRow[] = [];
  for (const entry of ungrouped) {
    pushProjectRows(rows, entry);
  }
  for (const { group, projects } of groups) {
    rows.push({
      kind: 'group',
      key: `group:${group.id}`,
      group,
      count: projects.length,
      urgentStatus: mostUrgentStatus(projects),
    });
    if (!group.collapsed) {
      for (const entry of projects) {
        pushProjectRows(rows, entry);
      }
    }
  }
  return rows;
}

function mostUrgentStatus(projects: readonly SidebarProjectEntry[]): DisplayStatus | null {
  let best: DisplayStatus | null = null;
  for (const { sessions } of projects) {
    const urgent: SessionSummary | null = mostUrgent(sessions);
    if (urgent === null) {
      continue;
    }
    const status: DisplayStatus = displayStatus(urgent, sessions);
    if (best === null || URGENCY.indexOf(status) < URGENCY.indexOf(best)) {
      best = status;
    }
  }
  return best;
}

function pushProjectRows(
  rows: SidebarRow[],
  { project, sessions, implementations, isExpanded, showAll, pinnedIds }: SidebarProjectEntry,
): void {
  rows.push({ kind: 'project', key: `project:${project.id}`, project, sessions, isExpanded });
  if (!isExpanded) {
    return;
  }
  const ownSessions: SessionSummary[] = visibleSessions(sessions);
  implementations.forEach((implementation: ImplementationView, index: number) => {
    rows.push({
      kind: 'implementation',
      key: `implementation:${implementation.id}`,
      implementation,
      isLast: ownSessions.length === 0 && index === implementations.length - 1,
    });
  });
  const isLong: boolean = ownSessions.length >= COLLAPSE_FROM;
  const shown: SessionSummary[] =
    isLong && !showAll
      ? ownSessions.filter(
          (session: SessionSummary, index: number) =>
            index < VISIBLE_COUNT || pinnedIds.has(session.id),
        )
      : ownSessions;
  const hiddenCount: number = ownSessions.length - shown.length;
  const hasMoreRow: boolean = isLong && (showAll || hiddenCount > 0);
  shown.forEach((session: SessionSummary, index: number) => {
    rows.push({
      kind: 'session',
      key: `session:${session.id}`,
      session,
      isLast: !hasMoreRow && index === shown.length - 1,
    });
  });
  if (hasMoreRow) {
    rows.push({
      kind: 'more',
      key: `more:${project.id}`,
      projectId: project.id,
      hiddenCount: showAll ? ownSessions.length - VISIBLE_COUNT : hiddenCount,
      isShowingAll: showAll,
    });
  }
}
