import type { ReactElement } from 'react';
import { Popover } from '@/components/Popover';
import './AttachMenu.css';

const ATTACH_MENU_WIDTH = 330;

interface AttachMenuProps {
  onPick: () => void;
  /** Fehlt in „Neue Session“: dort gibt es noch keinen Verlauf, den man fragen könnte. */
  onSide?: () => void;
  onClose: () => void;
  placement?: 'above' | 'below';
}

/** Das `+`-Menü der Eingabeleiste. */
export function AttachMenu({
  onPick,
  onSide,
  onClose,
  placement = 'above',
}: AttachMenuProps): ReactElement {
  return (
    <Popover
      label="Hinzufügen"
      placement={placement}
      align="start"
      width={ATTACH_MENU_WIDTH}
      onClose={onClose}
    >
      <button type="button" className="attach-menu__item" onClick={onPick}>
        <span className="attach-menu__label">Datei oder Bild anhängen …</span>
        <kbd className="attach-menu__key">Ctrl U</kbd>
      </button>
      {onSide !== undefined && (
        <button
          type="button"
          className="attach-menu__item"
          title="Frage zur Session, die nicht im Chat landet. Auch mit /btw."
          onClick={onSide}
        >
          <span className="attach-menu__label">Nebenfrage stellen …</span>
          <kbd className="attach-menu__key">Ctrl ;</kbd>
        </button>
      )}
      <div className="attach-menu__foot">
        Bilder, PDFs, Text und Code gehen. Einfach ins Fenster ziehen oder mit Ctrl+V einfügen.
      </div>
    </Popover>
  );
}
