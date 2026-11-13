import type { StationRef } from '@/features/implementation/implementationOrder';
import type { PlanEdit } from '@/lib/bindings/PlanEdit';
import type { StationPatch } from '@/lib/bindings/StationPatch';

// Kanten der Umsetzung (ADR 061) für die Oberfläche: Stufe und Zyklus eines Entwurfs, den der Core
// noch nicht kennt („+ Lane“, Kanten-Chips). Für den gespeicherten Plan liefert der Core `level`,
// `after` und `next`; dieselbe Regel steht in `schedule::levels` und `schedule::check_plan`.

/** Ein Knoten des Plans: Plan-Kennung und die Kennungen, auf die er wartet. */
export interface PlanNode {
  planId: string;
  after: readonly string[];
}

export function nodesOf(refs: readonly StationRef[]): PlanNode[] {
  return refs.map((ref: StationRef) => ({ planId: ref.station.planId, after: ref.station.after }));
}

/** Stufe je Plan-Kennung: die längste Kette bis zur Station, Wurzeln haben Stufe 1; ein unbekannter
 *  Vorgänger zählt nicht. Ein Zyklus endet nach n Durchläufen wie im Core. */
export function levelsOf(nodes: readonly PlanNode[]): Map<string, number> {
  const level = new Map<string, number>(nodes.map((node: PlanNode) => [node.planId, 1]));
  // ponytail: wiederholte Durchläufe wie `schedule::levels`, O(n²); ein Plan hat wenige Dutzend Stationen.
  for (let round = 0; round < nodes.length; round += 1) {
    let isChanged = false;
    for (const node of nodes) {
      const deepest: number = Math.max(
        0,
        ...node.after.map((before: string) => level.get(before) ?? 0),
      );
      if (deepest + 1 > (level.get(node.planId) ?? 1)) {
        level.set(node.planId, deepest + 1);
        isChanged = true;
      }
    }
    if (!isChanged) {
      break;
    }
  }
  return level;
}

/** Ein Weg `from` → … → `to` entlang der Nachfolger (umgekehrte `after`); `null`, wenn keiner führt. */
export function pathBetween(nodes: readonly PlanNode[], from: string, to: string): string[] | null {
  const previous = new Map<string, string>();
  const queue: string[] = [from];
  const seen = new Set<string>([from]);
  while (queue.length > 0) {
    const current: string = queue.shift() ?? from;
    if (current === to) {
      const way: string[] = [to];
      let step: string | undefined = previous.get(to);
      while (step !== undefined) {
        way.unshift(step);
        step = previous.get(step);
      }
      return way;
    }
    for (const node of nodes) {
      if (node.after.includes(current) && !seen.has(node.planId)) {
        seen.add(node.planId);
        previous.set(node.planId, current);
        queue.push(node.planId);
      }
    }
  }
  return null;
}

/** Ergäbe die neue Kante „`waiter` wartet auf `before`“ einen Zyklus? Dann der Weg als Kennungen
 *  (`waiter` → … → `before` → `waiter`), sonst `null`. */
export function cycleWith(
  nodes: readonly PlanNode[],
  before: string,
  waiter: string,
): string[] | null {
  if (before === waiter) {
    return [waiter, waiter];
  }
  const way: string[] | null = pathBetween(nodes, waiter, before);
  return way === null ? null : [...way, waiter];
}

/** Ein Delta, das nur wartende Stationen ändert (Kanten, Auftrag), für `editPlan`. */
export function patchEdit(...patches: StationPatch[]): PlanEdit {
  return { lanes: [], add: [], change: patches, remove: [] };
}
