import { useMemo } from 'react';
import type { ReactElement } from 'react';
import {
  buildFileRows,
  foreignLineStat,
  type FileRow,
  type GitRowsInput,
} from '@/features/changes/buildFileRows';
import { ResizeHandle } from '@/components/ResizeHandle';
import { ChangesOverview } from '@/features/changes/ChangesOverview';
import { ChangesToolbar } from '@/features/changes/ChangesToolbar';
import { statOf } from '@/features/changes/changesScope';
import { DiffView } from '@/features/changes/DiffView';
import { FileTree } from '@/features/changes/FileTree';
import { fileDiffKey } from '@/features/changes/useFileDiff';
import type { ChangeScope } from '@/lib/bindings/ChangeScope';
import type { ChangesReach } from '@/lib/bindings/ChangesReach';
import type { FileChange } from '@/lib/bindings/FileChange';
import type { GitEntryStatus } from '@/lib/bindings/GitEntryStatus';
import type { GitForeignFile } from '@/lib/bindings/GitForeignFile';
import type { GitSessionStatus } from '@/lib/bindings/GitSessionStatus';
import type { LineStat } from '@/lib/bindings/LineStat';
import type { RepositoryChanges } from '@/lib/bindings/RepositoryChanges';
import type { SessionChanges } from '@/lib/bindings/SessionChanges';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import { usePanelWidth } from '@/lib/usePanelWidth';
import {
  DEFAULT_SELECTION,
  useChangesStore,
  type ChangesSelection,
  type OpenFile,
} from '@/stores/changes';
import { useGitStore } from '@/stores/git';
import './ChangesView.css';

interface ChangesViewProps {
  session: SessionSummary;
  changes: SessionChanges | null;
  error: string | null;
  reach: ChangesReach;
  /** Git-Zustand der Session; `null` in der Reichweite Projekt (dort gibt es keine Git-Bedienung) und solange er
   *  nicht gelesen ist. */
  git: GitSessionStatus | null;
  /** Lädt nach einer Git-Aktion Git-Zustand und Changes neu. */
  onChanged: () => void;
}

interface OpenTarget {
  repository: RepositoryChanges;
  file: FileChange;
  /** Blickwinkel, in dem der Diff der Datei zeigt: der der Zeile, sonst der gewählte. */
  scope: ChangeScope;
}

const NO_FLAGS: Readonly<Record<string, boolean>> = {};

/** Eine fremde Datei hat keinen Eintrag in den Changes; für den Diff reicht ein Stand unter „Uncommitted“. */
function foreignChange(file: GitForeignFile): FileChange {
  const stat: LineStat = foreignLineStat(file);
  return { path: file.path, all: stat, committed: null, uncommitted: stat };
}

/** Die geöffnete Datei, sofern sie in ihrem Blickwinkel noch existiert; sonst zeigt die Ansicht die Übersicht. */
function findOpenTarget(
  changes: SessionChanges,
  git: GitSessionStatus | null,
  openFile: OpenFile | null,
  selectedScope: ChangeScope,
): OpenTarget | null {
  if (openFile === null) {
    return null;
  }
  const repository: RepositoryChanges | undefined = changes.repositories.find(
    (candidate: RepositoryChanges) => candidate.key === openFile.key,
  );
  if (repository === undefined) {
    return null;
  }
  const scope: ChangeScope = openFile.scope ?? selectedScope;
  const file: FileChange | undefined = repository.files.find(
    (candidate: FileChange) => candidate.path === openFile.path,
  );
  if (file !== undefined && statOf(file, scope) !== null) {
    return { repository, file, scope };
  }
  const foreign: GitForeignFile | undefined = git?.entries
    .find((entry: GitEntryStatus) => entry.key === openFile.key)
    ?.foreign.find((candidate: GitForeignFile) => candidate.path === openFile.path);
  if (foreign !== undefined && scope === 'uncommitted') {
    return { repository, file: foreignChange(foreign), scope };
  }
  return null;
}

export function ChangesView({
  session,
  changes,
  error,
  reach,
  git,
  onChanged,
}: ChangesViewProps): ReactElement {
  const sessionId: string = session.id;
  const treeWidth = usePanelWidth('changesTree');
  const selection: ChangesSelection = useChangesStore(
    (state) => state.selections[sessionId] ?? DEFAULT_SELECTION,
  );
  const setRepositoryFilter = useChangesStore((state) => state.setRepositoryFilter);
  const setScope = useChangesStore((state) => state.setScope);
  const openFile = useChangesStore((state) => state.openFile);
  const closeFile = useChangesStore((state) => state.closeFile);
  const open: Readonly<Record<string, boolean>> = useGitStore(
    (state) => state.sessions[sessionId]?.open ?? NO_FLAGS,
  );
  const showForeign: Readonly<Record<string, boolean>> = useGitStore(
    (state) => state.sessions[sessionId]?.showForeign ?? NO_FLAGS,
  );

  // Git-Bedienung gibt es nur in der Reichweite Session.
  const gitStatus: GitSessionStatus | null = reach === 'session' ? git : null;

  const rows: FileRow[] = useMemo(() => {
    if (changes === null) {
      return [];
    }
    const gitInput: GitRowsInput | null =
      gitStatus === null ? null : { status: gitStatus, open, showForeign };
    return buildFileRows(changes, selection.repositoryFilter, selection.scope, gitInput);
  }, [changes, gitStatus, open, showForeign, selection.repositoryFilter, selection.scope]);

  const openTarget: OpenTarget | null = useMemo(
    () =>
      changes === null
        ? null
        : findOpenTarget(changes, gitStatus, selection.openFile, selection.scope),
    [changes, gitStatus, selection.openFile, selection.scope],
  );

  if (changes === null) {
    return <p className="changes-view__notice">{error ?? 'Changes werden gelesen …'}</p>;
  }

  return (
    <div className="changes-view">
      <ChangesToolbar
        changes={changes}
        selection={selection}
        onRepositoryFilter={(key: string | null): void => {
          setRepositoryFilter(sessionId, key);
        }}
        onScope={(scope: ChangeScope): void => {
          setScope(sessionId, scope);
        }}
      />
      <div className="changes-view__body">
        <FileTree
          width={treeWidth.width}
          rows={rows}
          scope={selection.scope}
          openFile={openTarget === null ? null : selection.openFile}
          onOpen={(file): void => {
            openFile(sessionId, file);
          }}
          git={gitStatus === null ? null : { sessionId, busy: gitStatus.busy, onChanged }}
        />
        <ResizeHandle edge="end" label="Breite der Dateiliste" {...treeWidth.handleProps} />
        <div className="changes-view__detail" data-resize-fill>
          {openTarget === null ? (
            <ChangesOverview
              changes={changes}
              reach={reach}
              untrackedBefore={changes.untrackedBefore}
              scope={selection.scope}
              hasVisibleFiles={rows.some(
                (row: FileRow) => row.kind === 'file' || row.kind === 'commitFile',
              )}
              git={
                gitStatus === null
                  ? null
                  : {
                      sessionId,
                      status: gitStatus,
                      repositoryFilter: selection.repositoryFilter,
                      onChanged,
                    }
              }
            />
          ) : (
            <DiffView
              key={fileDiffKey(
                { key: openTarget.repository.key, path: openTarget.file.path },
                openTarget.scope,
              )}
              sessionId={sessionId}
              reach={reach}
              repository={openTarget.repository}
              file={openTarget.file}
              scope={openTarget.scope}
              onClose={(): void => {
                closeFile(sessionId);
              }}
            />
          )}
        </div>
      </div>
    </div>
  );
}
