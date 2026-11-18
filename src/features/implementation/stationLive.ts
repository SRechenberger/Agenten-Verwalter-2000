// Was eine Station von ihrer Session zeigt, wenn sie fertig oder gemeldet ist: Text, Symbol, Grund einer
// Pause. Reine Regeln; die Datei importiert nur Typen, damit `src/app/sidebarPlacement.selftest.ts` sie
// unter Node prüfen kann (`node` löst den Alias `@/` nicht auf).
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

/** Text der Zeile, Kartei und Leiste, wenn die Session mehr sagt als `state`; sonst `null`. */
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
    case 'waitsForQuota':
      return lead === null ? null : `${lead} · pausiert`;
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
