import { useEffect } from 'react';
import type { EnvironmentStatus } from '@/lib/bindings/EnvironmentStatus';
import { loadEnvironment, mirrorEnvironment } from '@/lib/environment';
import { commandErrorText } from '@/lib/errors';
import { useEnvironmentStore } from '@/stores/environment';

interface Environment {
  /** `null`, bis die erste Prüfung zurück ist. */
  status: EnvironmentStatus | null;
  error: string | null;
  mirrorError: string | null;
  isChecking: boolean;
  isMirroring: boolean;
  /** Prüft erneut. */
  check: () => void;
  /** Kopiert aus dem Master-Ordner und zeigt danach den neuen Stand. */
  mirror: () => void;
}

// Zähler auf Modulebene: mehrere Komponenten benutzen den Hook, und nur die jüngste Antwort zählt.
let latestRequest = 0;

function check(): void {
  latestRequest += 1;
  const request: number = latestRequest;
  const store = useEnvironmentStore.getState();
  store.setIsChecking(true);
  loadEnvironment()
    .then((loaded: EnvironmentStatus) => {
      if (latestRequest === request) {
        store.setStatus(loaded);
        store.setError(null);
      }
    })
    .catch((reason: unknown) => {
      console.error('Umgebung nicht prüfbar', reason);
      if (latestRequest === request) {
        store.setError(`Umgebung nicht prüfbar: ${commandErrorText(reason)}`);
      }
    })
    .finally(() => {
      if (latestRequest === request) {
        store.setIsChecking(false);
      }
    });
}

function mirror(): void {
  latestRequest += 1;
  const request: number = latestRequest;
  const store = useEnvironmentStore.getState();
  store.setIsMirroring(true);
  store.setMirrorError(null);
  mirrorEnvironment()
    .then((mirrored: EnvironmentStatus) => {
      if (latestRequest === request) {
        store.setStatus(mirrored);
        store.setError(null);
      }
    })
    .catch((reason: unknown) => {
      console.error('Umgebung nicht gespiegelt', reason);
      store.setMirrorError(`Nicht gespiegelt: ${commandErrorText(reason)}`);
    })
    .finally(() => {
      store.setIsMirroring(false);
      if (latestRequest === request) {
        store.setIsChecking(false);
      }
      // Eine Prüfung, die das Spiegeln überholt hat, trug den Stand von vor dem Kopieren.
      if (latestRequest !== request) {
        check();
      }
    });
}

/** Prüft die Umgebung beim Einhängen einmal und danach nur auf `check`/`mirror` (kein Zeitgeber).
 *  Das Ergebnis liegt im Store, damit die Sidebar es ebenfalls sieht. */
export function useEnvironmentCheck(): void {
  useEffect(() => {
    check();
  }, []);
}

export function useEnvironment(): Environment {
  const status: EnvironmentStatus | null = useEnvironmentStore((state) => state.status);
  const error: string | null = useEnvironmentStore((state) => state.error);
  const mirrorError: string | null = useEnvironmentStore((state) => state.mirrorError);
  const isChecking: boolean = useEnvironmentStore((state) => state.isChecking);
  const isMirroring: boolean = useEnvironmentStore((state) => state.isMirroring);
  return { status, error, mirrorError, isChecking, isMirroring, check, mirror };
}
