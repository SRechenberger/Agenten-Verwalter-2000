import type { ReactElement } from 'react';
import { abortLabel, FAILURE, LOCKED_TITLE, operationText } from '@/features/git/gitTexts';
import { useGitActions } from '@/features/git/useGitActions';
import { useSessionIsRemote } from '@/features/remote/useProjectAccess';
import type { GitEntryStatus } from '@/lib/bindings/GitEntryStatus';
import { gitAbortOperation, gitOpen } from '@/lib/git';
import './GitOperationRow.css';

interface GitOperationRowProps {
  sessionId: string;
  entry: GitEntryStatus;
  isBusy: boolean;
  onChanged: () => void;
}

/** Ein Merge oder Rebase, den Git mit Konflikten hat stehen lassen. Das Commit-Feld bleibt nutzbar: Konflikte
 *  lösen und committen beendet den Merge. */
export function GitOperationRow({
  sessionId,
  entry,
  isBusy,
  onChanged,
}: GitOperationRowProps): ReactElement {
  const { run, isRunning } = useGitActions(sessionId, onChanged);
  // VS Code ginge am Bildschirm des Hosts auf, nicht an diesem.
  const isRemote: boolean = useSessionIsRemote(sessionId);

  function abort(): void {
    run(() => gitAbortOperation(sessionId, entry.key), FAILURE.abort).catch(() => undefined);
  }

  function openInVsCode(): void {
    run(() => gitOpen(sessionId, entry.key, 'vsCode'), FAILURE.open).catch(() => undefined);
  }

  return (
    <div className="git-operation-row" role="status">
      <svg
        width="13"
        height="13"
        viewBox="0 0 16 16"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
        strokeLinejoin="round"
        aria-hidden="true"
      >
        <path d="M8 2 1.8 13h12.4Z" />
        <path d="M8 6.5v3M8 11.6v.1" />
      </svg>
      <div className="git-operation-row__text">
        {operationText(entry.operation, entry.conflicted.length)}
      </div>
      {!isRemote && (
        <button
          type="button"
          className="git-operation-row__open"
          disabled={isRunning}
          onClick={openInVsCode}
        >
          In VS Code öffnen
        </button>
      )}
      <button
        type="button"
        className="git-operation-row__abort"
        disabled={isBusy || isRunning}
        title={isBusy ? LOCKED_TITLE : undefined}
        onClick={abort}
      >
        {abortLabel(entry.operation)}
      </button>
    </div>
  );
}
