// Befehls-Stationen (ADR 073) in der Oberfläche: Beschriftung und Kürzen der Schritte. Reine Regeln, nur
// Typ-Importe (Selbsttest ohne Build). Die Oberfläche zeigt, was der Kern liefert; sie setzt nichts ein.
import type { StationView } from '../../lib/bindings/StationView.ts';

/** Zeichen eines Schritts in der Zeile, bevor „…“ folgt; der Tooltip zeigt ihn ganz. */
export const STEP_SHORT_CHARS = 48;

/** Eine Befehls-Station: die Art kommt aus `kind`, nicht aus `byUser`. */
export function isCommandStation(station: Pick<StationView, 'kind'>): boolean {
  return station.kind === 'command';
}

/** `startError` heißt „startet nicht“, außer bei einer Befehls-Station im Fehler: Dort trägt es den Grund des
 *  Laufs („Schritt 2: Exit 1“), nicht den eines Starts ohne Session. */
export function isStartFailure(
  station: Pick<StationView, 'kind' | 'state' | 'startError'>,
): boolean {
  return station.startError !== null && !(isCommandStation(station) && station.state === 'error');
}

/** „Merge · Befehl“ bei einer Befehls-Station, sonst die Stufe. */
export function stageText(station: Pick<StationView, 'kind' | 'stage'>): string {
  return isCommandStation(station) ? `${station.stage} · Befehl` : station.stage;
}

/** Ein Schritt auf eine Zeile gekürzt; `{{…}}` und `{{env:…}}` bleiben, wie sie stehen. */
export function shortStep(step: string, max: number = STEP_SHORT_CHARS): string {
  const line: string = step.replace(/\s+/g, ' ').trim();
  return line.length <= max ? line : `${line.slice(0, max - 1)}…`;
}

/** Alle Schritte gekürzt, hintereinander: „gh pr checks {{pr}} … · gh pr merge {{pr}} …“. */
export function stepsSummary(steps: readonly string[]): string {
  return steps.map((step: string) => shortStep(step)).join(' · ');
}

/** Die Programme der Schritte (erstes Wort), je Programm einmal, in der Reihenfolge des ersten Auftretens. */
export function stepPrograms(steps: readonly string[]): string[] {
  const names: string[] = steps.map((step: string) => step.trim().split(/\s+/)[0] ?? '');
  return names.filter(
    (name: string, index: number) => name !== '' && names.indexOf(name) === index,
  );
}

/** Alle Schritte ganz, einer je Zeile, nummeriert: für den Tooltip. */
export function stepsTitle(steps: readonly string[]): string {
  return steps.map((step: string, index: number) => `${String(index + 1)}. ${step}`).join('\n');
}

/** „Schritt 1 von 2“ für die Kopfzeile des laufenden Befehls; `null`, solange kein Schritt läuft. */
export function stepProgress(step: number | null, total: number): string | null {
  return step === null ? null : `Schritt ${String(step)} von ${String(total)}`;
}

/** Warum ein Name nicht in die Freigabeliste gehört, oder `null`. Der Kern prüft zusätzlich und lehnt ab. */
export function programNameProblem(name: string, existing: readonly string[]): string | null {
  const trimmed: string = name.trim();
  if (trimmed === '') {
    return 'Name fehlt.';
  }
  if (trimmed.includes('/') || trimmed.includes('\\')) {
    return 'Nur der Name, ohne Pfad.';
  }
  return existing.includes(trimmed) ? 'Steht schon in der Liste.' : null;
}

/** Der Grund, den der Kern einer Station setzt, die beim Ende der App lief (`REASON_INTERRUPTED`). */
export const INTERRUPTED_REASON = 'unterbrochen';

/** Der Hinweis in der Kartei einer unterbrochenen Befehls-Station: Nach einem harten Ende der App kann ein
 *  Schritt weitergelaufen sein. */
export const INTERRUPTED_HINT =
  'Ein Schritt kann weitergelaufen sein. Prüfe den Stand, bevor du ▶ drückst.';

/** Eine Befehls-Station, die beim Ende der App lief. */
export function isInterrupted(
  station: Pick<StationView, 'kind' | 'state' | 'startError'>,
): boolean {
  return (
    isCommandStation(station) &&
    station.state === 'error' &&
    station.startError === INTERRUPTED_REASON
  );
}

/** „Werte von 102.3 · PR: pr = 83, head = 4f2c9e1“: die Werte, die in den Schritten stehen, gelesen aus
 *  Vorlage und eingesetzten Schritten (die Ansicht trägt die Werte selbst nicht). `{{env:…}}` bleibt in
 *  `resolved` wie es ist und gibt nie einen Wert; `null`, wenn nichts eingesetzt ist. Ein Wert mit Leerraum
 *  steht in Anführungszeichen, wie der Kern ihn einsetzt. */
export function valuesLine(
  command: readonly string[],
  resolved: readonly string[],
  from: readonly string[],
): string | null {
  const values: string[] = [];
  const seen = new Set<string>();
  command.forEach((template: string, index: number): void => {
    const names: string[] = [];
    const pattern: string = template
      .split(/(\{\{[^}]*\}\})/)
      .map((part: string): string => {
        const name: string = /^\{\{([^}]*)\}\}$/.exec(part)?.[1] ?? '';
        if (name === '' || name.startsWith('env:')) {
          return part.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
        }
        names.push(name);
        return '(.+?)';
      })
      .join('');
    const match: RegExpExecArray | null = new RegExp(`^${pattern}$`).exec(resolved[index] ?? '');
    names.forEach((name: string, position: number): void => {
      const value: string | undefined = match?.[position + 1];
      if (value !== undefined && !seen.has(name)) {
        seen.add(name);
        values.push(`${name} = ${value}`);
      }
    });
  });
  if (values.length === 0) {
    return null;
  }
  return `Werte von ${from.join(', ')}: ${values.join(', ')}`;
}
