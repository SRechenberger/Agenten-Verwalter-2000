// Befehls-Stationen (ADR 073) in der Oberfläche: Beschriftung und Kürzen der Schritte, dazu die Anzeige jeder
// Station (ADR 061, Nachtrag 2026-11-22). Reine Regeln, nur
// Typ-Importe (Selbsttest ohne Build). Die Oberfläche zeigt, was der Kern liefert; sie setzt nichts ein.
import type { StationView } from '../../lib/bindings/StationView.ts';

/** Zeichen eines Schritts in der Zeile, bevor „…“ folgt; der Tooltip zeigt ihn ganz. */
export const STEP_SHORT_CHARS = 48;

/** Eine Befehls-Station: die Art kommt aus `kind`, nicht aus `byUser`. */
export function isCommandStation(station: Pick<StationView, 'kind'>): boolean {
  return station.kind === 'command';
}

/** Eine Station „Nachricht“ (Taiga 133): Der Verwalter stellt eine Vorlage zu, ohne Agent. */
export function isMessageStation(station: Pick<StationView, 'kind'>): boolean {
  return station.kind === 'message';
}

/** `startError` heißt „startet nicht“, außer bei einer Befehls-Station oder Station „Nachricht“ im Fehler:
 *  Dort trägt es den Grund des Laufs („Schritt 2: Exit 1“, „Die Station … hat keine Session.“), nicht den
 *  eines Starts ohne Session. */
export function isStartFailure(
  station: Pick<StationView, 'kind' | 'state' | 'startError'>,
): boolean {
  const isRun: boolean = isCommandStation(station) || isMessageStation(station);
  return station.startError !== null && !(isRun && station.state === 'error');
}

/** „Merge · Befehl“ bei einer Befehls-Station, „Info · Nachricht“ bei einer Station „Nachricht“, sonst die
 *  Stufe. */
export function stageText(station: Pick<StationView, 'kind' | 'stage'>): string {
  if (isCommandStation(station)) {
    return `${station.stage} · Befehl`;
  }
  return isMessageStation(station) ? `${station.stage} · Nachricht` : station.stage;
}

/** Wer die Nachricht bekommt: „Master“ oder die Station mit dieser Kennung, wie sie überall heißt
 *  (`stationDisplay`); eine Kennung, die es nicht gibt, steht, wie sie ist. */
export function recipientText(
  to: string,
  stations: readonly Pick<StationView, 'kind' | 'planId' | 'stage'>[],
): string {
  if (to === 'master') {
    return 'Master';
  }
  const found: Pick<StationView, 'kind' | 'planId' | 'stage'> | undefined = stations.find(
    (station) => station.planId === to,
  );
  return found === undefined ? to : stationDisplay(found);
}

/** Wie eine Station überall heißt (ADR 061, Nachtrag 2026-11-22), wie `station_label` im Core: „C-5 · Review“,
 *  eine ältere Umsetzung mit ihrer gespeicherten Kennung („a-umsetzung · Umsetzung“); eine Befehls-Station
 *  trägt dazu „· Befehl“ (`stageText`). Die Kennung vergibt der Core; die Oberfläche rechnet sie nicht nach. */
export function stationDisplay(station: Pick<StationView, 'kind' | 'planId' | 'stage'>): string {
  return `${station.planId} · ${stageText(station)}`;
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

/** Der Name eines Platzhalters wie im Kern (`command.rs`); alles andere in `{{…}}` ist Text. */
const PLACEHOLDER_NAME = /^[a-z][a-z0-9_]{0,31}$/;

/** „Werte von 102.3 · PR: pr = 83, head = 4f2c9e1“: die Werte, die in den Schritten stehen, gelesen aus
 *  Vorlage und eingesetzten Schritten (die Ansicht trägt die Werte selbst nicht). `{{env:…}}` und `{{.x}}` einer
 *  Go-Vorlage sind kein Name und geben nie einen Wert; `null`, wenn nichts eingesetzt ist. Ein Wert mit Leerraum
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
        if (!PLACEHOLDER_NAME.test(name)) {
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
