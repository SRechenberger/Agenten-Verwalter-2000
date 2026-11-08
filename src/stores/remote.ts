import { create } from 'zustand';
import type { HostProfile } from '@/lib/bindings/HostProfile';
import type { RemoteDevice } from '@/lib/bindings/RemoteDevice';
import type { RemoteHostStatus } from '@/lib/bindings/RemoteHostStatus';
import type { RemoteServerStatus } from '@/lib/bindings/RemoteServerStatus';

interface RemoteState {
  /** Der Server dieses Rechners; `null`, bis die erste Abfrage zurück ist. */
  server: RemoteServerStatus | null;
  devices: RemoteDevice[];
  hosts: RemoteHostStatus[];
  /** Host-ID → Betriebsart und Modelle des Hosts; ein Host ohne Eintrag ist unerreichbar oder noch nicht gelesen. */
  profiles: Readonly<Record<string, HostProfile>>;
  /** Adresse, unter der der Server dieses Geräts Bilder und Anhänge eines Hosts liefert; `null`, bis sie geladen ist. */
  assetBase: string | null;
  /** Satz zum Lade- oder Abo-Fehler der Synchronisation; `null` ohne Fehler. */
  syncError: string | null;
  setServer: (server: RemoteServerStatus) => void;
  setDevices: (devices: RemoteDevice[]) => void;
  setHosts: (hosts: RemoteHostStatus[]) => void;
  setProfiles: (profiles: Readonly<Record<string, HostProfile>>) => void;
  setAssetBase: (assetBase: string) => void;
  setSyncError: (error: string | null) => void;
}

/** Stand der Fernsteuerung, wie ihn die Einstellungen zeigen; der Core hält ihn, jedes
 *  `remote://changed` lädt ihn neu. */
export const useRemoteStore = create<RemoteState>((set) => ({
  server: null,
  devices: [],
  hosts: [],
  profiles: {},
  assetBase: null,
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
  setProfiles: (profiles: Readonly<Record<string, HostProfile>>): void => {
    set({ profiles });
  },
  setAssetBase: (assetBase: string): void => {
    set({ assetBase });
  },
  setSyncError: (syncError: string | null): void => {
    set({ syncError });
  },
}));
