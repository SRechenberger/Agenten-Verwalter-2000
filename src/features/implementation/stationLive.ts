// Was eine Station von ihrer Session zeigt, wenn sie fertig oder gemeldet ist: Text, Symbol, Grund einer
// Pause. Reine Regeln; die Datei importiert nur Typen, damit `src/app/sidebarPlacement.selftest.ts` sie
// unter Node prüfen kann (`node` löst den Alias `@/` nicht auf).
import type { DisplayStatus } from '../sessions/sessionStatus.ts';
import type { ImplementationView } from '../../lib/bindings/ImplementationView.ts';
import type { StationState } from '../../lib/bindings/StationState.ts';
import type { StationView } from '../../lib/bindings/StationView.ts';

type LiveFacts = Pick<StationView, 'state' | 'live' | 'pause'>;

/** „17:04“ in Ortszeit. */
export function clockText(at: number): string {
  return new Date(at).toLocaleTimeString('de-DE', { hour: '2-digit', minute: '2-digit' });
}

/** Zustand, der Symbol und Farbe einer Station bestimmt, und ob ein Haken dazukommt: Eine fertige Station
 *  mit laufender Session behält ihren Haken in der Farbe der Session; eine gemeldete zeigt das Symbol der
 *  Session. Ohne `live` gilt `state`. */
export function shownState(station: LiveFacts): { state: StationState; isChecked: boolean } {
  if (station.live === null) {
    return { state: station.state, isChecked: false };
  }
  return { state: station.live, isChecked: station.state === 'done' };
}

/** Text der Zeile, Kartei und Leiste, wenn die Session mehr sagt als `state`; sonst `null`. Ein scharfer
 *  Zeitgeber heißt auch hinter „fertig“ und „gemeldet“ „setzt automatisch fort“, ohne ihn „pausiert“
 *  (Taiga #227; Chip dazu: `waitChip`). */
export function liveText(station: LiveFacts): string | null {
  const { state, live, pause } = station;
  if (state === 'waitsForYou' && pause !== null) {
    return 'pausiert · wartet auf dich';
  }
  const lead: string | null =
    state === 'done' ? 'fertig' : state === 'reported' ? 'gemeldet' : null;
  switch (live) {
    case 'starting':
    case 'running':
    case 'waitsForScript':
      return lead === 'fertig' ? 'fertig · läuft wieder' : null;
    case 'asking':
      return lead === 'fertig' ? 'fertig · fragt dich' : null;
    case 'waitsForYou':
      return lead === null ? null : `${lead} · pausiert`;
    case 'waitsForQuota':
      return lead === null ? null : `${lead} · setzt automatisch fort`;
    case 'error':
      return lead === null ? null : `${lead} · Fehler`;
    default:
      return null;
  }
}

/** Grund einer Pause als Chip: „Kontingent bis 14:00“, „Kontingent“ bei unbekannter Zeit; keiner, wenn der
 *  Benutzer pausiert hat oder die App neu gestartet ist. */
export function pauseChip(station: Pick<StationView, 'pause'>): string | null {
  const { pause } = station;
  if (pause?.reason !== 'quota') {
    return null;
  }
  return pause.resetsAt === null ? 'Kontingent' : `Kontingent bis ${clockText(pause.resetsAt)}`;
}

/** Chip hinter „setzt automatisch fort“ und „wartet auf Kontingent“: „ab 17:04“. */
export function resumesAtChip(at: number): string {
  return `ab ${clockText(at)}`;
}

/** Chip einer Station mit Text; `isWarn` färbt ihn bernsteinfarben. */
export interface WaitChip {
  text: string;
  isWarn: boolean;
}

/** Die eine Regel für den Chip beim Warten auf das Kontingent (Taiga #227), für Zeitleiste, Leiste, Kartei und
 *  Sidebar; der Text dazu kommt aus `liveText` und `rowStateText`. Nach dem gezeigten Zustand der Session
 *  (`shownState`), nicht nach `state`:
 *  - scharfer Zeitgeber (`waitsForQuota`): der neutrale Chip „ab 13:30“, nie der bernsteinfarbene;
 *  - vom Halt gehalten (`heldUntil`, ISO-Zeit aus `quotaUntil` der Warteschlange; eine bereite Station in der
 *    Warteschlange oder ein Neustart): ebenso „ab 13:30“;
 *  - sonst bei einer Kontingent-Pause ohne Zeitgeber: der bernsteinfarbene „Kontingent bis 13:30“
 *    (`pauseChip`), denn dann muss jemand handeln. */
export function waitChip(
  station: LiveFacts & Pick<StationView, 'restarting' | 'queuePosition'>,
  heldUntil: string | null,
): WaitChip | null {
  if (shownState(station).state === 'waitsForQuota') {
    const resetsAt: number | null = station.pause?.resetsAt ?? null;
    return resetsAt === null ? null : { text: resumesAtChip(resetsAt), isWarn: false };
  }
  const isQueued: boolean =
    (station.state === 'ready' && station.queuePosition !== null) ||
    (station.state === 'error' && station.restarting);
  if (heldUntil !== null && isQueued) {
    return { text: resumesAtChip(Date.parse(heldUntil)), isWarn: false };
  }
  const chip: string | null = pauseChip(station);
  return chip === null ? null : { text: chip, isWarn: true };
}

/** Symbol der Sessions, damit Umsetzung und Session-Zeilen gleich aussehen. */
export const ICON: Record<StationState, DisplayStatus> = {
  pending: 'new',
  ready: 'new',
  countdown: 'handoff',
  starting: 'starting',
  running: 'running',
  waitsForScript: 'script',
  waitsForQuota: 'paused',
  asking: 'waiting',
  waitsForYou: 'waiting',
  reported: 'running',
  done: 'completed',
  error: 'error',
};

/** Symbol einer Station mit der Farbe, die vom Symbol abweicht: ein Haken in der Farbe der Session bei einer
 *  fertigen Station; eine pausierte Station zeigt das Pausenzeichen statt „wartet auf dich“. */
export function stationIcon(station: LiveFacts): { status: DisplayStatus; tint?: DisplayStatus } {
  const { state, isChecked } = shownState(station);
  if (isChecked) {
    return { status: 'completed', tint: ICON[state] };
  }
  return { status: state === 'waitsForYou' && station.pause !== null ? 'paused' : ICON[state] };
}

/** Eckiges Symbol der Umsetzung: Ist sie fertig, die Dringlichkeit aber nicht `done`, ein Haken in der Farbe
 *  der Dringlichkeit. */
export function implementationIcon(
  implementation: Pick<ImplementationView, 'finishedAt' | 'urgency'>,
): {
  status: DisplayStatus;
  tint?: DisplayStatus;
} {
  if (implementation.finishedAt !== null && implementation.urgency !== 'done') {
    return { status: 'completed', tint: ICON[implementation.urgency] };
  }
  return { status: ICON[implementation.urgency] };
}
