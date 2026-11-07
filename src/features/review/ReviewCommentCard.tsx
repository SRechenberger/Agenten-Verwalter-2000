import { useEffect, useMemo, useRef, useState } from 'react';
import type { ChangeEvent, KeyboardEvent, ReactElement } from 'react';
import { useAssetUrl } from '@/features/remote/assetUrl';
import { kindLabel, whereLabel } from '@/features/review/reviewLabels';
import type { CommentTarget } from '@/lib/bindings/CommentTarget';
import type { DiffLineKind } from '@/lib/bindings/DiffLineKind';
import type { ReviewComment } from '@/lib/bindings/ReviewComment';
import { highlightLine, languageOf } from '@/lib/syntax';
import type { SyntaxLine, SyntaxSegment } from '@/lib/syntax';
import './ReviewCommentCard.css';

interface ReviewCommentCardProps {
  comment: ReviewComment;
  /** Die Session der gesendeten Nachricht: Das Bild eines Kommentars liegt dann beim Host der Session. Fehlt sie, liegt es auf diesem Gerät. */
  sessionId?: string | undefined;
  /** Fehlt in der gesendeten Nachricht: dort gibt es weder Stift noch Bearbeiten. */
  onEdit?: (text: string) => void;
  onRemove?: () => void;
}

export function ReviewCommentCard({
  comment,
  sessionId,
  onEdit,
  onRemove,
}: ReviewCommentCardProps): ReactElement {
  const [isEditing, setIsEditing] = useState<boolean>(false);
  const [editText, setEditText] = useState<string>(comment.text);
  const editRef = useRef<HTMLTextAreaElement>(null);
  const codeLine: SyntaxLine = useMemo(
    () => highlightLine(comment.code, languageOf(comment.path)),
    [comment.code, comment.path],
  );
  const where: string = whereLabel(comment);
  const kind: string = kindLabel(comment.target);
  const imagePath: string = comment.target.kind === 'image' ? comment.target.path : '';
  const imageSrc: string = useAssetUrl(imagePath, sessionId ?? '', sessionId !== undefined);
  const isEmpty: boolean = editText.trim() === '';

  useEffect(() => {
    if (isEditing) {
      editRef.current?.focus();
    }
  }, [isEditing]);

  function startEditing(): void {
    setEditText(comment.text);
    setIsEditing(true);
  }

  function cancelEditing(): void {
    setIsEditing(false);
  }

  function saveEditing(): void {
    if (isEmpty || onEdit === undefined) {
      return;
    }
    onEdit(editText.trim());
    setIsEditing(false);
  }

  function handleKeyDown(event: KeyboardEvent<HTMLTextAreaElement>): void {
    if (event.key === 'Escape') {
      event.stopPropagation();
      cancelEditing();
      return;
    }
    if (event.key === 'Enter' && event.ctrlKey) {
      event.preventDefault();
      event.stopPropagation();
      saveEditing();
    }
  }

  function renderEditor(): ReactElement {
    return (
      <div className="review-card__editor">
        <textarea
          ref={editRef}
          className="review-card__input"
          aria-label="Kommentar bearbeiten"
          value={editText}
          onChange={(event: ChangeEvent<HTMLTextAreaElement>): void => {
            setEditText(event.target.value);
          }}
          onKeyDown={handleKeyDown}
        />
        <div className="review-card__editor-bar">
          <button type="button" className="review-card__cancel" onClick={cancelEditing}>
            Abbrechen
          </button>
          <button
            type="button"
            className="review-card__save"
            disabled={isEmpty}
            onClick={saveEditing}
          >
            Speichern
          </button>
        </div>
      </div>
    );
  }

  function renderBody(target: CommentTarget): ReactElement {
    if (target.kind === 'artifact') {
      return (
        <div className="review-card__code review-card__code--element" title={comment.code}>
          {comment.code}
        </div>
      );
    }
    if (target.kind === 'image') {
      const scale: number = Math.min(56 / target.imageWidth, 34 / target.imageHeight);
      const imageWidth: number = target.imageWidth * scale;
      const imageHeight: number = target.imageHeight * scale;
      const offsetX: number = (56 - imageWidth) / 2;
      const offsetY: number = (34 - imageHeight) / 2;
      const left: number = (target.x / target.imageWidth) * 100;
      const top: number = (target.y / target.imageHeight) * 100;
      const width: number = (target.width / target.imageWidth) * 100;
      const height: number = (target.height / target.imageHeight) * 100;
      return (
        <div className="review-card__crop">
          <span className="review-card__crop-thumb">
            <span
              className="review-card__crop-image"
              style={{
                left: `${String(offsetX)}px`,
                top: `${String(offsetY)}px`,
                width: `${String(imageWidth)}px`,
                height: `${String(imageHeight)}px`,
              }}
            >
              <img src={imageSrc} alt="" />
              <span
                className="review-card__crop-frame"
                style={{
                  left: `${String(left)}%`,
                  top: `${String(top)}%`,
                  width: `${String(width)}%`,
                  height: `${String(height)}%`,
                }}
              />
            </span>
          </span>
          <span>Bereich auf dem Bild markiert · das Bild geht mit</span>
        </div>
      );
    }
    return (
      <div className={`review-card__code review-card__code--${comment.kind}`}>
        <span className={`review-card__sign review-card__sign--${comment.kind}`}>
          {signOf(comment.kind)}
        </span>
        <span className="review-card__code-text">
          {codeLine.map((segment: SyntaxSegment, index: number) =>
            segment.className === null ? (
              segment.text
            ) : (
              <span key={index} className={segment.className}>
                {segment.text}
              </span>
            ),
          )}
        </span>
      </div>
    );
  }

  return (
    <div className="review-card">
      <div className="review-card__head">
        <span className="review-card__kind">{kind}</span>
        <span className="review-card__where" title={where}>
          {where}
        </span>
        {onEdit !== undefined && !isEditing && (
          <button
            type="button"
            className="review-card__action"
            aria-label="Kommentar bearbeiten"
            title="Kommentar bearbeiten"
            onClick={startEditing}
          >
            <PencilIcon />
          </button>
        )}
        {onRemove !== undefined && (
          <button
            type="button"
            className="review-card__action"
            aria-label="Kommentar entfernen"
            title="Kommentar entfernen"
            onClick={onRemove}
          >
            <CrossIcon />
          </button>
        )}
      </div>
      {renderBody(comment.target)}
      {isEditing ? renderEditor() : <p className="review-card__text">{comment.text}</p>}
    </div>
  );
}

function signOf(kind: DiffLineKind): string {
  if (kind === 'added') {
    return '+';
  }
  if (kind === 'deleted') {
    return '-';
  }
  return '';
}

function PencilIcon(): ReactElement {
  return (
    <svg
      width="12"
      height="12"
      viewBox="0 0 12 12"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d="M8.2 1.8l2 2L4 10H2V8z" />
    </svg>
  );
}

function CrossIcon(): ReactElement {
  return (
    <svg
      width="12"
      height="12"
      viewBox="0 0 12 12"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.4"
      strokeLinecap="round"
      aria-hidden="true"
    >
      <path d="M3 3l6 6" />
      <path d="M9 3l-6 6" />
    </svg>
  );
}
