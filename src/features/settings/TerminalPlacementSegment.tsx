import type { ReactElement } from 'react';
import type { TerminalPlacement } from '@/lib/bindings/TerminalPlacement';

interface PlacementOption {
  readonly id: TerminalPlacement;
  readonly label: string;
}

const PLACEMENT_OPTIONS: readonly PlacementOption[] = [
  { id: 'bottom', label: 'Unten' },
  { id: 'right', label: 'Rechts' },
] as const;

interface TerminalPlacementSegmentProps {
  value: TerminalPlacement;
  onChange: (placement: TerminalPlacement) => void;
}

/** Stile stehen in `SettingsView.css` unter `settings-view__segment`. */
export function TerminalPlacementSegment({
  value,
  onChange,
}: TerminalPlacementSegmentProps): ReactElement {
  return (
    <div className="settings-view__segment" role="group" aria-label="Terminal">
      {PLACEMENT_OPTIONS.map((option: PlacementOption) => {
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
