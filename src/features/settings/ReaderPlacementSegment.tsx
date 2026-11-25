import type { ReactElement } from 'react';
import type { ReaderPlacement } from '@/lib/bindings/ReaderPlacement';

interface ReaderOption {
  readonly id: ReaderPlacement;
  readonly label: string;
}

const READER_OPTIONS: readonly ReaderOption[] = [
  { id: 'right', label: 'rechts neben dem Postfach' },
  { id: 'bottom', label: 'unter dem Postfach' },
] as const;

interface ReaderPlacementSegmentProps {
  value: ReaderPlacement;
  onChange: (placement: ReaderPlacement) => void;
}

/** Stile stehen in `SettingsView.css` unter `settings-view__segment`. */
export function ReaderPlacementSegment({
  value,
  onChange,
}: ReaderPlacementSegmentProps): ReactElement {
  return (
    <div className="settings-view__segment" role="group" aria-label="Lesebereich im Dashboard">
      {READER_OPTIONS.map((option: ReaderOption) => {
        const isPressed: boolean = option.id === value;
        return (
          <button
            key={option.id}
            type="button"
            className={`settings-view__segment-button${isPressed ? ' settings-view__segment-button--pressed' : ''}`}
            aria-pressed={isPressed}
            onClick={(): void => {
              onChange(option.id);
            }}
          >
            {option.label}
          </button>
        );
      })}
    </div>
  );
}
