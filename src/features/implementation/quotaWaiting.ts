// Zählung der Warnleiste „Kontingent aufgebraucht“ (ADR 064). Nur Typimporte: `pnpm selftest` lässt die
// Datei von `node` laufen, das nur Typen streicht.
import type { PauseReason } from '@/lib/bindings/PauseReason';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import type { StationView } from '@/lib/bindings/StationView';

/** Eine Station, deren Session am Kontingent pausiert. */
export interface QuotaPause {
  /** Die Station ist fertig gemeldet; sie zählt nicht. */
  isDone: boolean;
  /** Rücksetzzeit in ms seit der Epoche; `null`: unbekannt, die Station setzt nicht fort. */
  resetsAt: number | null;
  /** Der Zeitgeber ist scharf (`pauseReason.autoResume`). */
  autoResume: boolean;
}

export interface QuotaWaiting {
  /** Nicht fertige Stationen am Kontingent. */
  waiting: number;
  /** Davon mit scharfem Zeitgeber und bekannter Rücksetzzeit. */
  resuming: number;
  /** Davon mit bekannter Rücksetzzeit, aber Schalter aus. */
  waitsForYou: number;
  /** Davon ohne bekannte Rücksetzzeit. */
  unknownReset: number;
  /** Früheste noch kommende Rücksetzzeit der fortsetzenden Stationen; `null`, wenn keine mehr kommt. */
  resumesAt: number | null;
  /** Jede bekannte Rücksetzzeit liegt hinter `now`. */
  isReset: boolean;
}

/** Was `quotaPauses` von einer Station und einer Session liest. */
type StationRef = Pick<StationView, 'sessionId' | 'state'>;
type SessionRef = Pick<SessionSummary, 'id' | 'pauseReason'>;

/** Die Stationen einer Umsetzung, deren Session am Kontingent pausiert. */
export function quotaPauses(
  implementation: { phases: readonly { stations: readonly StationRef[] }[] },
  sessions: readonly SessionRef[],
): QuotaPause[] {
  const pauses: QuotaPause[] = [];
  for (const station of implementation.phases.flatMap(
    (phase: { stations: readonly StationRef[] }) => phase.stations,
  )) {
    const reason: PauseReason | null | undefined = sessions.find(
      (session: SessionRef) => session.id === station.sessionId,
    )?.pauseReason;
    if (reason?.kind === 'quota') {
      pauses.push({
        isDone: station.state === 'done',
        resetsAt: reason.resetsAt,
        autoResume: reason.autoResume,
      });
    }
  }
  return pauses;
}

/** Was die Warnleiste nennt; `null`, solange keine nicht fertige Station am Kontingent wartet. */
export function quotaWaiting(pauses: readonly QuotaPause[], now: number): QuotaWaiting | null {
  const open: QuotaPause[] = pauses.filter((pause: QuotaPause) => !pause.isDone);
  if (open.length === 0) {
    return null;
  }
  const known: number[] = open
    .map((pause: QuotaPause) => pause.resetsAt)
    .filter((at: number | null): at is number => at !== null);
  const resumingAt: number[] = open
    .filter((pause: QuotaPause) => pause.autoResume)
    .map((pause: QuotaPause) => pause.resetsAt)
    .filter((at: number | null): at is number => at !== null);
  const coming: number[] = resumingAt.filter((at: number) => at > now);
  return {
    waiting: open.length,
    resuming: resumingAt.length,
    waitsForYou: open.filter((pause: QuotaPause) => !pause.autoResume && pause.resetsAt !== null)
      .length,
    unknownReset: open.length - known.length,
    resumesAt: coming.length === 0 ? null : Math.min(...coming),
    isReset: known.length > 0 && known.every((at: number) => at <= now),
  };
}
