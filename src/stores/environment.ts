import { create } from 'zustand';
import type { EnvironmentStatus } from '@/lib/bindings/EnvironmentStatus';

interface EnvironmentState {
  /** `null`, bis die erste Prüfung zurück ist. */
  status: EnvironmentStatus | null;
  /** Satz, wenn die Prüfung scheiterte. */
  error: string | null;
  /** Satz, wenn das Spiegeln scheiterte. */
  mirrorError: string | null;
  isChecking: boolean;
  isMirroring: boolean;
  setStatus: (status: EnvironmentStatus) => void;
  setError: (error: string | null) => void;
  setMirrorError: (mirrorError: string | null) => void;
  setIsChecking: (isChecking: boolean) => void;
  setIsMirroring: (isMirroring: boolean) => void;
}

/** Ergebnis der letzten Prüfung der Umgebung — gelesen von den Einstellungen und vom Punkt am
 *  Knopf „Einstellungen“ in der Sidebar. */
export const useEnvironmentStore = create<EnvironmentState>((set) => ({
  status: null,
  error: null,
  mirrorError: null,
  isChecking: true,
  isMirroring: false,
  setStatus: (status: EnvironmentStatus): void => {
    set({ status });
  },
  setError: (error: string | null): void => {
    set({ error });
  },
  setMirrorError: (mirrorError: string | null): void => {
    set({ mirrorError });
  },
  setIsChecking: (isChecking: boolean): void => {
    set({ isChecking });
  },
  setIsMirroring: (isMirroring: boolean): void => {
    set({ isMirroring });
  },
}));
