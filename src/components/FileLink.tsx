import type { MouseEvent, ReactElement, ReactNode } from 'react';
import { commandErrorText } from '@/lib/errors';
import { fileHrefOf, openFileLink } from '@/lib/fileLinks';
import { useSessionErrorsStore } from '@/stores/sessionErrors';
import './FileLink.css';

interface FileLinkProps {
  sessionId: string;
  path: string;
  children: ReactNode;
}

/** Dateiverweis im Chat: öffnet die Datei mit dem Standardprogramm. Ob sie existiert und geöffnet
 *  werden darf, entscheidet erst der Core beim Klick. */
export function FileLink({ sessionId, path, children }: FileLinkProps): ReactElement {
  const reportSessionError = useSessionErrorsStore((state) => state.report);
  const clearSessionError = useSessionErrorsStore((state) => state.clear);

  function open(event: MouseEvent<HTMLAnchorElement>): void {
    // Ohne preventDefault würde das App-Fenster selbst navigieren.
    event.preventDefault();
    openFileLink(sessionId, path)
      .then(() => {
        clearSessionError(sessionId);
      })
      .catch((reason: unknown) => {
        console.error('Datei nicht geöffnet', reason);
        reportSessionError(sessionId, `Datei nicht geöffnet: ${commandErrorText(reason)}`);
      });
  }

  const href: string | undefined = fileHrefOf(path);
  return (
    <a
      className="file-link"
      href={href}
      role={href === undefined ? 'link' : undefined}
      tabIndex={href === undefined ? 0 : undefined}
      title={`Öffnen: ${path}`}
      onClick={open}
      onAuxClick={open}
      onKeyDown={(event) => {
        if (href === undefined && event.key === 'Enter') {
          event.currentTarget.click();
        }
      }}
    >
      {children}
    </a>
  );
}
