import type { ReactElement } from 'react';
import { StatusIcon } from '@/components/StatusIcon';
import { formatCount } from '@/features/changes/changesScope';
import { activeLaneCount } from '@/features/implementation/implementationOrder';
import {
  implementationTitle,
  INQUIRY_LABEL,
  INQUIRY_TITLE,
  lanesActiveText,
  REPORT_LABEL,
  REPORT_TITLE,
  VIEW_LABEL,
} from '@/features/implementation/implementationTexts';
import { stateIcon, stateLabel } from '@/features/implementation/stationState';
import type { ImplementationView } from '@/lib/bindings/ImplementationView';
import { IMPLEMENTATION_VIEWS, type ImplementationViewKind } from '@/stores/sessions';
import '@/app/SessionHeader.css';
import './ImplementationBar.css';

interface ImplementationHeaderProps {
  implementation: ImplementationView;
  projectName: string;
  activeView: ImplementationViewKind;
  /** Ohne Repository gibt es keine Changes und keinen Reiter dafür. */
  hasChanges: boolean;
  /** Dateien unter „Alle“; `null`, solange die Changes nicht gelesen sind. */
  changesCount: number | null;
  isReportShown: boolean;
  onToggleReport: () => void;
  onNewInquiry: () => void;
  onShowView: (view: ImplementationViewKind) => void;
  onOpenProject: () => void;
}

/** Kopfzeile „<Vorhaben> › Umsetzung <Name>“ mit den Reitern „Ablauf · Changes“ und dem Fortschritt. */
export function ImplementationHeader({
  implementation,
  projectName,
  activeView,
  hasChanges,
  changesCount,
  isReportShown,
  onToggleReport,
  onNewInquiry,
  onShowView,
  onOpenProject,
}: ImplementationHeaderProps): ReactElement {
  const views: readonly ImplementationViewKind[] = IMPLEMENTATION_VIEWS.filter(
    (view: ImplementationViewKind) => view === 'flow' || hasChanges,
  );
  const activeLanes: number = activeLaneCount(implementation);
  return (
    <header className="session-header">
      <div className="session-header__title">
        <button
          type="button"
          className="session-header__project"
          title="Übersicht des Vorhabens öffnen"
          onClick={onOpenProject}
        >
          {projectName}
        </button>
        <svg
          className="session-header__crumb"
          width="11"
          height="11"
          viewBox="0 0 12 12"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.5"
          strokeLinecap="round"
          strokeLinejoin="round"
          aria-hidden="true"
        >
          <path d="M4.5 2.5 8 6l-3.5 3.5" />
        </svg>
        <h1 className="session-header__name">{implementationTitle(implementation)}</h1>
        <span
          className={`session-header__status session-header__status--${stateIcon(implementation.urgency)}`}
        >
          <StatusIcon status={stateIcon(implementation.urgency)} size={10} />
          <span>{stateLabel(implementation.urgency)}</span>
        </span>
        {activeLanes >= 2 && (
          <span className="implementation-bar__lanes">{lanesActiveText(activeLanes)}</span>
        )}
      </div>
      <div className="session-header__tabs" role="tablist" aria-label="Ansicht">
        {views.map((view: ImplementationViewKind) => (
          <button
            key={view}
            type="button"
            className={`session-header__tab${view === activeView ? ' session-header__tab--active' : ''}`}
            role="tab"
            aria-selected={view === activeView}
            onClick={(): void => {
              onShowView(view);
            }}
          >
            {VIEW_LABEL[view]}
            {view === 'changes' && changesCount !== null && changesCount > 0 && (
              <span className="session-header__tab-count">{formatCount(changesCount)}</span>
            )}
          </button>
        ))}
      </div>
      <div className="session-header__tools">
        <button
          type="button"
          className="implementation-bar__button"
          title={INQUIRY_TITLE}
          onClick={onNewInquiry}
        >
          {INQUIRY_LABEL}
        </button>
        <button
          type="button"
          className="implementation-bar__button"
          aria-pressed={isReportShown}
          title={REPORT_TITLE}
          onClick={onToggleReport}
        >
          {REPORT_LABEL}
        </button>
      </div>
    </header>
  );
}
