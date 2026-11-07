import type { ReactElement } from 'react';
import { EndShareButton } from '@/features/remote/EndShareButton';
import { RemoteMark } from '@/features/remote/RemoteMark';
import type { ProjectLock } from '@/features/remote/projectAccess';
import { useProjectName } from '@/features/remote/useProjectAccess';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import { useChatStore } from '@/stores/chat';
import './RemoteProject.css';

const DRAFT_PREVIEW_LENGTH = 80;

interface LockedComposerProps {
  session: SessionSummary;
  lock: ProjectLock;
}

/** Die Sperrzeile statt der Eingabeleiste. Am Host liest der Nutzer mit, solange ein Gast steuert; am
 *  Gast wartet die Eingabe auf den Host und der getippte Entwurf bleibt erhalten. */
export function LockedComposer({ session, lock }: LockedComposerProps): ReactElement {
  const draft: string = useChatStore((state) => state.drafts[session.id] ?? '').trim();
  const projectName: string = useProjectName(session.projectId);

  function renderText(): ReactElement {
    switch (lock.kind) {
      case 'guest':
        return (
          <span className="remote-lock__text">
            <RemoteMark className="remote-lock__mark" />{' '}
            {lock.lostSince === null ? 'Gesteuert von' : 'Reserviert für'} <b>{lock.device.name}</b>
            . Du liest mit.
          </span>
        );
      case 'hostOffline':
        return (
          <span className="remote-lock__text">
            Host nicht erreichbar. Der Agent läuft dort weiter, und der Verwalter verbindet sich von
            selbst neu.
          </span>
        );
      case 'notHeld':
        return (
          <span className="remote-lock__text">
            Dieses Vorhaben hält ein anderes Gerät. Antworten geht wieder, sobald du es übernommen
            hast.
          </span>
        );
    }
  }

  return (
    <div className="composer-lock">
      <div className="remote-lock">
        {renderText()}
        {lock.kind === 'guest' && (
          <EndShareButton
            projectId={session.projectId}
            projectName={projectName}
            holder={lock.device}
            className="remote-lock__button"
          />
        )}
      </div>
      {lock.kind === 'hostOffline' && draft !== '' && (
        <p className="remote-lock__draft">
          Dein Entwurf bleibt erhalten: „
          {draft.length > DRAFT_PREVIEW_LENGTH
            ? `${draft.slice(0, DRAFT_PREVIEW_LENGTH)} …`
            : draft}
          “
        </p>
      )}
    </div>
  );
}
