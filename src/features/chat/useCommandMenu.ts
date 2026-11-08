import { useState } from 'react';
import type { KeyboardEvent } from 'react';
import {
  buildCommandSections,
  moveHighlight,
  pickableRows,
  rowKey,
} from '@/features/chat/commandMenuRows';
import { useOperatingMode } from '@/features/remote/useOperatingMode';
import { useSessionIsRemote } from '@/features/remote/useProjectAccess';
import type { CommandRow, CommandScope, CommandSection } from '@/features/chat/commandMenuRows';
import { useSkills } from '@/features/skills/useSkills';
import type { SkillSource } from '@/features/skills/useSkills';
import type { OperatingMode } from '@/lib/bindings/OperatingMode';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';

const SLASH_AT_START = /^\/(\S*)$/;

interface UseCommandMenuOptions {
  draft: string;
  /** Der Knopf `/` hat das Menü geöffnet (Zustand liegt beim Aufrufer, weil die Menüs sich gegenseitig schließen). */
  isButtonOpen: boolean;
  /** Ein anderes Menü ist offen oder das Feld gesperrt: `/` am Feldanfang öffnet nichts. */
  isSuppressed: boolean;
  buttonScope: CommandScope;
  slashScope: CommandScope;
  skillSource: SkillSource;
  session: SessionSummary | null;
}

export interface CommandMenuState {
  isOpen: boolean;
  isSlashOpen: boolean;
  /** Filterfeld nur beim Knopf-Menü; im Slash-Menü filtert das Textfeld selbst. */
  showFilter: boolean;
  filter: string;
  setFilter: (text: string) => void;
  resetFilter: () => void;
  sections: CommandSection[];
  highlighted: CommandRow | null;
  setHighlighted: (row: CommandRow) => void;
  /** Wahl einer Zeile per Klick oder Enter; `onPick` bekommt alles außer der Klapp-Zeile. */
  pick: (row: CommandRow, onPick: (row: CommandRow) => void) => void;
  /** Hält das Slash-Menü zu, bis der Entwurf nicht mehr mit `/` beginnt. */
  dismissSlash: () => void;
  /** ↑/↓/Enter des offenen Slash-Menüs im Textfeld; `true`, wenn die Taste verbraucht wurde. */
  handleSlashKeyDown: (
    event: KeyboardEvent<HTMLTextAreaElement>,
    onPick: (row: CommandRow) => void,
  ) => boolean;
  /** Bewegt die Markierung; `onPick` für Enter, wenn ein Eingabefeld des Menüs die Taste bekam. */
  handleNavigationKey: (
    event: KeyboardEvent<HTMLElement>,
    onPick: (row: CommandRow) => void,
  ) => boolean;
}

export function useCommandMenu({
  draft,
  isButtonOpen,
  isSuppressed,
  buttonScope,
  slashScope,
  skillSource,
  session,
}: UseCommandMenuOptions): CommandMenuState {
  const [buttonFilter, setButtonFilter] = useState<string>('');
  const [highlightedKey, setHighlightedKey] = useState<string | null>(null);
  const [isSlashDismissed, setIsSlashDismissed] = useState<boolean>(false);
  const [isUnusedOpen, setIsUnusedOpen] = useState<boolean>(false);
  // Claude in Chrome gibt es nur mit der Claude-Kommandozeile und dem Claude-Abo, und nicht an einem
  // entfernten Vorhaben: Dort ginge der Browser am Bildschirm des Hosts auf.
  const isRemote: boolean = useSessionIsRemote(session?.id ?? null);
  const operatingMode: OperatingMode = useOperatingMode(session?.projectId ?? null);
  const hasBrowser: boolean = operatingMode === 'claude' && !isRemote;

  // Codex nutzt seine eigenen MCP-Server aus `~/.codex/config.toml`; der Dialog zeigt nur die der Claude-Kommandozeile.
  const hasMcp: boolean = operatingMode !== 'codex';

  const slashMatch: RegExpExecArray | null = SLASH_AT_START.exec(draft);
  if (slashMatch === null && isSlashDismissed) {
    setIsSlashDismissed(false);
  }
  const isSlashOpen: boolean =
    slashMatch !== null && !isSlashDismissed && !isButtonOpen && !isSuppressed;
  const isOpen: boolean = isButtonOpen || isSlashOpen;
  const scope: CommandScope = isButtonOpen ? buttonScope : slashScope;
  const filter: string = isButtonOpen ? buttonFilter : (slashMatch?.[1] ?? '');

  if (!isOpen && isUnusedOpen) {
    setIsUnusedOpen(false);
  }

  const { skills, uses } = useSkills(skillSource, isOpen);
  // Einklappen gibt es nur im Knopf-Menü ohne Filter; sonst stehen alle Treffer in einer Liste.
  const isFoldable: boolean = isButtonOpen && filter.trim() === '';
  const sections: CommandSection[] = buildCommandSections(
    scope,
    filter,
    skills,
    session,
    hasBrowser,
    hasMcp,
    uses,
    isFoldable ? isUnusedOpen : null,
  );
  const rows: CommandRow[] = pickableRows(sections);
  // Ohne (noch gültige) Markierung gilt die erste wählbare Zeile als markiert.
  const highlighted: CommandRow | null =
    rows.find((row: CommandRow) => rowKey(row) === highlightedKey) ?? rows[0] ?? null;

  /** Die Klapp-Zeile schaltet nur um und lässt das Menü offen; jede andere Zeile geht an `onPick`. */
  function pick(row: CommandRow, onPick: (row: CommandRow) => void): void {
    if (row.kind === 'unusedToggle') {
      setIsUnusedOpen(!row.isOpen);
      return;
    }
    onPick(row);
  }

  function handleNavigationKey(
    event: KeyboardEvent<HTMLElement>,
    onPick: (row: CommandRow) => void,
  ): boolean {
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      const next: CommandRow | null = moveHighlight(
        rows,
        highlighted,
        event.key === 'ArrowDown' ? 1 : -1,
      );
      setHighlightedKey(next === null ? null : rowKey(next));
      return true;
    }
    if (event.key === 'Enter' && !event.ctrlKey && highlighted !== null) {
      event.preventDefault();
      pick(highlighted, onPick);
      return true;
    }
    return false;
  }

  return {
    isOpen,
    isSlashOpen,
    showFilter: isButtonOpen,
    filter,
    setFilter: setButtonFilter,
    resetFilter: (): void => {
      setButtonFilter('');
    },
    sections,
    highlighted,
    setHighlighted: (row: CommandRow): void => {
      setHighlightedKey(rowKey(row));
    },
    pick,
    dismissSlash: (): void => {
      setIsSlashDismissed(true);
    },
    handleSlashKeyDown: (event, onPick): boolean =>
      isSlashOpen ? handleNavigationKey(event, onPick) : false,
    handleNavigationKey,
  };
}
