import type { ConnectedGuest } from '@/features/remote/projectAccess';
import type { RemoteDevice } from '@/lib/bindings/RemoteDevice';
import type { RemoteHostStatus } from '@/lib/bindings/RemoteHostStatus';

const LAST_SEEN_FORMAT = new Intl.DateTimeFormat('de-DE', {
  day: '2-digit',
  month: '2-digit',
  hour: '2-digit',
  minute: '2-digit',
});

const MAX_PORT = 65535;

/** Der Fingerabdruck in Vierergruppen, gekürzt auf Anfang und Ende: `3fa9 7c1e … b02d`.
 *  Der volle Wert steht im `title`. */
export function shortFingerprint(hex: string): string {
  const groups: string[] = hex.match(/.{1,4}/g) ?? [];
  if (groups.length <= 3) {
    return groups.join(' ');
  }
  return `${groups.slice(0, 2).join(' ')} … ${groups[groups.length - 1] ?? ''}`;
}

/** Der Fingerabdruck in Vierergruppen, vollständig. */
export function fullFingerprint(hex: string): string {
  return (hex.match(/.{1,4}/g) ?? []).join(' ');
}

/** Der Port als Zahl, wenn der Text einen gültigen enthält; sonst `null`. */
export function parsePort(text: string): number | null {
  const trimmed: string = text.trim();
  if (!/^\d{1,5}$/.test(trimmed)) {
    return null;
  }
  const port: number = Number(trimmed);
  return port >= 1 && port <= MAX_PORT ? port : null;
}

export const PORT_INVALID_TEXT = `Der Port ist eine Zahl von 1 bis ${String(MAX_PORT)}.`;

export function deviceStateText(device: RemoteDevice): string {
  if (device.online) {
    return 'Verbunden';
  }
  if (device.lastSeenAt === null) {
    return 'Noch nicht angemeldet';
  }
  return `Zuletzt gesehen ${LAST_SEEN_FORMAT.format(new Date(device.lastSeenAt))}`;
}

/** Zustand eines Hosts am Gast: verbunden, mit Grund getrennt oder einfach nicht verbunden. */
export function hostStateText(host: RemoteHostStatus): string {
  if (host.online) {
    return 'Verbunden';
  }
  return host.error ?? 'Getrennt';
}

/** Ein Satz je verbundenem Gast: wer verbunden ist und welches Vorhaben er steuert. */
function guestSentence(guest: ConnectedGuest): string {
  return guest.projectName === null
    ? `${guest.name} ist verbunden.`
    : `${guest.name} ist verbunden und steuert „${guest.projectName}“.`;
}

/** Warnung vor dem Schließen der App: Die Agenten laufen auf diesem Rechner und enden mit ihm. */
export function closeWarning(guests: readonly ConnectedGuest[]): string {
  const rest: string =
    guests.length === 1
      ? 'der Gast kann erst nach dem nächsten Start wieder übernehmen'
      : 'die Gäste können erst nach dem nächsten Start wieder übernehmen';
  return `${guests.map(guestSentence).join(' ')} Beim Schließen enden alle Agenten-Prozesse auf diesem Rechner, und ${rest}.`;
}

/** Warnung vor dem Installieren eines Updates: Der Verwalter startet dabei neu. */
export function updateWarning(guests: readonly ConnectedGuest[]): string {
  const rest: string =
    guests.length === 1
      ? 'der Gast kann erst nach dem Neustart wieder übernehmen'
      : 'die Gäste können erst nach dem Neustart wieder übernehmen';
  return `${guests.map(guestSentence).join(' ')} Beim Neustart enden alle Agenten-Prozesse auf diesem Rechner, und ${rest}.`;
}
