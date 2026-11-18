import {
  BELOW_IMPLEMENTATION_STATES,
  placeImplementations,
  shownSessions,
  type Placement,
} from '@/app/sidebarPlacement';
import {
  liveStations,
  NEEDS_USER_STATES,
  type StationRef,
} from '@/features/implementation/implementationOrder';
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
/** Ablegeziel „ohne Gruppe“ neben den Gruppen-IDs in `dropGroup`. */
export const DROP_NONE = 'none';

export type SidebarRow = (
  | { kind: 'dropNone'; key: string }
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
  | {
      kind: 'implementation';
      key: string;
      implementation: ImplementationView;
      /** `true`: eine Ebene unter der Master-Session, die sie leitet (ADR 070). */
      underMaster: boolean;
      isLast: boolean;
    }
  | {
      kind: 'station';
      key: string;
      implementationId: string;
      entry: StationRef;
      /** `true`: eine Ebene unter der Umsetzung; `false`: auf Höhe der Umsetzungen (zugeklapptes Vorhaben). */
      isNested: boolean;
      /** `true`: ihre Umsetzung steht unter einem Master, die Station eine Ebene tiefer. */
      underMaster: boolean;
      isLast: boolean;
    }
  | {
      kind: 'session';
      key: string;
      session: SessionSummary;
      /** `true`: die Session leitet eine Umsetzung, die unter ihr steht (ADR 070). */
      isMaster: boolean;
      isLast: boolean;
    }
  | {
      kind: 'more';
      key: string;
      projectId: string;
      /** Anzahl der ausgeblendeten Sessions; im aufgeklappten Zustand die der eingeklappten Ansicht. */
      hiddenCount: number;
      isShowingAll: boolean;
    }
) & {
  /** Worauf ein Ablegen auf dieser Zeile zielt: die ID ihrer Gruppe oder `DROP_NONE`. */
  dropGroup: string;
};

export interface SidebarProjectEntry {
  project: ProjectSummary;
  /** Sessions des Vorhabens samt Stationen, zuletzt aktive zuerst; Stationen bekommen keine Session-Zeile,
   *  aktive erscheinen als Stationszeile (ADR 065). */
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
 *  samt aktiven Stationen und Sessions, bei zugeklappten die Stationen, die auf den Benutzer warten. Mit
 *  `showDropNone` (während des Ziehens) steht die Ablegefläche „Ohne Gruppe“ ganz oben. */
export function buildSidebarRows(
  ungrouped: readonly SidebarProjectEntry[],
  groups: readonly SidebarGroupEntry[],
  showDropNone: boolean,
): SidebarRow[] {
  const rows: SidebarRow[] = [];
  if (showDropNone) {
    rows.push({ kind: 'dropNone', key: 'drop-none', dropGroup: DROP_NONE });
  }
  for (const entry of ungrouped) {
    pushProjectRows(rows, entry, DROP_NONE);
  }
  for (const { group, projects } of groups) {
    rows.push({
      kind: 'group',
      key: `group:${group.id}`,
      group,
      count: projects.length,
      urgentStatus: mostUrgentStatus(projects),
      dropGroup: group.id,
    });
    if (!group.collapsed) {
      for (const entry of projects) {
        pushProjectRows(rows, entry, group.id);
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

function pushProjectRows(rows: SidebarRow[], entry: SidebarProjectEntry, dropGroup: string): void {
  const { project, sessions, isExpanded } = entry;
  rows.push({
    kind: 'project',
    key: `project:${project.id}`,
    project,
    sessions,
    isExpanded,
    dropGroup,
  });
  const children: SidebarRow[] = isExpanded
    ? expandedChildren(entry, dropGroup)
    : waitingChildren(entry, dropGroup);
  rows.push(...markLast(children));
}

function expandedChildren(
  { project, sessions, implementations, showAll, pinnedIds }: SidebarProjectEntry,
  dropGroup: string,
): SidebarRow[] {
  const children: SidebarRow[] = [];
  const ownSessions: SessionSummary[] = visibleSessions(sessions);
  const isLong: boolean = ownSessions.length >= COLLAPSE_FROM;
  const placement: Placement<ImplementationView> = placeImplementations(
    implementations,
    new Set<string>(ownSessions.map((session: SessionSummary) => session.id)),
  );
  const shown: SessionSummary[] = shownSessions(ownSessions, {
    collapseFrom: COLLAPSE_FROM,
    visibleCount: VISIBLE_COUNT,
    showAll,
    pinnedIds,
    masterIds: new Set<string>(placement.under.keys()),
  });
  const hiddenCount: number = ownSessions.length - shown.length;
  for (const implementation of placement.inProject) {
    children.push(...implementationRows(implementation, false, dropGroup));
  }
  for (const session of shown) {
    children.push({
      kind: 'session',
      key: `session:${session.id}`,
      session,
      isMaster: placement.under.has(session.id),
      isLast: false,
      dropGroup,
    });
    for (const implementation of placement.under.get(session.id) ?? []) {
      children.push(...implementationRows(implementation, true, dropGroup));
    }
  }
  if (isLong && (showAll || hiddenCount > 0)) {
    children.push({
      kind: 'more',
      key: `more:${project.id}`,
      projectId: project.id,
      hiddenCount: showAll ? ownSessions.length - VISIBLE_COUNT : hiddenCount,
      isShowingAll: showAll,
      dropGroup,
    });
  }
  return children;
}

/** Zugeklappt bleiben nur die Stationen sichtbar, die auf den Benutzer warten. */
function waitingChildren(
  { implementations }: SidebarProjectEntry,
  dropGroup: string,
): SidebarRow[] {
  const children: SidebarRow[] = [];
  for (const implementation of implementations) {
    for (const entry of liveStations(implementation)) {
      if (NEEDS_USER_STATES.includes(entry.station.state)) {
        children.push(stationRow(implementation.id, entry, false, false, dropGroup));
      }
    }
  }
  return children;
}

/** Die Zeile einer Umsetzung, darunter nur ihre Stationen, die den Benutzer brauchen (ADR 070, Variante A). */
function implementationRows(
  implementation: ImplementationView,
  underMaster: boolean,
  dropGroup: string,
): SidebarRow[] {
  const rows: SidebarRow[] = [
    {
      kind: 'implementation',
      key: `implementation:${implementation.id}`,
      implementation,
      underMaster,
      isLast: false,
      dropGroup,
    },
  ];
  for (const entry of liveStations(implementation)) {
    if (BELOW_IMPLEMENTATION_STATES.includes(entry.station.state)) {
      rows.push(stationRow(implementation.id, entry, true, underMaster, dropGroup));
    }
  }
  return rows;
}

function stationRow(
  implementationId: string,
  entry: StationRef,
  isNested: boolean,
  underMaster: boolean,
  dropGroup: string,
): SidebarRow {
  return {
    kind: 'station',
    key: `station:${entry.station.id}`,
    implementationId,
    entry,
    isNested,
    underMaster,
    isLast: false,
    dropGroup,
  };
}

/** Die letzte Kindzeile schließt die Baumlinie ab; `more` kennt keine Linie und bleibt unberührt. */
function markLast(children: SidebarRow[]): SidebarRow[] {
  const last: SidebarRow | undefined = children[children.length - 1];
  if (last === undefined || !('isLast' in last)) {
    return children;
  }
  return [...children.slice(0, -1), { ...last, isLast: true }];
}
