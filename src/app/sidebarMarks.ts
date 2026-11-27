/** Marke an einer Session, die als Rückfrage zu einer Umsetzung angelegt wurde (ADR 076, Taiga 254). */
export const INQUIRY_MARK = 'Rückfrage';
export const INQUIRY_MARK_TITLE =
  'Rückfrage zu einer Umsetzung: eine frische Session mit dem Lagebericht';

/** Zeile des Vorhabens in der Leiste: „1 Umsetzung läuft“, „2 Umsetzungen laufen“; ohne unfertige `null`. */
export function runningImplementationsText(count: number): string | null {
  if (count <= 0) {
    return null;
  }
  return count === 1 ? '1 Umsetzung läuft' : `${String(count)} Umsetzungen laufen`;
}
