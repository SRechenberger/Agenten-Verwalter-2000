import { useState } from 'react';
import type { ReactElement } from 'react';
import { EndShareDialog, GroupEndDialog } from '@/features/remote/EndShareDialog';
import { useViaGroup } from '@/features/remote/useProjectAccess';
import type { ProjectSummary } from '@/lib/bindings/ProjectSummary';
import { useProjectAccessStore } from '@/stores/projectAccess';
import type { DeviceRef } from '@/lib/bindings/DeviceRef';

interface EndShareButtonProps {
  projectId: string;
  projectName: string;
  holder: DeviceRef | null;
  className: string;
}

/** Der Knopf „Freigabe beenden“ samt seinem Dialog; `className` gibt der Stelle ihre Form. Gibt eine Gruppe das
 *  Vorhaben frei, beendet der Knopf nichts (G2): Er öffnet statt dessen den Dialog der Gruppe. */
export function EndShareButton({
  projectId,
  projectName,
  holder,
  className,
}: EndShareButtonProps): ReactElement {
  const [isOpen, setIsOpen] = useState<boolean>(false);
  const viaGroup = useViaGroup(projectId);
  const projects: readonly ProjectSummary[] = useProjectAccessStore((state) => state.projects);
  return (
    <>
      <button
        type="button"
        className={className}
        onClick={(): void => {
          setIsOpen(true);
        }}
      >
        {viaGroup === null ? 'Freigabe beenden' : `Gruppe „${viaGroup.name}“ bearbeiten …`}
      </button>
      {isOpen && viaGroup !== null && (
        <GroupEndDialog
          group={viaGroup}
          projects={projects}
          onClose={(): void => {
            setIsOpen(false);
          }}
        />
      )}
      {isOpen && viaGroup === null && (
        <EndShareDialog
          projectId={projectId}
          projectName={projectName}
          holder={holder}
          onClose={(): void => {
            setIsOpen(false);
          }}
        />
      )}
    </>
  );
}
