import { useState } from 'react';
import type { ReactElement } from 'react';
import { ClockIcon } from '@/components/ClockIcon';
import { SessionRefText } from '@/components/SessionRefText';
import { AttachmentRow } from '@/features/attachments/AttachmentRow';
import {
  ADOPT_LABEL,
  ADOPT_TITLE,
  ALREADY_SENT_TEXT,
  DISCARD_LABEL,
  DISCARD_TITLE,
  DISCARDED_MARK,
  discardedAttachmentsText,
  queuedMarkText,
} from '@/features/queue/queueTexts';
import { OriginLine } from '@/features/remote/OriginLine';
import { ReviewCommentCard } from '@/features/review/ReviewCommentCard';
import { commandErrorText } from '@/lib/errors';
import { discardQueued } from '@/lib/inbox';
import type { Attachment } from '@/lib/bindings/Attachment';
import type { ReviewComment } from '@/lib/bindings/ReviewComment';
import type { SkillRef } from '@/lib/bindings/SkillRef';
import { useMessageWait } from '@/stores/agentQueue';
import { useAttachmentsStore } from '@/stores/attachments';
import { useChatStore } from '@/stores/chat';
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
  /** Nachricht einer anderen Session: Linie links in der Farbe des Masters. */
  fromSession?: boolean;
  /** Der Eintrag im Chat (`ChatEntry.seq`); ohne ihn lässt sich die wartende Nachricht nicht verwerfen. */
  entrySeq?: number;
  /** Verworfen (Taiga-Story 218): ging nie an den Agenten; der Eintrag bietet „Text übernehmen“. */
  discarded?: boolean;
  /** Eine fremde wartende Nachricht (Taiga-Story 218): gestrichelt, mit dem Grund als Marke; nie verwerfbar. */
  waitMark?: string;
}

export function UserMessage({
  sessionId,
  text,
  attachments,
  skill,
  comments,
  origin,
  queued = false,
  fromSession = false,
  entrySeq,
  discarded = false,
  waitMark,
}: UserMessageProps): ReactElement {
  const openViewer = useAttachmentsStore((state) => state.openViewer);
  // Den Platz liefert die Warteschlange; alle wartenden Nachrichten der Session teilen ihn.
  const position: number | null = useMessageWait(queued ? sessionId : null);
  const draft: string = useChatStore((state) => state.drafts[sessionId] ?? '');
  const setDraft = useChatStore((state) => state.setDraft);
  const [discardNote, setDiscardNote] = useState<string | null>(null);
  const hasComments: boolean = comments.length > 0;
  const isWaiting: boolean = queued || waitMark !== undefined;
  const boxClass = `user-message__box${hasComments ? ' user-message__box--with-comments' : ''}${
    isWaiting ? ' user-message__box--queued' : ''
  }${discarded ? ' user-message__box--discarded' : ''}${fromSession ? ' user-message__box--session' : ''}`;
  const attachmentsNote: string | null = discardedAttachmentsText(attachments.length);

  function discard(): void {
    if (entrySeq === undefined) {
      return;
    }
    setDiscardNote(null);
    discardQueued(sessionId, entrySeq)
      .then((outcome) => {
        if (outcome === 'alreadySent') {
          setDiscardNote(ALREADY_SENT_TEXT);
        }
      })
      .catch((reason: unknown) => {
        setDiscardNote(commandErrorText(reason));
      });
  }
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
        {isWaiting && (
          <span className="user-message__queued">
            <ClockIcon size={CLOCK_SIZE} />
            {waitMark ?? queuedMarkText(position)}
          </span>
        )}
        {discarded && <span className="user-message__discarded">{DISCARDED_MARK}</span>}
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
            <SessionRefText text={body} />
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
        {queued && entrySeq !== undefined && (
          <div className="user-message__actions">
            <button
              type="button"
              className="user-message__action"
              title={DISCARD_TITLE}
              onClick={discard}
            >
              {DISCARD_LABEL}
            </button>
            {discardNote !== null && (
              <span className="user-message__note" role="status">
                {discardNote}
              </span>
            )}
          </div>
        )}
        {!(queued && entrySeq !== undefined) && discardNote !== null && (
          <div className="user-message__actions">
            <span className="user-message__note" role="status">
              {discardNote}
            </span>
          </div>
        )}
        {discarded && (
          <div className="user-message__actions">
            <button
              type="button"
              className="user-message__action"
              title={ADOPT_TITLE}
              disabled={draft !== ''}
              onClick={(): void => {
                setDraft(sessionId, text);
              }}
            >
              {ADOPT_LABEL}
            </button>
            {attachmentsNote !== null && (
              <span className="user-message__note">{attachmentsNote}</span>
            )}
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
