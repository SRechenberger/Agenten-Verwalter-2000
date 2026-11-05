import { useState } from 'react';
import type { ReactElement } from 'react';
import { ModelMenu } from '@/components/ModelMenu';
import { Switch } from '@/components/Switch';
import { SelectButton } from '@/features/settings/SelectButton';
import { SettingRow } from '@/features/settings/SettingRow';
import { settingEntry } from '@/features/settings/settingsCatalog';
import type { ModelId } from '@/lib/bindings/ModelId';
import type { SelfRelease } from '@/lib/bindings/SelfRelease';
import { commandErrorText } from '@/lib/errors';
import { MODEL_OPTIONS, modelName } from '@/lib/labels';
import './SelfReleaseSettings.css';

/** Die vier Zeilen der Selbstfreigabe im Katalog, in Anzeigereihenfolge. */
export const SELF_RELEASE_ROW_IDS = [
  'selfReleaseMessages',
  'selfReleaseTldr',
  'selfReleaseOpenSession',
  'selfReleaseTldrModel',
] as const;

export type SelfReleaseRowId = (typeof SELF_RELEASE_ROW_IDS)[number];

/** Welchen Schalter von `SelfRelease` eine Zeile setzt; die Zeile des Modells hat keinen. */
const SWITCH_KEY: Record<
  Exclude<SelfReleaseRowId, 'selfReleaseTldrModel'>,
  'messages' | 'tldr' | 'openSession'
> = {
  selfReleaseMessages: 'messages',
  selfReleaseTldr: 'tldr',
  selfReleaseOpenSession: 'openSession',
};

interface SelfReleaseRowProps {
  id: SelfReleaseRowId;
  value: SelfRelease;
  /** Gesperrt, solange ein Speichern läuft: Jede Zeile schreibt den ganzen Wert. */
  disabled: boolean;
  /** Bekommt den ganzen neuen Wert. */
  onChange: (next: SelfRelease) => void;
  /** Das Menü des Modells gehört dem Aufrufer, damit er es schließen kann, wo es nicht stehen bleiben darf. */
  isModelMenuOpen: boolean;
  onToggleModelMenu: () => void;
  onCloseModelMenu: () => void;
  /** Sagt im Modell-Menü, wann der Wert gilt. */
  modelNote: string;
}

/** Eine Zeile der Selbstfreigabe; Name und ⓘ-Text stehen im Katalog. Ohne eigenen Zustand. */
export function SelfReleaseRow({
  id,
  value,
  disabled,
  onChange,
  isModelMenuOpen,
  onToggleModelMenu,
  onCloseModelMenu,
  modelNote,
}: SelfReleaseRowProps): ReactElement {
  const label: string = settingEntry(id).label;
  if (id === 'selfReleaseTldrModel') {
    return (
      <SettingRow id={id}>
        <SelectButton
          label={label}
          value={modelName(value.tldrModel)}
          isOpen={isModelMenuOpen && !disabled}
          onToggle={disabled ? onCloseModelMenu : onToggleModelMenu}
        >
          <ModelMenu
            options={MODEL_OPTIONS}
            value={value.tldrModel}
            onChange={(model: ModelId): void => {
              onCloseModelMenu();
              onChange({ ...value, tldrModel: model });
            }}
            onClose={onCloseModelMenu}
            placement="below"
            title={label}
            note={modelNote}
          />
        </SelectButton>
      </SettingRow>
    );
  }
  const key = SWITCH_KEY[id];
  return (
    <SettingRow id={id}>
      <Switch
        checked={value[key]}
        label={label}
        disabled={disabled}
        onChange={(next: boolean): void => {
          onChange({ ...value, [key]: next });
        }}
      />
    </SettingRow>
  );
}

interface SelfReleaseSettingsProps {
  value: SelfRelease;
  /** Ein Fehler beim Speichern steht unter den Zeilen. */
  onSave: (next: SelfRelease) => Promise<void>;
  /** Sagt im Modell-Menü, wann der Wert gilt. */
  modelNote: string;
}

/** Selbstfreigabe je Werkzeug und Modell des TL;DR als Block unter der Übersicht eines Vorhabens. */
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
      {SELF_RELEASE_ROW_IDS.map((id: SelfReleaseRowId) => (
        <SelfReleaseRow
          key={id}
          id={id}
          value={value}
          disabled={isSaving}
          onChange={save}
          isModelMenuOpen={isModelMenuOpen}
          onToggleModelMenu={(): void => {
            setIsModelMenuOpen(!isModelMenuOpen);
          }}
          onCloseModelMenu={(): void => {
            setIsModelMenuOpen(false);
          }}
          modelNote={modelNote}
        />
      ))}
      {errorMessage !== null && (
        <p className="self-release-settings__error" role="alert">
          {errorMessage}
        </p>
      )}
    </div>
  );
}
