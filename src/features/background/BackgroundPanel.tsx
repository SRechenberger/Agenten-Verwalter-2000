import type { ReactElement } from 'react';
import { selectSubagents, splitProcesses } from '@/features/background/backgroundItems';
import type { ProcessGroups } from '@/features/background/backgroundItems';
import { ProcessesTab } from '@/features/background/ProcessesTab';
import { ScratchpadTab } from '@/features/background/ScratchpadTab';
import { SubagentsTab } from '@/features/background/SubagentsTab';
import { useNow } from '@/features/background/useNow';
import { SideTab } from '@/features/side/SideTab';
import { useSideQuestions } from '@/features/side/useSideQuestions';
import type { BackgroundItem } from '@/lib/bindings/BackgroundItem';
import type { SessionBackground } from '@/lib/bindings/SessionBackground';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import { BACKGROUND_TABS, useBackgroundStore } from '@/stores/background';
import type { BackgroundTab } from '@/stores/background';
import './BackgroundPanel.css';

const TAB_LABEL: Record<BackgroundTab, string> = {
  processes: 'Prozesse',
  subagents: 'Subagenten',
  scratchpad: 'Scratchpad',
  side: 'Nebenfragen',
};

const NO_ITEMS: readonly BackgroundItem[] = [];

interface BackgroundPanelProps {
  session: SessionSummary;
  background: SessionBackground | null;
  /** Fehler beim Laden der Einträge; `null` ohne. */
  error: string | null;
}

export function BackgroundPanel({
  session,
  background,
  error,
}: BackgroundPanelProps): ReactElement {
  const tab: BackgroundTab = useBackgroundStore((state) => state.tab);
  const showTab = useBackgroundStore((state) => state.showTab);
  const close = useBackgroundStore((state) => state.close);
  const sideQuestions = useSideQuestions(session.id);

  const items: readonly BackgroundItem[] = background === null ? NO_ITEMS : background.items;
  const now: number = useNow(items.some((item: BackgroundItem) => item.state === 'running'));

  function countFor(target: BackgroundTab): number | null {
    if (target === 'subagents') {
      return selectSubagents(items).length;
    }
    if (target === 'processes') {
      const groups: ProcessGroups = splitProcesses(items);
      return groups.running.length + groups.executed.length;
    }
    if (target === 'side') {
      return sideQuestions.questions.length;
    }
    // Die Zahl der Dateien kennt nur der geöffnete Reiter.
    return null;
  }

  function renderTab(): ReactElement {
    switch (tab) {
      case 'processes':
        return <ProcessesTab sessionId={session.id} items={items} now={now} />;
      case 'subagents':
        return <SubagentsTab sessionId={session.id} items={items} now={now} />;
      case 'scratchpad':
        return <ScratchpadTab session={session} />;
      case 'side':
        return <SideTab session={session} />;
    }
  }

  return (
    <aside className="background-panel" aria-label="Hintergrund der Session">
      <div className="background-panel__head">
        <div className="background-panel__tabs" role="tablist" aria-label="Hintergrund">
          {BACKGROUND_TABS.map((target: BackgroundTab) => {
            const count: number | null = countFor(target);
            return (
              <button
                key={target}
                type="button"
                className={`background-panel__tab${target === tab ? ' background-panel__tab--active' : ''}`}
                role="tab"
                aria-selected={target === tab}
                onClick={(): void => {
                  showTab(target);
                }}
              >
                {TAB_LABEL[target]}
                {count !== null && count > 0 && (
                  <span className="background-panel__tab-count">{String(count)}</span>
                )}
              </button>
            );
          })}
        </div>
        <button
          type="button"
          className="background-panel__close"
          aria-label="Hintergrund schließen"
          onClick={close}
        >
          <svg
            width="13"
            height="13"
            viewBox="0 0 14 14"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
            aria-hidden="true"
          >
            <path d="M3.5 3.5l7 7M10.5 3.5l-7 7" />
          </svg>
        </button>
      </div>
      {error !== null && (
        <p className="background-panel__error" role="alert">
          {error}
        </p>
      )}
      {renderTab()}
    </aside>
  );
}
