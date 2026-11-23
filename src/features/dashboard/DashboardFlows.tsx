import type { ReactElement } from 'react';
import { StatusIcon } from '@/components/StatusIcon';
import {
  FLOW_STATE_LABEL,
  flowOpenText,
  flowPercent,
  flowTone,
  remoteHostOf,
  remoteTitle,
  SESSION_STATE_LABEL,
  sessionTone,
} from '@/features/dashboard/dashboardTexts';
import { RemoteMark } from '@/features/remote/RemoteMark';
import type { DisplayStatus } from '@/features/sessions/sessionStatus';
import type { DashboardFlow } from '@/lib/bindings/DashboardFlow';
import type { DashboardFlowState } from '@/lib/bindings/DashboardFlowState';
import type { DashboardSession } from '@/lib/bindings/DashboardSession';
import type { ProjectSummary } from '@/lib/bindings/ProjectSummary';

/** Gleiche Formbedeutung wie in der Leiste; „wartet auf Master“ hat die Form von „wartet“ und eine eigene Farbe. */
const FLOW_ICON: Record<DashboardFlowState, DisplayStatus> = {
  running: 'running',
  waitsForYou: 'waiting',
  waitsForMaster: 'waiting',
  error: 'error',
  done: 'completed',
};

interface DashboardFlowsProps {
  flows: readonly DashboardFlow[];
  sessions: readonly DashboardSession[];
  projects: readonly ProjectSummary[];
  onOpenImplementation: (implementationId: string) => void;
  onOpenSession: (sessionId: string) => void;
}

/** Marke „entfernt“ (Issue 198, Variante D): ⇄ in Akzentfarbe wie in der Leiste, der Host in `title` und
 *  `aria-label`. */
export function DashboardRemoteMark({ host }: { host: string }): ReactElement {
  return (
    <span
      className="dashboard__remote"
      role="img"
      aria-label={remoteTitle(host)}
      title={remoteTitle(host)}
    >
      <RemoteMark size={11} />
    </span>
  );
}

/** Zusatz am `title` einer Zeile eines entfernten Vorhabens. */
function remoteSuffix(host: string | null): string {
  return host === null ? '' : ` · ${remoteTitle(host)}`;
}

/** Rechte Spalte: die Abläufe mit Zustand und Fortschritt, darunter die aktiven Sessions ohne Ablauf. */
export function DashboardFlows({
  flows,
  sessions,
  projects,
  onOpenImplementation,
  onOpenSession,
}: DashboardFlowsProps): ReactElement {
  return (
    <aside className="dashboard__flows" aria-label="Abläufe">
      <h3 className="dashboard__subhead">Abläufe</h3>
      {flows.map((flow: DashboardFlow) => {
        const host: string | null = remoteHostOf(flow.projectId, projects);
        return (
          <button
            key={flow.implementationId}
            type="button"
            className={`dashboard-flow dashboard-flow--${flowTone(flow.state)}`}
            title={`Öffnet die Umsetzung${remoteSuffix(host)}`}
            onClick={(): void => {
              onOpenImplementation(flow.implementationId);
            }}
          >
            <span className="dashboard-flow__line">
              <StatusIcon status={FLOW_ICON[flow.state]} size={12} shape="square" />
              {host !== null && <DashboardRemoteMark host={host} />}
              <span className="dashboard-flow__name">{flow.name}</span>
              <span className="dashboard-flow__state">{FLOW_STATE_LABEL[flow.state]}</span>
            </span>
            <span className="dashboard-flow__line">
              <span
                className="dashboard-flow__bar"
                role="progressbar"
                aria-label="Fortschritt"
                aria-valuemin={0}
                aria-valuemax={flow.stationsTotal}
                aria-valuenow={flow.stationsDone}
              >
                <i style={{ width: `${String(flowPercent(flow))}%` }} />
              </span>
              <span className="dashboard-flow__fraction">
                {flow.stationsDone}/{flow.stationsTotal}
              </span>
              <span className="dashboard-flow__open">{flowOpenText(flow)}</span>
            </span>
          </button>
        );
      })}
      <h3 className="dashboard__subhead dashboard__subhead--sessions">Sessions ohne Ablauf</h3>
      {sessions.map((session: DashboardSession) => {
        const host: string | null = remoteHostOf(session.projectId, projects);
        return (
          <button
            key={session.sessionId}
            type="button"
            className={`dashboard-session dashboard-session--${sessionTone(session.state)}`}
            title={`Öffnet die Session${remoteSuffix(host)}`}
            onClick={(): void => {
              onOpenSession(session.sessionId);
            }}
          >
            <span className="dashboard-session__dot" aria-hidden="true" />
            {host !== null && <DashboardRemoteMark host={host} />}
            <span className="dashboard-session__name">
              {session.projectName} · #{session.number}
              {session.title === '' ? '' : ` ${session.title}`}
            </span>
            <span className="dashboard-session__state">{SESSION_STATE_LABEL[session.state]}</span>
          </button>
        );
      })}
    </aside>
  );
}
