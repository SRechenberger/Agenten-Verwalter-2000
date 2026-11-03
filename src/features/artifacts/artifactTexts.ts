import { formatClock } from '@/features/context/formatTokens';
import type { Artifact } from '@/lib/bindings/Artifact';
import type { ArtifactKind } from '@/lib/bindings/ArtifactKind';

export const ARTIFACTS_HEADING = 'Artefakte des Vorhabens';
export const ARTIFACTS_INFO =
  'Dateien und Ordner, die ein Agent dieses Vorhabens zum Ansehen abgelegt hat – Bericht, Dokument, Bild, Diagramm, Video, Ton, Daten. Sie liegen im Ordner .artefakte des Vorhabens, stehen unter der Session, die sie angelegt hat, und aktualisieren sich, sobald der Agent sie neu speichert. Unterordner, deren Name mit _ beginnt, sind Zubehör einer Seite und erscheinen nicht.';
export const JUST_UPDATED = 'gerade aktualisiert';
export const OWN_SESSION = 'diese Session';
export const UNASSIGNED = 'Ohne Zuordnung';

/** Kurztext im Typ-Abzeichen. */
export const KIND_LABELS: Record<ArtifactKind, string> = {
  html: 'HTML',
  site: 'SITE',
  folder: 'DIR',
  markdown: 'MD',
  json: 'JSON',
  'json-lines': 'JSONL',
  yaml: 'YAML',
  config: 'CONF',
  xml: 'XML',
  csv: 'CSV',
  code: 'CODE',
  text: 'TXT',
  log: 'LOG',
  diff: 'DIFF',
  image: 'BILD',
  video: 'VIDEO',
  audio: 'TON',
  pdf: 'PDF',
  mermaid: 'MMD',
  other: 'DATEI',
};

/** HTML öffnet im Browser, alles andere im zugeordneten Programm, Ordner im Explorer. */
export function openLabel(kind: ArtifactKind): string {
  return kind === 'html' ? 'Im Browser öffnen' : 'Mit Standardprogramm öffnen';
}

/** Uhrzeit der letzten Änderung; liegt sie nicht am heutigen Tag, steht das Datum davor. */
export function clockLabel(modifiedAt: number): string {
  const clock: string = formatClock(modifiedAt);
  if (new Date(modifiedAt).toDateString() === new Date().toDateString()) {
    return clock;
  }
  const date: string = new Date(modifiedAt).toLocaleDateString('de-DE', {
    day: '2-digit',
    month: '2-digit',
  });
  return `${date} ${clock}`;
}

export function sizeLabel(bytes: number): string {
  if (bytes < 1024) return `${String(bytes)} B`;
  const kilobytes: number = bytes / 1024;
  if (kilobytes < 1024) return `${kilobytes.toFixed(1).replace('.', ',')} KB`;
  return `${(kilobytes / 1024).toFixed(1).replace('.', ',')} MB`;
}

/** Dateiname (bei Ordnern die Zahl der Dateien) · Uhrzeit · ggf. die Session, die zuletzt schrieb. */
export function metaLine(item: Artifact): string {
  const parts: string[] = [
    item.entries === null ? item.file : `${String(item.entries)} Dateien`,
    clockLabel(item.modifiedAt),
  ];
  if (item.lastSessionNumber !== null) {
    parts.push(`zuletzt #${String(item.lastSessionNumber)}`);
  }
  return parts.join(' · ');
}
