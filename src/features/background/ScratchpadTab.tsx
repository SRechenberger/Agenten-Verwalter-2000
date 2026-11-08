import { useRef } from 'react';
import type { ReactElement } from 'react';
import { revealItemInDir } from '@tauri-apps/plugin-opener';
import { formatBytes } from '@/features/attachments/formatBytes';
import { formatTime } from '@/features/background/backgroundLabels';
import { BackgroundRow } from '@/features/background/BackgroundRow';
import { buildScratchpadRows } from '@/features/background/buildScratchpadRows';
import type { ScratchpadRow } from '@/features/background/buildScratchpadRows';
import { buildPanelRows } from '@/features/background/buildPanelRows';
import type { PanelRow } from '@/features/background/buildPanelRows';
import { DetailHead } from '@/features/background/DetailHead';
import type { DetailAction } from '@/features/background/DetailHead';
import { mentionInChat } from '@/features/background/mention';
import { OutputPane } from '@/features/background/OutputPane';
import { PanelLayout } from '@/features/background/PanelLayout';
import { useAssetUrl } from '@/features/remote/assetUrl';
import { useSessionIsRemote } from '@/features/remote/useProjectAccess';
import { useActionError } from '@/lib/useActionError';
import { useScratchpad } from '@/features/background/useScratchpad';
import { useScratchpadFile } from '@/features/background/useScratchpadFile';
import type { ScratchpadFileState } from '@/features/background/useScratchpadFile';
import { VirtualPanelList } from '@/features/background/VirtualPanelList';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import type { TextPreview } from '@/lib/bindings/TextPreview';
import { useBackgroundStore } from '@/stores/background';
import './ScratchpadTab.css';

const NO_FOLDER_TEXT =
  'Der Agent hat noch keinen Scratchpad-Ordner gemeldet — er entsteht beim ersten Start.';
const EMPTY_TEXT = 'Der Scratchpad-Ordner ist leer.';
const TRUNCATED_LIST_TEXT = 'Die Liste ist gekürzt.';
const TRUNCATED_FILE_NOTICE = 'Gekürzt — nur der Anfang wird gezeigt.';
const BINARY_NOTICE = 'Keine Textvorschau';
const ROW_INDENT_BASE = 8;
const ROW_INDENT_PER_LEVEL = 18;
const ESTIMATED_ROW_HEIGHT = 26;
/** Laufwerksbuchstabe (`C:`) oder UNC-Pfad (`\\`) am Anfang: der Core liefert dann Pfade mit `\`. */
const WINDOWS_PATH_PATTERN = /^(?:[A-Za-z]:|\\\\)/;

interface ScratchpadTabProps {
  session: SessionSummary;
}

export function ScratchpadTab({ session }: ScratchpadTabProps): ReactElement {
  const listRef = useRef<HTMLDivElement>(null);
  const { listing, error: listError } = useScratchpad(session);
  const selectedPath: string | null = useBackgroundStore(
    (state) => state.selections[session.id]?.scratchpad ?? null,
  );
  const select = useBackgroundStore((state) => state.select);

  const rows: ScratchpadRow[] = listing === null ? [] : buildScratchpadRows(listing.entries);
  const files: ScratchpadRow[] = rows.filter((row: ScratchpadRow) => !row.isDir);
  const selected: ScratchpadRow | null =
    files.find((row: ScratchpadRow) => row.path === selectedPath) ?? files[0] ?? null;
  const folder: string | null = listing === null ? null : listing.dir;
  // Gehört die Session einem Host, liegt der Ordner dort: Das Bild kommt über die Verbindung, und der
  // Explorer hier öffnete einen Pfad, den es auf diesem Gerät nicht gibt.
  const isRemote: boolean = useSessionIsRemote(session.id);
  const imageSrc: string = useAssetUrl(
    folder === null || selected === null ? '' : fullPath(folder, selected.path),
    session.id,
  );

  const textPath: string | null = selected !== null && !selected.isImage ? selected.path : null;
  const file: ScratchpadFileState = useScratchpadFile(
    session.id,
    textPath,
    selected === null ? null : selected.modifiedMs,
  );
  const { error: actionError, run } = useActionError(selected === null ? null : selected.path);

  function buildActions(row: ScratchpadRow): DetailAction[] {
    const actions: DetailAction[] = [
      {
        label: 'Im Chat besprechen',
        hint: 'Setzt einen Verweis in das Eingabefeld.',
        isDanger: false,
        isDisabled: false,
        onClick: (): void => {
          mentionInChat(session.id, row.name);
        },
      },
    ];
    if (!isRemote) {
      actions.push({
        label: 'Im Explorer zeigen',
        hint: 'Öffnet den Ordner im Windows-Explorer.',
        isDanger: false,
        isDisabled: folder === null,
        onClick: (): void => {
          if (folder !== null) {
            run(() => revealItemInDir(fullPath(folder, row.path)));
          }
        },
      });
    }
    return actions;
  }

  function renderPreview(row: ScratchpadRow): ReactElement {
    if (row.isImage) {
      return (
        <div className="scratchpad-tab__image">
          <img
            className="scratchpad-tab__image-content"
            alt={row.name}
            src={folder === null ? '' : imageSrc}
          />
        </div>
      );
    }
    const preview: TextPreview | null = file.preview;
    return (
      <OutputPane
        text={preview === null ? '' : preview.text}
        notice={previewNotice(preview)}
        followsEnd={false}
      />
    );
  }

  function renderDetail(): ReactElement | null {
    if (selected === null) {
      return null;
    }
    return (
      <>
        <DetailHead
          title={selected.name}
          isMono
          meta={`${formatBytes(selected.sizeBytes)} · geändert ${formatTime(selected.modifiedMs)} · im Scratchpad der Session`}
          result={null}
          actions={buildActions(selected)}
          error={actionError ?? file.error ?? listError}
        />
        {renderPreview(selected)}
      </>
    );
  }

  function renderRow(row: ScratchpadRow): ReactElement {
    return (
      <BackgroundRow
        icon={rowIcon(row)}
        iconTone="muted"
        title={row.name}
        isMono
        subtitle={
          row.isDir ? null : `${formatBytes(row.sizeBytes)} · ${formatTime(row.modifiedMs)}`
        }
        right=""
        rightTone="muted"
        indent={ROW_INDENT_BASE + row.depth * ROW_INDENT_PER_LEVEL}
        isCurrent={selected !== null && selected.path === row.path}
        onPick={
          row.isDir
            ? null
            : (): void => {
                select(session.id, 'scratchpad', row.path);
              }
        }
      />
    );
  }

  // Genau ein Text unter der Liste: Fehler, fehlender Ordner, leerer Ordner oder der Hinweis auf die gekürzte Liste.
  function footerText(): string | null {
    if (listing === null) {
      return listError;
    }
    if (listing.dir === null) {
      return NO_FOLDER_TEXT;
    }
    if (rows.length === 0) {
      return EMPTY_TEXT;
    }
    return listing.truncated ? TRUNCATED_LIST_TEXT : null;
  }

  const rowGroups: PanelRow<ScratchpadRow>[] = buildPanelRows<ScratchpadRow>([
    {
      title: folder ?? 'Scratchpad',
      ...(folder === null ? {} : { titleHint: folder }),
      items: rows,
      emptyText: footerText(),
      isEmptyError: listing === null,
      itemKey: (row: ScratchpadRow): string => row.path,
    },
  ]);

  const list: ReactElement = (
    <VirtualPanelList
      rows={rowGroups}
      listRef={listRef}
      renderItem={renderRow}
      estimateItem={(): number => ESTIMATED_ROW_HEIGHT}
      selectedKey={selected === null ? null : selected.path}
    />
  );

  return <PanelLayout listRef={listRef} list={list} detail={renderDetail()} />;
}

function rowIcon(row: ScratchpadRow): string {
  if (row.isDir) {
    return '▾';
  }
  return row.isImage ? '▣' : '≡';
}

/** Der volle Pfad: Scratchpad-Ordner plus relativer Pfad, mit dem Trenner, den der Ordnerpfad selbst trägt. */
function fullPath(folder: string, relativePath: string): string {
  const separator: string = WINDOWS_PATH_PATTERN.test(folder) ? '\\' : '/';
  return `${folder}${separator}${relativePath.split('/').join(separator)}`;
}

function previewNotice(preview: TextPreview | null): string | null {
  if (preview === null) {
    return null;
  }
  if (preview.binary) {
    return BINARY_NOTICE;
  }
  return preview.truncated ? TRUNCATED_FILE_NOTICE : null;
}
