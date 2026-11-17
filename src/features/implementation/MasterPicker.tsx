import { useState } from 'react';
import type { ReactElement } from 'react';
import { Popover } from '@/components/Popover';
import { SeverityIcon } from '@/components/SeverityIcon';
import { StatusIcon } from '@/components/StatusIcon';
import {
  MASTER_CHANGE,
  MASTER_CHANGE_TITLE,
  MASTER_LABEL,
  MASTER_NEW_LABEL,
  MASTER_NEW_NOT_CREATED,
  MASTER_NONE,
  MASTER_NOT_SET,
  MASTER_POPOVER_TITLE,
  MASTER_STATION_TITLE,
  masterMissingText,
  masterName,
  masterOf,
} from '@/features/implementation/masterTexts';
import { sessionsOf } from '@/features/projects/projectStatus';
import { displayStatus } from '@/features/sessions/sessionStatus';
import type { ImplementationView } from '@/lib/bindings/ImplementationView';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import { setMaster } from '@/lib/implementation';
import { createSessionInProject } from '@/lib/sessions';
import type { ActionError } from '@/lib/useActionError';
import './MasterPicker.css';

const ICON_SIZE = 14;

interface MasterPickerProps {
  implementation: ImplementationView;
  sessions: readonly SessionSummary[];
  isLocked: boolean;
  run: ActionError['run'];
}

/** „Master: #1 Architekt · ändern“ im Kopf der Umsetzung; „ändern“ öffnet die Sessions des Vorhabens.
 *  Stationen stehen gesperrt in der Liste (ADR 070). */
export function MasterPicker({
  implementation,
  sessions,
  isLocked,
  run,
}: MasterPickerProps): ReactElement {
  const [isOpen, setIsOpen] = useState<boolean>(false);
  const master: string | null = masterOf(implementation, sessions);
  const candidates: SessionSummary[] = sessionsOf(implementation.projectId, sessions);

  function choose(sessionId: string): void {
    setIsOpen(false);
    run(() => setMaster(implementation.id, sessionId), MASTER_NOT_SET);
  }

  function chooseNew(): void {
    setIsOpen(false);
    run(async (): Promise<void> => {
      const created: SessionSummary = await createSessionInProject(implementation.projectId);
      await setMaster(implementation.id, created.id);
    }, MASTER_NEW_NOT_CREATED);
  }

  return (
    <div className="master-picker">
      <span className="master-picker__label">{MASTER_LABEL}</span>
      <b
        className={`master-picker__name${implementation.masterMissing ? ' master-picker__name--missing' : ''}`}
      >
        {master ?? MASTER_NONE}
      </b>
      <span aria-hidden="true">·</span>
      <button
        type="button"
        className="master-picker__change"
        aria-haspopup="dialog"
        aria-expanded={isOpen}
        title={MASTER_CHANGE_TITLE}
        disabled={isLocked}
        onClick={(): void => {
          setIsOpen(!isOpen);
        }}
      >
        {MASTER_CHANGE}
      </button>
      {isOpen && (
        <Popover
          label={MASTER_POPOVER_TITLE}
          placement="below"
          align="start"
          width={280}
          onClose={(): void => {
            setIsOpen(false);
          }}
        >
          <p className="master-picker__title">{MASTER_POPOVER_TITLE}</p>
          {candidates.map((session: SessionSummary) => {
            const isStation: boolean = session.stationOf !== null;
            return (
              <button
                key={session.id}
                type="button"
                role="menuitem"
                className="master-picker__item"
                disabled={isStation}
                aria-current={session.id === implementation.masterSessionId ? 'true' : undefined}
                title={isStation ? MASTER_STATION_TITLE : undefined}
                onClick={(): void => {
                  choose(session.id);
                }}
              >
                <StatusIcon status={displayStatus(session, sessions)} size={10} />
                <span className="master-picker__item-name">
                  {masterName(session.number, session.name)}
                  {isStation ? ' (Station)' : ''}
                </span>
                {session.id === implementation.masterSessionId && (
                  <span className="master-picker__item-mark">◆</span>
                )}
              </button>
            );
          })}
          <button
            type="button"
            role="menuitem"
            className="master-picker__item master-picker__item--new"
            onClick={chooseNew}
          >
            + {MASTER_NEW_LABEL}
          </button>
        </Popover>
      )}
    </div>
  );
}

interface MasterWarningProps {
  implementation: ImplementationView;
  sessions: readonly SessionSummary[];
}

/** Warnleiste unter dem Kopf, solange der Master ausgefallen ist; „ändern“ steht im Kopf. */
export function MasterWarning({
  implementation,
  sessions,
}: MasterWarningProps): ReactElement | null {
  if (!implementation.masterMissing) {
    return null;
  }
  return (
    <div className="master-warning" role="alert">
      <span className="master-warning__icon">
        <SeverityIcon severity="critical" size={ICON_SIZE} />
      </span>
      <p className="master-warning__text">
        {masterMissingText(masterOf(implementation, sessions))}
      </p>
    </div>
  );
}
