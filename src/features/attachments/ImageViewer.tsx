import { useEffect, useRef, useState } from 'react';
import type { PointerEvent as ReactPointerEvent, ReactElement } from 'react';
import { Dialog } from '@/components/Dialog';
import { isClickSized, toImageRect } from '@/features/attachments/imageRect';
import type { ImageRect, Point, Size } from '@/features/attachments/imageRect';
import { useAssetUrl } from '@/features/remote/assetUrl';
import { CommentBox } from '@/features/review/CommentBox';
import type { Attachment } from '@/lib/bindings/Attachment';
import { useAttachmentsStore } from '@/stores/attachments';
import { imageLineId, NO_COMMENTS, useReviewStore } from '@/stores/review';
import type { CollectedComment, ImageTarget } from '@/stores/review';
import './ImageViewer.css';

const COMMENT_BOX_WIDTH = 420;
const COMMENT_BOX_GAP = 8;

interface ImageViewerProps {
  sessionId: string;
  attachment: Attachment;
  onClose: () => void;
}

interface Drag {
  start: Point;
  current: Point;
}

function imageTargetOf(entry: CollectedComment, attachmentId: string): ImageTarget | null {
  const target = entry.comment.target;
  if (target.kind !== 'image' || target.attachmentId !== attachmentId) {
    return null;
  }
  return target;
}

export function ImageViewer({ sessionId, attachment, onClose }: ImageViewerProps): ReactElement {
  const [isActualSize, setIsActualSize] = useState<boolean>(false);
  const [naturalSize, setNaturalSize] = useState<Size | null>(null);
  const [displayWidth, setDisplayWidth] = useState<number>(0);
  const [drag, setDrag] = useState<Drag | null>(null);
  const [pending, setPending] = useState<ImageRect | null>(null);
  const [text, setText] = useState<string>('');
  const imageRef = useRef<HTMLImageElement>(null);
  const collected = useReviewStore((state) => state.collected[sessionId]) ?? NO_COMMENTS;
  const upsert = useReviewStore((state) => state.upsert);
  // Ein noch nicht gesendeter Anhang liegt auf diesem Gerät, ein gesendeter beim Host der Session.
  const isUnsent: boolean = useAttachmentsStore((state) =>
    (state.pending[sessionId] ?? []).some(
      (candidate: Attachment) => candidate.id === attachment.id,
    ),
  );
  const imageSrc: string = useAssetUrl(attachment.path, sessionId, !isUnsent);

  // Eingepasst und 1:1 haben unterschiedliche Anzeigebreiten; der Maßstab folgt der echten Breite.
  useEffect(() => {
    const image: HTMLImageElement | null = imageRef.current;
    if (image === null) {
      return undefined;
    }
    const observer = new ResizeObserver((): void => {
      setDisplayWidth(image.clientWidth);
    });
    observer.observe(image);
    return (): void => {
      observer.disconnect();
    };
  }, []);

  const scale: number =
    naturalSize === null || displayWidth === 0 ? 1 : naturalSize.width / displayWidth;
  const frames: { entry: CollectedComment; target: ImageTarget }[] = [];
  for (const entry of collected) {
    const target: ImageTarget | null = imageTargetOf(entry, attachment.id);
    if (target !== null) {
      frames.push({ entry, target });
    }
  }

  function targetOf(rect: ImageRect): ImageTarget {
    return {
      kind: 'image',
      attachmentId: attachment.id,
      path: attachment.path,
      name: attachment.name,
      ...rect,
      imageWidth: naturalSize?.width ?? 0,
      imageHeight: naturalSize?.height ?? 0,
    };
  }

  function existingFor(rect: ImageRect): CollectedComment | undefined {
    const lineId: string = imageLineId(targetOf(rect));
    return frames.find((frame) => frame.entry.lineId === lineId)?.entry;
  }

  function ordinalFor(rect: ImageRect): number {
    const lineId: string = imageLineId(targetOf(rect));
    const index: number = frames.findIndex((frame) => frame.entry.lineId === lineId);
    return index === -1 ? frames.length + 1 : index + 1;
  }

  function openComment(rect: ImageRect): void {
    setText(existingFor(rect)?.comment.text ?? '');
    setPending(rect);
  }

  function pointOf(event: ReactPointerEvent<HTMLElement>): Point {
    const bounds: DOMRect | undefined = imageRef.current?.getBoundingClientRect();
    return {
      x: event.clientX - (bounds?.left ?? 0),
      y: event.clientY - (bounds?.top ?? 0),
    };
  }

  function rectOfDrag(currentDrag: Drag): ImageRect | null {
    if (naturalSize === null) {
      return null;
    }
    return toImageRect(currentDrag.start, currentDrag.current, scale, naturalSize);
  }

  function handlePointerDown(event: ReactPointerEvent<HTMLDivElement>): void {
    if (event.button !== 0 || naturalSize === null) {
      return;
    }
    event.currentTarget.setPointerCapture(event.pointerId);
    setPending(null);
    const point: Point = pointOf(event);
    setDrag({ start: point, current: point });
  }

  function handlePointerMove(event: ReactPointerEvent<HTMLDivElement>): void {
    if (drag === null) {
      return;
    }
    setDrag({ start: drag.start, current: pointOf(event) });
  }

  function handlePointerUp(event: ReactPointerEvent<HTMLDivElement>): void {
    if (drag === null) {
      return;
    }
    event.currentTarget.releasePointerCapture(event.pointerId);
    const rect: ImageRect | null = rectOfDrag({ start: drag.start, current: pointOf(event) });
    setDrag(null);
    if (rect === null || isClickSized(rect)) {
      return;
    }
    openComment(rect);
  }

  // Esc schließt erst ein offenes Kommentarfeld, dann die Ansicht.
  function handleClose(): void {
    if (pending !== null) {
      setPending(null);
      return;
    }
    onClose();
  }

  function submit(rect: ImageRect): void {
    upsert(sessionId, imageLineId(targetOf(rect)), {
      target: targetOf(rect),
      repositoryKey: '',
      repositoryName: '',
      path: attachment.name,
      kind: 'context',
      line: 0,
      code: '',
      text: text.trim(),
    });
    setPending(null);
  }

  function renderDrag(currentDrag: Drag): ReactElement | null {
    const rect: ImageRect | null = rectOfDrag(currentDrag);
    if (rect === null) {
      return null;
    }
    return (
      <div className="image-viewer__drag" style={rectStyle(rect)}>
        <span className="image-viewer__size">
          {`${String(rect.width)} × ${String(rect.height)} bei ${String(rect.x)}, ${String(rect.y)}`}
        </span>
      </div>
    );
  }

  function rectStyle(rect: ImageRect): {
    left: number;
    top: number;
    width: number;
    height: number;
  } {
    return {
      left: rect.x / scale,
      top: rect.y / scale,
      width: rect.width / scale,
      height: rect.height / scale,
    };
  }

  function renderCommentBox(rect: ImageRect): ReactElement {
    const maxLeft: number = Math.max(0, displayWidth - COMMENT_BOX_WIDTH);
    return (
      <div
        className="image-viewer__comment"
        style={{
          left: Math.min(rect.x / scale, maxLeft),
          top: (rect.y + rect.height) / scale + COMMENT_BOX_GAP,
          width: COMMENT_BOX_WIDTH,
        }}
        onPointerDown={(event: ReactPointerEvent<HTMLDivElement>): void => {
          event.stopPropagation();
        }}
      >
        <CommentBox
          fileLabel={attachment.name}
          lineLabel={`Bereich ${String(ordinalFor(rect))}`}
          text={text}
          isEditing={existingFor(rect) !== undefined}
          shouldReveal={true}
          onRevealed={(): void => {}}
          onChange={setText}
          onSubmit={(): void => {
            submit(rect);
          }}
          onCancel={(): void => {
            setPending(null);
          }}
        />
      </div>
    );
  }

  return (
    <Dialog labelledBy="image-viewer-title" className="image-viewer" onClose={handleClose}>
      <header className="image-viewer__head">
        <div className="image-viewer__head-text">
          <h2 id="image-viewer-title" className="image-viewer__title" title={attachment.name}>
            {attachment.name}
          </h2>
          {naturalSize !== null && (
            <span className="image-viewer__meta">
              {`${String(naturalSize.width)} × ${String(naturalSize.height)}`}
            </span>
          )}
          <span className="image-viewer__hint">Bereich aufziehen, um ihn zu kommentieren</span>
        </div>
        <div className="image-viewer__actions">
          <button
            type="button"
            className="image-viewer__button"
            aria-pressed={isActualSize}
            onClick={(): void => {
              setIsActualSize((value: boolean) => !value);
              setPending(null);
            }}
          >
            Originalgröße
          </button>
          <button type="button" className="image-viewer__button" onClick={onClose}>
            Schließen
          </button>
        </div>
      </header>
      <div className="image-viewer__body">
        <div
          className="image-viewer__surface"
          onPointerDown={handlePointerDown}
          onPointerMove={handlePointerMove}
          onPointerUp={handlePointerUp}
        >
          <img
            ref={imageRef}
            className={`image-viewer__image${isActualSize ? ' image-viewer__image--actual' : ''}`}
            src={imageSrc}
            alt={attachment.name}
            draggable={false}
            onLoad={(event): void => {
              setNaturalSize({
                width: event.currentTarget.naturalWidth,
                height: event.currentTarget.naturalHeight,
              });
              setDisplayWidth(event.currentTarget.clientWidth);
            }}
          />
          {frames.map((frame, index: number) => (
            <button
              key={frame.entry.id}
              type="button"
              className="image-viewer__frame"
              style={rectStyle(frame.target)}
              aria-label={`Bereich ${String(index + 1)} bearbeiten`}
              onPointerDown={(event: ReactPointerEvent<HTMLButtonElement>): void => {
                event.stopPropagation();
              }}
              onClick={(): void => {
                openComment(frame.target);
              }}
            >
              <span className="image-viewer__number">{index + 1}</span>
            </button>
          ))}
          {drag !== null && renderDrag(drag)}
          {pending !== null && renderCommentBox(pending)}
        </div>
      </div>
    </Dialog>
  );
}

/** Öffnet die Bildansicht, solange der Store sie für diese Session offen hält. */
export function SessionImageViewer({ sessionId }: { sessionId: string }): ReactElement | null {
  const viewer = useAttachmentsStore((state) => state.viewer);
  const closeViewer = useAttachmentsStore((state) => state.closeViewer);
  if (viewer === null || viewer.sessionId !== sessionId) {
    return null;
  }
  return <ImageViewer sessionId={sessionId} attachment={viewer.attachment} onClose={closeViewer} />;
}
