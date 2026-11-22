import type { ReactElement } from 'react';
import { ClockIcon } from '@/components/ClockIcon';
import { AttachmentRow } from '@/features/attachments/AttachmentRow';
import { queuedMarkText } from '@/features/queue/queueTexts';
import { OriginLine } from '@/features/remote/OriginLine';
import { ReviewCommentCard } from '@/features/review/ReviewCommentCard';
import type { Attachment } from '@/lib/bindings/Attachment';
import type { ReviewComment } from '@/lib/bindings/ReviewComment';
import type { SkillRef } from '@/lib/bindings/SkillRef';
import { useMessageWait } from '@/stores/agentQueue';
import { useAttachmentsStore } from '@/stores/attachments';
import './UserMessage.css';

const CLOCK_SIZE = 11;

interface UserMessageProps {
  sessionId: string;
  text: string;
  attachments: readonly Attachment[];
  skill: SkillRef | null;
  comments: readonly ReviewComment[];
  /** Das Gerät, von dem ein Gast die Nachricht schickte; `null`: diese Oberfläche. */
  origin: string | null;
  /** Wartet auf einen freien Platz (Taiga-Story 190): Marke „wartet · Platz n“ im Kopf, Rahmen gestrichelt. */
  queued?: boolean;
}

export function UserMessage({
  sessionId,
  text,
  attachments,
  skill,
  comments,
  origin,
  queued = false,
}: UserMessageProps): ReactElement {
  const openViewer = useAttachmentsStore((state) => state.openViewer);
  // Den Platz liefert die Warteschlange; alle wartenden Nachrichten der Session teilen ihn.
  const position: number | null = useMessageWait(queued ? sessionId : null);
  const hasComments: boolean = comments.length > 0;
  const boxClass = `user-message__box${hasComments ? ' user-message__box--with-comments' : ''}${
    queued ? ' user-message__box--queued' : ''
  }`;
  const commentCount: string =
    comments.length === 1 ? '1 Review-Kommentar' : `${String(comments.length)} Review-Kommentare`;
  const skillMark: string | null = skill === null ? null : `/${skill.name}`;
  const trimmedStart: string = text.trimStart();
  const hasMark: boolean = skillMark !== null && trimmedStart.startsWith(skillMark);
  const body: string =
    hasMark && skillMark !== null ? trimmedStart.slice(skillMark.length).trimStart() : text;

  return (
    <div className="user-message">
      <div className={boxClass}>
        {queued && (
          <span className="user-message__queued">
            <ClockIcon size={CLOCK_SIZE} />
            {queuedMarkText(position)}
          </span>
        )}
        <OriginLine origin={origin} />
        {attachments.length > 0 && (
          <AttachmentRow
            attachments={attachments}
            size="sent"
            sessionId={sessionId}
            onOpen={(attachment: Attachment): void => {
              openViewer(sessionId, attachment);
            }}
          />
        )}
        {(hasMark || body !== '') && (
          <p className="user-message__text">
            {hasMark && <span className="user-message__mark">{skillMark}</span>}
            {body}
          </p>
        )}
        {hasComments && (
          <div className="user-message__comments">
            <span className="user-message__comments-count">{commentCount}</span>
            {comments.map((comment: ReviewComment, index: number) => (
              <ReviewCommentCard key={index} comment={comment} sessionId={sessionId} />
            ))}
          </div>
        )}
      </div>
      {skill !== null && (
        <div className="user-message__skill">
          <span className="user-message__dot" />
          <span className="user-message__skill-word">Skill</span>
          <span className="user-message__skill-name">{skill.name}</span>
          <span className="user-message__skill-note">
            {skill.origin.kind === 'user' && 'geladen · aus deinem Benutzerordner'}
            {skill.origin.kind === 'repository' && `geladen · aus ${skill.origin.name}`}
            {skill.origin.kind === 'builtin' && 'geladen · mitgeliefert'}
          </span>
        </div>
      )}
    </div>
  );
}
