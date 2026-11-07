import type { Effort } from '@/lib/bindings/Effort';
import type { Mode } from '@/lib/bindings/Mode';
import type { ModelId } from '@/lib/bindings/ModelId';
import type { OperatingMode } from '@/lib/bindings/OperatingMode';

const NO_LOCAL_MODEL_LABEL = 'Kein Modell gewählt';

export interface OperatingModeOption {
  readonly id: OperatingMode;
  readonly label: string;
}

export interface ModelOption<Id extends string = ModelId> {
  readonly id: Id;
  readonly name: string;
  readonly hint: string;
}

export interface ModeOption {
  readonly id: Mode;
  readonly icon: string;
  readonly label: string;
  readonly hint: string;
}

export interface EffortOption {
  readonly id: Effort;
  readonly label: string;
}

export const OPERATING_MODE_OPTIONS: readonly OperatingModeOption[] = [
  { id: 'claude', label: 'Claude' },
  { id: 'claudeCodeLocal', label: 'Claude Code + LM Studio' },
  { id: 'standalone', label: 'Autark' },
  { id: 'codex', label: 'Codex' },
] as const;

/** Text zu `CommandError` `codexNotFound` — die Variante trägt keine eigene Meldung. */
export const CODEX_NOT_FOUND_MESSAGE =
  'Codex nicht gefunden — installiere die Codex-Erweiterung für VS Code oder setze VERWALTER_CODEX_PATH.';

/** Betriebsarten, deren Modell in LM Studio läuft — dort gelten „Lokales Modell“ und dessen Wege. */
export function isLocalOperatingMode(mode: OperatingMode): boolean {
  return mode === 'claudeCodeLocal' || mode === 'standalone';
}

export const MODEL_OPTIONS: readonly ModelOption[] = [
  { id: 'fable', name: 'Fable 5.1', hint: 'Am stärksten, für Planung und Architektur' },
  { id: 'opus', name: 'Opus 5.5', hint: 'Gründlich, für schwierige Umsetzung' },
  { id: 'opusplan', name: 'Opus Plan', hint: 'Opus plant, Sonnet setzt um' },
  { id: 'sonnet', name: 'Sonnet 5.5', hint: 'Schnell und ausgewogen' },
  { id: 'haiku', name: 'Haiku 5.5', hint: 'Am schnellsten, für Kleinkram' },
] as const;

export const MODE_OPTIONS: readonly ModeOption[] = [
  {
    id: 'manual',
    icon: '✋',
    label: 'Manuell',
    hint: 'Claude fragt vor jeder Änderung um Erlaubnis',
  },
  {
    id: 'edit',
    icon: '</>',
    label: 'Automatisch bearbeiten',
    hint: 'Claude ändert Dateien ohne Rückfrage, fragt aber vor Befehlen',
  },
  {
    id: 'plan',
    icon: '☰',
    label: 'Planen',
    hint: 'Claude untersucht den Code und legt einen Plan vor, bevor er etwas ändert',
  },
  {
    id: 'auto',
    icon: 'ϟ',
    label: 'Auto',
    hint: 'Claude führt aus, was eine Sicherheitsprüfung besteht, und hält bei Riskantem an',
  },
] as const;

export const EFFORT_OPTIONS: readonly EffortOption[] = [
  { id: 'low', label: 'Niedrig' },
  { id: 'medium', label: 'Mittel' },
  { id: 'high', label: 'Hoch' },
  { id: 'xhigh', label: 'Sehr hoch' },
  { id: 'max', label: 'Max' },
] as const;

export function modelName(id: ModelId): string {
  const option: ModelOption | undefined = MODEL_OPTIONS.find(
    (candidate: ModelOption) => candidate.id === id,
  );
  return option === undefined ? id : option.name;
}

/** Anzeigename eines Modells, das die Kommandozeile gemeldet hat; eine fremde Kennung bleibt roh. */
export function reportedModelName(id: ModelId | null, rawId: string): string {
  return id === null ? rawId : modelName(id);
}

/** Der Teil der LM-Studio-Kennung nach dem letzten `/`; `null` heißt: kein Modell gewählt. */
export function localModelLabel(id: string | null): string {
  if (id === null) {
    return NO_LOCAL_MODEL_LABEL;
  }
  return id.slice(id.lastIndexOf('/') + 1);
}

export function effortLabel(id: Effort): string {
  const option: EffortOption | undefined = EFFORT_OPTIONS.find(
    (candidate: EffortOption) => candidate.id === id,
  );
  return option === undefined ? id : option.label;
}

/** Codex-Abos, die sich nicht durch Großschreibung des ersten Buchstabens lesen lassen. */
const PLAN_LABEL: Readonly<Record<string, string>> = {
  prolite: 'Pro Lite',
  promax: 'Pro Max',
  edu_plus: 'Edu Plus',
  edu_pro: 'Edu Pro',
};

/** Abo-Name aus der Kommandozeile (`pro`) für die Anzeige (`Pro`); Unbekanntes bleibt lesbar. */
export function planLabel(plan: string): string {
  return PLAN_LABEL[plan] ?? plan.charAt(0).toUpperCase() + plan.slice(1);
}

export function repositoryCountLabel(count: number): string {
  if (count === 0) {
    return 'Kein Repository';
  }
  if (count === 1) {
    return '1 Repository';
  }
  return `${String(count)} Repositories`;
}

export function modeOption(id: Mode): ModeOption {
  const option: ModeOption | undefined = MODE_OPTIONS.find(
    (candidate: ModeOption) => candidate.id === id,
  );
  if (option === undefined) {
    throw new Error(`Unbekannter Modus: ${id}`);
  }
  return option;
}

/** Der Name der Betriebsart, wie die Einstellungen ihn zeigen. */
export function operatingModeLabel(mode: OperatingMode): string {
  return (
    OPERATING_MODE_OPTIONS.find((option: OperatingModeOption) => option.id === mode)?.label ?? mode
  );
}
