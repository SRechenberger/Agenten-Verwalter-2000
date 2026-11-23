// „Umsetzung starten“ (#135) und „Erledigt“ ohne Freigabe (#136): wann der Knopf steht und wann „Erledigt“
// nachfragt. Reine Regeln, nur Typ-Importe (Selbsttest ohne Build).
import type { ImplementationView } from '../../lib/bindings/ImplementationView.ts';
import type { PhaseView } from '../../lib/bindings/PhaseView.ts';
import type { StationView } from '../../lib/bindings/StationView.ts';

type StartFields = Pick<
  ImplementationView,
  'cancelledAt' | 'pendingCancel' | 'stopAfterCurrent' | 'finishedAt'
> & {
  phases: readonly Pick<PhaseView, 'stations'>[];
};

type StartStation = Pick<
  StationView,
  | 'state'
  | 'queuePosition'
  | 'autoStart'
  | 'startsAutomatically'
  | 'sessionId'
  | 'startedAt'
  | 'completedAt'
>;

/** Wie `isBegun` in `implementationOrder` und `station_is_begun` im Core. */
function isBegun(station: StartStation): boolean {
  return station.sessionId !== null || station.startedAt !== null || station.completedAt !== null;
}

/** Der Knopf „Umsetzung starten“ steht, solange eine Station bereit ist, die er startet und die nicht von
 *  selbst startet: Die Umsetzung hat nicht begonnen, oder ihre Automatik ist aus. Eine Station, die schon in
 *  der Warteschlange wartet, startet von selbst; eine mit eigenem „Automatisch starten“ aus startet er nicht.
 *  Gesperrt (abgebrochen, Anfrage zum Abbrechen, Halt) oder fertig fehlt er. */
export function showsStartButton(implementation: StartFields): boolean {
  if (
    implementation.cancelledAt !== null ||
    implementation.pendingCancel !== null ||
    implementation.stopAfterCurrent ||
    implementation.finishedAt !== null
  ) {
    return false;
  }
  const stations: StartStation[] = implementation.phases.flatMap(
    (phase: Pick<PhaseView, 'stations'>) => phase.stations,
  );
  const hasBegun: boolean = stations.some(isBegun);
  return stations.some(
    (station: StartStation) =>
      station.state === 'ready' &&
      station.queuePosition === null &&
      station.autoStart !== false &&
      (!hasBegun || !station.startsAutomatically),
  );
}

/** „Erledigt“ in Zeile, Kartei und Popup gilt einer Station „durch dich“, die an der Reihe ist. */
export function isDoneByYou(station: Pick<StationView, 'byUser' | 'state'>): boolean {
  return station.byUser && station.state === 'waitsForYou';
}

/** „Erledigt“ fragt einmal nach, wenn die Umsetzung einen Master hat und er die Station nicht freigegeben
 *  hat; ohne Master fragt es nie. Der Core prüft nur, dass die Station „durch dich“ ist. */
export function asksBeforeDone(
  station: Pick<StationView, 'released'>,
  implementation: Pick<ImplementationView, 'masterSessionId'>,
): boolean {
  return implementation.masterSessionId !== null && !station.released;
}
