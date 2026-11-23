import type { ReactElement } from 'react';
import { Popover } from '@/components/Popover';
import {
  MENU_HEADING,
  MENU_SETTINGS_LINK,
  textPreview,
} from '@/features/prompttemplates/promptTemplateTexts';
import type { PromptTemplate } from '@/lib/bindings/PromptTemplate';
import '@/features/sessions/GroupMenu.css';
import './TemplateMenu.css';

const TEMPLATE_MENU_WIDTH = 340;

interface TemplateMenuProps {
  promptTemplates: readonly PromptTemplate[];
  onPick: (promptTemplate: PromptTemplate) => void;
  onOpenSettings: () => void;
  onClose: () => void;
}

/** Das Menü am Knopf ▤ (Bild 1B): die Vorlagen mit ihrer ersten Zeile, darunter der Weg in die Einstellungen.
 *  Tab geht durch die Einträge, Enter wählt, Esc schließt. */
export function TemplateMenu({
  promptTemplates,
  onPick,
  onOpenSettings,
  onClose,
}: TemplateMenuProps): ReactElement {
  return (
    <Popover
      label={MENU_HEADING}
      placement="below"
      align="start"
      width={TEMPLATE_MENU_WIDTH}
      onClose={onClose}
    >
      <div className="group-menu__head">{MENU_HEADING}</div>
      {promptTemplates.map((promptTemplate: PromptTemplate) => (
        <button
          key={promptTemplate.id}
          type="button"
          role="menuitem"
          className="group-menu__item template-menu__item"
          onClick={(): void => {
            onPick(promptTemplate);
          }}
        >
          <span className="group-menu__name">{promptTemplate.name}</span>
          <span className="template-menu__preview">{textPreview(promptTemplate.text)}</span>
        </button>
      ))}
      <div className="group-menu__separator" role="separator" />
      <button
        type="button"
        role="menuitem"
        className="group-menu__item template-menu__link"
        onClick={onOpenSettings}
      >
        {MENU_SETTINGS_LINK}
      </button>
    </Popover>
  );
}
