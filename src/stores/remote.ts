import { create } from 'zustand';
import type { HostProfile } from '@/lib/bindings/HostProfile';
import type { RemoteDevice } from '@/lib/bindings/RemoteDevice';
import type { RemoteHostStatus } from '@/lib/bindings/RemoteHostStatus';
import type { RemoteServerStatus } from '@/lib/bindings/RemoteServerStatus';

interface RemoteState {
  /** Der Server dieses Rechners; `null`, bis die erste Abfrage zurück ist. */
  server: RemoteServerStatus | null;
  devices: RemoteDevice[];
  /** Die Gruppen, die ihren Mitgliedern die Freigabe geben (am Host). */
  sharedGroupIds: string[];
  /** Die Vorhaben mit eigener Freigabe (am Host). */
  ownSharedIds: string[];
  hosts: RemoteHostStatus[];
  /** Host-ID → Betriebsart und Modelle des Hosts; ein Host ohne Eintrag ist unerreichbar oder noch nicht gelesen. */
  profiles: Readonly<Record<string, HostProfile>>;
  /** Adresse, unter der der Server dieses Geräts Bilder und Anhänge eines Hosts liefert; `null`, bis sie geladen ist. */
  assetBase: string | null;
  /** Satz zum Lade- oder Abo-Fehler der Synchronisation; `null` ohne Fehler. */
  syncError: string | null;
  setServer: (server: RemoteServerStatus) => void;
  setDevices: (devices: RemoteDevice[]) => void;
  setSharedGroupIds: (groupIds: string[]) => void;
  setOwnSharedIds: (projectIds: string[]) => void;
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
  sharedGroupIds: [],
  ownSharedIds: [],
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
  setSharedGroupIds: (sharedGroupIds: string[]): void => {
    set({ sharedGroupIds });
  },
  setOwnSharedIds: (ownSharedIds: string[]): void => {
    set({ ownSharedIds });
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
