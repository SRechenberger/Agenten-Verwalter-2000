import type { ReactElement } from 'react';
import { ModelMenu } from '@/components/ModelMenu';
import type { ModelMenuEffort, ModelMenuWarning } from '@/components/ModelMenu';
import { useNow } from '@/features/background/useNow';
import { formatThousands } from '@/features/context/formatTokens';
import { promptCacheState } from '@/features/promptcache/promptCacheState';
import type { PromptCacheState } from '@/features/promptcache/promptCacheState';
import { useOperatingMode } from '@/features/remote/useOperatingMode';
import type { ModelId } from '@/lib/bindings/ModelId';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import { MODEL_OPTIONS, modelName } from '@/lib/labels';

interface CacheAwareModelMenuProps {
  session: SessionSummary;
  onChange: (model: ModelId) => void;
  onClose: () => void;
  note: string;
  /** Wird unverändert an `ModelMenu` durchgereicht. */
  effort?: ModelMenuEffort | undefined;
}

/** Das Modell-Menü für Claude-Modelle; warnt, solange ein Modellwechsel einen warmen Cache verwirft. */
export function CacheAwareModelMenu({
  session,
  onChange,
  onClose,
  note,
  effort,
}: CacheAwareModelMenuProps): ReactElement {
  // Eine eigene Uhr, die nur läuft, solange das Menü offen ist: die Eingabeleiste rendert nicht jede Sekunde mit.
  const now: number = useNow(true);
  const hasPromptCache: boolean = useOperatingMode(session.projectId) === 'claude';
  const state: PromptCacheState = promptCacheState(session.promptCache, session.model, now);
  let warning: ModelMenuWarning | undefined;
  if (hasPromptCache && state.kind === 'warm' && session.promptCache !== null) {
    warning = {
      lead: `Der Cache gilt nur für ${modelName(session.promptCache.model)}`,
      text: `und hält noch ${String(state.minutesLeft)} min. Mit einem anderen Modell schreibt die nächste Nachricht rund ${formatThousands(session.promptCache.tokens)} Token neu.`,
    };
  }
  return (
    <ModelMenu
      options={MODEL_OPTIONS}
      value={session.model}
      onChange={onChange}
      onClose={onClose}
      placement="above"
      note={note}
      warning={warning}
      effort={effort}
    />
  );
}
