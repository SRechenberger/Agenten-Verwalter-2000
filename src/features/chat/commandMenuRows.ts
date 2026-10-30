import type { SessionStatus } from '@/lib/bindings/SessionStatus';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import type { SkillInfo } from '@/lib/bindings/SkillInfo';
import { sortByUse, useCount } from '@/features/skills/skillUses';
import { modelName } from '@/lib/labels';

/** `full`: Knopf `/` in der Eingabeleiste · `slash`: `/` am Feldanfang · `skills`: nur Skills („Neue Session“). */
export type CommandScope = 'full' | 'slash' | 'skills';
const SESSION_COMMANDS = ['rename', 'changes', 'mcp', 'pause', 'chrome', 'btw'] as const;
export type SessionCommand = (typeof SESSION_COMMANDS)[number];
export type CommandRow =
  | { kind: 'attach' }
  | { kind: 'model' }
  | { kind: 'effort' }
  /** `count`: Aufrufe der letzten 30 Tage; `null`, wenn die Zählung fehlt. */
  | { kind: 'skill'; skill: SkillInfo; count: number | null }
  | { kind: 'unusedToggle'; count: number; isOpen: boolean }
  | { kind: 'session'; command: SessionCommand };

export interface CommandSection {
  title: 'Kontext' | 'Modell' | 'Skills' | 'Session';
  rows: CommandRow[];
}

interface RowText {
  label: string;
  description: string;
}

const UNUSED_TOGGLE_LABEL = 'Seit 30 Tagen nicht benutzt';

const PAUSABLE_STATUSES: readonly SessionStatus[] = ['starting', 'running', 'waiting'];

const SESSION_COMMAND_TEXT: Record<Exclude<SessionCommand, 'chrome'>, RowText> = {
  rename: { label: '/rename', description: 'Session umbenennen' },
  changes: { label: '/changes', description: 'Changes-Ansicht öffnen' },
  mcp: { label: '/mcp', description: 'MCP-Server anzeigen, ausschalten, neu verbinden' },
  pause: { label: '/pause', description: 'Agent pausieren' },
  btw: { label: '/btw', description: 'Nebenfrage stellen, ohne den Chat zu verlängern' },
};

export function rowText(row: CommandRow, session: SessionSummary | null): RowText {
  switch (row.kind) {
    case 'attach':
      return { label: 'Datei oder Bild anhängen …', description: '' };
    case 'model':
      return {
        label: 'Modell wechseln …',
        description: session === null ? '' : modelName(session.model),
      };
    case 'effort':
      return { label: 'Denkaufwand', description: '' };
    case 'skill':
      return { label: `/${row.skill.name}`, description: row.skill.description };
    case 'unusedToggle':
      return { label: `${row.isOpen ? '▾' : '▸'} ${UNUSED_TOGGLE_LABEL}`, description: '' };
    case 'session':
      if (row.command === 'chrome') {
        return {
          label: '/chrome',
          description:
            session?.chrome === true
              ? 'Browser-Zugriff ausschalten'
              : 'Browser-Zugriff einschalten',
        };
      }
      return SESSION_COMMAND_TEXT[row.command];
  }
}

/** Stabile Kennung einer Zeile; gleiche Skill-Namen aus verschiedenen Quellen bleiben unterscheidbar. */
export function rowKey(row: CommandRow): string {
  switch (row.kind) {
    case 'skill': {
      const origin: string =
        row.skill.origin.kind === 'user' ? 'user' : `repository:${row.skill.origin.name}`;
      return `skill:${origin}:${row.skill.kind}:${row.skill.name}`;
    }
    case 'session':
      return `session:${row.command}`;
    default:
      return row.kind;
  }
}

export function buildCommandSections(
  scope: CommandScope,
  filter: string,
  skills: readonly SkillInfo[],
  session: SessionSummary | null,
  hasBrowser: boolean,
  hasMcp: boolean,
  uses: ReadonlyMap<string, number> | null,
  isUnusedOpen: boolean | null,
): CommandSection[] {
  const needle: string = filter.trim().toLowerCase();

  function matches(row: CommandRow): boolean {
    if (needle === '') {
      return true;
    }
    const { label, description } = rowText(row, session);
    return `${label} ${description}`.toLowerCase().includes(needle);
  }

  const sections: CommandSection[] = [
    { title: 'Kontext', rows: scope === 'full' ? [{ kind: 'attach' }] : [] },
    { title: 'Modell', rows: scope === 'full' ? [{ kind: 'model' }, { kind: 'effort' }] : [] },
    { title: 'Skills', rows: skillRows(skills, uses, isUnusedOpen) },
    { title: 'Session', rows: scope === 'skills' ? [] : sessionRows(session, hasBrowser, hasMcp) },
  ];

  // Ein exakt getippter Session-Befehl gewinnt gegen Skills, die den Namen nur enthalten.
  const typedName: string = needle.replace(/^\//, '');
  if (scope !== 'skills' && SESSION_COMMANDS.some((command: string) => command === typedName)) {
    sections.sort(
      (first: CommandSection, second: CommandSection) =>
        Number(second.title === 'Session') - Number(first.title === 'Session'),
    );
  }

  return sections
    .map((section: CommandSection): CommandSection => ({
      title: section.title,
      rows: section.rows.filter(matches),
    }))
    .filter((section: CommandSection) => section.rows.length > 0);
}

/**
 * Nach Gebrauch sortiert; fehlt die Zählung, bleibt es bei der Reihenfolge der Eingabe.
 * `isUnusedOpen`: `null` = alles in einer Liste (Filter, Slash-Menü), sonst hängt die Klapp-Zeile für ungenutzte Skills am Ende.
 */
function skillRows(
  skills: readonly SkillInfo[],
  uses: ReadonlyMap<string, number> | null,
  isUnusedOpen: boolean | null,
): CommandRow[] {
  if (uses === null) {
    return skills.map((skill: SkillInfo): CommandRow => ({ kind: 'skill', skill, count: null }));
  }
  const rows: CommandRow[] = sortByUse(skills, uses).map((skill: SkillInfo): CommandRow => ({
    kind: 'skill',
    skill,
    count: useCount(skill, uses),
  }));
  if (isUnusedOpen === null) {
    return rows;
  }
  const usedRows: CommandRow[] = rows.filter(
    (row: CommandRow) => row.kind === 'skill' && row.count !== 0,
  );
  const unusedRows: CommandRow[] = rows.filter(
    (row: CommandRow) => row.kind === 'skill' && row.count === 0,
  );
  if (unusedRows.length === 0) {
    return usedRows;
  }
  const toggle: CommandRow = {
    kind: 'unusedToggle',
    count: unusedRows.length,
    isOpen: isUnusedOpen,
  };
  return isUnusedOpen ? [...usedRows, toggle, ...unusedRows] : [...usedRows, toggle];
}

function sessionRows(
  session: SessionSummary | null,
  hasBrowser: boolean,
  hasMcp: boolean,
): CommandRow[] {
  if (session === null) {
    return [];
  }
  const rows: CommandRow[] = [{ kind: 'session', command: 'rename' }];
  if (session.repositoryCount > 0) {
    rows.push({ kind: 'session', command: 'changes' });
  }
  if (hasMcp) {
    rows.push({ kind: 'session', command: 'mcp' });
  }
  if (hasBrowser) {
    rows.push({ kind: 'session', command: 'chrome' });
  }
  if (PAUSABLE_STATUSES.includes(session.status)) {
    rows.push({ kind: 'session', command: 'pause' });
  }
  // Auch in Betriebsarten ohne Nebenfragen: die Auswahl sagt dort, dass es noch nicht geht.
  rows.push({ kind: 'session', command: 'btw' });
  return rows;
}

/** Alle Zeilen, die sich mit ↑/↓/Enter wählen lassen — der Denkaufwand hat seine eigenen Punkte. */
export function pickableRows(sections: readonly CommandSection[]): CommandRow[] {
  return sections
    .flatMap((section: CommandSection) => section.rows)
    .filter((row: CommandRow) => row.kind !== 'effort');
}

/** Die Zeile `delta` Schritte weiter, umlaufend; ohne gültige Markierung zählt die erste Zeile als markiert. */
export function moveHighlight(
  rows: readonly CommandRow[],
  current: CommandRow | null,
  delta: 1 | -1,
): CommandRow | null {
  if (rows.length === 0) {
    return null;
  }
  const currentKey: string | null = current === null ? null : rowKey(current);
  const currentIndex: number = Math.max(
    rows.findIndex((row: CommandRow) => rowKey(row) === currentKey),
    0,
  );
  return rows[(currentIndex + delta + rows.length) % rows.length] ?? null;
}

const LEADING_SLASH_TOKEN = /^\/\S*\s?/;

/** Setzt `/name ` an den Anfang des Entwurfs; ein schon vorhandenes `/…` am Anfang wird ersetzt. */
export function applySkillToDraft(draft: string, skillName: string): string {
  return `/${skillName} ${draft.replace(LEADING_SLASH_TOKEN, '')}`;
}
