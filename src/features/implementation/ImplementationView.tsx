import { useEffect, useMemo, useState } from 'react';
import type { ReactElement, ReactNode } from 'react';
import { useAgentTypes } from '@/features/agenttypes/useAgentTypes';
import { useNow } from '@/features/background/useNow';
import { countChangedFiles } from '@/features/changes/changesScope';
import { useSessionChanges } from '@/features/changes/useSessionChanges';
import { CardStack } from '@/features/implementation/CardStack';
import { ImplementationBar } from '@/features/implementation/ImplementationBar';
import { ImplementationHeader } from '@/features/implementation/ImplementationHeader';
import {
  autoFocus,
  flatStations,
  focusOrder,
  isFocusValid,
  type StationRef,
} from '@/features/implementation/implementationOrder';
import { patchEdit } from '@/features/implementation/implementationGraph';
import {
  APPLY_FAILED,
  START_FAILED,
  stationNotFound,
} from '@/features/implementation/implementationTexts';
import { NewLaneForm } from '@/features/implementation/NewLaneForm';
import type { ProposalOverride } from '@/features/implementation/PlanChangeCard';
import { ReportPanel } from '@/features/implementation/ReportPanel';
import { Timeline } from '@/features/implementation/Timeline';
import { useProjectLocked } from '@/features/remote/useProjectAccess';
import type { ImplementationView as Implementation } from '@/lib/bindings/ImplementationView';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import type { StationPatch } from '@/lib/bindings/StationPatch';
import {
  applyImplementationChange,
  cancelStationCountdown,
  completeStationByUser,
  editPlan,
  listImplementations,
  loadImplementationReport,
  setImplementationAutoStart,
  setImplementationStopAfterCurrent,
  setMaxParallel,
  setStationAgent,
  startStation,
} from '@/lib/implementation';
import { useActionError } from '@/lib/useActionError';
import { useImplementationStore } from '@/stores/implementation';
import { useSessionsStore, type ImplementationViewKind } from '@/stores/sessions';
import './ImplementationView.css';

interface ImplementationViewProps {
  implementation: Implementation;
  projectName: string;
  activeView: ImplementationViewKind;
  hasChanges: boolean;
  changesCount: number | null;
  /** Alle Sessions; die Kachel der Station im Fokus liest daraus die Changes ihrer Session. */
  sessions: readonly SessionSummary[];
  /** Inhalt des Reiters „Changes“ (Reichweite Vorhaben); gezeigt, wenn er aktiv ist. */
  changes: ReactNode;
  /** Legt eine Rückfrage zur Umsetzung an und öffnet sie (`useInquiry`). */
  onNewInquiry: (implementationId: string) => Promise<void>;
  onShowView: (view: ImplementationViewKind) => void;
  onOpenProject: () => void;
  /** Laufende Terminals des Vorhabens. */
  terminalCount: number;
  isTerminalOpen: boolean;
  onToggleTerminal: () => void;
}

function isTextTarget(target: EventTarget | null): boolean {
  return (
    target instanceof HTMLElement &&
    target.closest('input, textarea, select, [contenteditable]:not([contenteditable="false"])') !==
      null
  );
}

/**
 * Ein Dialog (Einstellungen der Station, Agententyp) steht über der Kartei: Die Pfeiltasten blättern
 * nicht dahinter. Gefragt wird, ob einer offen ist, nicht, wo das Ziel der Taste liegt: Nach einem
 * Klick auf Text im Dialog oder auf einen Knopf, der sich selbst entfernt („Zurücksetzen“), liegt
 * der Fokus auf `body`.
 */
function isDialogOpen(): boolean {
  return document.querySelector('[aria-modal="true"]') !== null;
}

/** Schritt in der Stationsfolge für eine Taste; 0, wenn sie nicht blättert. */
function arrowStep(key: string): number {
  if (key === 'ArrowDown') {
    return 1;
  }
  return key === 'ArrowUp' ? -1 : 0;
}

/** Die Stationen einer Umsetzung, wie der Core sie jetzt hält; nach einem Befehl, bevor das Ereignis da ist. */
async function freshStations(implementationId: string): Promise<StationRef[]> {
  const fresh: Implementation | undefined = (await listImplementations()).find(
    (candidate: Implementation) => candidate.id === implementationId,
  );
  return fresh === undefined ? [] : flatStations(fresh);
}

/** „Übernehmen“ mit geänderten Werten: erst der Vorschlag wie vorgeschlagen (damit die Plan-Änderung mit
 *  ihrer Quelle vermerkt ist), dann Typ, Modell und Denkaufwand je Station und der Auftrag als Delta. Die
 *  Stationen findet die Funktion über ihre Kennung, neue ohne Kennung über Lane und Name unter den neuen. */
async function applyEditedProposal(
  implementationId: string,
  before: ReadonlySet<string>,
  overrides: readonly ProposalOverride[],
): Promise<void> {
  await applyImplementationChange(implementationId, true);
  const refs: StationRef[] = await freshStations(implementationId);
  const used = new Set<string>();
  const briefs: StationPatch[] = [];
  for (const override of overrides) {
    const target: StationRef | undefined = refs.find(
      (ref: StationRef) =>
        !used.has(ref.station.planId) &&
        (override.planId === null
          ? !before.has(ref.station.planId) &&
            ref.phase.key === override.lane &&
            ref.station.stage === override.stage
          : ref.station.planId === override.planId),
    );
    if (target === undefined) {
      throw new Error(stationNotFound(override.stage, override.lane));
    }
    used.add(target.station.planId);
    const { station } = target;
    if (override.changed.agent || override.changed.model || override.changed.effort) {
      await setStationAgent(implementationId, station.id, {
        agent: override.agent,
        model: override.model,
        effort: override.effort,
        prompt: station.prompt,
        skill: station.skill,
      });
    }
    if (override.changed.brief) {
      briefs.push({ id: station.planId, brief: override.brief });
    }
  }
  if (briefs.length > 0) {
    await editPlan(implementationId, patchEdit(...briefs));
  }
}

/** Hauptfläche einer Umsetzung (ADR 048): Kopfzeile, darunter Zeitleiste mit Kartei oder die Changes. */
export function ImplementationView({
  implementation,
  projectName,
  activeView,
  hasChanges,
  changesCount,
  sessions,
  changes,
  onNewInquiry,
  onShowView,
  onOpenProject,
  terminalCount,
  isTerminalOpen,
  onToggleTerminal,
}: ImplementationViewProps): ReactElement {
  // Text des geöffneten Lageberichts; `null`: das Fenster ist zu.
  const [report, setReport] = useState<string | null>(null);
  const [isReportBusy, setIsReportBusy] = useState<boolean>(false);
  const [isInquiryBusy, setIsInquiryBusy] = useState<boolean>(false);
  const [isNewLaneOpen, setIsNewLaneOpen] = useState<boolean>(false);
  const storedFocus: string | undefined = useImplementationStore(
    (state) => state.focused[implementation.id],
  );
  const setFocused = useImplementationStore((state) => state.setFocused);
  const clearFocused = useImplementationStore((state) => state.clearFocused);
  const selectSession = useSessionsStore((state) => state.selectSession);
  const showSessionView = useSessionsStore((state) => state.showView);
  const { error: actionError, run } = useActionError(implementation.id);
  const isLocked: boolean = useProjectLocked(implementation.projectId);
  const { agentTypes } = useAgentTypes();

  const stations: StationRef[] = useMemo(() => flatStations(implementation), [implementation]);
  const isFlow: boolean = !(activeView === 'changes' && hasChanges);
  const isTicking: boolean = stations.some((ref: StationRef) =>
    ['running', 'starting', 'waitsForScript', 'countdown'].includes(ref.station.state),
  );
  const now: number = useNow(isTicking);

  const automaticId: string | null = autoFocus(implementation);
  const order: string[] = useMemo(() => focusOrder(implementation), [implementation]);
  const isStoredValid: boolean =
    storedFocus !== undefined && isFocusValid(implementation, storedFocus);
  const focusedId: string | null =
    isStoredValid && storedFocus !== undefined ? storedFocus : automaticId;
  const isManualFocus: boolean = isStoredValid && storedFocus !== automaticId;
  const implementationId: string = implementation.id;

  // Die Kachel der Station im Fokus zählt nur die Changes ihrer eigenen Session (Reichweite Session).
  const focusedSessionId: string | null =
    stations.find((ref: StationRef) => ref.station.id === focusedId)?.station.sessionId ?? null;
  const focusedSession: SessionSummary | null =
    focusedSessionId === null
      ? null
      : (sessions.find((session: SessionSummary) => session.id === focusedSessionId) ?? null);
  const { changes: stationChanges } = useSessionChanges(focusedSession, 'session', isFlow);
  const stationChangesCount: number | null =
    stationChanges === null ? null : countChangedFiles(stationChanges);

  useEffect(() => {
    if (!isFlow || focusedId === null) {
      return undefined;
    }
    const currentFocus: string = focusedId;
    function handleKeyDown(event: KeyboardEvent): void {
      const step: number = arrowStep(event.key);
      if (step === 0 || event.defaultPrevented || event.altKey || event.ctrlKey || event.metaKey) {
        return;
      }
      if (isTextTarget(event.target) || isDialogOpen()) {
        return;
      }
      const position: number = order.indexOf(currentFocus);
      const target: string | undefined = order[position + step];
      if (position < 0 || target === undefined) {
        return;
      }
      event.preventDefault();
      setFocused(implementationId, target);
    }
    window.addEventListener('keydown', handleKeyDown);
    return (): void => {
      window.removeEventListener('keydown', handleKeyDown);
    };
  }, [isFlow, focusedId, order, implementationId, setFocused]);

  // Der Bericht wird gelesen, bevor das Fenster aufgeht; bis dahin dreht sich der Knopf.
  function openReport(): void {
    setIsReportBusy(true);
    run(
      () =>
        loadImplementationReport(implementationId)
          .then((text: string): void => {
            setReport(text);
          })
          .finally((): void => {
            setIsReportBusy(false);
          }),
      'Lagebericht nicht ladbar',
    );
  }

  function startInquiry(): void {
    setIsInquiryBusy(true);
    run(
      () =>
        onNewInquiry(implementationId).finally((): void => {
          setIsInquiryBusy(false);
        }),
      'Rückfrage nicht angelegt',
    );
  }

  /** Die neue Lane steht an ihrer Stufe; ihre erste Station kommt in den Fokus. */
  function focusCreated(firstPlanId: string): void {
    setIsNewLaneOpen(false);
    run(async (): Promise<void> => {
      const created: StationRef | undefined = (await freshStations(implementationId)).find(
        (ref: StationRef) => ref.station.planId === firstPlanId,
      );
      if (created !== undefined) {
        setFocused(implementationId, created.station.id);
      }
    });
  }

  function openChat(stationId: string): void {
    const sessionId: string | null =
      stations.find((ref: StationRef) => ref.station.id === stationId)?.station.sessionId ?? null;
    if (sessionId !== null) {
      selectSession(sessionId);
    }
  }

  // Die Changes einer Station öffnen sich in ihrer Session, nicht in der Reichweite des Vorhabens.
  function openStationChanges(stationId: string): void {
    const sessionId: string | null =
      stations.find((ref: StationRef) => ref.station.id === stationId)?.station.sessionId ?? null;
    if (sessionId !== null) {
      selectSession(sessionId);
      showSessionView('changes');
    }
  }

  function renderActionError(): ReactElement | null {
    if (actionError === null) {
      return null;
    }
    return (
      <div className="implementation-view__error" role="alert">
        {actionError}
      </div>
    );
  }

  function renderCards(): ReactElement | null {
    if (focusedId === null) {
      return null;
    }
    return (
      <CardStack
        implementation={implementation}
        focusedId={focusedId}
        isManualFocus={isManualFocus}
        now={now}
        changesCount={hasChanges ? stationChangesCount : null}
        sessions={sessions}
        onFocus={(focusId: string): void => {
          setFocused(implementationId, focusId);
        }}
        onBackToCurrent={(): void => {
          clearFocused(implementationId);
        }}
        onOpenChat={openChat}
        onOpenSession={selectSession}
        onShowChanges={(stationId: string): void => {
          openStationChanges(stationId);
        }}
        onStart={(stationId: string): void => {
          run(() => startStation(implementationId, stationId), START_FAILED);
        }}
        onCancelCountdown={(stationId: string): void => {
          run(() => cancelStationCountdown(implementationId, stationId));
        }}
        onCompleteByUser={(stationId: string, result: string): void => {
          run(() => completeStationByUser(implementationId, stationId, result));
        }}
        onApplyChange={(accept: boolean): void => {
          run(() => applyImplementationChange(implementationId, accept), APPLY_FAILED);
        }}
        onApplyEdited={(overrides: readonly ProposalOverride[]): void => {
          const before = new Set<string>(stations.map((ref: StationRef) => ref.station.planId));
          run(() => applyEditedProposal(implementationId, before, overrides), APPLY_FAILED);
        }}
        run={run}
      />
    );
  }

  function renderBody(): ReactNode {
    if (!isFlow) {
      return changes;
    }
    return (
      <div className="implementation-view">
        <div className="implementation-view__timeline">
          <Timeline
            implementation={implementation}
            focusedId={focusedId}
            now={now}
            sessions={sessions}
            isLocked={isLocked}
            onFocus={(focusId: string): void => {
              setFocused(implementationId, focusId);
            }}
            onOpenChat={openChat}
            onStart={(stationId: string): void => {
              run(() => startStation(implementationId, stationId), START_FAILED);
            }}
            onCancelCountdown={(stationId: string): void => {
              run(() => cancelStationCountdown(implementationId, stationId));
            }}
          />
        </div>
        <div className="implementation-view__cards">{renderCards()}</div>
      </div>
    );
  }

  return (
    <>
      <ImplementationHeader
        implementation={implementation}
        projectName={projectName}
        activeView={activeView}
        hasChanges={hasChanges}
        changesCount={changesCount}
        onShowView={onShowView}
        onOpenProject={onOpenProject}
        terminalCount={terminalCount}
        isTerminalOpen={isTerminalOpen}
        onToggleTerminal={onToggleTerminal}
      />
      <ImplementationBar
        implementation={implementation}
        isReportShown={report !== null}
        isInquiryBusy={isInquiryBusy}
        isLocked={isLocked}
        isReportBusy={isReportBusy}
        onNewInquiry={startInquiry}
        onOpenReport={openReport}
        onSetAutoStart={(enabled: boolean): void => {
          run(() => setImplementationAutoStart(implementationId, enabled));
        }}
        onSetStopAfterCurrent={(enabled: boolean): void => {
          run(() => setImplementationStopAfterCurrent(implementationId, enabled));
        }}
        onSetMaxParallel={(maxParallel: number | null): void => {
          run(() => setMaxParallel(implementationId, maxParallel));
        }}
        isNewLaneOpen={isNewLaneOpen}
        onToggleNewLane={(): void => {
          setIsNewLaneOpen(!isNewLaneOpen);
        }}
      />
      {renderActionError()}
      {isNewLaneOpen && isFlow && (
        <NewLaneForm
          implementation={implementation}
          agentTypes={agentTypes ?? []}
          sessions={sessions}
          run={run}
          isLocked={isLocked}
          onClose={(): void => {
            setIsNewLaneOpen(false);
          }}
          onCreated={focusCreated}
        />
      )}
      {renderBody()}
      {report !== null && (
        <ReportPanel
          implementation={implementation}
          initialText={report}
          onClose={(): void => {
            setReport(null);
          }}
        />
      )}
    </>
  );
}
