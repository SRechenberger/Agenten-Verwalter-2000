import { useCallback, useEffect, useRef, useState } from 'react';
import { useOpenStation } from '@/features/implementation/useOpenStation';
import { nextWaitingIndex, waitingQueue, type WaitingEntry } from '@/features/waiting/waitingQueue';
import { NOBODY_WAITING_TEXT, waitingPositionText } from '@/features/waiting/waitingTexts';
import type { ImplementationView } from '@/lib/bindings/ImplementationView';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import { useSessionsStore } from '@/stores/sessions';

/** Wie lange die Einblendung nach einem Druck steht. */
const NOTICE_MS = 2500;

export interface ShownTarget {
  sessionId: string | null;
  /** Bei geöffneter Umsetzung die Station im Fokus. */
  stationId: string | null;
}

/** Das Terminal behält Ctrl J: dort ist es ein Zeilenumbruch. */
function isTerminalTarget(target: EventTarget | null): boolean {
  return target instanceof Element && target.closest('.xterm') !== null;
}

/** `Ctrl J` springt zur Session oder Station, die auf dich wartet (die am längsten wartende zuerst),
 *  erneut zur nächsten. Gibt den Text der Einblendung zurück; `null`, wenn keine steht. */
export function useWaitingJump(
  sessions: readonly SessionSummary[],
  implementations: readonly ImplementationView[],
  shown: ShownTarget,
): string | null {
  const [notice, setNotice] = useState<string | null>(null);
  const timerRef = useRef<number | null>(null);
  const selectSession = useSessionsStore((state) => state.selectSession);
  const openStation = useOpenStation();
  const { sessionId: shownSessionId, stationId: shownStationId } = shown;

  const flash = useCallback((text: string): void => {
    if (timerRef.current !== null) {
      window.clearTimeout(timerRef.current);
    }
    setNotice(text);
    timerRef.current = window.setTimeout((): void => {
      timerRef.current = null;
      setNotice(null);
    }, NOTICE_MS);
  }, []);

  useEffect(() => {
    function jumpTo(entry: WaitingEntry): void {
      if (entry.sessionId !== null) {
        selectSession(entry.sessionId);
        return;
      }
      if (entry.implementationId !== null && entry.station !== null) {
        openStation(entry.implementationId, entry.station);
      }
    }
    function handleKeyDown(event: KeyboardEvent): void {
      if (!event.ctrlKey || event.shiftKey || event.altKey || event.key.toLowerCase() !== 'j') {
        return;
      }
      if (isTerminalTarget(event.target)) {
        return;
      }
      // Sonst öffnet das Webview den Download-Dialog.
      event.preventDefault();
      if (event.repeat) {
        return;
      }
      const queue: WaitingEntry[] = waitingQueue(sessions, implementations);
      const index: number | null = nextWaitingIndex(queue, shownSessionId, shownStationId);
      const target: WaitingEntry | undefined = index === null ? undefined : queue[index];
      if (index === null || target === undefined) {
        flash(NOBODY_WAITING_TEXT);
        return;
      }
      jumpTo(target);
      flash(waitingPositionText(index + 1, queue.length));
    }
    window.addEventListener('keydown', handleKeyDown);
    return (): void => {
      window.removeEventListener('keydown', handleKeyDown);
    };
  }, [
    sessions,
    implementations,
    shownSessionId,
    shownStationId,
    selectSession,
    openStation,
    flash,
  ]);

  useEffect(
    () => (): void => {
      if (timerRef.current !== null) {
        window.clearTimeout(timerRef.current);
      }
    },
    [],
  );

  return notice;
}
