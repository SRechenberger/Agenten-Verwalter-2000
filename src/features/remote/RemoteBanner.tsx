import type { ReactElement } from 'react';
import { EndShareButton } from '@/features/remote/EndShareButton';
import { RemoteMark } from '@/features/remote/RemoteMark';
import { formatReserved } from '@/features/remote/projectAccess';
import {
  useProjectLock,
  useProjectName,
  useReservedLeft,
} from '@/features/remote/useProjectAccess';
import './RemoteProject.css';

interface RemoteBannerProps {
  projectId: string;
}

/** Am Host, solange die Verbindung des Gastes abgerissen ist: Das Vorhaben bleibt noch eine Weile für ihn reserviert. */
export function RemoteBanner({ projectId }: RemoteBannerProps): ReactElement | null {
  const lock = useProjectLock(projectId);
  const reservedLeft: number | null = useReservedLeft(lock);
  const projectName: string = useProjectName(projectId);
  if (lock?.kind !== 'guest' || lock.lostSince === null) {
    return null;
  }
  return (
    <div className="remote-banner" role="status">
      <RemoteMark />
      <span>
        Verbindung zu <b>{lock.device.name}</b> unterbrochen.
        {reservedLeft !== null &&
          ` Reserviert noch ${formatReserved(reservedLeft)}, danach kannst du wieder schreiben.`}
      </span>
      <EndShareButton
        projectId={projectId}
        projectName={projectName}
        holder={lock.device}
        className="remote-lock__button"
      />
    </div>
  );
}
