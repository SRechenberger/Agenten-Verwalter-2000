import type { SkillInfo } from '@/lib/bindings/SkillInfo';
import type { SkillUse } from '@/lib/bindings/SkillUse';

/** Zählschlüssel eines Skills — dieselbe Bildung wie `db/skill_uses.rs` im Core. */
export function skillUseKey(skill: SkillInfo): string {
  switch (skill.origin.kind) {
    case 'user':
      return `user:${skill.name}`;
    case 'builtin':
      return `builtin:${skill.name}`;
    case 'repository':
      return `repository:${skill.origin.name}:${skill.name}`;
  }
}

export function usesByKey(uses: readonly SkillUse[]): ReadonlyMap<string, number> {
  return new Map(uses.map((use: SkillUse): [string, number] => [use.key, use.count]));
}

export function useCount(skill: SkillInfo, uses: ReadonlyMap<string, number>): number {
  return uses.get(skillUseKey(skill)) ?? 0;
}

/** Absteigend nach Aufrufen; bei Gleichstand bleibt die Reihenfolge der Eingabe (stabile Sortierung). */
export function sortByUse(
  skills: readonly SkillInfo[],
  uses: ReadonlyMap<string, number>,
): SkillInfo[] {
  return [...skills].sort(
    (first: SkillInfo, second: SkillInfo): number => useCount(second, uses) - useCount(first, uses),
  );
}
