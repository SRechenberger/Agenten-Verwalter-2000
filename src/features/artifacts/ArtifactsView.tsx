import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import type { ReactElement } from 'react';
import { ArtifactFrame } from '@/features/artifacts/ArtifactFrame';
import { ArtifactFullscreen } from '@/features/artifacts/ArtifactFullscreen';
import { ArtifactGroupHeader } from '@/features/artifacts/ArtifactGroupHeader';
import { ArtifactKindBadge } from '@/features/artifacts/ArtifactKindBadge';
import { ArtifactOtherCard } from '@/features/artifacts/ArtifactOtherCard';
import type { ArtifactMark, ArtifactPick } from '@/features/artifacts/artifactBridge';
import { useArtifactPicker } from '@/features/artifacts/useArtifactPicker';
import {
  ARTIFACTS_HEADING,
  ARTIFACTS_INFO,
  JUST_UPDATED,
  clockLabel,
  metaLine,
  openLabel,
} from '@/features/artifacts/artifactTexts';
import { groupArtifacts } from '@/features/artifacts/groupArtifacts';
import type { ArtifactGroup } from '@/features/artifacts/groupArtifacts';
import { mentionInChat } from '@/features/background/mention';
import { openArtifactDefault, revealArtifact } from '@/lib/artifacts';
import type { Artifact } from '@/lib/bindings/Artifact';
import type { ArtifactList } from '@/lib/bindings/ArtifactList';
import { commandErrorText } from '@/lib/errors';
import { CommentBox } from '@/features/review/CommentBox';
import { selectorTail } from '@/features/review/reviewLabels';
import { useArtifactsStore } from '@/stores/artifacts';
import { artifactLineId, NO_COMMENTS, useReviewStore } from '@/stores/review';
import { useSessionErrorsStore } from '@/stores/sessionErrors';
import './ArtifactsView.css';

const JUST_UPDATED_MS = 2500;

interface ArtifactsViewProps {
  /** Für Commands und Adresse; in der Übersicht die neueste Session des Vorhabens. */
  sessionId: string;
  projectId: string;
  /** `null` in der Übersicht: dort gibt es weder „diese Session“ noch „Im Chat besprechen“. */
  currentSessionId: string | null;
  list: ArtifactList;
}

export function ArtifactsView({
  sessionId,
  projectId,
  currentSessionId,
  list,
}: ArtifactsViewProps): ReactElement | null {
  const [reloadToken, setReloadToken] = useState<number>(0);
  const [isJustUpdated, setIsJustUpdated] = useState<boolean>(false);
  const [isCommenting, setIsCommenting] = useState<boolean>(false);
  const [pick, setPick] = useState<ArtifactPick | null>(null);
  const frameRef = useRef<HTMLIFrameElement>(null);
  const stageRef = useRef<HTMLDivElement>(null);
  const commentRef = useRef<HTMLDivElement>(null);
  const lastModified = useRef<Record<string, number>>({});
  const selectedFile: string | undefined = useArtifactsStore((state) => state.selected[projectId]);
  const select = useArtifactsStore((state) => state.select);
  const isFullscreen: boolean = useArtifactsStore((state) => state.fullscreen);
  const setFullscreen = useArtifactsStore((state) => state.setFullscreen);
  const collapsed: Record<string, boolean> | undefined = useArtifactsStore(
    (state) => state.collapsed[projectId],
  );
  const toggleGroup = useArtifactsStore((state) => state.toggleGroup);
  const reportSessionError = useSessionErrorsStore((state) => state.report);
  const clearSessionError = useSessionErrorsStore((state) => state.clear);
  const collected =
    useReviewStore((state) => state.collected[currentSessionId ?? '']) ?? NO_COMMENTS;
  const box = useReviewStore((state) => state.boxes[currentSessionId ?? '']) ?? null;
  const openBox = useReviewStore((state) => state.openBox);
  const setBoxText = useReviewStore((state) => state.setBoxText);
  const closeBox = useReviewStore((state) => state.closeBox);
  const upsert = useReviewStore((state) => state.upsert);

  // Verschwindet das gewählte Artefakt, gilt das neueste.
  const selected: Artifact | undefined =
    list.items.find((item: Artifact) => item.file === selectedFile) ?? list.items[0];
  const selectedKey: string | undefined = selected?.file;
  const selectedModifiedAt: number | undefined = selected?.modifiedAt;
  const groups: ArtifactGroup[] = groupArtifacts(list.items, currentSessionId);
  const artifactComments = collected.filter(
    (entry) =>
      entry.comment.target.kind === 'artifact' && entry.comment.target.file === selectedKey,
  );
  const marks: readonly ArtifactMark[] = artifactComments.map((entry, index) => {
    const target = entry.comment.target;
    return target.kind === 'artifact'
      ? { n: index + 1, selector: target.selector }
      : { n: index + 1, selector: '' };
  });

  function handlePick(nextPick: ArtifactPick): void {
    if (currentSessionId === null || selectedKey === undefined) return;
    const existing = artifactComments.find(
      (entry) => entry.lineId === artifactLineId(selectedKey, nextPick.selector),
    );
    const stageRect = stageRef.current?.getBoundingClientRect();
    const frameRect = frameRef.current?.getBoundingClientRect();
    const relativeX =
      stageRect === undefined || frameRect === undefined
        ? nextPick.rect.x
        : frameRect.left - stageRect.left + nextPick.rect.x;
    const relativeY =
      stageRect === undefined || frameRect === undefined
        ? nextPick.rect.y
        : frameRect.top - stageRect.top + nextPick.rect.y;
    const maxX = Math.max(0, (stageRef.current?.clientWidth ?? 0) - 420);
    setPick({
      ...nextPick,
      rect: {
        ...nextPick.rect,
        x: Math.max(0, Math.min(relativeX, maxX)),
        y: Math.max(0, relativeY),
      },
    });
    openBox(currentSessionId, {
      lineId: artifactLineId(selectedKey, nextPick.selector),
      text: existing?.comment.text ?? '',
    });
  }

  useArtifactPicker(frameRef, isCommenting, marks, handlePick);

  // Unter das Element, sonst auf dessen Oberkante — immer innerhalb der Bühne.
  useLayoutEffect(() => {
    const comment: HTMLDivElement | null = commentRef.current;
    const stage: HTMLDivElement | null = stageRef.current;
    if (comment === null || stage === null || pick === null) return;
    const maxTop: number = Math.max(0, stage.clientHeight - comment.offsetHeight);
    const below: number = pick.rect.y + pick.rect.height;
    const top: number = below <= maxTop ? below : pick.rect.y;
    comment.style.top = `${String(Math.max(0, Math.min(top, maxTop)))}px`;
  }, [pick, box]);

  useEffect(() => {
    if (selectedKey === undefined || selectedModifiedAt === undefined) {
      return undefined;
    }
    const previous: number | undefined = lastModified.current[selectedKey];
    lastModified.current[selectedKey] = selectedModifiedAt;
    const hasChanged: boolean = previous !== undefined && previous !== selectedModifiedAt;
    setIsJustUpdated(hasChanged);
    if (!hasChanged) {
      return undefined;
    }
    const timer: number = window.setTimeout(() => {
      setIsJustUpdated(false);
    }, JUST_UPDATED_MS);
    return (): void => {
      window.clearTimeout(timer);
    };
  }, [selectedKey, selectedModifiedAt]);

  // Ein Wechsel von Reiter, Session oder Vorhaben hängt die Ansicht aus und beendet damit das Vollbild.
  useEffect(() => {
    return (): void => {
      setFullscreen(false);
    };
  }, [setFullscreen]);

  useEffect(() => {
    if (currentSessionId === null) return undefined;
    function stopOnEscape(event: KeyboardEvent): void {
      if (event.key === 'Escape' && box === null) setIsCommenting(false);
    }
    window.addEventListener('keydown', stopOnEscape);
    return (): void => {
      window.removeEventListener('keydown', stopOnEscape);
    };
  }, [box, currentSessionId]);

  useEffect(() => {
    return (): void => {
      setIsCommenting(false);
      setPick(null);
      if (currentSessionId !== null) closeBox(currentSessionId);
    };
  }, [closeBox, currentSessionId, reloadToken, selectedKey]);

  if (selected === undefined) {
    return null;
  }

  function runOnFile(action: (id: string, file: string) => Promise<void>, item: Artifact): void {
    action(sessionId, item.file)
      .then(() => {
        clearSessionError(sessionId);
      })
      .catch((reason: unknown) => {
        console.error('Artefakt nicht geöffnet', reason);
        reportSessionError(sessionId, `Artefakt nicht geöffnet: ${commandErrorText(reason)}`);
      });
  }

  function renderEntry(item: Artifact): ReactElement {
    const isSelected: boolean = item.file === selected?.file;
    return (
      <button
        key={item.file}
        type="button"
        className={`artifacts__entry${isSelected ? ' artifacts__entry--selected' : ''}`}
        aria-current={isSelected ? 'true' : undefined}
        onClick={(): void => {
          setIsCommenting(false);
          setPick(null);
          if (currentSessionId !== null) closeBox(currentSessionId);
          select(projectId, item.file);
        }}
      >
        <ArtifactKindBadge kind={item.kind} />
        <span className="artifacts__entry-text">
          <span className="artifacts__entry-title">{item.title}</span>
          <span className="artifacts__entry-meta">{metaLine(item)}</span>
        </span>
      </button>
    );
  }

  function renderGroup(group: ArtifactGroup): ReactElement {
    const isOpen: boolean = collapsed?.[group.key] !== true;
    const itemsId: string = `artifacts-group-${group.key === '' ? 'none' : group.key}`;
    return (
      <div key={group.key} className="artifacts__group">
        <ArtifactGroupHeader
          group={group}
          isOpen={isOpen}
          controls={itemsId}
          onToggle={(): void => {
            toggleGroup(projectId, group.key);
          }}
        />
        <div id={itemsId} className="artifacts__group-items" hidden={!isOpen}>
          {group.items.map(renderEntry)}
        </div>
      </div>
    );
  }

  function renderDiscussButton(item: Artifact): ReactElement | null {
    if (currentSessionId === null) {
      return null;
    }
    return (
      <button
        type="button"
        className="artifacts__button"
        onClick={(): void => {
          mentionInChat(currentSessionId, `Artefakt „${item.title}“ (${list.dir}\\${item.file})`);
        }}
      >
        Im Chat besprechen
      </button>
    );
  }

  return (
    <div className="artifacts">
      <nav className="artifacts__list" aria-label="Artefakte">
        <div className="artifacts__heading">
          <span>
            {ARTIFACTS_HEADING} · {list.items.length}
          </span>
          <span className="artifacts__info" title={ARTIFACTS_INFO}>
            ?
          </span>
        </div>
        {groups.map(renderGroup)}
      </nav>
      <section className="artifacts__preview" aria-label="Vorschau">
        <header className="artifacts__head">
          <div className="artifacts__head-text">
            <h2 className="artifacts__title">{selected.title}</h2>
            <span
              className={`artifacts__subtitle${isJustUpdated ? ' artifacts__subtitle--fresh' : ''}`}
            >
              {isJustUpdated
                ? JUST_UPDATED
                : `${selected.file} · ${clockLabel(selected.modifiedAt)}`}
            </span>
          </div>
          <div className="artifacts__actions">
            <button
              type="button"
              className="artifacts__button artifacts__button--icon"
              title="Neu laden"
              aria-label="Neu laden"
              onClick={(): void => {
                setIsCommenting(false);
                setPick(null);
                if (currentSessionId !== null) closeBox(currentSessionId);
                setReloadToken((token: number) => token + 1);
              }}
            >
              <svg
                width="14"
                height="14"
                viewBox="0 0 16 16"
                fill="none"
                stroke="currentColor"
                strokeWidth="1.5"
                strokeLinecap="round"
                strokeLinejoin="round"
                aria-hidden="true"
              >
                <path d="M13 8a5 5 0 1 1-1.5-3.6" />
                <path d="M13 2.5V5h-2.5" />
              </svg>
            </button>
            {selected.viewable ? (
              <button
                type="button"
                className="artifacts__button artifacts__button--icon"
                title="Vollbild"
                aria-label="Vollbild"
                onClick={(): void => {
                  setFullscreen(true);
                }}
              >
                <svg
                  width="14"
                  height="14"
                  viewBox="0 0 16 16"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="1.5"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  aria-hidden="true"
                >
                  <path d="M2.5 6V2.5H6M10 2.5h3.5V6M13.5 10v3.5H10M6 13.5H2.5V10" />
                </svg>
              </button>
            ) : null}
            {currentSessionId !== null && selected.viewable ? (
              <button
                type="button"
                className={`artifacts__button${isCommenting ? ' artifacts__button--commenting' : ''}`}
                onClick={(): void => {
                  setIsCommenting((value: boolean) => !value);
                  if (isCommenting) {
                    setPick(null);
                    closeBox(currentSessionId);
                  }
                }}
              >
                <svg
                  width="14"
                  height="14"
                  viewBox="0 0 16 16"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="1.5"
                  aria-hidden="true"
                >
                  <path d="m3 11.8 1-.2 7.3-7.3a1.5 1.5 0 0 0-2.1-2.1L1.9 9.5 1.7 10.6 3 11.8Z" />
                  <path d="m8.2 3.2 2.1 2.1" />
                </svg>
                Kommentieren{isCommenting ? ` · ${String(artifactComments.length)}` : ''}
              </button>
            ) : null}
            {renderDiscussButton(selected)}
            <button
              type="button"
              className="artifacts__button"
              onClick={(): void => {
                runOnFile(openArtifactDefault, selected);
              }}
            >
              {openLabel(selected.kind)}
              <svg
                width="11"
                height="11"
                viewBox="0 0 14 14"
                fill="none"
                stroke="currentColor"
                strokeWidth="1.3"
                strokeLinecap="round"
                strokeLinejoin="round"
                aria-hidden="true"
              >
                <path d="M5.5 2.5h6v6M11.5 2.5 4 10" />
              </svg>
            </button>
          </div>
        </header>
        {isCommenting ? (
          <div className="artifacts__hint">
            <span>Element anklicken, um es zu kommentieren · Esc beendet</span>
            <button
              type="button"
              onClick={(): void => {
                setIsCommenting(false);
              }}
            >
              Fertig
            </button>
          </div>
        ) : null}
        <div ref={stageRef} className="artifacts__stage">
          {selected.viewable ? (
            <ArtifactFrame
              baseUrl={list.baseUrl}
              item={selected}
              reloadToken={reloadToken}
              frameRef={frameRef}
            />
          ) : (
            <ArtifactOtherCard
              item={selected}
              onOpen={(): void => {
                runOnFile(openArtifactDefault, selected);
              }}
              onReveal={(): void => {
                runOnFile(revealArtifact, selected);
              }}
            />
          )}
          {currentSessionId !== null && pick !== null && box !== null ? (
            <div ref={commentRef} className="artifacts__comment" style={{ left: pick.rect.x }}>
              <CommentBox
                fileLabel={pick.label === '' ? pick.selector : `„${pick.label}“`}
                lineLabel={selectorTail(pick.selector)}
                text={box.text}
                isEditing={artifactComments.some((entry) => entry.lineId === box.lineId)}
                shouldReveal={false}
                onRevealed={(): void => {}}
                onChange={(text: string): void => {
                  setBoxText(currentSessionId, text);
                }}
                onSubmit={(): void => {
                  upsert(currentSessionId, box.lineId, {
                    target: {
                      kind: 'artifact',
                      file: selected.file,
                      selector: pick.selector,
                      label: pick.label,
                    },
                    repositoryKey: '',
                    repositoryName: '',
                    path: selected.file,
                    kind: 'context',
                    line: 0,
                    code: pick.snippet,
                    text: box.text.trim(),
                  });
                  closeBox(currentSessionId);
                  setPick(null);
                }}
                onCancel={(): void => {
                  closeBox(currentSessionId);
                  setPick(null);
                }}
              />
            </div>
          ) : null}
        </div>
      </section>
      {isFullscreen && selected.viewable ? (
        <ArtifactFullscreen
          baseUrl={list.baseUrl}
          item={selected}
          onClose={(): void => {
            setFullscreen(false);
          }}
        />
      ) : null}
    </div>
  );
}
