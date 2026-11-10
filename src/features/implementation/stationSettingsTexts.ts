import type { ValueSource } from '@/features/implementation/stationAgent';

// Texte des Dialogs „Einstellungen der Station“ (Entwurf: docs/design/2026-11-07_agententypen/, Schritte 14 und 15).
// Die Meldungen der Prüfung kommen vom Core und stehen nicht hier. Abbrechen, Speichern und die Rückfrage vor dem
// Verwerfen sind die des Typ-Dialogs (`agentTypeTexts.ts`).

export const DIALOG_TITLE = 'Einstellungen der Station';

export function dialogSubtitle(phaseKey: string, stage: string): string {
  return `Phase ${phaseKey} · ${stage}. Gilt nur für diese Station und nur, solange sie nicht gestartet ist. Was leer bleibt, übernimmt die Station vom Typ.`;
}

export const LIKE_TYPE = 'Wie der Typ';
export const LIKE_RULE = 'Wie die allgemeine Regel';
export const SET_WORD = 'Abweichung';
export const RESET_LABEL = 'Zurücksetzen';
export const RESET_ALL_LABEL = 'Alles zurücksetzen';

export function resetAriaLabel(label: string): string {
  return `${label} zurücksetzen`;
}

export const TYPE_LABEL = 'Typ';
export const TYPE_INHERITED = 'nach Stationsname';
export const TYPE_SET_WORD = 'ausdrücklich';
export const TYPE_HINT =
  'Die Typen legst du in den Einstellungen an. „Nach Stationsname“ nimmt den Typ, der den Namen der Station führt.';

export function typeByStageOption(typeName: string | null): string {
  return `Nach Stationsname (${typeName ?? 'kein Typ'})`;
}

export function missingTypeOption(name: string): string {
  return `${name} (gelöscht)`;
}

export const MODEL_LABEL = 'Modell';
export const MODEL_HINT =
  'Gilt in jeder Betriebsart, wie das Modell einer Station heute. Das Modell des Typs gilt nur in der Betriebsart Claude.';
export const MODEL_MENU_TITLE = 'Modell für diese Station';
export const MODEL_MENU_NOTE = 'Ein Modell der Station gewinnt gegen das des Typs.';

export const EFFORT_LABEL = 'Denkaufwand';
export const EFFORT_HINT =
  'Etwa Max für ein besonders kritisches Review. Gilt unabhängig vom Modell.';

export const PROMPT_LABEL = 'Zusatz zum Systemprompt';
export const PROMPT_INHERITED = 'kein Zusatztext';
export const PROMPT_PLACEHOLDER =
  'Zusätzliche Anweisungen nur für diese Station, z. B. „Prüfe besonders streng …“';
export const PROMPT_HINT =
  'Steht hinter Text und Dateien des Typs im Systemprompt, in allen Betriebsarten. Dateien gibt es nur am Typ.';

export const SKILL_LABEL = 'Skill';
export const SKILL_INHERITED = 'kein eigener Skill';
export const SKILL_PLACEHOLDER = 'z. B. vault:role-code-reviewer';

export function skillHint(typeSkills: readonly string[]): string {
  const base =
    'Steht als Befehl in der ersten Zeile; die Skills des Typs folgen als Anweisung in der Nachricht.';
  return typeSkills.length === 0 ? base : `${base} Skills des Typs: ${typeSkills.join(', ')}.`;
}

export const PREVIEW_LEAD = 'So startet die Station:';
export const NO_TYPE = 'kein Typ';
export const MODEL_LIKE_SESSION = 'Modell wie die Session';
export const EFFORT_LIKE_SESSION = 'Denkaufwand wie die Session';

const SOURCE_WORD: Record<ValueSource, string> = {
  station: 'Station',
  type: 'Typ',
  rule: 'Regel',
};

export function sourceWord(source: ValueSource | null): string {
  return source === null ? '' : ` (${SOURCE_WORD[source]})`;
}

export function missingTypePreview(name: string): string {
  return `${name} (gelöscht)`;
}

export function skillPreview(name: string, source: 'station' | 'type'): string {
  return `Skill /${name} (${SOURCE_WORD[source]})`;
}

export function promptPreview(count: number): string {
  return `Zusatztext ${count.toLocaleString('de-DE')} Zeichen`;
}

export function saveBlockedText(count: number): string {
  return count === 1
    ? 'Speichern nicht möglich: 1 Angabe ist ungültig.'
    : `Speichern nicht möglich: ${String(count)} Angaben sind ungültig.`;
}

export function saveFailedText(reason: string): string {
  return `Nicht gespeichert: ${reason}`;
}
