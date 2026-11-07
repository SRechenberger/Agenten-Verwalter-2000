import { useRef } from 'react';
import type { ReactElement } from 'react';
import { openUrl } from '@tauri-apps/plugin-opener';
import { buildPanelRows } from '@/features/background/buildPanelRows';
import type { PanelRow } from '@/features/background/buildPanelRows';
import { pickSelected, splitProcesses } from '@/features/background/backgroundItems';
import type { ProcessGroups } from '@/features/background/backgroundItems';
import {
  formatDuration,
  formatTime,
  stateIcon,
  stateLabel,
  stateRightTone,
  stateTone,
} from '@/features/background/backgroundLabels';
import { BackgroundRow } from '@/features/background/BackgroundRow';
import type { RowTone } from '@/features/background/backgroundLabels';
import { DetailHead } from '@/features/background/DetailHead';
import type { DetailAction } from '@/features/background/DetailHead';
import { mentionInChat } from '@/features/background/mention';
import { OutputPane } from '@/features/background/OutputPane';
import { PanelLayout } from '@/features/background/PanelLayout';
import { useActionError } from '@/lib/useActionError';
import { useCopyFeedback } from '@/features/background/useCopyFeedback';
import { useItemOutput } from '@/features/background/useItemOutput';
import { VirtualPanelList } from '@/features/background/VirtualPanelList';
import type { BackgroundItem } from '@/lib/bindings/BackgroundItem';
import type { TextPreview } from '@/lib/bindings/TextPreview';
import { useSessionLocked } from '@/features/remote/useProjectAccess';
import { stopBackgroundItem } from '@/lib/background';
import { useBackgroundStore } from '@/stores/background';

const RUNNING_LABEL = 'läuft';
const EXIT_LABEL_PREFIX = 'Exit ';
const TRUNCATED_NOTICE = 'Gekürzt — nur das Ende wird gezeigt.';
const MISSING_NOTICE = 'Ausgabe nicht mehr verfügbar.';
const MISSING_RUNNING_NOTICE =
  'Ausgabe noch nicht verfügbar — sie liegt erst vor, wenn der Befehl endet.';
const COPY_LABEL: Record<'idle' | 'copied' | 'failed', string> = {
  idle: 'Ausgabe kopieren',
  copied: 'Kopiert',
  failed: 'Fehlgeschlagen',
};
const MENTION_MAX_LENGTH = 80;
const ESTIMATED_ROW_HEIGHT = 42;

interface ProcessesTabProps {
  sessionId: string;
  items: readonly BackgroundItem[];
  now: number;
}

export function ProcessesTab({ sessionId, items, now }: ProcessesTabProps): ReactElement {
  const isLocked: boolean = useSessionLocked(sessionId);
  const listRef = useRef<HTMLDivElement>(null);
  const groups: ProcessGroups = splitProcesses(items);
  const selectedId: string | null = useBackgroundStore(
    (state) => state.selections[sessionId]?.processes ?? null,
  );
  const select = useBackgroundStore((state) => state.select);
  const selected: BackgroundItem | null = pickSelected(
    [...groups.running, ...groups.executed],
    selectedId,
  );
  const { preview: output, error: outputError } = useItemOutput(sessionId, selected);
  const { error, run } = useActionError(selected === null ? null : selected.id);
  const copyFeedback = useCopyFeedback();

  function renderRow(item: BackgroundItem): ReactElement {
    return (
      <BackgroundRow
        icon={stateIcon(item.state)}
        iconTone={stateTone(item.state)}
        title={item.title}
        isMono
        subtitle={rowSubtitle(item, now)}
        right={rowRight(item)}
        rightTone={rowRightTone(item)}
        indent={8}
        isCurrent={selected !== null && selected.id === item.id}
        onPick={(): void => {
          select(sessionId, 'processes', item.id);
        }}
      />
    );
  }

  function buildActions(item: BackgroundItem): DetailAction[] {
    if (item.state === 'running') {
      const actions: DetailAction[] = [];
      const address: string | null = item.url;
      if (address !== null) {
        actions.push({
          label: 'Im Browser öffnen',
          hint: 'Öffnet die Adresse im Standardbrowser.',
          isDanger: false,
          isDisabled: false,
          onClick: (): void => {
            run(() => openUrl(address));
          },
        });
      }
      actions.push({
        label: 'Beenden',
        hint: 'Hält den Prozess an. Der Agent erfährt davon.',
        isDanger: true,
        isDisabled: isLocked,
        onClick: (): void => {
          run(() => stopBackgroundItem(sessionId, item.id));
        },
      });
      return actions;
    }
    return [
      {
        label: 'Im Chat besprechen',
        hint: 'Setzt einen Verweis in das Eingabefeld.',
        isDanger: false,
        isDisabled: false,
        onClick: (): void => {
          mentionInChat(sessionId, mentionLabel(item.title));
        },
      },
      {
        label: COPY_LABEL[copyFeedback.state],
        hint: 'Kopiert die Ausgabe in die Zwischenablage.',
        isDanger: false,
        isDisabled: output === null || output.missing,
        onClick: (): void => {
          copyFeedback.copy(output?.text ?? '');
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
          isMono
          meta={detailMeta(selected, now)}
          result={null}
          actions={buildActions(selected)}
          error={error}
        />
        <OutputPane
          text={output === null ? '' : output.text}
          notice={outputError ?? outputNotice(output, selected.state)}
          followsEnd={selected.state === 'running'}
        />
      </>
    );
  }

  const rows: PanelRow<BackgroundItem>[] = buildPanelRows<BackgroundItem>([
    {
      title: 'LÄUFT',
      items: groups.running,
      emptyText: groups.running.length === 0 ? 'Keine laufenden Prozesse.' : null,
      itemKey: (item: BackgroundItem): string => item.id,
    },
    {
      title: 'AUSGEFÜHRT',
      items: groups.executed,
      emptyText: groups.executed.length === 0 ? 'Noch keine Befehle ausgeführt.' : null,
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

function outputNotice(output: TextPreview | null, state: BackgroundItem['state']): string | null {
  if (output === null) {
    return null;
  }
  if (output.missing) {
    return state === 'running' ? MISSING_RUNNING_NOTICE : MISSING_NOTICE;
  }
  return output.truncated ? TRUNCATED_NOTICE : null;
}

function elapsedSince(item: BackgroundItem, now: number): string {
  return formatDuration(now - item.startedAt);
}

function durationOf(item: BackgroundItem): string | null {
  return item.endedAt === null ? null : formatDuration(item.endedAt - item.startedAt);
}

/** Unterzeile: läuft → `<Adresse> · seit <Laufzeit>`, sonst `<Uhrzeit> · <Dauer>`. */
function rowSubtitle(item: BackgroundItem, now: number): string {
  if (item.state === 'running') {
    const since = `seit ${elapsedSince(item, now)}`;
    return item.url === null ? since : `${item.url} · ${since}`;
  }
  const duration: string | null = durationOf(item);
  return duration === null
    ? formatTime(item.startedAt)
    : `${formatTime(item.startedAt)} · ${duration}`;
}

function rowRight(item: BackgroundItem): string {
  switch (item.state) {
    case 'running':
      return RUNNING_LABEL;
    case 'stopped':
    case 'interrupted':
      return stateLabel(item.state);
    case 'completed':
    case 'failed':
      if (item.exitCode !== null && item.exitCode !== 0) {
        return `${EXIT_LABEL_PREFIX}${String(item.exitCode)}`;
      }
      return item.state === 'failed' ? stateLabel('failed') : '';
  }
}

function rowRightTone(item: BackgroundItem): RowTone {
  return stateRightTone(item.state);
}

/** Meta-Zeile: läuft → `läuft seit <Laufzeit> · <Adresse>`, sonst `<Uhrzeit> · <Dauer> · Exit-Code n`. */
function detailMeta(item: BackgroundItem, now: number): string {
  if (item.state === 'running') {
    const since = `läuft seit ${elapsedSince(item, now)}`;
    return item.url === null ? since : `${since} · ${item.url}`;
  }
  const parts: string[] = [formatTime(item.startedAt)];
  const duration: string | null = durationOf(item);
  if (duration !== null) {
    parts.push(duration);
  }
  parts.push(item.exitCode === null ? 'Exit-Code unbekannt' : `Exit-Code ${String(item.exitCode)}`);
  return parts.join(' · ');
}

/** Ein Befehl über mehrere Zeilen würde den Entwurf fluten: nur die erste Zeile, gekürzt. */
function mentionLabel(command: string): string {
  const firstLine: string = command.split('\n')[0]?.trim() ?? '';
  const isCut: boolean = firstLine.length > MENTION_MAX_LENGTH || firstLine !== command.trim();
  const shortened: string = firstLine.slice(0, MENTION_MAX_LENGTH);
  return isCut ? `${shortened} …` : shortened;
}
