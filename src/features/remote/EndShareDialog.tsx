import { useState } from 'react';
import type { ReactElement } from 'react';
import { Dialog } from '@/components/Dialog';
import type { DeviceRef } from '@/lib/bindings/DeviceRef';
import { groupEndText, heldIn } from '@/features/remote/groupShareText';
import type { ProjectGroup } from '@/lib/bindings/ProjectGroup';
import type { ProjectSummary } from '@/lib/bindings/ProjectSummary';
import { commandErrorText } from '@/lib/errors';
import { setGroupShared, setProjectShared } from '@/lib/remote';
import { useRemoteStore } from '@/stores/remote';
import './RemoteDialog.css';

const TITLE_ID = 'remote-end-share-title';

interface EndShareDialogProps {
  projectId: string;
  projectName: string;
  /** Das Gerät, das das Vorhaben gerade hält; `null`, solange keins es hält. */
  holder: DeviceRef | null;
  onClose: () => void;
}

/** „Freigabe beenden?“ für ein Vorhaben. */
export function EndShareDialog({
  projectId,
  projectName,
  holder,
  onClose,
}: EndShareDialogProps): ReactElement {
  return (
    <EndShareConfirm
      text={
        holder === null
          ? `Kein Gerät kann „${projectName}“ danach übernehmen. Terminals laufen weiter.`
          : `${holder.name} verliert sofort die Kontrolle über „${projectName}“ und kann es nicht wieder übernehmen. Offene Rückfragen bleiben offen und gehen an dich, Terminals laufen weiter.`
      }
      onEnd={(stopAgents: boolean): Promise<unknown> =>
        setProjectShared(projectId, false, stopAgents)
      }
      onClose={onClose}
    />
  );
}

/** „Freigabe beenden?“ für eine ganze Gruppe: der eine Weg, den die Übersicht, der Chat und das Menü teilen. */
export function GroupEndDialog({
  group,
  projects,
  onClose,
}: {
  group: ProjectGroup;
  projects: readonly ProjectSummary[];
  onClose: () => void;
}): ReactElement {
  const ownSharedIds: string[] = useRemoteStore((state) => state.ownSharedIds);
  return (
    <EndShareConfirm
      text={groupEndText(group, heldIn(group, projects, ownSharedIds), 'end')}
      onEnd={(stopAgents: boolean): Promise<unknown> => setGroupShared(group.id, false, stopAgents)}
      onClose={onClose}
    />
  );
}

interface EndShareConfirmProps {
  /** Was das Beenden bewirkt, mit den Namen der Vorhaben und der Geräte, die sie verlieren. */
  text: string;
  /** Führt das Beenden aus, mit oder ohne Anhalten der Agenten; scheitert es, bleibt der Dialog offen. */
  onEnd: (stopAgents: boolean) => Promise<unknown>;
  /** Der bestätigende Knopf; „Freigabe beenden“, wenn nichts anderes passt. */
  confirmLabel?: string | undefined;
  onClose: () => void;
}

/** „Freigabe beenden?“: wirkt sofort und ohne Netz, wahlweise mit Anhalten der laufenden Agenten (Host).
 *  Auch das Herausnehmen aus einer freigegebenen Gruppe und das Löschen der Gruppe fragen so. */
export function EndShareConfirm({
  text,
  onEnd,
  confirmLabel = 'Freigabe beenden',
  onClose,
}: EndShareConfirmProps): ReactElement {
  const [stopAgents, setStopAgents] = useState<boolean>(false);
  const [isEnding, setIsEnding] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);

  function end(): void {
    setIsEnding(true);
    setError(null);
    onEnd(stopAgents)
      .then(onClose)
      .catch((reason: unknown) => {
        console.error('Freigabe nicht beendet', reason);
        setError(`Freigabe nicht beendet: ${commandErrorText(reason)}`);
        setIsEnding(false);
      });
  }

  return (
    <Dialog labelledBy={TITLE_ID} className="remote-dialog" onClose={onClose}>
      <h3 id={TITLE_ID} className="remote-dialog__title">
        Freigabe beenden?
      </h3>
      <p className="remote-dialog__text">{text}</p>
      <label className="remote-dialog__check">
        <input
          type="checkbox"
          checked={stopAgents}
          onChange={(): void => {
            setStopAgents((current: boolean) => !current);
          }}
        />
        Laufende Agenten anhalten
      </label>
      <p className="remote-dialog__note">
        Ohne den Haken arbeiten die Agenten weiter. Das Gerät bleibt gekoppelt; sperren kannst du es
        in den Einstellungen.
      </p>
      {error !== null && (
        <p className="remote-dialog__error" role="alert">
          {error}
        </p>
      )}
      <div className="remote-dialog__actions">
        <button type="button" className="remote-dialog__button" onClick={onClose}>
          Abbrechen
        </button>
        <button
          type="button"
          className="remote-dialog__button remote-dialog__button--primary"
          disabled={isEnding}
          onClick={end}
        >
          {confirmLabel}
        </button>
      </div>
    </Dialog>
  );
}
