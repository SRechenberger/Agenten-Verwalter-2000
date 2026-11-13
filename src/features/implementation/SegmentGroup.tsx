import type { ReactElement } from 'react';
import './SegmentGroup.css';

export interface SegmentOption<T> {
  id: T;
  label: string;
}

interface SegmentGroupProps<T> {
  /** Name der Gruppe für Screenreader. */
  label: string;
  options: readonly SegmentOption<T>[];
  value: T;
  disabled?: boolean;
  onChange: (value: T) => void;
}

/** Schaltergruppe statt nativer Auswahlliste (Entwurf: alle Optionen auf einmal, Farben aus dem Schema). */
export function SegmentGroup<T>({
  label,
  options,
  value,
  disabled = false,
  onChange,
}: SegmentGroupProps<T>): ReactElement {
  return (
    <span className="segment-group" role="group" aria-label={label}>
      {options.map((option: SegmentOption<T>) => {
        const isPressed: boolean = option.id === value;
        return (
          <button
            key={String(option.id)}
            type="button"
            className={`segment-group__button${isPressed ? ' segment-group__button--on' : ''}`}
            aria-pressed={isPressed}
            disabled={disabled}
            onClick={(): void => {
              if (!isPressed) {
                onChange(option.id);
              }
            }}
          >
            {option.label}
          </button>
        );
      })}
    </span>
  );
}
