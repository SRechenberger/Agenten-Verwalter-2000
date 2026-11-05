import { useRef } from 'react';
import type { ChangeEvent, ReactElement } from 'react';
import { useVirtualizer, type VirtualItem } from '@tanstack/react-virtual';
import type { FileRow } from '@/features/changes/buildFileRows';
import { EMPTY_SCOPE_TEXT, formatCount } from '@/features/changes/changesScope';
import { GitCommitBox } from '@/features/git/GitCommitBox';
import { GitDiscardButton } from '@/features/git/GitDiscardButton';
import { GitEntryBar } from '@/features/git/GitEntryBar';
import { CaretIcon } from '@/features/git/GitIcons';
import { GitLockHint } from '@/features/git/GitLockHint';
import { GitOperationRow } from '@/features/git/GitOperationRow';
import type { ChangeKind } from '@/lib/bindings/ChangeKind';
import type { ChangeScope } from '@/lib/bindings/ChangeScope';
import type { GitBusySession } from '@/lib/bindings/GitBusySession';
import type { OpenFile } from '@/stores/changes';
import { isPathChecked, useGitStore, type GitSessionUi } from '@/stores/git';
import './FileTree.css';

const OVERSCAN = 12;
const FILE_INDENT_BASE = 30;
const FILE_INDENT_STEP = 14;
const ESTIMATED_HEIGHT: Record<FileRow['kind'], number> = {
  repository: 36,
  branch: 19,
  error: 19,
  folder: 21,
  file: 23,
  operation: 40,
  commitBox: 76,
  group: 28,
  note: 24,
  foreignToggle: 34,
  commit: 22,
  commitFile: 24,
};
const FIRST_REPOSITORY_HEIGHT = 26;
const OPEN_REPOSITORY_HEIGHT = 58;

const KIND_LETTER: Record<ChangeKind, { letter: string; title: string }> = {
  added: { letter: 'A', title: 'Neu angelegt' },
  modified: { letter: 'M', title: 'Geändert' },
  deleted: { letter: 'D', title: 'Gelöscht' },
};

const CHECKED_TITLE = 'Geht in den nächsten Commit';
const UNCHECKED_TITLE = 'Bleibt beim Commit draußen';

/** Was die Git-Bedienung im Baum braucht; `null` in der Reichweite Projekt: dort ist der Baum nur lesend. */
export interface FileTreeGit {
  sessionId: string;
  busy: readonly GitBusySession[];
  /** Lädt Git-Zustand und Changes neu. */
  onChanged: () => void;
}

interface FileTreeProps {
  /** Gezogene Breite in Pixeln. */
  width: number;
  rows: readonly FileRow[];
  scope: ChangeScope;
  openFile: OpenFile | null;
  onOpen: (file: OpenFile) => void;
  git: FileTreeGit | null;
}

type FileRowOf<Kind extends FileRow['kind']> = Extract<FileRow, { kind: Kind }>;

export function FileTree({
  width,
  rows,
  scope,
  openFile,
  onOpen,
  git,
}: FileTreeProps): ReactElement {
  const scrollRef = useRef<HTMLDivElement>(null);
  const sessionId: string | null = git === null ? null : git.sessionId;
  const ui: GitSessionUi | undefined = useGitStore((state) =>
    sessionId === null ? undefined : state.sessions[sessionId],
  );
  const setChecked = useGitStore((state) => state.setChecked);
  const setAllOwn = useGitStore((state) => state.setAllOwn);
  const toggleForeign = useGitStore((state) => state.toggleForeign);
  const toggleOpen = useGitStore((state) => state.toggleOpen);
  const isBusy: boolean = git !== null && git.busy.length > 0;

  // Die Warnung gilt dem React Compiler, den das Projekt nicht nutzt; die Bibliothek schreibt ADR 002 vor.
  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: (): HTMLDivElement | null => scrollRef.current,
    estimateSize: (index: number): number => estimateHeight(rows[index]),
    getItemKey: (index: number): string => rows[index]?.key ?? String(index),
    overscan: OVERSCAN,
  });

  function renderRow(row: FileRow | undefined): ReactElement | null {
    if (row === undefined) {
      return null;
    }
    switch (row.kind) {
      case 'repository':
        return renderRepository(row);
      case 'branch':
        return <div className="file-tree__branch">{row.branch}</div>;
      case 'error':
        return <div className="file-tree__error">{row.message}</div>;
      case 'folder':
        return (
          <div
            className="file-tree__folder"
            style={{ paddingLeft: `${String(FILE_INDENT_BASE + row.depth * FILE_INDENT_STEP)}px` }}
          >
            {row.label}
          </div>
        );
      case 'file':
        return renderFile(row);
      case 'operation':
        return git === null ? null : (
          <GitOperationRow
            sessionId={git.sessionId}
            entry={row.entry}
            isBusy={isBusy}
            onChanged={git.onChanged}
          />
        );
      case 'commitBox':
        return git === null ? null : (
          <GitCommitBox
            sessionId={git.sessionId}
            entry={row.entry}
            ownPaths={row.ownPaths}
            foreignPaths={row.foreignPaths}
            isBusy={isBusy}
            onChanged={git.onChanged}
          />
        );
      case 'group':
        return renderGroup(row);
      case 'note':
        return <p className="file-tree__note">{row.text}</p>;
      case 'foreignToggle':
        return renderForeignToggle(row);
      case 'commit':
        return renderCommit(row);
      case 'commitFile':
        return renderCommitFile(row);
    }
  }

  function renderRepository(row: FileRowOf<'repository'>): ReactElement {
    const spacing: string = row.isFirst ? '' : ' file-tree__repository--spaced';
    const rowGit = row.git;
    const isOpen: boolean = rowGit !== null && rowGit.isOpen;
    return (
      <div className={`file-tree__repository-block${spacing}`}>
        <div className="file-tree__repository">
          <svg
            width="14"
            height="14"
            viewBox="0 0 14 14"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.4"
            strokeLinejoin="round"
            aria-hidden="true"
            className="file-tree__repository-icon"
          >
            <path d="M2 4.2a1 1 0 0 1 1-1h2.6l1.2 1.3H11a1 1 0 0 1 1 1V10a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1Z" />
          </svg>
          <span className="file-tree__repository-name" title={rowGit?.kindLabel}>
            {row.name}
          </span>
          <span className="file-tree__added">+{formatCount(row.added)}</span>
          <span className="file-tree__deleted">−{formatCount(row.deleted)}</span>
          {rowGit !== null && git !== null && (
            <button
              type="button"
              className={`file-tree__caret${isOpen ? ' file-tree__caret--open' : ''}`}
              aria-expanded={isOpen}
              title="Git-Werkzeuge"
              aria-label={`Git-Werkzeuge für ${row.name} ${isOpen ? 'zuklappen' : 'aufklappen'}`}
              onClick={(): void => {
                toggleOpen(git.sessionId, row.entryKey);
              }}
            >
              <CaretIcon />
            </button>
          )}
        </div>
        {isOpen && rowGit !== null && git !== null && (
          <div className="file-tree__git-bar">
            <GitEntryBar
              sessionId={git.sessionId}
              entry={rowGit.entry}
              entryName={row.name}
              dirtyPaths={rowGit.dirtyPaths}
              isBusy={isBusy}
              onChanged={git.onChanged}
            />
          </div>
        )}
      </div>
    );
  }

  function isOpenFile(entryKey: string, path: string, fileScope: ChangeScope | null): boolean {
    return (
      openFile !== null &&
      openFile.key === entryKey &&
      openFile.path === path &&
      openFile.scope === fileScope
    );
  }

  function renderFile(row: FileRowOf<'file'>): ReactElement {
    const isOpen: boolean = isOpenFile(row.entryKey, row.path, row.scope);
    const { letter, title } = KIND_LETTER[row.stat.kind];
    const indent: string = `${String(FILE_INDENT_BASE + row.depth * FILE_INDENT_STEP)}px`;
    const isForeign: boolean = row.checkable === 'foreign';
    const button: ReactElement = (
      <button
        type="button"
        className={`file-tree__file${isOpen ? ' file-tree__file--selected' : ''}${
          row.checkable === null ? '' : ' file-tree__file--inline'
        }${isForeign ? ' file-tree__file--foreign' : ''}`}
        style={row.checkable === null ? { paddingLeft: indent } : undefined}
        title={isForeign ? `${row.path} — nicht von dieser Session` : undefined}
        onClick={(): void => {
          onOpen({ key: row.entryKey, path: row.path, scope: row.scope });
        }}
      >
        <span className={`file-tree__letter file-tree__letter--${row.stat.kind}`} title={title}>
          {letter}
        </span>
        <span className="file-tree__name">{row.label}</span>
        {row.isUncommitted && (
          <span
            className="file-tree__badge"
            title="Uncommitted – nur im Arbeitsverzeichnis, noch nicht committed"
          >
            uncommitted
          </span>
        )}
        <span className="file-tree__stats">
          {row.stat.binary ? (
            <span className="file-tree__binary" title="Binärdatei – keine Zeilenzahl">
              binär
            </span>
          ) : (
            <>
              <span className="file-tree__added">+{formatCount(row.stat.added)}</span>
              <span className="file-tree__deleted">−{formatCount(row.stat.deleted)}</span>
            </>
          )}
        </span>
      </button>
    );
    if (row.checkable === null || git === null) {
      return button;
    }
    const isChecked: boolean = isPathChecked(ui, row.entryKey, row.path, row.checkable === 'own');
    return (
      <div className="file-tree__file-row" style={{ paddingLeft: indent }}>
        <input
          type="checkbox"
          className="file-tree__check"
          checked={isChecked}
          title={isChecked ? CHECKED_TITLE : UNCHECKED_TITLE}
          aria-label={`${row.label}: ${isChecked ? CHECKED_TITLE : UNCHECKED_TITLE}`}
          onChange={(event: ChangeEvent<HTMLInputElement>): void => {
            setChecked(
              git.sessionId,
              row.entryKey,
              row.path,
              event.target.checked,
              row.checkable === 'own',
            );
          }}
        />
        {button}
        <GitDiscardButton
          sessionId={git.sessionId}
          entryKey={row.entryKey}
          path={row.path}
          isBusy={isBusy}
          onChanged={git.onChanged}
        />
      </div>
    );
  }

  function renderGroup(row: FileRowOf<'group'>): ReactElement {
    if (row.group === 'committed') {
      return (
        <div className="file-tree__group">
          <span>Committed</span>
          <span className="file-tree__group-count">
            {row.count === 1 ? '1 Commit' : `${formatCount(row.count)} Commits`}
          </span>
          <span className="file-tree__group-spacer" />
          {row.hasUnpushed && <span className="file-tree__group-hint">noch nicht gepusht</span>}
        </div>
      );
    }
    const allChecked: boolean = row.ownPaths.every((path: string) =>
      isPathChecked(ui, row.entryKey, path, true),
    );
    return (
      <div className="file-tree__group">
        <span>Uncommitted</span>
        <span className="file-tree__group-count">{formatCount(row.count)}</span>
        <span className="file-tree__group-spacer" />
        {row.ownPaths.length > 0 && git !== null && (
          <button
            type="button"
            className="file-tree__group-link"
            onClick={(): void => {
              setAllOwn(git.sessionId, row.entryKey, row.ownPaths, !allChecked);
            }}
          >
            {allChecked ? 'keine vormerken' : 'alle vormerken'}
          </button>
        )}
      </div>
    );
  }

  function renderForeignToggle(row: FileRowOf<'foreignToggle'>): ReactElement {
    const changes: string =
      row.count === 1 ? '1 weitere Änderung' : `${formatCount(row.count)} weitere Änderungen`;
    return (
      <button
        type="button"
        className="file-tree__foreign-toggle"
        aria-expanded={row.isOpen}
        onClick={(): void => {
          if (git !== null) {
            toggleForeign(git.sessionId, row.entryKey);
          }
        }}
      >
        <span
          className={`file-tree__foreign-caret${row.isOpen ? '' : ' file-tree__foreign-caret--closed'}`}
        >
          <CaretIcon />
        </span>
        {row.isOpen ? 'Ausblenden: ' : ''}
        {changes} im Ordner — nicht von dieser Session
      </button>
    );
  }

  function renderCommit(row: FileRowOf<'commit'>): ReactElement {
    const { commit } = row;
    return (
      <div
        className="file-tree__commit"
        title={commit.pushed ? commit.subject : `${commit.subject} — noch nicht gepusht`}
      >
        <span
          className={`file-tree__commit-dot${commit.pushed ? '' : ' file-tree__commit-dot--unpushed'}`}
        />
        <span className="file-tree__commit-subject">{commit.subject}</span>
        <span className="file-tree__commit-id">{commit.shortId}</span>
      </div>
    );
  }

  function renderCommitFile(row: FileRowOf<'commitFile'>): ReactElement {
    const isOpen: boolean = isOpenFile(row.entryKey, row.path, 'committed');
    const kind: { letter: string; title: string } | null =
      row.change === null ? null : KIND_LETTER[row.change];
    return (
      <button
        type="button"
        className={`file-tree__file file-tree__file--commit${isOpen ? ' file-tree__file--selected' : ''}`}
        title={row.path}
        onClick={(): void => {
          onOpen({ key: row.entryKey, path: row.path, scope: 'committed' });
        }}
      >
        <span
          className={`file-tree__letter${row.change === null ? '' : ` file-tree__letter--${row.change}`}`}
          title={kind?.title}
        >
          {kind?.letter}
        </span>
        <span className="file-tree__commit-file-name">{row.name}</span>
        <span className="file-tree__commit-file-directory">{row.directory}</span>
      </button>
    );
  }

  return (
    <div ref={scrollRef} className="file-tree" style={{ width }}>
      {git !== null && <GitLockHint sessionId={git.sessionId} busy={git.busy} />}
      {rows.length === 0 ? (
        <p className="file-tree__empty">{EMPTY_SCOPE_TEXT[scope]}</p>
      ) : (
        <div
          className="file-tree__surface"
          style={{ height: `${String(virtualizer.getTotalSize())}px` }}
        >
          {virtualizer.getVirtualItems().map((item: VirtualItem) => (
            <div
              key={item.key}
              ref={virtualizer.measureElement}
              data-index={item.index}
              className="file-tree__item"
              style={{ transform: `translateY(${String(item.start)}px)` }}
            >
              {renderRow(rows[item.index])}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function estimateHeight(row: FileRow | undefined): number {
  if (row === undefined) {
    return ESTIMATED_HEIGHT.file;
  }
  if (row.kind === 'repository') {
    if (row.git?.isOpen === true) {
      return OPEN_REPOSITORY_HEIGHT;
    }
    if (row.isFirst) {
      return FIRST_REPOSITORY_HEIGHT;
    }
  }
  return ESTIMATED_HEIGHT[row.kind];
}
