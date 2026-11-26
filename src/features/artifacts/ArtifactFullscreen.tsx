import { getCurrentWindow } from '@tauri-apps/api/window';
import { useCallback, useEffect, useRef, useState } from 'react';
import type { ReactElement } from 'react';
import { createPortal } from 'react-dom';
import { ArtifactFrame } from '@/features/artifacts/ArtifactFrame';
import type { Artifact } from '@/lib/bindings/Artifact';
import './ArtifactFullscreen.css';

const EXIT_VISIBLE_MS = 2000;

interface ArtifactFullscreenProps {
  sessionId: string;
  /** `baseUrl` der Liste. */
  baseUrl: string;
  item: Artifact;
  onClose: () => void;
}

export function ArtifactFullscreen({
  sessionId,
  baseUrl,
  item,
  onClose,
}: ArtifactFullscreenProps): ReactElement {
  const [isExitVisible, setIsExitVisible] = useState<boolean>(true);
  const hideTimer = useRef<number | undefined>(undefined);

  const startHideTimer = useCallback((): void => {
    window.clearTimeout(hideTimer.current);
    hideTimer.current = window.setTimeout(() => {
      setIsExitVisible(false);
    }, EXIT_VISIBLE_MS);
  }, []);

  const showExit = useCallback((): void => {
    setIsExitVisible(true);
    startHideTimer();
  }, [startHideTimer]);

  // Ohne Fenster-Vollbild bleibt das Overlay nutzbar, darum nur Konsole.
  useEffect(() => {
    getCurrentWindow()
      .setFullscreen(true)
      .catch((reason: unknown) => {
        console.error('Vollbild nicht gesetzt', reason);
      });
    return (): void => {
      getCurrentWindow()
        .setFullscreen(false)
        .catch((reason: unknown) => {
          console.error('Vollbild nicht beendet', reason);
        });
    };
  }, []);

  // Der Knopf ist beim Öffnen sichtbar (Startzustand) und verschwindet nach der ersten Frist.
  useEffect(() => {
    startHideTimer();
    return (): void => {
      window.clearTimeout(hideTimer.current);
    };
  }, [startHideTimer]);

  // Fokus im Artefakt erreicht dieses Fenster nicht: Tasten dort gehören der Seite.
  useEffect(() => {
    function closeOnEscape(event: KeyboardEvent): void {
      if (event.key === 'Escape') {
        onClose();
      }
    }
    window.addEventListener('keydown', closeOnEscape);
    return (): void => {
      window.removeEventListener('keydown', closeOnEscape);
    };
  }, [onClose]);

  return createPortal(
    <div className="artifact-fullscreen">
      <ArtifactFrame
        key={item.file}
        sessionId={sessionId}
        baseUrl={baseUrl}
        item={item}
        reloadToken={0}
        className="artifact-fullscreen__frame"
      />
      <div
        className="artifact-fullscreen__edge"
        onMouseEnter={showExit}
        onMouseMove={showExit}
        aria-hidden="true"
      />
      <button
        type="button"
        className={`artifact-fullscreen__exit${isExitVisible ? '' : ' artifact-fullscreen__exit--hidden'}`}
        onClick={onClose}
      >
        Vollbild beenden
      </button>
    </div>,
    document.body,
  );
}
