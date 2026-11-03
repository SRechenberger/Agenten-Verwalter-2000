import type { ImplementationView } from '@/lib/bindings/ImplementationView';
import type { PhaseMark } from '@/lib/bindings/PhaseMark';
import type { PhaseView } from '@/lib/bindings/PhaseView';
import type { StationState } from '@/lib/bindings/StationState';
import type { StationView } from '@/lib/bindings/StationView';
import type { ImplementationViewKind } from '@/stores/sessions';

export const VIEW_LABEL: Record<ImplementationViewKind, string> = {
  flow: 'Ablauf',
  changes: 'Changes',
};

export const REPORT_LABEL = 'Lagebericht';
export const REPORT_LOADING = 'Lagebericht wird geladen …';
export const REPORT_COPY_LABEL = 'Kopieren';
export const REPORT_COPIED = 'In die Zwischenablage kopiert.';
export const REPORT_CLOSE_LABEL = 'Schließen';
export const INQUIRY_LABEL = 'Neue Rückfrage';
export const INQUIRY_TITLE =
  'Frage zur Umsetzung in einer frischen Session — der Lagebericht hängt dran';

/** „Phase 2 von 7 · 6 von 16 Sessions fertig“. */
export function progressLine(implementation: ImplementationView): string {
  return `Phase ${String(implementation.currentPhase)} von ${String(implementation.phases.length)} · ${String(implementation.stationsDone)} von ${String(implementation.stationsTotal)} Sessions fertig`;
}

/** „6/16“ in der Sidebar. */
export function progressShort(implementation: ImplementationView): string {
  return `${String(implementation.stationsDone)}/${String(implementation.stationsTotal)}`;
}

export const AUTO_START_LABEL = 'Automatisch weiter';
export const AUTO_START_TITLE =
  'Startet die nächste Station von selbst, 10 Sekunden nach der Fertigmeldung';
export const STOP_AFTER_LABEL = 'Anhalten nach dieser Session';
export const STOP_AFTER_TITLE = 'Die laufende Station läuft zu Ende, danach startet nichts mehr';
export const BACK_TO_CURRENT_LABEL = 'Zur aktuellen';
export const BACK_TO_CURRENT_TITLE = 'Springt zurück zur Station, die gerade läuft oder fragt';
export const OPEN_CHAT_LABEL = 'Chat öffnen';
export const OPEN_CHAT_TITLE = 'Öffnet die Session dieser Station; Fragen beantwortest du dort';
export const START_NOW_LABEL = '▶ Jetzt starten';
export const START_NOW_TITLE = 'Startet diese Station sofort als neue Session';
export const CANCEL_COUNTDOWN_LABEL = 'Abbrechen';
export const CANCEL_COUNTDOWN_TITLE = 'Hält den Countdown an; die Station bleibt bereit';
export const DONE_BY_USER_RESULT = 'vom Benutzer erledigt';
export const DONE_BY_USER_LABEL = 'Erledigt';
export const DONE_BY_USER_TITLE = 'Meldet diese Station als fertig, damit die nächste starten darf';
export const REPORT_TITLE =
  'Zeigt den Lagebericht, den auch der Agent über implementation_get liest';
export const TIMELINE_LABEL = 'Zeitleiste der Umsetzung';
export const NO_PHASES = 'Diese Umsetzung hat noch keine Phasen.';

export const PLAN_PROPOSAL_TITLE = 'Plan ändern?';
export const PLAN_CHANGED_TITLE = 'Plan geändert';
export const APPLY_CHANGE_LABEL = 'Übernehmen';
export const APPLY_CHANGE_TITLE = 'Ersetzt die kommenden Phasen durch den Vorschlag';
export const DISCARD_CHANGE_LABEL = 'Verwerfen';
export const DISCARD_CHANGE_TITLE = 'Lässt die kommenden Phasen, wie sie sind';
export const SHOW_PENDING_LABEL = 'Vorschlag ansehen';
export const SHOW_PENDING_TITLE = 'Holt die Karte „Plan ändern?“ nach vorn';
export const CHANGE_MARKER_TITLE = 'Zeigt diese Plan-Änderung vorn in der Kartei';
export const PHASE_MARK_LABEL: Record<PhaseMark, string> = {
  new: 'neu',
  moved: 'vorgezogen',
};
export const PHASE_MARK_TITLE: Record<PhaseMark, string> = {
  new: 'Diese Phase kam mit der letzten Plan-Änderung dazu',
  moved: 'Diese Phase wurde mit der letzten Plan-Änderung vorgezogen',
};

/** „2 Spuren aktiv“ in der Kopfzeile, solange eine Welle mehrere Phasen zugleich bearbeitet. */
export function lanesActiveText(count: number): string {
  return `${String(count)} Spuren aktiv`;
}

/** „Welle 2 · Phase 3 und 4 gleichzeitig“; bei drei und mehr „Phase 3, 4 und 5“. */
export function waveTitle(wave: number, phases: readonly PhaseView[]): string {
  const keys: string[] = phases.map((phase: PhaseView) => phase.key);
  const last: string | undefined = keys.pop();
  const names: string = keys.length === 0 ? (last ?? '') : `${keys.join(', ')} und ${last ?? ''}`;
  return `Welle ${String(wave)} · Phase ${names} gleichzeitig`;
}

/** „30.10. 14:05“ in Ortszeit. */
export function stampText(at: number): string {
  return new Date(at).toLocaleString('de-DE', {
    day: '2-digit',
    month: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  });
}

/** „aus #3 · 30.10. 14:05“; die Session gibt es nicht mehr, wenn ihre Nummer 0 ist. */
export function changeOrigin(sessionNumber: number, at: number): string {
  const origin: string =
    sessionNumber === 0 ? 'aus einer gelöschten Session' : `aus #${String(sessionNumber)}`;
  return `${origin} · ${stampText(at)}`;
}

/** Schlüssel einer Änderungsliste, „nichts“ bei leerer. */
export function keysText(keys: readonly string[]): string {
  return keys.length === 0 ? 'nichts' : keys.join(', ');
}

/** Zeile der Marke auf der Linie. */
export function changeMarkerText(sessionNumber: number): string {
  const origin: string = sessionNumber === 0 ? '' : ` · aus #${String(sessionNumber)}`;
  return `Plan geändert${origin} · von dir übernommen`;
}

const MINUTE_MS = 60_000;
const HOUR_MINUTES = 60;

/** „8 min“, „1 h 5 min“; unter einer Minute „< 1 min“. */
export function durationText(milliseconds: number): string {
  const minutes: number = Math.floor(milliseconds / MINUTE_MS);
  if (minutes < 1) {
    return '< 1 min';
  }
  if (minutes < HOUR_MINUTES) {
    return `${String(minutes)} min`;
  }
  const rest: number = minutes % HOUR_MINUTES;
  const hours: string = String(Math.floor(minutes / HOUR_MINUTES));
  return rest === 0 ? `${hours} h` : `${hours} h ${String(rest)} min`;
}

/** Zustand der Station als Satz für die große Karte („läuft seit 12 min“, „fertig · 24 min“). */
export function stateText(station: StationView, now: number): string {
  switch (station.state) {
    case 'running':
    case 'starting':
      return `läuft seit ${durationText(runningFor(station, now))}`;
    case 'asking':
      return 'fragt dich';
    case 'waitsForYou':
      return 'wartet auf dich';
    case 'done':
      return `fertig · ${durationText(station.runningMs)}`;
    case 'error':
      return 'Fehler';
    case 'countdown':
      return countdownText(station, now);
    case 'ready':
    case 'pending':
      return 'wartet auf Start';
  }
}

/** Kurzfassung für die Zeitleiste: Dauer, „läuft“, „fragt“ — leer, solange nichts war. */
export function stationMeta(station: StationView, now: number): string {
  switch (station.state) {
    case 'done':
      return durationText(station.runningMs);
    case 'running':
    case 'starting':
      return `läuft ${durationText(runningFor(station, now))}`;
    case 'asking':
      return 'fragt';
    case 'waitsForYou':
      return 'wartet';
    case 'error':
      return 'Fehler';
    case 'countdown':
      return countdownText(station, now);
    case 'ready':
    case 'pending':
      return '';
  }
}

/** Zustand einer ganzen Phase als Überschrift-Zusatz. */
export function phaseMeta(phase: PhaseView): string {
  const states: StationState[] = phase.stations.map((station: StationView) => station.state);
  if (states.every((state: StationState) => state === 'done')) {
    const total: number = phase.stations.reduce(
      (sum: number, station: StationView) => sum + station.runningMs,
      0,
    );
    return `fertig · ${durationText(total)}`;
  }
  const checks: [StationState[], string][] = [
    [['asking', 'waitsForYou'], 'fragt dich'],
    [['error'], 'Fehler'],
    [['running', 'starting', 'countdown'], 'läuft'],
    [['ready'], 'bereit'],
  ];
  for (const [matching, text] of checks) {
    if (states.some((state: StationState) => matching.includes(state))) {
      return text;
    }
  }
  return states.some((state: StationState) => state === 'done') ? 'läuft' : 'geplant';
}

function runningFor(station: StationView, now: number): number {
  if (station.startedAt === null) {
    return station.runningMs;
  }
  return Math.max(station.runningMs, now - station.startedAt);
}

function countdownText(station: StationView, now: number): string {
  if (station.startsAt === null) {
    return 'startet gleich';
  }
  const seconds: number = Math.max(0, Math.ceil((station.startsAt - now) / 1000));
  return `startet in ${String(seconds)} s`;
}

export function implementationTitle(implementation: ImplementationView): string {
  return `Umsetzung ${implementation.name}`;
}
