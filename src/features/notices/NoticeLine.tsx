import type { ReactElement, ReactNode } from 'react';
import './NoticeLine.css';

export type NoticeLineVariant = 'info' | 'warn' | 'model';

interface NoticeLineAction {
  label: string;
  onClick: () => void;
}

interface NoticeLineProps {
  variant: NoticeLineVariant;
  icon: ReactNode;
  /** Fetter Anfang der Zeile. */
  lead: string;
  text: string;
  /** Kleiner Zusatz unter der Zeile. */
  detail?: string | undefined;
  action?: NoticeLineAction | undefined;
  /** Bedienelement rechts in der Zeile, wenn ein Knopf nicht reicht. */
  control?: ReactNode;
}

export function NoticeLine({
  variant,
  icon,
  lead,
  text,
  detail,
  action,
  control,
}: NoticeLineProps): ReactElement {
  return (
    <div className={`notice-line notice-line--${variant}`} role="note">
      <span className="notice-line__icon">{icon}</span>
      <div className="notice-line__body">
        <p className="notice-line__text">
          <strong>{lead}</strong> {text}
        </p>
        {detail !== undefined && <p className="notice-line__detail">{detail}</p>}
      </div>
      {action !== undefined && (
        <button type="button" className="notice-line__action" onClick={action.onClick}>
          {action.label}
        </button>
      )}
      {control}
    </div>
  );
}
