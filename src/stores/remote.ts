import { create } from 'zustand';
import type { RemoteDevice } from '@/lib/bindings/RemoteDevice';
import type { RemoteHostStatus } from '@/lib/bindings/RemoteHostStatus';
import type { RemoteServerStatus } from '@/lib/bindings/RemoteServerStatus';

interface RemoteState {
  /** Der Server dieses Rechners; `null`, bis die erste Abfrage zurück ist. */
  server: RemoteServerStatus | null;
  devices: RemoteDevice[];
  hosts: RemoteHostStatus[];
  /** Satz zum Lade- oder Abo-Fehler der Synchronisation; `null` ohne Fehler. */
  syncError: string | null;
  setServer: (server: RemoteServerStatus) => void;
  setDevices: (devices: RemoteDevice[]) => void;
  setHosts: (hosts: RemoteHostStatus[]) => void;
  setSyncError: (error: string | null) => void;
}

/** Stand der Fernsteuerung, wie ihn die Einstellungen zeigen; der Core hält ihn, jedes
 *  `remote://changed` lädt ihn neu. */
export const useRemoteStore = create<RemoteState>((set) => ({
  server: null,
  devices: [],
  hosts: [],
  syncError: null,
  setServer: (server: RemoteServerStatus): void => {
    set({ server });
  },
  setDevices: (devices: RemoteDevice[]): void => {
    set({ devices });
  },
  setHosts: (hosts: RemoteHostStatus[]): void => {
    set({ hosts });
  },
  setSyncError: (syncError: string | null): void => {
    set({ syncError });
  },
}));
