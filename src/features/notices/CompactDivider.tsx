import type { ReactElement } from 'react';
import { Markdown } from '@/components/Markdown';
import { CompactIcon } from '@/features/notices/NoticeIcons';
import {
  COMPACT_CACHE_REBUILD,
  COMPACT_BY_VERWALTER,
  COMPACT_LEAD,
  SUMMARY_HIDE,
  SUMMARY_SHOW,
  compactTokensText,
  compactTriggerText,
} from '@/features/notices/noticeTexts';
import type { Notice } from '@/lib/bindings/Notice';
import './CompactDivider.css';

const ICON_SIZE = 13;
const PART_SEPARATOR = ' · ';

export type CompactedNotice = Extract<Notice, { kind: 'compacted' }>;

interface CompactDividerProps {
  notice: CompactedNotice;
  isExpanded: boolean;
  onToggle: () => void;
  /** Der Cache-Satz gilt nur, wenn die Claude-Kommandozeile mit dem Claude-Abo läuft. */
  isClaude: boolean;
}

export function CompactDivider({
  notice,
  isExpanded,
  onToggle,
  isClaude,
}: CompactDividerProps): ReactElement {
  const parts: string[] = [];
  // Der Verwalter schickt `/compact` selbst; die Kommandozeile meldet es trotzdem als „manual“.
  const triggerText: string | null = notice.byVerwalter
    ? COMPACT_BY_VERWALTER
    : compactTriggerText(notice.trigger);
  if (triggerText !== null) {
    parts.push(triggerText);
  }
  const tokensText: string | null = compactTokensText(notice.preTokens, notice.postTokens);
  if (tokensText !== null) {
    parts.push(tokensText);
  }
  if (isClaude) {
    parts.push(COMPACT_CACHE_REBUILD);
  }

  return (
    <div className="compact-divider">
      <div className="compact-divider__line">
        <span className="compact-divider__pill">
          <CompactIcon size={ICON_SIZE} />
          <span>
            <strong>{COMPACT_LEAD}</strong>
            {parts.map((part: string) => `${PART_SEPARATOR}${part}`).join('')}
          </span>
          {notice.summary !== null && (
            <button
              type="button"
              className="compact-divider__toggle"
              aria-expanded={isExpanded}
              onClick={onToggle}
            >
              {isExpanded ? SUMMARY_HIDE : SUMMARY_SHOW}
            </button>
          )}
        </span>
      </div>
      {isExpanded && notice.summary !== null && (
        <div className="compact-divider__summary">
          <Markdown text={notice.summary} />
        </div>
      )}
    </div>
  );
}
