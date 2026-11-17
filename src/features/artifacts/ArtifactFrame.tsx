import { useEffect, useRef } from 'react';
import type { ReactElement, RefObject } from 'react';
import { openUrl } from '@tauri-apps/plugin-opener';
import type { Artifact } from '@/lib/bindings/Artifact';
import { artifactUrl } from '@/lib/artifacts';
import { parseOpenRequest } from '@/features/artifacts/artifactBridge';

interface ArtifactFrameProps {
  /** `baseUrl` der Liste. */
  baseUrl: string;
  /** Nur Artefakte mit `viewable`; die übrigen zeigt die Rest-Karte. */
  item: Artifact;
  /** Erhöht sich bei „Neu laden“; neuer Schlüssel = neues iframe. */
  reloadToken: number;
  /** Vorschau und Vollbild gestalten den Rahmen verschieden. */
  className?: string;
  frameRef?: RefObject<HTMLIFrameElement | null>;
}

// Ohne `allow-same-origin`, `allow-popups` und `allow-top-navigation`: die Seite läuft als eigene, leere
// Herkunft und erreicht weder den Verwalter noch andere Fenster. Kein `srcdoc` — das erbte die App-CSP.
// Betrachter-Seiten (ADR 047) laufen im selben Rahmen wie HTML.
const FRAME_SANDBOX = 'allow-scripts allow-forms allow-modals';

// Die Seite kann die Nachricht auch ohne Klick schicken; mehr als ein Browser-Tab je Sekunde wäre Spam.
const OPEN_INTERVAL_MS = 1000;

export function ArtifactFrame({
  baseUrl,
  item,
  reloadToken,
  className = 'artifacts__frame',
  frameRef,
}: ArtifactFrameProps): ReactElement {
  const ownFrame = useRef<HTMLIFrameElement>(null);
  const frame: RefObject<HTMLIFrameElement | null> = frameRef ?? ownFrame;
  const lastOpenedAt = useRef<number>(0);

  // Links nach außen meldet die Brücke im Rahmen (`bridge.js`); geöffnet wird im Standardbrowser.
  useEffect(() => {
    function openLink(event: MessageEvent<unknown>): void {
      if (event.source === null || event.source !== frame.current?.contentWindow) return;
      const url: string | null = parseOpenRequest(event.data);
      const now: number = Date.now();
      if (url === null || now - lastOpenedAt.current < OPEN_INTERVAL_MS) return;
      lastOpenedAt.current = now;
      openUrl(url).catch((error: unknown): void => {
        console.error('Link aus dem Artefakt nicht geöffnet', error);
      });
    }
    window.addEventListener('message', openLink);
    return (): void => {
      window.removeEventListener('message', openLink);
    };
  }, [frame]);

  return (
    <iframe
      key={`${item.file}:${String(reloadToken)}`}
      ref={frame}
      className={className}
      src={artifactUrl(baseUrl, item)}
      sandbox={FRAME_SANDBOX}
      allow="fullscreen"
      title={item.title}
    />
  );
}
