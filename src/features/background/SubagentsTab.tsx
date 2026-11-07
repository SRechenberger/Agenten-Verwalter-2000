import { useRef } from 'react';
import type { ReactElement } from 'react';
import { buildPanelRows } from '@/features/background/buildPanelRows';
import type { PanelRow } from '@/features/background/buildPanelRows';
import { pickSelected, selectSubagents } from '@/features/background/backgroundItems';
import {
  formatDuration,
  stateIcon,
  stateLabel,
  stateRightTone,
  stateTone,
} from '@/features/background/backgroundLabels';
import { BackgroundRow } from '@/features/background/BackgroundRow';
import { DetailHead } from '@/features/background/DetailHead';
import type { DetailAction } from '@/features/background/DetailHead';
import { mentionInChat } from '@/features/background/mention';
import { OutputPane } from '@/features/background/OutputPane';
import { PanelLayout } from '@/features/background/PanelLayout';
import { useActionError } from '@/lib/useActionError';
import { VirtualPanelList } from '@/features/background/VirtualPanelList';
import type { BackgroundItem } from '@/lib/bindings/BackgroundItem';
import type { SubagentStep } from '@/lib/bindings/SubagentStep';
import { useSessionLocked } from '@/features/remote/useProjectAccess';
import { stopBackgroundItem } from '@/lib/background';
import { useBackgroundStore } from '@/stores/background';

const UNKNOWN = 'unbekannt';
const ESTIMATED_ROW_HEIGHT = 42;

interface SubagentsTabProps {
  sessionId: string;
  items: readonly BackgroundItem[];
  now: number;
}

export function SubagentsTab({ sessionId, items, now }: SubagentsTabProps): ReactElement {
  const isLocked: boolean = useSessionLocked(sessionId);
  const listRef = useRef<HTMLDivElement>(null);
  const subagents: BackgroundItem[] = selectSubagents(items);
  const selectedId: string | null = useBackgroundStore(
    (state) => state.selections[sessionId]?.subagents ?? null,
  );
  const select = useBackgroundStore((state) => state.select);
  const selected: BackgroundItem | null = pickSelected(subagents, selectedId);
  const { error, run } = useActionError(selected === null ? null : selected.id);

  function buildActions(item: BackgroundItem): DetailAction[] {
    if (item.state === 'running') {
      return [
        {
          label: 'Anhalten',
          hint: 'Hält den Subagenten an. Der Agent erfährt davon.',
          isDanger: true,
          isDisabled: isLocked,
          onClick: (): void => {
            run(() => stopBackgroundItem(sessionId, item.id));
          },
        },
      ];
    }
    return [
      {
        label: 'Ergebnis im Chat besprechen',
        hint: 'Setzt einen Verweis in das Eingabefeld.',
        isDanger: false,
        isDisabled: false,
        onClick: (): void => {
          mentionInChat(sessionId, `Subagent „${item.title}“`);
        },
      },
    ];
  }

  function renderDetail(): ReactElement | null {
    if (selected === null) {
      return null;
    }
    return (
      <>
        <DetailHead
          title={selected.title}
          isMono={false}
          meta={detailMeta(selected, now)}
          result={selected.result}
          actions={buildActions(selected)}
          error={error}
        />
        <OutputPane text={stepLines(selected.steps)} notice={null} followsEnd={false} />
      </>
    );
  }

  function renderRow(item: BackgroundItem): ReactElement {
    return (
      <BackgroundRow
        icon={stateIcon(item.state)}
        iconTone={stateTone(item.state)}
        title={item.title}
        isMono={false}
        subtitle={rowSubtitle(item, now)}
        right={stateLabel(item.state)}
        rightTone={stateRightTone(item.state)}
        indent={8}
        isCurrent={selected !== null && selected.id === item.id}
        onPick={(): void => {
          select(sessionId, 'subagents', item.id);
        }}
      />
    );
  }

  const rows: PanelRow<BackgroundItem>[] = buildPanelRows<BackgroundItem>([
    {
      title: 'SUBAGENTEN DIESER SESSION',
      items: subagents,
      emptyText: subagents.length === 0 ? 'Keine Subagenten gestartet.' : null,
      itemKey: (item: BackgroundItem): string => item.id,
    },
  ]);

  const list: ReactElement = (
    <VirtualPanelList
      rows={rows}
      listRef={listRef}
      renderItem={renderRow}
      estimateItem={(): number => ESTIMATED_ROW_HEIGHT}
      selectedKey={selected === null ? null : selected.id}
    />
  );

  return <PanelLayout listRef={listRef} list={list} detail={renderDetail()} />;
}

function typeAndModel(item: BackgroundItem): string {
  return `${item.subagentType ?? UNKNOWN} · ${item.model ?? UNKNOWN}`;
}

function durationOf(item: BackgroundItem, now: number): string {
  const end: number = item.endedAt ?? now;
  return formatDuration(end - item.startedAt);
}

/** Unterzeile: `<Typ> · <Modell> · n Aufrufe · <Dauer>`. */
function rowSubtitle(item: BackgroundItem, now: number): string {
  return `${typeAndModel(item)} · ${String(item.toolUses)} Aufrufe · ${durationOf(item, now)}`;
}

/** Meta-Zeile: `<Typ> · <Modell> · läuft seit / fertig nach <Dauer> · n Werkzeugaufrufe`. */
function detailMeta(item: BackgroundItem, now: number): string {
  const timing: string =
    item.state === 'running'
      ? `läuft seit ${durationOf(item, now)}`
      : `${stateLabel(item.state)} nach ${durationOf(item, now)}`;
  return `${typeAndModel(item)} · ${timing} · ${String(item.toolUses)} Werkzeugaufrufe`;
}

function stepLines(steps: readonly SubagentStep[]): string {
  return steps.map((step: SubagentStep) => `⎿ ${step.tool}  ${step.target}`).join('\n');
}
