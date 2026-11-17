// Beschriftungen des Masters einer Umsetzung (ADR 070): Rückgaben, Bericht, Kopf. Die Datei importiert
// nur Typen, damit `src/app/sidebarPlacement.selftest.ts` sie unter Node prüfen kann.
import type { ImplementationView } from '../../lib/bindings/ImplementationView.ts';
import type { SessionSummary } from '../../lib/bindings/SessionSummary.ts';

/** Die Rückgaben „n von 3“; die Grenze liefert der Core als `returnsLeft`, die Oberfläche rechnet sie nicht nach. */
export function returnsText(returns: number, returnsLeft: number): string {
  return `${String(returns)} von ${String(returns + returnsLeft)}`;
}

/** Marke in der Zeitleiste hinter einer zurückgegebenen Station. */
export function returnsChip(returns: number, returnsLeft: number): string {
  return `zurückgegeben ${String(returns)}/${String(returns + returnsLeft)}`;
}

export const RELEASED_CHIP = '✓ freigegeben';
export const RELEASED_MARK = '✓ vom Master freigegeben';
export const RELEASED_TITLE =
  'Der Master hat diese Station „durch dich“ freigegeben; „Erledigt“ bleibt bei dir.';

export const REPORT_TITLE = 'Bericht an den Master';
export const REPORT_FIX_MARK = 'Befund · Fix-Runde bei Annahme';
export const REPORT_NO_FIX_MARK = 'ohne Befund';

/** „gemeldet 11:42 · zurückgegeben 1 von 3“. */
export function reportLine(stamp: string, returns: number, returnsLeft: number): string {
  return `gemeldet ${stamp} · zurückgegeben ${returnsText(returns, returnsLeft)}`;
}

/** Name des Masters in Kopf, Hinweisen und Auswahl: „#1 Architekt“. */
export function masterName(number: number, name: string): string {
  return `#${String(number)} ${name}`;
}

/** Name des Masters der Umsetzung; `null`, wenn sie keinen hat oder es die Session nicht mehr gibt. */
export function masterOf(
  implementation: Pick<ImplementationView, 'masterSessionId'>,
  sessions: readonly Pick<SessionSummary, 'id' | 'number' | 'name'>[],
): string | null {
  const master = sessions.find((session) => session.id === implementation.masterSessionId);
  return master === undefined ? null : masterName(master.number, master.name);
}

export const MASTER_LABEL = 'Master:';
export const MASTER_CHANGE = 'ändern';
export const MASTER_CHANGE_TITLE =
  'Bestimmt eine andere Session des Vorhabens zum Master dieser Umsetzung';
export const MASTER_NONE = 'kein Master';
export const MASTER_POPOVER_TITLE = 'Neuer Master (Sessions des Vorhabens)';
export const MASTER_NEW_LABEL = 'Neue Session als Master';
export const MASTER_STATION_TITLE = 'Stationen können nicht Master werden';
export const MASTER_NOT_SET = 'Master nicht gewechselt';
export const MASTER_NEW_NOT_CREATED = 'Neue Session nicht angelegt';

/** Warnung im Kopf, solange der Master ausgefallen ist (gelöscht oder auf Fehler). */
export function masterMissingText(master: string | null): string {
  return master === null
    ? 'Die Master-Session dieser Umsetzung fehlt. Meldungen warten, bis du einen neuen Master bestimmst.'
    : `Die Master-Session ${master} ist ausgefallen. Meldungen warten, bis du einen neuen Master bestimmst.`;
}

export function waitsForMasterText(master: string | null): string {
  return master === null
    ? 'Wartet auf einen neuen Master: Der bisherige ist ausgefallen.'
    : `Wartet auf den Master ${master}. Er nimmt an oder gibt zurück.`;
}

export const DECIDE_TITLE = 'Du entscheidest';
export function exhaustedText(returns: number): string {
  return `Der Master hat diese Station ${String(returns)} Mal zurückgegeben. Jetzt entscheidest du.`;
}
export const RETURN_PLACEHOLDER = 'Nachricht an die Station, wenn du zurückgibst …';
export const RETURN_LABEL = 'Nachricht an die Station';
export const RETURN_BUTTON = 'Zurückgeben';
export const RETURN_EMPTY_TITLE = 'Erst Text eingeben';
export const RETURN_TITLE = 'Schickt die Station mit deiner Nachricht zurück an die Arbeit';
export const ACCEPT_BUTTON = 'Annehmen';
export const ACCEPT_TITLE =
  'Nimmt die Meldung an; Fix-Runde, Übergabe und Nachfolger starten wie bei einer Fertigmeldung';
export const ACCEPT_FAILED = 'Annehmen fehlgeschlagen';
export const RETURN_FAILED = 'Zurückgeben fehlgeschlagen';
