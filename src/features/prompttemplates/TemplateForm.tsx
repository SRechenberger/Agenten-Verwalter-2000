import { useEffect, useRef } from 'react';
import type { ReactElement } from 'react';
import type { TemplateField } from '@/features/prompttemplates/mustache';
import { FORM_DISCARD, FORM_KIND, FORM_OK } from '@/features/prompttemplates/promptTemplateTexts';
import { TemplateFields } from '@/features/prompttemplates/TemplateFields';
import type { TemplateValues } from '@/features/prompttemplates/TemplateFields';
import './TemplateForm.css';

interface TemplateFormProps {
  templateName: string;
  fields: readonly TemplateField[];
  values: TemplateValues;
  onChange: (name: string, value: string | boolean) => void;
  onConfirm: () => void;
  onDiscard: () => void;
}

/** Das Formular einer Vorlage über dem Auftrag (Bild 2A, E4): Der Kopf nennt die Vorlage, „OK“ gibt den
 *  Auftrag frei, „Verwerfen“ holt den Text von vor der Wahl zurück. Beim Öffnen steht der Fokus im ersten Feld. */
export function TemplateForm({
  templateName,
  fields,
  values,
  onChange,
  onConfirm,
  onDiscard,
}: TemplateFormProps): ReactElement {
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    rootRef.current?.querySelector<HTMLElement>('textarea, input')?.focus();
  }, []);

  return (
    <div
      ref={rootRef}
      className="template-form"
      role="group"
      aria-label={`${templateName} ${FORM_KIND}`}
    >
      <div className="template-form__head">
        <b className="template-form__name">{templateName}</b>
        <span className="template-form__kind">{FORM_KIND}</span>
        <span className="template-form__actions">
          <button type="button" className="template-form__button" onClick={onDiscard}>
            {FORM_DISCARD}
          </button>
          <button
            type="button"
            className="template-form__button template-form__button--primary"
            onClick={onConfirm}
          >
            {FORM_OK}
          </button>
        </span>
      </div>
      <TemplateFields fields={fields} values={values} onChange={onChange} />
    </div>
  );
}
