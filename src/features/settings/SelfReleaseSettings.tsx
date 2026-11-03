import { useState } from 'react';
import type { ReactElement } from 'react';
import { ModelMenu } from '@/components/ModelMenu';
import { Switch } from '@/components/Switch';
import { SelectButton } from '@/features/settings/SelectButton';
import { SettingRow } from '@/features/settings/SettingRow';
import type { ModelId } from '@/lib/bindings/ModelId';
import type { SelfRelease } from '@/lib/bindings/SelfRelease';
import { commandErrorText } from '@/lib/errors';
import { MODEL_OPTIONS, modelName } from '@/lib/labels';
import './SelfReleaseSettings.css';

const CHAIN_BRAKE_INFO =
  'Nach 6 selbst freigegebenen Aufrufen in Folge fragt der Kasten trotzdem nach.';

interface SwitchRow {
  key: 'messages' | 'tldr' | 'openSession';
  label: string;
  info: string;
}

const SWITCH_ROWS: readonly SwitchRow[] = [
  {
    key: 'messages',
    label: 'Selbstfreigabe für Nachrichten',
    info: `Ein Agent schickt Nachrichten an andere Sessions des Vorhabens ohne Kasten. ${CHAIN_BRAKE_INFO}`,
  },
  {
    key: 'tldr',
    label: 'Selbstfreigabe für TL;DR',
    info: 'Ein Agent liest das TL;DR einer anderen Session des Vorhabens ohne Kasten.',
  },
  {
    key: 'openSession',
    label: 'Selbstfreigabe für neue Sessions',
    info: `Ein Agent öffnet neue Sessions im Vorhaben ohne Kasten. ${CHAIN_BRAKE_INFO}`,
  },
];

interface SelfReleaseSettingsProps {
  value: SelfRelease;
  /** Ein Fehler beim Speichern steht unter den Zeilen. */
  onSave: (next: SelfRelease) => Promise<void>;
  /** Sagt im Modell-Menü, wann der Wert gilt. */
  modelNote: string;
}

/** Selbstfreigabe je Werkzeug und Modell des TL;DR; für ein Vorhaben oder als Standard neuer Vorhaben. */
export function SelfReleaseSettings({
  value,
  onSave,
  modelNote,
}: SelfReleaseSettingsProps): ReactElement {
  const [isSaving, setIsSaving] = useState<boolean>(false);
  const [isModelMenuOpen, setIsModelMenuOpen] = useState<boolean>(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  function save(next: SelfRelease): void {
    setIsSaving(true);
    setErrorMessage(null);
    onSave(next)
      .catch((reason: unknown) => {
        console.error('Selbstfreigabe nicht gespeichert', reason);
        setErrorMessage(`Nicht gespeichert: ${commandErrorText(reason)}`);
      })
      .finally(() => {
        setIsSaving(false);
      });
  }

  return (
    <div className="self-release-settings">
      {SWITCH_ROWS.map((row: SwitchRow) => (
        <SettingRow key={row.key} label={row.label} info={row.info}>
          <Switch
            checked={value[row.key]}
            label={row.label}
            disabled={isSaving}
            onChange={(next: boolean): void => {
              save({ ...value, [row.key]: next });
            }}
          />
        </SettingRow>
      ))}
      <SettingRow
        label="Modell des TL;DR"
        info="Mit diesem Modell entsteht das TL;DR, das ein Agent über das Session-Werkzeug abruft."
      >
        <SelectButton
          label="Modell des TL;DR"
          value={modelName(value.tldrModel)}
          isOpen={isModelMenuOpen}
          onToggle={(): void => {
            setIsModelMenuOpen(!isModelMenuOpen);
          }}
        >
          <ModelMenu
            options={MODEL_OPTIONS}
            value={value.tldrModel}
            onChange={(model: ModelId): void => {
              setIsModelMenuOpen(false);
              save({ ...value, tldrModel: model });
            }}
            onClose={(): void => {
              setIsModelMenuOpen(false);
            }}
            placement="below"
            title="Modell des TL;DR"
            note={modelNote}
          />
        </SelectButton>
      </SettingRow>
      {errorMessage !== null && (
        <p className="self-release-settings__error" role="alert">
          {errorMessage}
        </p>
      )}
    </div>
  );
}
