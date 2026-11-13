import type { ReactElement } from 'react';
import { SeverityIcon } from '@/components/SeverityIcon';
import { useNow } from '@/features/background/useNow';
import { CompactDivider } from '@/features/notices/CompactDivider';
import { InfoIcon, LockIcon, ModelSwitchIcon, ShieldIcon } from '@/features/notices/NoticeIcons';
import { NoticeLine } from '@/features/notices/NoticeLine';
import { PlanMissingActions } from '@/features/notices/PlanMissingActions';
import { QuotaResumeSwitch } from '@/features/notices/QuotaResumeSwitch';
import {
  INFO_LEAD,
  MODEL_SWITCH_LEAD,
  MODE_CHANGED_LEAD,
  PLAN_MISSING_LEAD,
  PLAN_MISSING_SETTLED,
  PLAN_MISSING_TEXT,
  QUOTA_LEAD,
  isFallbackRevert,
  modeChangedText,
  modelFallbackActionLabel,
  modelFallbackLead,
  modelFallbackText,
  modelSwitchedText,
  REPORT_UPDATED_LEAD,
  reportUpdatedText,
  quotaResetText,
} from '@/features/notices/noticeTexts';
import { TurnSummaryLine } from '@/features/notices/TurnSummaryLine';
import type { ModelId } from '@/lib/bindings/ModelId';
import type { Notice } from '@/lib/bindings/Notice';
import type { PauseReason } from '@/lib/bindings/PauseReason';
import { commandErrorText } from '@/lib/errors';
import { reportedModelName } from '@/lib/labels';
import { useSessionLocked } from '@/features/remote/useProjectAccess';
import { setSessionModel } from '@/lib/sessions';
import { useSessionErrorsStore } from '@/stores/sessionErrors';
import { useSettingsStore } from '@/stores/settings';

const ICON_SIZE = 14;
const NO_SEQS: readonly number[] = [];

interface NoticeBlockProps {
  sessionId: string;
  /** `seq` des Eintrags; damit adressieren die Knöpfe von „Kein Plan im Umsetzungstool“ ihren Hinweis. */
  seq: number;
  notice: Notice;
  isExpanded: boolean;
  onToggle: () => void;
  /** Die Claude-Kommandozeile mit dem Claude-Abo ist die Betriebsart; nur dort gibt es den Prompt-Cache. */
  isClaude: boolean;
  /** Die Session antwortet gerade mit einem Ersatzmodell; nur dann führt der Knopf zurück. */
  isFallbackActive: boolean;
  /** Dies ist die jüngste Kontingent-Karte im Verlauf; nur sie trägt den Schalter. */
  isLatestQuotaNotice: boolean;
  /** Begründung der aktuellen Pause; `null`, sobald die Session weiterläuft. */
  pauseReason: PauseReason | null;
  /** Scrollt zum Text mit dieser `seq` und setzt den Fokus auf seine erste Angabe ohne Fundstelle. */
  onJumpToEntry: (seq: number) => void;
  /** Summe der Angaben in den Texten mit diesen `seq`; `null`, wenn einer davon nicht geladen ist. */
  countUnsourced: (seqs: readonly number[]) => number | null;
}

export function NoticeBlock({
  sessionId,
  seq,
  notice,
  isExpanded,
  onToggle,
  isClaude,
  isFallbackActive,
  isLatestQuotaNotice,
  pauseReason,
  onJumpToEntry,
  countUnsourced,
}: NoticeBlockProps): ReactElement {
  const isLocked: boolean = useSessionLocked(sessionId);
  const reportSessionError = useSessionErrorsStore((state) => state.report);
  const marksUnsourced: boolean = useSettingsStore(
    (state) => state.settings?.markUnsourced ?? true,
  );
  // Tickt, damit der Satz zur Rücksetzzeit verschwindet, sobald sie vorbei ist.
  const now: number = useNow(notice.kind === 'quotaExhausted' && notice.resetsAt !== null);

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
                  isDisabled: isLocked,
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
      return (
        <NoticeLine
          variant="info"
          icon={<InfoIcon size={ICON_SIZE} />}
          lead={INFO_LEAD}
          text={notice.text}
        />
      );
    case 'quotaExhausted':
      return (
        <NoticeLine
          variant="warn"
          icon={<SeverityIcon severity="caution" size={ICON_SIZE} />}
          lead={QUOTA_LEAD}
          text={notice.text}
          detail={quotaResetText(notice.resetsAt, now) ?? undefined}
          control={
            isLatestQuotaNotice &&
            pauseReason?.kind === 'quota' &&
            pauseReason.resetsAt !== null ? (
              <QuotaResumeSwitch sessionId={sessionId} enabled={pauseReason.autoResume} />
            ) : undefined
          }
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
    case 'planMissing':
      return (
        <NoticeLine
          variant="warn"
          icon={<SeverityIcon severity="caution" size={ICON_SIZE} />}
          lead={PLAN_MISSING_LEAD}
          text={PLAN_MISSING_TEXT}
          detail={notice.settled ? PLAN_MISSING_SETTLED : undefined}
          control={
            notice.settled ? undefined : <PlanMissingActions sessionId={sessionId} seq={seq} />
          }
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
