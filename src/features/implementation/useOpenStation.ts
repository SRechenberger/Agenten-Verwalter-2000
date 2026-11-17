import { useCallback } from 'react';
import type { StationView } from '@/lib/bindings/StationView';
import { useImplementationStore } from '@/stores/implementation';
import { useSessionsStore } from '@/stores/sessions';

/** Öffnet eine Station: mit Session deren Session, sonst die Umsetzung mit der Karte der Station vorn. */
export function useOpenStation(): (implementationId: string, station: StationView) => void {
  const selectSession = useSessionsStore((state) => state.selectSession);
  const selectImplementation = useSessionsStore((state) => state.selectImplementation);
  const setFocused = useImplementationStore((state) => state.setFocused);

  return useCallback(
    (implementationId: string, station: StationView): void => {
      if (station.sessionId !== null) {
        selectSession(station.sessionId);
        return;
      }
      setFocused(implementationId, station.id);
      selectImplementation(implementationId);
    },
    [selectSession, selectImplementation, setFocused],
  );
}
