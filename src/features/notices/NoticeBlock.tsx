import type { ReactElement } from 'react';
import { CompactDivider } from '@/features/notices/CompactDivider';
import { InfoIcon, LockIcon, ModelSwitchIcon, ShieldIcon } from '@/features/notices/NoticeIcons';
import { NoticeLine } from '@/features/notices/NoticeLine';
import {
  INFO_LEAD,
  MODEL_SWITCH_LEAD,
  MODE_CHANGED_LEAD,
  isFallbackRevert,
  modeChangedText,
  modelFallbackActionLabel,
  modelFallbackLead,
  modelFallbackText,
  modelSwitchedText,
  REPORT_UPDATED_LEAD,
  reportUpdatedText,
} from '@/features/notices/noticeTexts';
import { TurnSummaryLine } from '@/features/notices/TurnSummaryLine';
import type { ModelId } from '@/lib/bindings/ModelId';
import type { Notice } from '@/lib/bindings/Notice';
import { commandErrorText } from '@/lib/errors';
import { reportedModelName } from '@/lib/labels';
import { setSessionModel } from '@/lib/sessions';
import { useSessionErrorsStore } from '@/stores/sessionErrors';
import { useSettingsStore } from '@/stores/settings';

const ICON_SIZE = 14;
const NO_SEQS: readonly number[] = [];

interface NoticeBlockProps {
  sessionId: string;
  notice: Notice;
  isExpanded: boolean;
  onToggle: () => void;
  /** Die Claude-Kommandozeile mit dem Claude-Abo ist die Betriebsart; nur dort gibt es den Prompt-Cache. */
  isClaude: boolean;
  /** Die Session antwortet gerade mit einem Ersatzmodell; nur dann führt der Knopf zurück. */
  isFallbackActive: boolean;
  /** Scrollt zum Text mit dieser `seq` und setzt den Fokus auf seine erste Angabe ohne Fundstelle. */
  onJumpToEntry: (seq: number) => void;
  /** Summe der Angaben in den Texten mit diesen `seq`; `null`, wenn einer davon nicht geladen ist. */
  countUnsourced: (seqs: readonly number[]) => number | null;
}

export function NoticeBlock({
  sessionId,
  notice,
  isExpanded,
  onToggle,
  isClaude,
  isFallbackActive,
  onJumpToEntry,
  countUnsourced,
}: NoticeBlockProps): ReactElement {
  const reportSessionError = useSessionErrorsStore((state) => state.report);
  const marksUnsourced: boolean = useSettingsStore(
    (state) => state.settings?.markUnsourced ?? true,
  );

  switch (notice.kind) {
    case 'compacted':
      return (
        <CompactDivider
          notice={notice}
          isExpanded={isExpanded}
          onToggle={onToggle}
          isClaude={isClaude}
        />
      );
    case 'modelSwitched':
      return (
        <NoticeLine
          variant="model"
          icon={<ModelSwitchIcon size={ICON_SIZE} />}
          lead={MODEL_SWITCH_LEAD}
          text={modelSwitchedText(notice.from, notice.to, notice.recacheTokens)}
        />
      );
    case 'modelFallback': {
      const originalName: string = reportedModelName(notice.originalId, notice.originalModel);
      const fallbackName: string = reportedModelName(notice.fallbackId, notice.fallbackModel);
      const returnModel: ModelId | null =
        isFallbackActive && !isFallbackRevert(notice.direction) ? notice.originalId : null;
      return (
        <NoticeLine
          variant="warn"
          icon={<ShieldIcon size={ICON_SIZE} />}
          lead={modelFallbackLead(notice.direction)}
          text={modelFallbackText(notice.direction, originalName, fallbackName)}
          detail={notice.text ?? undefined}
          action={
            returnModel !== null
              ? {
                  label: modelFallbackActionLabel(originalName),
                  onClick: (): void => {
                    setSessionModel(sessionId, returnModel).catch((reason: unknown) => {
                      console.error('Modell nicht gewechselt', reason);
                      reportSessionError(
                        sessionId,
                        `Modell nicht gewechselt: ${commandErrorText(reason)}`,
                      );
                    });
                  },
                }
              : undefined
          }
        />
      );
    }
    case 'info':
    case 'quotaExhausted':
      return (
        <NoticeLine
          variant="info"
          icon={<InfoIcon size={ICON_SIZE} />}
          lead={INFO_LEAD}
          text={notice.text}
        />
      );
    case 'modeChanged':
      return (
        <NoticeLine
          variant="info"
          icon={<LockIcon size={ICON_SIZE} />}
          lead={MODE_CHANGED_LEAD}
          text={modeChangedText(notice.from, notice.to)}
        />
      );
    case 'reportUpdated':
      return (
        <NoticeLine
          variant="info"
          icon={<InfoIcon size={ICON_SIZE} />}
          lead={REPORT_UPDATED_LEAD}
          text={reportUpdatedText(notice.at, notice.summary)}
        />
      );
    case 'turnSummary': {
      const unsourcedSeqs: readonly number[] = marksUnsourced ? notice.unsourced : NO_SEQS;
      return (
        <TurnSummaryLine
          denials={notice.denials}
          costUsd={notice.costUsd}
          unsourced={unsourcedSeqs}
          unsourcedCount={countUnsourced(unsourcedSeqs)}
          onJump={onJumpToEntry}
          isExpanded={isExpanded}
          onToggle={onToggle}
        />
      );
    }
  }
}
