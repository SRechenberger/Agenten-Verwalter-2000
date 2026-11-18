import { useState } from 'react';
import type { ReactElement, ReactNode } from 'react';
import { QuestionBlock } from '@/features/chat/QuestionBlock';
import { SessionToolBlock } from '@/features/chat/SessionToolBlock';
import {
  useQuestionAnswering,
  useQuestionKeys,
  type QuestionAnswering,
} from '@/features/chat/useQuestionAnswering';
import type { QuestionEntry } from '@/features/chat/questionDraft';
import { classifiedText, isMessageEmpty } from '@/features/dashboard/dashboardTexts';
import { useLastExchange } from '@/features/dashboard/useLastExchange';
import { stationsAfterProposal } from '@/features/implementation/CardStack';
import { CancelRequestCard } from '@/features/implementation/CancelRequestCard';
import {
  answerCancelRequest,
  applyEditedProposal,
} from '@/features/implementation/implementationActions';
import { flatStations, type StationRef } from '@/features/implementation/implementationOrder';
import {
  APPLY_FAILED,
  CANCELLED_NOT_DELETED,
  DONE_BY_USER_LABEL,
  DONE_BY_USER_RESULT,
  DONE_BY_USER_TITLE,
  RESULT_LABEL,
  RESULT_PLACEHOLDER,
  START_FAILED,
} from '@/features/implementation/implementationTexts';
import { MasterWarning, MasterPicker } from '@/features/implementation/MasterPicker';
import { masterOf, RELEASED_MARK, RELEASED_TITLE } from '@/features/implementation/masterTexts';
import { PlanChangeCard } from '@/features/implementation/PlanChangeCard';
import { StationReportSection } from '@/features/implementation/StationReportSection';
import { lockNote } from '@/features/remote/projectAccess';
import type { DashboardTask } from '@/lib/bindings/DashboardTask';
import type { ImplementationView } from '@/lib/bindings/ImplementationView';
import type { ProjectSummary } from '@/lib/bindings/ProjectSummary';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import { sendMessage } from '@/lib/chat';
import { dismissCard } from '@/lib/dashboard';
import {
  applyImplementationChange,
  completeStationByUser,
  startStation,
} from '@/lib/implementation';
import { restartSession, resumeSession } from '@/lib/sessions';
import type { ActionError } from '@/lib/useActionError';
import './PopupBodies.css';

/** Was jeder Inhalt eines Popups braucht; `open` ist der Knopf „Session öffnen“ bzw. „Umsetzung öffnen“. */
export interface PopupProps {
  task: DashboardTask;
  sessions: readonly SessionSummary[];
  implementations: readonly ImplementationView[];
  projects: readonly ProjectSummary[];
  /** Ein anderes Gerät steuert das Vorhaben, oder der Host fehlt: Die Knöpfe, die schreiben, ruhen. */
  isLocked: boolean;
  run: ActionError['run'];
  open: ReactElement | null;
}

const NOT_OPEN_ANYMORE = 'Diese Sache ist nicht mehr offen.';

function Layout({
  open,
  children,
  actions,
}: {
  open: ReactElement | null;
  children?: ReactNode;
  actions?: ReactNode;
}): ReactElement {
  return (
    <>
      <div className="popup-body">{children}</div>
      <div className="popup-foot">
        {open}
        {actions}
      </div>
    </>
  );
}

function implementationOf(props: PopupProps): ImplementationView | undefined {
  return props.implementations.find(
    (candidate: ImplementationView) => candidate.id === props.task.implementationId,
  );
}

function stationOf(props: PopupProps, implementation: ImplementationView): StationRef | undefined {
  return flatStations(implementation).find(
    (ref: StationRef) => ref.station.id === props.task.stationId,
  );
}

/** Rückfrage, Rechte-Abfrage und Kasten: dieselben Bausteine und dieselbe Antwort-Logik wie im Chat. */
export function RequestBody(props: PopupProps): ReactElement {
  const { task, open } = props;
  const entry = task.entry;
  if (task.sessionId === null || entry === null || entry.kind !== 'question') {
    return <Layout open={open}>{NOT_OPEN_ANYMORE}</Layout>;
  }
  return <OpenRequestBody {...props} sessionId={task.sessionId} entry={entry} />;
}

function OpenRequestBody({
  task,
  sessions,
  projects,
  open,
  sessionId,
  entry,
}: PopupProps & { sessionId: string; entry: QuestionEntry }): ReactElement {
  const answering: QuestionAnswering = useQuestionAnswering(sessionId, task.projectId);
  useQuestionKeys(entry, answering);
  const request = entry.toolRequest;
  const sessionNumber: number | null =
    sessions.find((session: SessionSummary) => session.id === sessionId)?.number ??
    task.sessionNumber;
  if (request !== null && request.kind === 'agentType') {
    return (
      <Layout open={open}>
        <p>Ein Agent schlägt einen Agententyp vor. Beantworte den Vorschlag im Chat der Session.</p>
      </Layout>
    );
  }
  if (request !== null) {
    return (
      <Layout open={open}>
        <SessionToolBlock
          entry={entry}
          request={request}
          sessionId={sessionId}
          sessionNumber={sessionNumber ?? 0}
          projectId={task.projectId}
          selfRelease={
            projects.find((project: ProjectSummary) => project.id === task.projectId)
              ?.selfRelease ?? null
          }
          draft={answering.toolDraftFor(entry.requestId)}
          isSending={answering.isSending(entry.requestId)}
          onPick={(optionIndex: number): void => {
            answering.pick(entry, 0, optionIndex);
          }}
          onSubmit={(answer): void => {
            answering.submit(entry, answer);
          }}
          onDraft={(next): void => {
            answering.storeToolDraft(entry.requestId, next);
          }}
        />
      </Layout>
    );
  }
  return (
    <Layout open={open}>
      <QuestionBlock
        entry={entry}
        draft={answering.draftFor(entry)}
        lockNote={answering.lock === null ? null : lockNote(answering.lock)}
        isSending={answering.isSending(entry.requestId)}
        onPick={(questionIndex: number, optionIndex: number): void => {
          answering.pick(entry, questionIndex, optionIndex);
        }}
        onSubmit={(answer): void => {
          answering.submit(entry, answer);
        }}
        onDraft={(next): void => {
          answering.storeDraft(entry, next);
        }}
      />
    </Layout>
  );
}

export function PlanChangeBody(props: PopupProps): ReactElement {
  const { sessions, isLocked, run, open } = props;
  const implementation: ImplementationView | undefined = implementationOf(props);
  const pending = implementation?.pendingChange ?? null;
  if (implementation === undefined || pending === null) {
    return <Layout open={open}>{NOT_OPEN_ANYMORE}</Layout>;
  }
  const refs: StationRef[] = flatStations(implementation);
  return (
    <Layout open={open}>
      <PlanChangeCard
        key={pending.at}
        kind="proposal"
        sessionNumber={pending.sessionNumber}
        at={pending.at}
        summary={pending.summary}
        stationsBefore={implementation.stationsTotal}
        stationsAfter={stationsAfterProposal(implementation, pending)}
        edit={pending.edit ?? null}
        stations={refs}
        projectId={implementation.projectId}
        sessions={sessions}
        onApply={(accept: boolean): void => {
          run(() => applyImplementationChange(implementation.id, accept), APPLY_FAILED);
        }}
        onApplyEdited={(overrides): void => {
          const before = new Set<string>(refs.map((ref: StationRef) => ref.station.planId));
          run(() => applyEditedProposal(implementation.id, before, overrides), APPLY_FAILED);
        }}
        isLocked={isLocked}
      />
    </Layout>
  );
}

export function CancelRequestBody(props: PopupProps): ReactElement {
  const { isLocked, run, open } = props;
  const implementation: ImplementationView | undefined = implementationOf(props);
  const request = implementation?.pendingCancel ?? null;
  if (implementation === undefined || request === null) {
    return <Layout open={open}>{NOT_OPEN_ANYMORE}</Layout>;
  }
  return (
    <Layout open={open}>
      <CancelRequestCard
        request={request}
        isLocked={isLocked}
        onApply={(accept: boolean): void => {
          run(
            async (): Promise<void> => {
              const notDeleted: string | null = await answerCancelRequest(
                implementation.id,
                accept,
              );
              if (notDeleted !== null) {
                throw new Error(`${CANCELLED_NOT_DELETED}: ${notDeleted}`);
              }
            },
            accept ? 'Bestätigen fehlgeschlagen' : 'Verwerfen fehlgeschlagen',
          );
        }}
      />
    </Layout>
  );
}

/** Station „durch dich“: der Auftrag, ein Ergebnis nach Wahl und „Erledigt“. */
export function ByYouBody(props: PopupProps): ReactElement {
  const { task, isLocked, run, open } = props;
  const [result, setResult] = useState<string>('');
  const implementation: ImplementationView | undefined = implementationOf(props);
  const ref: StationRef | undefined =
    implementation === undefined ? undefined : stationOf(props, implementation);
  if (implementation === undefined || ref === undefined) {
    return <Layout open={open}>{NOT_OPEN_ANYMORE}</Layout>;
  }
  return (
    <Layout
      open={open}
      actions={
        <button
          type="button"
          className="popup-button popup-button--primary"
          title={DONE_BY_USER_TITLE}
          disabled={isLocked}
          onClick={(): void => {
            run(
              () =>
                completeStationByUser(
                  implementation.id,
                  ref.station.id,
                  result.trim() === '' ? DONE_BY_USER_RESULT : result.trim(),
                ),
              'Erledigen fehlgeschlagen',
            );
          }}
        >
          {DONE_BY_USER_LABEL}
        </button>
      }
    >
      {task.released && (
        <span className="popup-approve" title={RELEASED_TITLE}>
          {RELEASED_MARK}
        </span>
      )}
      <div className="popup-message">{ref.station.brief}</div>
      <input
        type="text"
        className="popup-input"
        maxLength={200}
        placeholder={RESULT_PLACEHOLDER}
        aria-label={RESULT_LABEL}
        value={result}
        onChange={(event): void => {
          setResult(event.target.value);
        }}
      />
    </Layout>
  );
}

export function ReturnsExhaustedBody(props: PopupProps): ReactElement {
  const { sessions, isLocked, run, open } = props;
  const implementation: ImplementationView | undefined = implementationOf(props);
  const ref: StationRef | undefined =
    implementation === undefined ? undefined : stationOf(props, implementation);
  if (implementation === undefined || ref === undefined) {
    return <Layout open={open}>{NOT_OPEN_ANYMORE}</Layout>;
  }
  return (
    <Layout open={open}>
      <StationReportSection
        implementationId={implementation.id}
        station={ref.station}
        master={masterOf(implementation, sessions)}
        isLocked={isLocked}
        run={run}
      />
    </Layout>
  );
}

export function MasterMissingBody(props: PopupProps): ReactElement {
  const { sessions, isLocked, run, open } = props;
  const implementation: ImplementationView | undefined = implementationOf(props);
  if (implementation === undefined) {
    return <Layout open={open}>{NOT_OPEN_ANYMORE}</Layout>;
  }
  return (
    <Layout open={open}>
      <MasterWarning implementation={implementation} sessions={sessions} />
      <MasterPicker
        implementation={implementation}
        sessions={sessions}
        isLocked={isLocked}
        run={run}
      />
    </Layout>
  );
}

/** „Gelesen“ gibt es nur bei einer Session ohne Station. */
function ReadButton({ task, run }: Pick<PopupProps, 'task' | 'run'>): ReactElement | null {
  const { sessionId } = task;
  if (sessionId === null || task.stationId !== null) {
    return null;
  }
  return (
    <button
      type="button"
      className="popup-button"
      onClick={(): void => {
        run(() => dismissCard(sessionId), 'Gelesen fehlgeschlagen');
      }}
    >
      Gelesen
    </button>
  );
}

export function ErrorBody(props: PopupProps): ReactElement {
  const { task, isLocked, run, open } = props;
  const { sessionId } = task;
  return (
    <Layout
      open={open}
      actions={
        <>
          <ReadButton task={task} run={run} />
          {sessionId !== null && (
            <button
              type="button"
              className="popup-button popup-button--primary"
              disabled={isLocked}
              onClick={(): void => {
                run(() => restartSession(sessionId), 'Neustart fehlgeschlagen');
              }}
            >
              Agent neu starten
            </button>
          )}
        </>
      }
    >
      <div className="popup-code">{task.title}</div>
    </Layout>
  );
}

export function PausedBody(props: PopupProps): ReactElement {
  const { task, isLocked, run, open } = props;
  const { sessionId } = task;
  return (
    <Layout
      open={open}
      actions={
        <>
          <ReadButton task={task} run={run} />
          {sessionId !== null && (
            <button
              type="button"
              className="popup-button popup-button--primary"
              disabled={isLocked}
              onClick={(): void => {
                run(() => resumeSession(sessionId), 'Fortsetzen fehlgeschlagen');
              }}
            >
              Fortsetzen
            </button>
          )}
        </>
      }
    >
      <p>{task.title}</p>
    </Layout>
  );
}

export function ReadyBody(props: PopupProps): ReactElement {
  const { task, isLocked, run, open } = props;
  const { implementationId, stationId } = task;
  return (
    <Layout
      open={open}
      actions={
        implementationId !== null &&
        stationId !== null && (
          <button
            type="button"
            className="popup-button popup-button--primary"
            disabled={isLocked}
            onClick={(): void => {
              run(() => startStation(implementationId, stationId), START_FAILED);
            }}
          >
            ▶ Jetzt starten
          </button>
        )
      }
    >
      <p>
        {task.stationLabel ?? task.title} hat keine offenen Vorgänger und startet nicht von selbst.
      </p>
    </Layout>
  );
}

/** `asks`, `requests`, `done`, `answer`: die letzte Antwort, die Nachricht davor und ein Antwortfeld. */
export function AnswerBody(props: PopupProps): ReactElement {
  const { task, open } = props;
  if (task.sessionId === null) {
    return <Layout open={open}>{NOT_OPEN_ANYMORE}</Layout>;
  }
  return <OpenAnswerBody {...props} sessionId={task.sessionId} />;
}

function OpenAnswerBody({
  task,
  isLocked,
  run,
  open,
  sessionId,
}: PopupProps & { sessionId: string }): ReactElement {
  const [text, setText] = useState<string>('');
  const { exchange, error } = useLastExchange(sessionId);
  const isEmpty: boolean = isMessageEmpty(text);
  return (
    <Layout
      open={open}
      actions={
        <>
          <ReadButton task={task} run={run} />
          <button
            type="button"
            className="popup-button popup-button--primary"
            disabled={isLocked || isEmpty}
            title={isEmpty ? 'Leere Nachricht: erst Text eingeben' : undefined}
            onClick={(): void => {
              run(() => sendMessage(sessionId, text.trim(), [], []), 'Senden fehlgeschlagen');
            }}
          >
            Senden
          </button>
        </>
      }
    >
      <p className="popup-note">{classifiedText(task)}</p>
      {error !== null && <p role="alert">{error}</p>}
      {exchange !== null && exchange.before !== null && (
        <div className="popup-message popup-message--before">{exchange.before}</div>
      )}
      <div className="popup-message">{exchange?.answer ?? task.title}</div>
      <textarea
        className="popup-input"
        rows={3}
        placeholder="Antwort …"
        aria-label="Antwort"
        value={text}
        onChange={(event): void => {
          setText(event.target.value);
        }}
      />
    </Layout>
  );
}
