import { useId } from 'react';
import type { ReactElement } from 'react';
import type { TemplateField } from '@/features/prompttemplates/mustache';
import './TemplateFields.css';

export type TemplateValues = Record<string, string | boolean>;

interface TemplateFieldsProps {
  fields: readonly TemplateField[];
  values: TemplateValues;
  /** Fehlt er, ist es die Vorschau im Editor: alles gesperrt. */
  onChange?: (name: string, value: string | boolean) => void;
}

/** Die Felder einer Vorlage (E2): Variable → Textfeld, das mitwächst, Abschnitt → Checkbox, der Name ist die
 *  Beschriftung, ein Kommentar der Hilfetext. Enter macht im Textfeld eine neue Zeile und startet nichts. */
export function TemplateFields({ fields, values, onChange }: TemplateFieldsProps): ReactElement {
  const baseId: string = useId();
  const isPreview: boolean = onChange === undefined;
  return (
    <div className="template-fields">
      {fields.map((field: TemplateField, index: number) => {
        const id = `${baseId}-${String(index)}`;
        const value: string | boolean | undefined = values[field.name];
        if (field.kind === 'flag') {
          return (
            <label key={field.name} className="template-fields__check" htmlFor={id}>
              <input
                id={id}
                type="checkbox"
                checked={value === true}
                disabled={isPreview}
                onChange={(event): void => {
                  onChange?.(field.name, event.target.checked);
                }}
              />
              {field.name}
            </label>
          );
        }
        return (
          <div key={field.name} className="template-fields__field">
            <label className="template-fields__label" htmlFor={id}>
              {field.name}
            </label>
            {field.help !== null && <span className="template-fields__help">{field.help}</span>}
            <textarea
              id={id}
              className="template-fields__input"
              rows={1}
              disabled={isPreview}
              value={typeof value === 'string' ? value : ''}
              onChange={(event): void => {
                onChange?.(field.name, event.target.value);
              }}
            />
          </div>
        );
      })}
    </div>
  );
}
