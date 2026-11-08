import { useState } from 'react';
import type { ReactElement } from 'react';
import { Popover } from '@/components/Popover';
import { ContextDonut } from '@/features/context/ContextDonut';
import { ContextPanel } from '@/features/context/ContextPanel';
import { useNow } from '@/features/background/useNow';
import { PromptCachePanel } from '@/features/promptcache/PromptCachePanel';
import { promptCacheState } from '@/features/promptcache/promptCacheState';
import { useOperatingMode } from '@/features/remote/useOperatingMode';
import type {
  KnownPromptCacheState,
  PromptCacheState,
} from '@/features/promptcache/promptCacheState';
import { PromptCacheSymbol, promptCacheView } from '@/features/promptcache/PromptCacheSymbol';
import type { PromptCache } from '@/lib/bindings/PromptCache';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import './SessionGauge.css';

const GAUGE_POPOVER_WIDTH = 360;
const CLICK_HINT = 'Klick für Details';

interface SessionGaugeProps {
  session: SessionSummary;
  /** Bisherige Laufzeit, schon formatiert; tickt im Aufrufer. */
  runtime: string;
}

/** Kontext, Prompt-Cache und Laufzeit der Session als eine Anzeige. Ein Klick öffnet ein gemeinsames
 *  Fenster mit allen Einzelheiten. Bei wenig Platz fallen Cache-Text und Laufzeit per CSS weg. */
export function SessionGauge({ session, runtime }: SessionGaugeProps): ReactElement {
  const [isOpen, setIsOpen] = useState<boolean>(false);
  // Den Prompt-Cache mit Haltedauer gibt es nur mit der Claude-Kommandozeile und dem Claude-Abo.
  const hasPromptCache: boolean = useOperatingMode(session.projectId) === 'claude';
  const cacheNow: number = useNow(hasPromptCache && session.promptCache !== null);
  const cacheState: PromptCacheState = promptCacheState(
    session.promptCache,
    session.model,
    cacheNow,
  );
  const contextPercent: number =
    session.contextWindow === 0 ? 0 : (session.contextUsed / session.contextWindow) * 100;
  const cache: PromptCache | null =
    hasPromptCache && cacheState.kind !== 'unknown' ? session.promptCache : null;
  const knownCacheState: KnownPromptCacheState | null =
    cacheState.kind === 'unknown' ? null : cacheState;

  const titleLines: string[] = [`Kontext ${String(Math.round(contextPercent))} % belegt`];
  if (cache !== null && knownCacheState !== null) {
    titleLines.push(promptCacheView(knownCacheState).title);
  }
  titleLines.push(`Laufzeit ${runtime}`, CLICK_HINT);

  function close(): void {
    setIsOpen(false);
  }

  return (
    <div className="session-gauge">
      <button
        type="button"
        className="session-gauge__trigger"
        aria-expanded={isOpen}
        aria-label="Kontext, Prompt-Cache und Laufzeit"
        title={titleLines.join('\n')}
        onClick={(): void => {
          setIsOpen((current: boolean) => !current);
        }}
      >
        <ContextDonut percent={contextPercent} />
        {cache !== null && knownCacheState !== null && (
          <PromptCacheSymbol state={knownCacheState} />
        )}
        <span className="session-gauge__runtime">{runtime}</span>
      </button>
      {isOpen && (
        <Popover
          label="Kontext, Prompt-Cache und Laufzeit"
          placement="below"
          align="end"
          width={GAUGE_POPOVER_WIDTH}
          onClose={close}
        >
          <ContextPanel session={session} onClose={close} />
          {cache !== null && knownCacheState !== null && (
            <>
              <hr className="session-gauge__divider" />
              <PromptCachePanel
                cache={cache}
                state={knownCacheState}
                sessionModel={session.model}
              />
            </>
          )}
          <hr className="session-gauge__divider" />
          <p className="session-gauge__foot">
            <span>Laufzeit der Session</span>
            <span className="session-gauge__foot-value">{runtime}</span>
          </p>
        </Popover>
      )}
    </div>
  );
}
