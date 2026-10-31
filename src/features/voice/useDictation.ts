import { useEffect, useEffectEvent, useRef } from 'react';
import type { RefObject } from 'react';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { insertDictation } from '@/features/voice/insertDictation';
import type { DictationBase } from '@/features/voice/insertDictation';
import type { VoiceLevelEvent } from '@/lib/bindings/VoiceLevelEvent';
import type { VoicePartialEvent } from '@/lib/bindings/VoicePartialEvent';
import { commandErrorText, isCommandError } from '@/lib/errors';
import {
  cancelDictation,
  onVoiceLevel,
  onVoicePartial,
  startDictation,
  stopDictation,
} from '@/lib/voice';
import { useVoiceStore } from '@/stores/voice';
import type { DictationPhase } from '@/stores/voice';

/** Länger nimmt Claude Code auch nicht auf. */
const MAX_DICTATION_MS = 120_000;

interface UseDictationOptions {
  /** Schlüssel der Eingabe: Session-ID bzw. `NEW_SESSION_OWNER`. */
  owner: string;
  inputRef: RefObject<HTMLTextAreaElement | null>;
  /** Aktueller Text des Felds zum Zeitpunkt des Aufrufs, nicht des Renderns. */
  getValue: () => string;
  setValue: (next: string) => void;
  onError: (message: string | null) => void;
  onNeedsSetup: () => void;
}

export interface DictationControl {
  /** Phase dieser Eingabe; läuft das Diktat woanders, ist sie `idle`. */
  phase: DictationPhase;
  /** Ein Diktat läuft in einer anderen Eingabe. */
  isBlocked: boolean;
  toggle: () => void;
  cancel: () => void;
}

/** Steuert das Diktat einer Eingabe: starten, stoppen, abbrechen, den erkannten Text an der Stelle des Cursors
 *  einsetzen. Zeigt der Core `voiceModelMissing`, ruft sie `onNeedsSetup`. */
export function useDictation({
  owner,
  inputRef,
  getValue,
  setValue,
  onError,
  onNeedsSetup,
}: UseDictationOptions): DictationControl {
  const storePhase: DictationPhase = useVoiceStore((state) => state.phase);
  const storeOwner: string | null = useVoiceStore((state) => state.owner);
  const baseRef = useRef<DictationBase | null>(null);
  const isMountedRef = useRef<boolean>(true);
  const isStartingRef = useRef<boolean>(false);
  const isCancelledRef = useRef<boolean>(false);
  /** Zählt jedes Diktat und jeden Abbruch; ein Ergebnis von `stopDictation` gilt nur, wenn die Nummer noch stimmt. */
  const runRef = useRef<number>(0);

  const isOwner: boolean = storeOwner === owner;
  const phase: DictationPhase = isOwner ? storePhase : 'idle';
  const isBlocked: boolean = storeOwner !== null && !isOwner;
  const isActive: boolean = phase !== 'idle';
  const isRecording: boolean = phase === 'recording';

  const handlePartial = useEffectEvent((text: string): void => {
    const state = useVoiceStore.getState();
    const base: DictationBase | null = baseRef.current;
    // Ein Text, der nach Ende oder Abbruch noch eintrifft, gehört nicht mehr in den Entwurf.
    if (
      state.owner !== owner ||
      state.phase === 'idle' ||
      base === null ||
      isCancelledRef.current
    ) {
      return;
    }
    setValue(insertDictation(base, text).value);
  });

  const handleEscape = useEffectEvent((event: KeyboardEvent): void => {
    if (event.key !== 'Escape') {
      return;
    }
    // Das Esc gehört dem Diktat; die Session wird nicht zusätzlich pausiert.
    event.preventDefault();
    event.stopPropagation();
    cancel();
  });

  const handleTimeout = useEffectEvent((): void => {
    stopRecording();
  });

  useEffect(() => {
    isMountedRef.current = true;
    return (): void => {
      isMountedRef.current = false;
      if (useVoiceStore.getState().owner === owner) {
        cancelDictation().catch(logCancelFailure);
        useVoiceStore.getState().resetDictation();
      }
    };
  }, [owner]);

  useEffect(() => {
    if (!isActive) {
      return undefined;
    }
    return listenUntilCleanup(onVoicePartial, (event: VoicePartialEvent): void => {
      handlePartial(event.text);
    });
  }, [isActive]);

  useEffect(() => {
    if (!isRecording) {
      return undefined;
    }
    return listenUntilCleanup(onVoiceLevel, (event: VoiceLevelEvent): void => {
      useVoiceStore.getState().setLevel(event.level);
    });
  }, [isRecording]);

  useEffect(() => {
    if (!isRecording) {
      return undefined;
    }
    const timer: number = window.setTimeout(() => {
      handleTimeout();
    }, MAX_DICTATION_MS);
    return (): void => {
      window.clearTimeout(timer);
    };
  }, [isRecording]);

  // Capture-Phase: vor dem Esc-Listener des Chats, der pausiert sonst die Session.
  useEffect(() => {
    if (!isActive) {
      return undefined;
    }
    function handleKeyDown(event: KeyboardEvent): void {
      handleEscape(event);
    }
    window.addEventListener('keydown', handleKeyDown, true);
    return (): void => {
      window.removeEventListener('keydown', handleKeyDown, true);
    };
  }, [isActive]);

  function toggle(): void {
    if (isStartingRef.current || isBlocked) {
      return;
    }
    if (phase === 'idle') {
      startRecording();
    } else if (phase === 'recording') {
      stopRecording();
    } else {
      cancel();
    }
  }

  function startRecording(): void {
    onError(null);
    isCancelledRef.current = false;
    runRef.current += 1;
    const value: string = getValue();
    const element: HTMLTextAreaElement | null = inputRef.current;
    baseRef.current = {
      value,
      start: element?.selectionStart ?? value.length,
      end: element?.selectionEnd ?? value.length,
    };
    isStartingRef.current = true;
    startDictation()
      .then(() => {
        if (!isMountedRef.current) {
          // Die Eingabe ist während des Starts verschwunden: niemand würde die Aufnahme je beenden.
          cancelDictation().catch(logCancelFailure);
          return;
        }
        useVoiceStore.getState().setPhase('recording', owner);
      })
      .catch((reason: unknown) => {
        baseRef.current = null;
        if (!isMountedRef.current) {
          return;
        }
        if (isCommandError(reason) && reason.kind === 'voiceModelMissing') {
          onNeedsSetup();
          return;
        }
        onError(commandErrorText(reason));
      })
      .finally(() => {
        isStartingRef.current = false;
      });
  }

  function stopRecording(): void {
    const base: DictationBase | null = baseRef.current;
    if (base === null) {
      return;
    }
    const run: number = runRef.current;
    useVoiceStore.getState().setPhase('transcribing', owner);
    stopDictation()
      .then((text: string) => {
        if (!isMountedRef.current || run !== runRef.current) {
          return;
        }
        const inserted = insertDictation(base, text);
        setValue(inserted.value);
        focusAt(inserted.caret);
      })
      .catch((reason: unknown) => {
        if (!isMountedRef.current || run !== runRef.current) {
          return;
        }
        // Der zuletzt angezeigte Text ist echt erkannt und bleibt im Entwurf stehen.
        if (isCommandError(reason) && reason.kind === 'voiceCancelled') {
          return;
        }
        onError(commandErrorText(reason));
      })
      .finally(() => {
        if (isMountedRef.current && run === runRef.current) {
          finishDictation();
        }
      });
  }

  function cancel(): void {
    const base: DictationBase | null = baseRef.current;
    if (base === null || isCancelledRef.current) {
      return;
    }
    isCancelledRef.current = true;
    // Ein noch ausstehendes `stopDictation` ist damit abgehängt: der Core rechnet womöglich noch, die
    // Oberfläche wartet nicht darauf, und ein neuer Start ist sofort möglich.
    runRef.current += 1;
    setValue(base.value);
    // Die Sperre hebt erst auf, wenn der Core den Abbruch angenommen hat: ein Start davor liefe in `voiceBusy`.
    useVoiceStore.getState().setPhase('transcribing', owner);
    cancelDictation()
      .catch(logCancelFailure)
      .finally(() => {
        if (isMountedRef.current) {
          finishDictation();
        }
      });
  }

  function finishDictation(): void {
    baseRef.current = null;
    useVoiceStore.getState().resetDictation();
  }

  function focusAt(caret: number): void {
    requestAnimationFrame(() => {
      const element: HTMLTextAreaElement | null = inputRef.current;
      if (element === null) {
        return;
      }
      element.focus();
      element.setSelectionRange(caret, caret);
    });
  }

  return { phase, isBlocked, toggle, cancel };
}

function logCancelFailure(reason: unknown): void {
  console.error('Diktat nicht abbrechbar', commandErrorText(reason));
}

/** Abonniert ein Core-Ereignis und gibt das Abo über die zurückgegebene Cleanup-Funktion frei, auch wenn
 *  `listen` zu diesem Zeitpunkt noch nicht zurück ist. */
function listenUntilCleanup<T>(
  subscribe: (callback: (event: T) => void) => Promise<UnlistenFn>,
  handler: (event: T) => void,
): () => void {
  const controller = new AbortController();
  let unlisten: UnlistenFn | null = null;
  subscribe(handler)
    .then((stop: UnlistenFn) => {
      if (controller.signal.aborted) {
        stop();
        return;
      }
      unlisten = stop;
    })
    .catch((reason: unknown) => {
      console.error('Diktat-Ereignisse nicht abonnierbar', commandErrorText(reason));
    });
  return (): void => {
    controller.abort();
    if (unlisten !== null) {
      unlisten();
    }
  };
}
