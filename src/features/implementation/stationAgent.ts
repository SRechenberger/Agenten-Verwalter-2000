import type { AgentType } from '@/lib/bindings/AgentType';
import type { AgentValueSource } from '@/lib/bindings/AgentValueSource';
import type { Effort } from '@/lib/bindings/Effort';
import type { ModelId } from '@/lib/bindings/ModelId';
import type { OperatingMode } from '@/lib/bindings/OperatingMode';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import type { StationView } from '@/lib/bindings/StationView';

// Was die Karte und der Dialog einer Station über ihren Typ, ihr Modell und ihren Denkaufwand sagen
// (ADR 060; Plan „Agententypen für Stationen“, Abschnitte 4.3 und 4.5). Die Karte liest die
// Auflösung, die der Core vor dem Start in `StationView` liefert; nur die Vorschau im Dialog rechnet
// sie für einen Entwurf selbst nach, denn der Core kennt den Entwurf nicht.

/** Woher ein Wert kommt: Station, Typ oder die allgemeine Regel. */
export type ValueSource = AgentValueSource | 'rule';

/** Ein Wert mit Herkunft; `source` ist `null`, wo die Herkunft unbekannt ist (der Wert der Session nach dem Start). */
export interface AgentValue<T> {
  value: T;
  source: ValueSource | null;
}

/** Schlüssel eines Typnamens wie im Core (`agent_types::name_key`): getrimmt, ohne Beachtung der Schreibweise. */
export function nameKey(name: string): string {
  return name.trim().toLowerCase();
}

export function typeNamed(types: readonly AgentType[], name: string): AgentType | null {
  const key: string = nameKey(name);
  return types.find((candidate: AgentType) => nameKey(candidate.name) === key) ?? null;
}

/** Der Typ, der den Stationsnamen führt; keiner, wenn kein Typ passt. */
export function typeForStage(types: readonly AgentType[], stage: string): AgentType | null {
  const key: string = nameKey(stage);
  return (
    types.find((candidate: AgentType) =>
      candidate.stages.some((name: string) => nameKey(name) === key),
    ) ?? null
  );
}

/** Die Session mit der höchsten Nummer im Vorhaben: Von ihr erbt eine neue Session Modell und Denkaufwand
 *  (die „allgemeine Regel“ des Core, `create_fresh_in_project`; Plan „Agententypen“, N2: später gelten dafür
 *  die Einstellungen, dann ändert sich nur diese Funktion). */
export function latestSessionOf(
  sessions: readonly SessionSummary[],
  projectId: string,
): SessionSummary | null {
  let latest: SessionSummary | null = null;
  for (const session of sessions) {
    if (session.projectId === projectId && (latest === null || session.number > latest.number)) {
      latest = session;
    }
  }
  return latest;
}

/** Was in einer Betriebsart außer Claude vom Typ übrig bleibt: nichts. Modell und Denkaufwand des Typs
 *  wendet nur die Betriebsart Claude an, ausdrückliche Werte der Station gelten in jeder (Abschnitt 4.5). */
export function isClaude(mode: OperatingMode): boolean {
  return mode === 'claude';
}

/** Die Werte, aus denen die Karte Modell oder Denkaufwand einer Station zeigt. */
interface CardChoice<T extends string> {
  /** Ausdrücklicher Wert der Station. */
  explicit: T | null;
  /** Vor dem Start vom Core aufgelöst, unabhängig von der Betriebsart. */
  resolved: T | null;
  from: AgentValueSource | null;
  /** Wert der Session der Station, nach dem Start. */
  session: T | null;
  /** Wert der Session, von der eine neue erbt (allgemeine Regel). */
  rule: T | null;
}

/** Der Wert, den die Karte zeigt; `null` heißt „wie die Session“. Außerhalb der Betriebsart Claude nur ein
 *  ausdrücklicher Wert der Station, nach dem Start in Claude der der Session (der Nutzer kann ihn in der
 *  Eingabeleiste geändert haben, seine Herkunft ist dann unbekannt), davor die Auflösung des Core. */
function cardValue<T extends string>(
  choice: CardChoice<T>,
  isStarted: boolean,
  isClaudeMode: boolean,
): AgentValue<T> | null {
  if (!isClaudeMode) {
    return choice.explicit === null ? null : { value: choice.explicit, source: 'station' };
  }
  if (isStarted) {
    return choice.session === null ? null : { value: choice.session, source: null };
  }
  if (choice.resolved !== null) {
    return { value: choice.resolved, source: choice.from ?? 'type' };
  }
  return choice.rule === null ? null : { value: choice.rule, source: 'rule' };
}

export interface CardAgentFacts {
  isStarted: boolean;
  model: AgentValue<ModelId> | null;
  effort: AgentValue<Effort> | null;
}

/** Modell und Denkaufwand der Karte. `session` ist die Session der Station, `rule` die, von der eine neue erbt. */
export function cardAgentFacts(
  station: StationView,
  mode: OperatingMode,
  session: SessionSummary | null,
  rule: SessionSummary | null,
): CardAgentFacts {
  const isStarted: boolean = station.sessionId !== null;
  const claude: boolean = isClaude(mode);
  return {
    isStarted,
    model: cardValue<ModelId>(
      {
        explicit: station.model,
        resolved: station.agentModel,
        from: station.agentModelFrom,
        session: session?.model ?? null,
        rule: rule?.model ?? null,
      },
      isStarted,
      claude,
    ),
    effort: cardValue<Effort>(
      {
        explicit: station.effort,
        resolved: station.agentEffort,
        from: station.agentEffortFrom,
        session: session?.effort ?? null,
        rule: rule?.effort ?? null,
      },
      isStarted,
      claude,
    ),
  };
}

/** Die fünf Einstellungen, wie der Dialog sie hält; leer heißt „wie der Typ“ (Typ: „nach Stationsname“). */
export interface StationDraft {
  agent: string;
  model: ModelId | null;
  effort: Effort | null;
  prompt: string;
  skill: string;
}

/** Der Typ, als der die Station mit diesem Entwurf liefe. `missing` ist der Name, den der Entwurf nennt und
 *  den es nicht gibt (ein gelöschter Typ); der Start scheiterte dann. */
export interface DraftType {
  type: AgentType | null;
  missing: string | null;
}

export function draftType(types: readonly AgentType[], stage: string, agent: string): DraftType {
  if (agent.trim() === '') {
    return { type: typeForStage(types, stage), missing: null };
  }
  const named: AgentType | null = typeNamed(types, agent);
  return { type: named, missing: named === null ? agent.trim() : null };
}

/** Wert nach der Auflösung des Entwurfs (Abschnitt 4.3 mit 4.5): Station, sonst Typ (nur in Claude), sonst Regel
 *  (nur in Claude; sonst gilt „wie die Session“, `null`). */
export function draftValue<T extends string>(
  explicit: T | null,
  ofType: T | null,
  rule: T | null,
  isClaudeMode: boolean,
): AgentValue<T> | null {
  if (explicit !== null) {
    return { value: explicit, source: 'station' };
  }
  if (!isClaudeMode) {
    return null;
  }
  if (ofType !== null) {
    return { value: ofType, source: 'type' };
  }
  return rule === null ? null : { value: rule, source: 'rule' };
}

/** Der Skill, mit dem die Station startet, und woher er kommt. */
export interface DraftSkill {
  name: string;
  source: 'station' | 'type';
}

/** Der Skill, mit dem die Station startet: ihr eigener, sonst der erste des Typs. */
export function draftSkill(skill: string, type: AgentType | null): DraftSkill | null {
  const own: string = skill.trim().replace(/^\//, '');
  if (own !== '') {
    return { name: own, source: 'station' };
  }
  const first: string | undefined = type?.skills[0];
  return first === undefined ? null : { name: first, source: 'type' };
}
