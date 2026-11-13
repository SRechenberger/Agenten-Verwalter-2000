import type { ReactElement, ReactNode } from 'react';

interface CardSectionProps {
  title: string;
  /** Die Werte des Abschnitts; stehen in der Kopfzeile auch zugeklappt. */
  summary: string;
  isOpen: boolean;
  onToggle: () => void;
  children: ReactNode;
}

/** Ein Abschnitt der Kartei (Ablauf, Auftrag, Agent, Automatik) mit Kopfzeile zum Auf- und Zuklappen. */
export function CardSection({
  title,
  summary,
  isOpen,
  onToggle,
  children,
}: CardSectionProps): ReactElement {
  return (
    <section className="station-card__section">
      <button
        type="button"
        className="station-card__section-head"
        aria-expanded={isOpen}
        onClick={onToggle}
      >
        <span className="station-card__caret" aria-hidden="true">
          {isOpen ? '▾' : '▸'}
        </span>
        {title}
        <span className="station-card__summary">{summary}</span>
      </button>
      {isOpen && <div className="station-card__section-body">{children}</div>}
    </section>
  );
}
