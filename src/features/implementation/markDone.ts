// „Als fertig markieren“ (ADR 048): wann der Knopf steht. Spiegelt `may_be_marked_done` im Core; der
// Core prüft beim Befehl noch einmal. Reine Regeln, nur Typ-Importe (Selbsttest ohne Build).
import type { StationView } from '../../lib/bindings/StationView.ts';

/** Vorbelegung des Felds „Ergebnis“. */
export const MARK_DONE_RESULT = 'vom Benutzer als fertig markiert';
/** Höchstzahl der Zeichen des Ergebnisses; wie beim Core. */
export const MARK_DONE_MAX_CHARS = 200;

type MarkableStation = Pick<
  StationView,
  'byUser' | 'completedAt' | 'state' | 'live' | 'pause' | 'restarting' | 'startError'
>;

/** Eine Station des Agenten in einer nicht abgebrochenen Umsetzung im Fehler ohne anstehenden Neustart,
 *  mit Startfehler oder mit pausierter Session ohne scharfen Zeitgeber „Automatisch fortsetzen“ (er zeigt
 *  `waitsForQuota`, bei einer gemeldeten Station in `live`). */
export function canMarkDone(station: MarkableStation, isImplementationCancelled: boolean): boolean {
  if (
    isImplementationCancelled ||
    station.byUser ||
    station.completedAt !== null ||
    station.state === 'done'
  ) {
    return false;
  }
  const isTimerArmed: boolean =
    station.state === 'waitsForQuota' || station.live === 'waitsForQuota';
  const isFailed: boolean =
    (station.state === 'error' && !station.restarting) || station.startError !== null;
  return isFailed || (station.pause !== null && !isTimerArmed);
}

/** Gesperrt, solange das Feld leer ist oder nur Leerzeichen enthält. */
export function isMarkDoneResultValid(result: string): boolean {
  return result.trim() !== '';
}
