import type { ReactElement } from 'react';
import { RemoteMark } from '@/features/remote/RemoteMark';
import { useProjectAccess } from '@/features/remote/useProjectAccess';
import type { ProjectAccess } from '@/lib/bindings/ProjectAccess';
import { giveBackRemoteProject } from '@/lib/remote';
import { useActionError } from '@/lib/useActionError';
import { useSessionsStore } from '@/stores/sessions';
import './RemoteHeaderTools.css';

type RemoteAccess = Extract<ProjectAccess, { kind: 'remote' }>;

/** Der Chip mit dem Host in der Kopfzeile eines entfernten Vorhabens; ist der Host weg, wird er rot. */
function RemoteHostChip({ access }: { access: RemoteAccess }): ReactElement {
  if (!access.online) {
    return (
      <span className="remote-chip remote-chip--offline">
        <RemoteMark />
        <b>{access.hostName}</b> nicht erreichbar
      </span>
    );
  }
  return (
    <span className="remote-chip" title={`Dieses Vorhaben läuft auf ${access.hostName}`}>
      <RemoteMark />
      <b>{access.hostName}</b>
    </span>
  );
}

/** „Zurückgeben“: gibt das Vorhaben an den Host zurück. Die Freigabe bleibt, die Ansicht zeigt danach das Angebot. */
function GiveBackButton({ projectId }: { projectId: string }): ReactElement {
  const { error, run } = useActionError(projectId);
  const selectProject = useSessionsStore((state) => state.selectProject);
  return (
    <>
      {error !== null && (
        <span className="remote-chip__error" role="alert" title={error}>
          {error}
        </span>
      )}
      <button
        type="button"
        className="remote-give-back"
        title="Gibt das Vorhaben an den Host zurück. Die Freigabe bleibt, du kannst es wieder übernehmen."
        onClick={(): void => {
          run(async (): Promise<void> => {
            await giveBackRemoteProject(projectId);
            selectProject(projectId);
          }, 'Zurückgeben fehlgeschlagen');
        }}
      >
        Zurückgeben
      </button>
    </>
  );
}

interface RemoteHeaderToolsProps {
  projectId: string;
}

/** Links in den Werkzeugen der Kopfzeile eines entfernten Vorhabens: der Host. Bei eigenen Vorhaben nichts. */
export function RemoteHostBadge({ projectId }: RemoteHeaderToolsProps): ReactElement | null {
  const access: ProjectAccess = useProjectAccess(projectId);
  return access.kind === 'remote' ? <RemoteHostChip access={access} /> : null;
}

/** Ganz rechts in der Kopfzeile: „Zurückgeben“, solange das Vorhaben diesem Gerät gehört und der Host da ist. */
export function RemoteGiveBack({ projectId }: RemoteHeaderToolsProps): ReactElement | null {
  const access: ProjectAccess = useProjectAccess(projectId);
  return access.kind === 'remote' && access.mine && access.online ? (
    <GiveBackButton projectId={projectId} />
  ) : null;
}
