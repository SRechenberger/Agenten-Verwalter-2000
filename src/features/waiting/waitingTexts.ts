/** Anhang an den Tooltip von Stationszeile und ‹ › der Rückweg-Leiste. */
export const WAITING_JUMP_HINT = ' · Ctrl J: nächste, die auf dich wartet';

export const NOBODY_WAITING_TEXT = 'Gerade wartet niemand auf dich';

/** „1 von 3 warten auf dich“: Platz des Ziels in der Warteliste. */
export function waitingPositionText(position: number, total: number): string {
  const verb: string = total === 1 ? 'wartet' : 'warten';
  return `${String(position)} von ${String(total)} ${verb} auf dich`;
}
