import type { ReactElement } from 'react';
import { ReviewCommentCard } from '@/features/review/ReviewCommentCard';
import { NO_COMMENTS, useReviewStore } from '@/stores/review';
import type { CollectedComment } from '@/stores/review';
import './ReviewCommentList.css';

interface ReviewCommentListProps {
  sessionId: string;
}

export function ReviewCommentList({ sessionId }: ReviewCommentListProps): ReactElement | null {
  const collected: readonly CollectedComment[] = useReviewStore(
    (state) => state.collected[sessionId] ?? NO_COMMENTS,
  );
  const updateText = useReviewStore((state) => state.updateText);
  const remove = useReviewStore((state) => state.remove);
  const clear = useReviewStore((state) => state.clear);

  if (collected.length === 0) {
    return null;
  }

  return (
    <section className="review-list" aria-label="Review-Kommentare">
      <div className="review-list__head">
        <span className="review-list__title">Review-Kommentare</span>
        <span className="review-list__note">
          {collected.length} · gehen mit der nächsten Nachricht raus
        </span>
        <button
          type="button"
          className="review-list__clear"
          onClick={(): void => {
            clear(sessionId);
          }}
        >
          Alle entfernen
        </button>
      </div>
      <div className="review-list__cards">
        {collected.map((entry: CollectedComment) => (
          <ReviewCommentCard
            key={entry.id}
            comment={entry.comment}
            sessionId={sessionId}
            onEdit={(text: string): void => {
              updateText(sessionId, entry.id, text);
            }}
            onRemove={(): void => {
              remove(sessionId, entry.id);
            }}
          />
        ))}
      </div>
    </section>
  );
}
