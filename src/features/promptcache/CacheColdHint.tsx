import { useState } from 'react';
import type { ReactElement } from 'react';
import { SnowflakeIcon } from '@/components/SnowflakeIcon';
import { useNow } from '@/features/background/useNow';
import { focusComposer, setHandoffDraft } from '@/features/chat/composerDraft';
import { formatThousands } from '@/features/context/formatTokens';
import {
  COLD_HINT_MIN_TOKENS,
  formatIdle,
  promptCacheState,
} from '@/features/promptcache/promptCacheState';
import type { PromptCacheState } from '@/features/promptcache/promptCacheState';
import type { HandoffBrief } from '@/lib/bindings/HandoffBrief';
import type { PromptCache } from '@/lib/bindings/PromptCache';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import { commandErrorText } from '@/lib/errors';
import { modelName } from '@/lib/labels';
import { createSessionInProject } from '@/lib/sessions';
import { createHandoffBrief } from '@/lib/tldr';
import { useSettingsStore } from '@/stores/settings';
import './CacheColdHint.css';

const ICON_SIZE = 14;

const EXPLANATION =
  'Claude hält den Verlauf nach jeder Antwort eine Zeit lang bereit (Prompt-Cache). Danach liest er ihn neu ein — das zählt deutlich stärker aufs Kontingent als das Lesen aus dem Cache.';
const CONTINUE_TITLE =
  'Haiku fasst den Gesprächstext dieser Session zusammen (wenige Sekunden, kostet wenig), legt im Vorhaben eine neue Session an und setzt die Zusammenfassung in ihr Eingabefeld. Gesendet wird erst, wenn du auf Senden klickst.';

type ContinuePhase = 'idle' | 'summarizing' | 'creating';

const CONTINUE_LABELS: Record<ContinuePhase, string> = {
  idle: 'In neuer Session weiter',
  summarizing: 'Fasse zusammen …',
  creating: 'Lege Session an …',
};

interface CacheColdHintProps {
  session: SessionSummary;
  /** Nimmt die angelegte Session in die Liste und wählt sie aus (`handleSessionCreated` in `App`). */
  onSessionCreated: (created: SessionSummary) => void;
}

export function CacheColdHint({
  session,
  onSessionCreated,
}: CacheColdHintProps): ReactElement | null {
  const [phase, setPhase] = useState<ContinuePhase>('idle');
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  // Cache gibt es nur mit der Claude-Kommandozeile und dem Claude-Abo.
  const hasPromptCache: boolean = useSettingsStore(
    (state) => (state.settings?.operatingMode ?? 'claude') === 'claude',
  );
  const now: number = useNow(hasPromptCache && session.promptCache !== null);
  const cache: PromptCache | null = session.promptCache;
  const state: PromptCacheState = promptCacheState(cache, session.model, now);

  async function continueInNewSession(): Promise<void> {
    setPhase('summarizing');
    setErrorMessage(null);
    let brief: HandoffBrief;
    try {
      brief = await createHandoffBrief(session.id);
    } catch (reason: unknown) {
      setErrorMessage(commandErrorText(reason));
      setPhase('idle');
      return;
    }
    setPhase('creating');
    let created: SessionSummary;
    try {
      created = await createSessionInProject(session.projectId);
    } catch (reason: unknown) {
      setErrorMessage(commandErrorText(reason));
      setPhase('idle');
      return;
    }
    onSessionCreated(created);
    setHandoffDraft(created.id, brief.text);
    focusComposer();
    setPhase('idle');
  }

  function renderText(
    coldState: Extract<PromptCacheState, { kind: 'cold' }>,
    activeCache: PromptCache,
  ): ReactElement {
    const amount: string = formatThousands(activeCache.tokens);
    if (coldState.reason === 'expired') {
      return (
        <>
          <strong>{formatIdle(coldState.idleMs)} Pause, der Cache ist abgelaufen.</strong> Deine
          nächste Nachricht schreibt rund {amount} Token neu in den Cache, statt sie günstig daraus
          zu lesen.
        </>
      );
    }
    return (
      <>
        <strong>Der Cache gilt für {modelName(activeCache.model)}.</strong> Mit{' '}
        {modelName(session.model)} schreibt deine nächste Nachricht rund {amount} Token neu in den
        Cache.
      </>
    );
  }

  if (
    !hasPromptCache ||
    cache === null ||
    state.kind !== 'cold' ||
    cache.tokens < COLD_HINT_MIN_TOKENS ||
    session.status === 'new'
  ) {
    return null;
  }

  return (
    <div className="cache-cold-hint" role="note">
      <div className="cache-cold-hint__row">
        <span className="cache-cold-hint__icon" title={EXPLANATION}>
          <SnowflakeIcon size={ICON_SIZE} />
        </span>
        <p className="cache-cold-hint__text">{renderText(state, cache)}</p>
        <button
          type="button"
          className="cache-cold-hint__button"
          title={CONTINUE_TITLE}
          disabled={phase !== 'idle'}
          onClick={(): void => {
            void continueInNewSession();
          }}
        >
          {CONTINUE_LABELS[phase]}
        </button>
      </div>
      {errorMessage !== null && (
        <p className="cache-cold-hint__error" role="alert">
          {errorMessage}
        </p>
      )}
    </div>
  );
}
