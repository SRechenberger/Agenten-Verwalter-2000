import type { DeviceRef } from '@/lib/bindings/DeviceRef';
import type { ProjectAccess } from '@/lib/bindings/ProjectAccess';
import type { ProjectSummary } from '@/lib/bindings/ProjectSummary';
import type { RemoteDevice } from '@/lib/bindings/RemoteDevice';

/** So lange hält der Host ein Vorhaben für einen Gast zurück, dessen Verbindung weg ist
 *  (`LOST_LIMIT_MS` im Core, Vertrag der Fernsteuerung). */
export const RESERVED_MS = 5 * 60 * 1000;

const MS_PER_SECOND = 1000;
const SECONDS_PER_MINUTE = 60;

/** Warum die Bedienung eines Vorhabens gesperrt ist; `null` im Feld heißt: nicht gesperrt. */
export type ProjectLock =
  /** Am Host: Ein Gast hält das Vorhaben. `lostSince` ist der Zeitpunkt, an dem seine Verbindung
   *  abriss; `null`, solange er verbunden ist oder der Host den Zeitpunkt nicht kennt. */
  | { kind: 'guest'; device: DeviceRef; lostSince: number | null }
  /** Am Gast: Der Host ist nicht erreichbar. */
  | { kind: 'hostOffline'; hostName: string }
  /** Am Gast: Dieses Gerät hält das Vorhaben nicht. */
  | { kind: 'notHeld'; hostName: string };

export function isRemote(
  access: ProjectAccess,
): access is Extract<ProjectAccess, { kind: 'remote' }> {
  return access.kind === 'remote';
}

/** Ein Vorhaben eines Hosts, das dieses Gerät nicht hält: Es zeigt weder Sessions noch Verlauf. */
export function isOffered(access: ProjectAccess): boolean {
  return access.kind === 'remote' && !access.mine;
}

/** Am Host: das Gerät, das das Vorhaben gerade hält. */
export function holderOf(access: ProjectAccess): DeviceRef | null {
  return access.kind === 'shared' ? access.holder : null;
}

/** Der Zeitpunkt, an dem die Verbindung des Inhabers abriss. Der Host schreibt ihn beim Abriss
 *  als `lastSeenAt` des Geräts; solange das Gerät verbunden ist, gibt es keinen. */
export function lostSinceOf(holder: DeviceRef, devices: readonly RemoteDevice[]): number | null {
  const device: RemoteDevice | undefined = devices.find(
    (candidate: RemoteDevice) => candidate.id === holder.id,
  );
  if (device === undefined || device.online) {
    return null;
  }
  return device.lastSeenAt;
}

export function lockOf(
  access: ProjectAccess,
  devices: readonly RemoteDevice[],
): ProjectLock | null {
  switch (access.kind) {
    case 'own':
      return null;
    case 'shared':
      return access.holder === null
        ? null
        : { kind: 'guest', device: access.holder, lostSince: lostSinceOf(access.holder, devices) };
    case 'remote':
      if (!access.mine) {
        return { kind: 'notHeld', hostName: access.hostName };
      }
      return access.online ? null : { kind: 'hostOffline', hostName: access.hostName };
  }
}

/** Der Hinweis an einem Knopf, der gesperrt ist, weil ein anderes Gerät steuert oder der Host fehlt. */
export const LOCKED_TITLE =
  'Gesperrt, solange ein anderes Gerät das Vorhaben steuert oder der Host nicht erreichbar ist.';

/** Der Satz unter einer offenen Rückfrage, die gerade niemand an diesem Gerät beantworten kann. */
export function lockNote(lock: ProjectLock): string {
  switch (lock.kind) {
    case 'guest':
      return `Antworten kann gerade nur ${lock.device.name}.`;
    case 'hostOffline':
      return 'Antworten geht wieder, sobald der Host erreichbar ist.';
    case 'notHeld':
      return 'Antworten geht, sobald du das Vorhaben übernommen hast.';
  }
}

/** Ob irgendetwas das Vorhaben sperrt; ohne die Geräteliste, denn nur der Grund braucht sie. */
export function isLocked(access: ProjectAccess): boolean {
  return lockOf(access, []) !== null;
}

/** Verbleibende Millisekunden der Reservierung; `null`, wenn die Verbindung nicht abgerissen ist. */
export function reservedLeftMs(lock: ProjectLock, now: number): number | null {
  if (lock.kind !== 'guest' || lock.lostSince === null) {
    return null;
  }
  return Math.max(0, lock.lostSince + RESERVED_MS - now);
}

/** `4:12`. */
export function formatReserved(milliseconds: number): string {
  const totalSeconds: number = Math.ceil(milliseconds / MS_PER_SECOND);
  const minutes: number = Math.floor(totalSeconds / SECONDS_PER_MINUTE);
  const seconds: number = totalSeconds % SECONDS_PER_MINUTE;
  return `${String(minutes)}:${String(seconds).padStart(2, '0')}`;
}

/** Der Host eines entfernten Vorhabens; `null` bei eigenen. */
export function hostNameOf(access: ProjectAccess): string | null {
  return access.kind === 'remote' ? access.hostName : null;
}

/** Zugriff aller Vorhaben der Liste nach ID, für den Store. */
export function accessById(projects: readonly ProjectSummary[]): Record<string, ProjectAccess> {
  const byId: Record<string, ProjectAccess> = {};
  for (const project of projects) {
    byId[project.id] = project.access;
  }
  return byId;
}

export const OWN_ACCESS: ProjectAccess = { kind: 'own' };

/** Die Meta-Zeile eines Vorhabens in der Leiste, wenn Fernsteuerung im Spiel ist. `tone` färbt sie:
 *  `accent` für ein Vorhaben, das ein Gast steuert, `error` für einen Host, der fehlt. */
export interface RemoteMeta {
  text: string | null;
  /** Der volle Satz, wenn `text` für die schmale Zeile gekürzt ist. */
  title?: string | undefined;
  tone: 'accent' | 'error' | null;
}

/** Aus der gewohnten Meta-Zeile (`base`, `null` ohne Sessions) und dem Zustand der Fernsteuerung.
 *  Eigene Vorhaben ohne Freigabe bleiben, wie sie sind; am Host ersetzt der Zustand der Freigabe die Zeile,
 *  das Symbol links zeigt weiter den dringendsten Status. */
export function remoteMeta(
  access: ProjectAccess,
  base: string | null,
  lock: ProjectLock | null,
  reservedLeft: number | null,
): RemoteMeta {
  switch (access.kind) {
    case 'own':
      return { text: base, tone: null };
    case 'shared':
      if (lock?.kind !== 'guest') {
        return { text: 'Für Gäste freigegeben', tone: null };
      }
      if (lock.lostSince === null) {
        return { text: `Gesteuert von ${lock.device.name}`, tone: 'accent' };
      }
      // Die Zeile ist schmal: Die Restzeit darf nicht abgeschnitten werden, der volle Satz steht im Hinweis.
      return {
        text:
          reservedLeft === null
            ? `${lock.device.name} getrennt`
            : `${lock.device.name} getrennt · ${formatReserved(reservedLeft)}`,
        title:
          reservedLeft === null
            ? undefined
            : `${lock.device.name} getrennt · frei in ${formatReserved(reservedLeft)}`,
        tone: 'accent',
      };
    case 'remote':
      if (!access.online) {
        return { text: `${access.hostName} nicht erreichbar`, tone: 'error' };
      }
      if (!access.mine) {
        return { text: `Frei · ${access.hostName}`, tone: null };
      }
      // Der Host steht vorn: Die Zeile ist schmal, und das Ende schneidet sie ab.
      return { text: base === null ? access.hostName : `${access.hostName} · ${base}`, tone: null };
  }
}

/** Ein Gast, der mit diesem Host verbunden ist, samt dem Vorhaben, das er gerade steuert. */
export interface ConnectedGuest {
  id: string;
  name: string;
  /** Der Name des Vorhabens, das der Gast hält; `null`, wenn er keins hält. */
  projectName: string | null;
}

/** Die verbundenen Gäste des Hosts: jedes Gerät, das online ist; zu jedem das Vorhaben, das es hält. */
export function connectedGuests(
  devices: readonly RemoteDevice[],
  access: Readonly<Record<string, ProjectAccess>>,
  names: Readonly<Record<string, string>>,
): ConnectedGuest[] {
  return devices
    .filter((device: RemoteDevice) => device.online)
    .map((device: RemoteDevice): ConnectedGuest => {
      const projectId: string | undefined = Object.keys(access).find(
        (id: string) => holderOf(access[id] ?? OWN_ACCESS)?.id === device.id,
      );
      return {
        id: device.id,
        name: device.name,
        projectName: projectId === undefined ? null : (names[projectId] ?? null),
      };
    });
}
