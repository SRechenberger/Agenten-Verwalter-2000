import { useState } from 'react';
import type { ReactElement } from 'react';
import { EndShareDialog } from '@/features/remote/EndShareDialog';
import type { DeviceRef } from '@/lib/bindings/DeviceRef';

interface EndShareButtonProps {
  projectId: string;
  projectName: string;
  holder: DeviceRef | null;
  className: string;
}

/** Der Knopf „Freigabe beenden“ samt seinem Dialog; `className` gibt der Stelle ihre Form. */
export function EndShareButton({
  projectId,
  projectName,
  holder,
  className,
}: EndShareButtonProps): ReactElement {
  const [isOpen, setIsOpen] = useState<boolean>(false);
  return (
    <>
      <button
        type="button"
        className={className}
        onClick={(): void => {
          setIsOpen(true);
        }}
      >
        Freigabe beenden
      </button>
      {isOpen && (
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
