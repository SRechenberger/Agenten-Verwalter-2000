import type { AppUpdateProgress } from '@/lib/bindings/AppUpdateProgress';
import type { InstallKind } from '@/lib/bindings/InstallKind';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';

const BYTES_PER_MEGABYTE = 1024 * 1024;

/** Bytes als Megabyte mit einer Nachkommastelle und deutschem Komma, ohne Einheit. */
export function formatMegabytes(bytes: number): string {
  return (bytes / BYTES_PER_MEGABYTE).toLocaleString('de-DE', {
    minimumFractionDigits: 1,
    maximumFractionDigits: 1,
  });
}

/** `TT.MM.JJJJ`; leer, wenn das Release kein Datum trägt oder es nicht lesbar ist. */
export function formatReleaseDate(isoDate: string | null): string {
  if (isoDate === null) {
    return '';
  }
  const date = new Date(isoDate);
  if (Number.isNaN(date.getTime())) {
    return '';
  }
  return date.toLocaleDateString('de-DE', { day: '2-digit', month: '2-digit', year: 'numeric' });
}

/** Sessions, deren Agent beim Neustart des Verwalters mitten in der Arbeit unterbrochen würde. */
export function runningSessionCount(summaries: readonly SessionSummary[]): number {
  return summaries.filter(
    (summary: SessionSummary) => summary.status === 'running' || summary.status === 'starting',
  ).length;
}

export function runningSessionsWarning(count: number): string {
  if (count === 1) {
    return '1 Session arbeitet gerade und wird unterbrochen.';
  }
  return `${String(count)} Sessions arbeiten gerade und werden unterbrochen.`;
}

/** Was ein Klick auf „Installieren“ bewirkt, je nach Art der Installation. */
export function installNote(kind: InstallKind, targetPath: string | null): string {
  switch (kind) {
    case 'portable':
      return `Die Datei ${targetPath ?? 'des Verwalters'} wird ersetzt und der Verwalter startet neu.`;
    case 'installer':
      return 'Der Installer wird gestartet und der Verwalter beendet.';
    case 'manual':
      return 'Dieses Paket aktualisiert sich nicht selbst.';
  }
}

export function progressText(progress: AppUpdateProgress | null): string {
  if (progress === null) {
    return 'Starte …';
  }
  switch (progress.phase) {
    case 'downloading':
      if (progress.total === null) {
        return `Lade ${formatMegabytes(progress.downloaded)} MB`;
      }
      return `Lade ${formatMegabytes(progress.downloaded)} von ${formatMegabytes(progress.total)} MB`;
    case 'verifying':
      return 'Prüfe …';
    case 'installing':
      return 'Installiere …';
  }
}

/** Anteil 0–1 für die Leiste; nach dem Download steht sie voll, ohne bekannte Größe leer. */
export function progressFraction(progress: AppUpdateProgress | null): number {
  if (progress === null) {
    return 0;
  }
  if (progress.phase !== 'downloading') {
    return 1;
  }
  if (progress.total === null || progress.total === 0) {
    return 0;
  }
  return Math.min(progress.downloaded / progress.total, 1);
}
