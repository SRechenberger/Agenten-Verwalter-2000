import { useState } from 'react';
import type { ReactElement } from 'react';
import { StatusIcon } from '@/components/StatusIcon';
import { TldrButton } from '@/features/tldr/TldrButton';
import { TldrEmptyBar } from '@/features/tldr/TldrEmptyBar';
import { TldrIcon } from '@/features/tldr/TldrIcon';
import { TldrSkeleton } from '@/features/tldr/TldrSkeleton';
import { newEntryCount, stampText } from '@/features/tldr/tldrTexts';
import { useSessionTldr } from '@/features/tldr/useSessionTldr';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import type { SessionTldr } from '@/lib/bindings/SessionTldr';
import type { SessionTldrView } from '@/lib/bindings/SessionTldrView';
import { commandErrorText } from '@/lib/errors';
import { useSessionLocked } from '@/features/remote/useProjectAccess';
import { createSessionTldr } from '@/lib/tldr';
import { useTldrStore } from '@/stores/tldr';
import './SessionTldrCard.css';

const EMPTY_TEXT =
  'Noch keins für diese Session. Fasst den bisherigen Verlauf in drei, vier Zeilen zusammen.';
const EMPTY_HINT = 'Haiku · im Hintergrund';
const LOADING_TEXT =
  'Haiku liest den Verlauf in einer eigenen Session im Hintergrund. Du kannst hier weiterarbeiten.';

interface TldrRow {
  label: string;
  value: string;
  needsUser: boolean;
}

interface SessionTldrCardProps {
  session: SessionSummary;
  /** Anzahl der Chat-Einträge der Session, für „N neue Einträge seitdem“. */
  entryCount: number;
}

export function SessionTldrCard({
  session,
  entryCount,
}: SessionTldrCardProps): ReactElement | null {
  const [actionError, setActionError] = useState<string | null>(null);
  const isLocked: boolean = useSessionLocked(session.id);
  const isCollapsed: boolean = useTldrStore((state) => state.collapsed[session.id] ?? false);
  const toggleCollapsed = useTldrStore((state) => state.toggleCollapsed);
  const { view, error: loadError } = useSessionTldr(session.id);

  function create(): void {
    setActionError(null);
    createSessionTldr(session.id).catch((reason: unknown) => {
      setActionError(commandErrorText(reason));
    });
  }

  function renderError(): ReactElement | null {
    const message: string | null = actionError ?? view?.error ?? loadError;
    if (message === null) {
      return null;
    }
    return <p className="session-tldr__error">TL;DR nicht erstellt: {message}</p>;
  }

  function renderBody(current: SessionTldrView): ReactElement | null {
    const tldr: SessionTldr | null = current.tldr;
    if (tldr === null) {
      if (!current.isRunning) {
        return (
          <TldrEmptyBar text={EMPTY_TEXT} hint={EMPTY_HINT} disabled={isLocked} onCreate={create} />
        );
      }
      return (
        <div className="session-tldr__card">
          <div className="session-tldr__head">
            <span className="session-tldr__label">TL;DR</span>
            {renderRunning()}
          </div>
          <div className="session-tldr__content">
            <TldrSkeleton kind="session" text={LOADING_TEXT} />
          </div>
        </div>
      );
    }
    return renderCard(current, tldr);
  }

  function renderRunning(): ReactElement {
    return (
      <span className="session-tldr__stamp session-tldr__stamp--running">
        <StatusIcon status="running" size={10} />
        Wird erstellt …
      </span>
    );
  }

  function renderStamp(current: SessionTldrView): ReactElement {
    if (current.isRunning) {
      return renderRunning();
    }
    const isStale: boolean = newEntryCount(current, entryCount) > 0;
    return (
      <span className="session-tldr__stamp">
        {isStale && <span className="session-tldr__dot" />}
        {stampText(current, entryCount)}
      </span>
    );
  }

  function renderRefresh(current: SessionTldrView): ReactElement | null {
    if (current.isRunning) {
      return null;
    }
    if (newEntryCount(current, entryCount) > 0) {
      return (
        <TldrButton variant="compact" disabled={isLocked} onClick={create}>
          <TldrIcon name="refresh" size={12} />
          Aktualisieren
        </TldrButton>
      );
    }
    return (
      <TldrButton
        variant="icon"
        ariaLabel="TL;DR neu erstellen"
        title="TL;DR neu erstellen"
        disabled={isLocked}
        onClick={create}
      >
        <TldrIcon name="refresh" size={13} />
      </TldrButton>
    );
  }

  function renderCard(current: SessionTldrView, tldr: SessionTldr): ReactElement {
    return (
      <div className="session-tldr__card">
        <div className="session-tldr__head">
          <span className="session-tldr__label">TL;DR</span>
          {isCollapsed && <span className="session-tldr__short">{tldr.short}</span>}
          {renderStamp(current)}
          {renderRefresh(current)}
          <TldrButton
            variant="icon"
            ariaLabel={isCollapsed ? 'TL;DR aufklappen' : 'TL;DR einklappen'}
            onClick={(): void => {
              toggleCollapsed(session.id);
            }}
          >
            <TldrIcon name={isCollapsed ? 'chevronDown' : 'chevronUp'} size={12} />
          </TldrButton>
        </div>
        {!isCollapsed && <div className="session-tldr__content">{renderRows(tldr)}</div>}
      </div>
    );
  }

  function renderRows(tldr: SessionTldr): ReactElement[] {
    const rows: TldrRow[] = [
      { label: 'Ziel', value: tldr.goal, needsUser: false },
      { label: 'Erledigt', value: tldr.done, needsUser: false },
      { label: 'Läuft', value: tldr.ongoing, needsUser: false },
      { label: 'Offen', value: tldr.open, needsUser: tldr.openNeedsUser },
    ];
    return rows
      .filter((row: TldrRow) => row.value !== '')
      .map((row: TldrRow) => (
        <div key={row.label} className="session-tldr__row">
          <span className="session-tldr__row-label">{row.label}</span>
          <span
            className={`session-tldr__row-value ${row.needsUser ? 'session-tldr__row-value--waiting' : ''}`}
          >
            {row.value}
          </span>
        </div>
      ));
  }

  if (session.status === 'new' || view === null) {
    return null;
  }
  return (
    <section className="session-tldr" aria-label="TL;DR der Session">
      <div className="session-tldr__column">
        {renderBody(view)}
        {renderError()}
      </div>
    </section>
  );
}
