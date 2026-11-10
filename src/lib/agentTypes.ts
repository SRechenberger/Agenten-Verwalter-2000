import { invoke } from '@tauri-apps/api/core';
import { listen, type Event, type UnlistenFn } from '@tauri-apps/api/event';
import type { AgentType } from '@/lib/bindings/AgentType';
import type { AgentTypeInput } from '@/lib/bindings/AgentTypeInput';
import type { AgentTypeSaved } from '@/lib/bindings/AgentTypeSaved';

const AGENT_TYPES_CHANGED_EVENT = 'agent-types://changed';

// Die einzigen Regeln, die die Oberfläche selbst rechnet: die Zähler an Beschreibung und Text. Alle
// anderen Regeln prüft der Core beim Speichern und liefert die Meldung ans Feld (ADR 060; Plan
// „Agententypen für Stationen“, Abschnitt 4.2). Die Zahlen sind die des Core (`agent_types::validate`).
export const MAX_DESCRIPTION_CHARS = 500;
export const MAX_PROMPT_UNITS = 16_000;
// Grenze des Zusatztexts einer Station (Plan „Agententypen für Stationen“, Abschnitt 4.10 und N13; im
// Core `implementation::model::MAX_PROMPT_CHARS`). Anders als der Systemprompt zählt sie Zeichen
// (Codepunkte), nicht UTF-16-Einheiten, und der Text zählt unbeschnitten.
export const MAX_STATION_PROMPT_CHARS = 4_000;

/** Länge der Beschreibung, wie der Core sie prüft: getrimmt, in Zeichen (Codepunkten). `Array.from`
 *  zählt wie `[...text]` Codepunkte, nicht UTF-16-Einheiten; die Spread-Schreibweise verbietet die Lint-Regel. */
export function descriptionLength(description: string): number {
  return Array.from(description.trim()).length;
}

/** Länge des Zusatztexts einer Station, wie der Core sie prüft: nicht getrimmt, in Codepunkten (`Array.from`,
 *  aus demselben Grund wie bei `descriptionLength`). */
export function stationPromptLength(prompt: string): number {
  return Array.from(prompt).length;
}

/** Länge des Systemprompts, wie der Core sie prüft: nicht getrimmt, in UTF-16-Einheiten, denn so
 *  zählt Windows die Kommandozeile, auf die der Text später als Argument geht. */
export function promptLength(prompt: string): number {
  return prompt.length;
}

/** Alle Typen in der Reihenfolge, in der sie angelegt wurden.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `database` */
export function listAgentTypes(): Promise<AgentType[]> {
  return invoke<AgentType[]>('agent_types_list');
}

/** Legt einen Typ an. Meldet die Prüfung etwas, ist nichts gespeichert und `agentType` ist `null`.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `database` */
export function createAgentType(input: AgentTypeInput): Promise<AgentTypeSaved> {
  return invoke<AgentTypeSaved>('agent_type_create', { input });
}

/** Ändert den Typ; ein anderer Name benennt ihn um. Meldet die Prüfung etwas, ist nichts gespeichert.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `internal` (unbekannter Typ), `database` */
export function updateAgentType(id: string, input: AgentTypeInput): Promise<AgentTypeSaved> {
  return invoke<AgentTypeSaved>('agent_type_update', { id, input });
}

/** Löscht den Typ.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `internal` (unbekannter Typ), `database` */
export async function deleteAgentType(id: string): Promise<void> {
  await invoke('agent_type_delete', { id });
}

/** Meldet nach jeder Änderung die ganze Liste. */
export function onAgentTypesChanged(
  callback: (agentTypes: AgentType[]) => void,
): Promise<UnlistenFn> {
  return listen<AgentType[]>(AGENT_TYPES_CHANGED_EVENT, (event: Event<AgentType[]>) => {
    callback(event.payload);
  });
}
