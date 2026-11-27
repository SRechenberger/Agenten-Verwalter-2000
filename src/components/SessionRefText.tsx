import { useContext } from 'react';
import type { ReactElement, ReactNode } from 'react';
import { SessionRefsContext } from '@/components/SessionRefsContext';
import { SessionLink } from '@/features/chat/SessionLink';
import { resolveSessionRef, sessionRefTitle, splitSessionRefs } from '@/lib/sessionRefs';
import type { RefSession, SessionRefPart } from '@/lib/sessionRefs';

interface SessionRefLinkProps {
  /** Ziel nach Nummer oder ID; ohne Session dazu steht `children` als Text. */
  target: { number: number } | { id: string };
  children: ReactNode;
}

/** Link auf eine Session im Chattext; ohne Kontext oder ohne Session zum Verweis bleibt es Text. */
export function SessionRefLink({ target, children }: SessionRefLinkProps): ReactElement {
  const refs = useContext(SessionRefsContext);
  const session: RefSession | null =
    refs === null ? null : resolveSessionRef(target, refs.sessions, refs.projectId);
  if (refs === null || session === null) {
    return <>{children}</>;
  }
  const foreign: boolean = session.projectId !== refs.projectId;
  return (
    <SessionLink
      id={session.id}
      number={session.number}
      title={session.name}
      tooltip={sessionRefTitle(session, foreign ? refs.projectName(session.projectId) : null)}
      canJump
      inline
    >
      {children}
    </SessionLink>
  );
}

interface SessionRefTextProps {
  text: string;
}

/** Freier Text, in dem Verweise auf Sessions Links sind (Nutzernachrichten). */
export function SessionRefText({ text }: SessionRefTextProps): ReactElement {
  return (
    <>
      {splitSessionRefs(text).map((part: SessionRefPart, index: number) =>
        part.kind === 'text' ? (
          part.text
        ) : (
          <SessionRefLink
            key={index}
            target={part.kind === 'number' ? { number: part.number } : { id: part.id }}
          >
            {part.text}
          </SessionRefLink>
        ),
      )}
    </>
  );
}
