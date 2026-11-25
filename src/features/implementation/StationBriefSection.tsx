import { useState } from 'react';
import type { ReactElement } from 'react';
import { CardSection } from '@/features/implementation/CardSection';
import { DoneConfirm } from '@/features/implementation/DoneConfirm';
import { patchEdit } from '@/features/implementation/implementationGraph';
import { isBegun, type StationRef } from '@/features/implementation/implementationOrder';
import { stationDisplay } from '@/features/implementation/commandStation';
import {
  BRIEF_CHANGE_BEFORE,
  BRIEF_EDIT_DROPS_CHANGES,
  BRIEF_LABEL,
  BRIEF_NOT_SAVED,
  briefChangeAction,
  briefChangeLead,
  briefSummary,
  DONE_BY_USER_LABEL,
  DONE_BY_USER_RESULT,
  DONE_BY_USER_TITLE,
  INSTRUCTION_LABEL,
  MAX_BRIEF_CHARS,
  overLimitReason,
  NO_BRIEF_CHANGES,
  NOT_REMOVED,
  NOT_SAVED,
  PREDECESSORS_MAY_LABEL,
  PREDECESSORS_MAY_OPTIONS,
  RESULT_LABEL,
  RESULT_PLACEHOLDER,
  SECTION_BRIEF,
  SECTION_INSTRUCTION,
} from '@/features/implementation/implementationTexts';
import { LimitCounter } from '@/features/implementation/LimitCounter';
import { SegmentGroup } from '@/features/implementation/SegmentGroup';
import { asksBeforeDone, isDoneByYou } from '@/features/implementation/stationControls';
import type { BriefChange } from '@/lib/bindings/BriefChange';
import type { PredecessorsMay } from '@/lib/bindings/PredecessorsMay';
import type { StationView } from '@/lib/bindings/StationView';
import { editPlan, removeBriefChange, setStationOptions } from '@/lib/implementation';
import type { ActionError } from '@/lib/useActionError';

interface StationBriefSectionProps {
  implementationId: string;
  stationRef: StationRef;
  refs: readonly StationRef[];
  isOpen: boolean;
  onToggle: () => void;
  isLocked: boolean;
  run: ActionError['run'];
  onCompleteByUser: (stationId: string, result: string) => void;
  /** Master der Umsetzung: ohne Freigabe fragt „Erledigt“ nach (#136). */
  masterSessionId: string | null;
}

/** Der Auftrag vor einer Änderung: der Grundtext mit allen Änderungen davor (wie `effective_brief` im Core). */
function briefBefore(station: StationView, index: number): string {
  return station.briefChanges
    .slice(0, index)
    .reduce(
      (text: string, change: BriefChange) =>
        change.kind === 'replace'
          ? change.text
          : text === ''
            ? change.text
            : `${text}\n${change.text}`,
      station.brief,
    );
}

interface BriefFieldProps {
  initial: string;
  isOpen: boolean;
  label: string;
  onSave: (brief: string) => void;
}

/** Das Textfeld des Auftrags; gespeichert wird beim Verlassen, damit Tippen nichts neu zeichnet. Neu
 *  angelegt (Schlüssel), sobald der gespeicherte Auftrag sich ändert. */
function BriefField({ initial, isOpen, label, onSave }: BriefFieldProps): ReactElement {
  const [draft, setDraft] = useState<string>(initial);
  const isOver: boolean = draft.length > MAX_BRIEF_CHARS;
  return (
    <>
      <textarea
        className="station-card__textarea"
        aria-label={label}
        rows={3}
        title={isOver ? overLimitReason(label, MAX_BRIEF_CHARS) : undefined}
        value={draft}
        disabled={!isOpen}
        onChange={(event): void => {
          setDraft(event.target.value);
        }}
        onBlur={(): void => {
          if (draft !== initial && !isOver) {
            onSave(draft);
          }
        }}
      />
      <LimitCounter
        length={draft.length}
        max={MAX_BRIEF_CHARS}
        note={isOpen ? 'bis zum Start änderbar' : 'gesendet, festgeschrieben 🔒'}
      />
    </>
  );
}

/** „Auftrag“ der Kartei, bei „durch dich“ „Anweisung“: Text bis 1000 Zeichen, die Änderungen durch
 *  Vorgänger mit Quelle und „Vorgänger dürfen“; bei „durch dich“ an der Reihe Ergebnisfeld und „Erledigt“. */
export function StationBriefSection({
  implementationId,
  stationRef,
  refs,
  isOpen,
  onToggle,
  isLocked,
  run,
  onCompleteByUser,
  masterSessionId,
}: StationBriefSectionProps): ReactElement {
  const [result, setResult] = useState<string>('');
  const [isAskingDone, setIsAskingDone] = useState<boolean>(false);
  const { station } = stationRef;
  const isWaiting: boolean = !isBegun(station);

  function sourceName(change: BriefChange): string {
    const source: StationRef | undefined = refs.find(
      (ref: StationRef) =>
        ref.station.id === change.sourceStationId || ref.station.planId === change.sourceStationId,
    );
    if (source === undefined) {
      return change.sourceStationId;
    }
    return stationDisplay(source.station);
  }

  function saveBrief(brief: string): void {
    run(
      () => editPlan(implementationId, patchEdit({ id: station.planId, brief })),
      BRIEF_NOT_SAVED,
    );
  }

  function setPredecessorsMay(predecessorsMay: PredecessorsMay): void {
    run(
      () =>
        setStationOptions(implementationId, station.id, {
          autoStart: station.autoStart,
          onError: station.onError,
          predecessorsMay,
        }),
      NOT_SAVED,
    );
  }

  function renderChange(change: BriefChange, index: number): ReactElement {
    return (
      <div key={change.id} className="station-card__note">
        <b>{briefChangeLead(change.kind, sourceName(change))}</b>
        {change.kind === 'replace' && (
          <div className="station-card__note-was">
            {BRIEF_CHANGE_BEFORE} {briefBefore(station, index)}
          </div>
        )}
        <div className="station-card__note-text">{change.text}</div>
        {isWaiting && (
          <button
            type="button"
            className="station-card__mini"
            disabled={isLocked}
            onClick={(): void => {
              run(() => removeBriefChange(implementationId, station.id, change.id), NOT_REMOVED);
            }}
          >
            {briefChangeAction(change.kind)}
          </button>
        )}
      </div>
    );
  }

  function renderUserResult(): ReactElement | null {
    if (!isDoneByYou(station)) {
      return null;
    }
    const complete = (): void => {
      onCompleteByUser(station.id, result.trim() === '' ? DONE_BY_USER_RESULT : result.trim());
    };
    return (
      <>
        <div className="station-card__line">
          <span className="station-card__label">{RESULT_LABEL}</span>
          <input
            type="text"
            className="station-card__input"
            maxLength={200}
            placeholder={RESULT_PLACEHOLDER}
            aria-label={RESULT_LABEL}
            value={result}
            onChange={(event): void => {
              setResult(event.target.value);
            }}
          />
          <button
            type="button"
            className="station-card__chip station-card__chip--add"
            title={DONE_BY_USER_TITLE}
            disabled={isLocked}
            aria-expanded={isAskingDone}
            onClick={(): void => {
              if (asksBeforeDone(station, { masterSessionId })) {
                setIsAskingDone(true);
              } else {
                complete();
              }
            }}
          >
            {DONE_BY_USER_LABEL}
          </button>
        </div>
        {isAskingDone && (
          <DoneConfirm
            isLocked={isLocked}
            onConfirm={(): void => {
              setIsAskingDone(false);
              complete();
            }}
            onBack={(): void => {
              setIsAskingDone(false);
            }}
          />
        )}
      </>
    );
  }

  const label: string = station.byUser ? INSTRUCTION_LABEL : BRIEF_LABEL;
  return (
    <CardSection
      title={station.byUser ? SECTION_INSTRUCTION : SECTION_BRIEF}
      summary={briefSummary(station.brief.length, station.briefChanges.length)}
      isOpen={isOpen}
      onToggle={onToggle}
    >
      <span className="station-card__label">{label}</span>
      <BriefField
        key={station.brief}
        initial={station.brief}
        isOpen={isWaiting && !isLocked}
        label={label}
        onSave={saveBrief}
      />
      {isWaiting && station.briefChanges.length > 0 && (
        <p className="station-card__hint">{BRIEF_EDIT_DROPS_CHANGES}</p>
      )}
      {renderUserResult()}
      {!station.byUser && (
        <div className="station-card__line">
          <span className="station-card__label">{PREDECESSORS_MAY_LABEL}</span>
          <SegmentGroup<PredecessorsMay>
            label={PREDECESSORS_MAY_LABEL}
            options={PREDECESSORS_MAY_OPTIONS}
            value={station.predecessorsMay}
            disabled={!isWaiting || isLocked}
            onChange={setPredecessorsMay}
          />
        </div>
      )}
      {station.briefChanges.length === 0 ? (
        <p className="station-card__hint">{NO_BRIEF_CHANGES}</p>
      ) : (
        station.briefChanges.map(renderChange)
      )}
    </CardSection>
  );
}
