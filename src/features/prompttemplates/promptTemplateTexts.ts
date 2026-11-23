import type { PromptTemplate } from '@/lib/bindings/PromptTemplate';

// Die Texte der Prompt-Vorlagen (ADR 075), wie im Mockup `docs/design/2026-11-23_prompt-vorlagen/`.

// ---- Einstellungen › Agent › Prompt-Vorlagen ----
export const CREATE_LABEL = '＋ Vorlage';
export const LIST_INTRO =
  'Für den Auftrag eines neuen Vorhabens. Jede Variable wird ein Textfeld, jeder Abschnitt eine Checkbox. Gilt für alle Vorhaben, nur auf diesem Rechner.';
export const LOADING_TEXT = 'Lädt …';
export const EMPTY_TEXT =
  'Noch keine Vorlagen. Ohne Vorlage sieht „Neues Vorhaben“ aus wie bisher.';
export const NAME_TEMPLATE_TAG = 'Namensvorlage';
export const EDIT_LABEL = 'Bearbeiten';
export const DELETE_LABEL = 'Löschen';

export function editAriaLabel(promptTemplate: PromptTemplate): string {
  return `Vorlage ${promptTemplate.name} bearbeiten`;
}

export function deleteAriaLabel(promptTemplate: PromptTemplate): string {
  return `Vorlage ${promptTemplate.name} löschen`;
}

// ---- Editor ----
export const DIALOG_CREATE_TITLE = 'Vorlage anlegen';
export const DIALOG_EDIT_TITLE = 'Vorlage bearbeiten';
export const NAME_LABEL = 'Name';
export const NAME_PLACEHOLDER = 'z. B. Bugfix';
export const TEXT_LABEL = 'Vorlagentext';
export const TEXT_PLACEHOLDER = 'Behebe den Fehler: {{Fehler}} in {{Betroffene Komponente}}.';
export const NAME_TEMPLATE_LABEL = 'Namensvorlage';
export const NAME_TEMPLATE_OPTIONAL = '(freiwillig, dieselben Felder)';
export const NAME_TEMPLATE_PLACEHOLDER = 'z. B. Bugfix: {{Betroffene Komponente}}';
export const SYNTAX_HINT =
  'Variable {{Name}} · Abschnitt {{#Name}}…{{/Name}} (Haken an) · {{^Name}}…{{/Name}} (Haken aus) · Kommentar {{! … }} wird Hilfetext der nächsten Variable';
export const PREVIEW_TITLE = 'Formular';
export const PREVIEW_EMPTY = 'Die Vorlage hat noch keine Felder.';
export const PREVIEW_BLOCKED = 'Erst ohne Syntaxfehler zeigt die Vorschau das Formular.';
export const SAVE_BLOCKED = 'Speichern geht erst ohne Syntaxfehler.';
export const CANCEL_LABEL = 'Abbrechen';
export const SAVE_LABEL = 'Speichern';
export const SAVING_LABEL = 'Speichert …';
export const DISCARD_TEXT = 'Die Eingaben sind nicht gespeichert. Verwerfen?';
export const KEEP_EDITING_LABEL = 'Weiter bearbeiten';
export const DISCARD_LABEL = 'Verwerfen';

export function previewNameText(nameTemplate: string): string {
  return `Namensfeld: „${nameTemplate}“`;
}

export function saveFailedText(reason: string): string {
  return `Nicht gespeichert: ${reason}`;
}

export const DELETE_TEXT =
  'Die Vorlage verschwindet aus „Neues Vorhaben“. Vorhaben, die mit ihr gestartet sind, bleiben, wie sie sind.';

export function deleteTitle(promptTemplate: PromptTemplate): string {
  return `Vorlage „${promptTemplate.name}“ löschen?`;
}

export function deleteFailedText(reason: string): string {
  return `Nicht gelöscht: ${reason}`;
}

// ---- Neues Vorhaben ----
export const PICK_TITLE = 'Vorlage wählen';
export const MENU_HEADING = 'Vorlage';
export const MENU_SETTINGS_LINK = 'Vorlagen in den Einstellungen bearbeiten …';
export const BAR_HINT = 'Mit / startest du mit einem Skill, mit ▤ aus einer Vorlage.';
export const FORM_KIND = '· Formular';
export const FORM_OK = 'OK';
export const FORM_DISCARD = 'Verwerfen';
export const LOCK_LINE = 'Der Auftrag entsteht aus dem Formular. Mit „OK“ wird er wieder frei.';
export const START_HINT = 'Erst das Formular mit „OK“ schließen.';
export const NAME_FIELD_LABEL = 'Name des Vorhabens';
export const NAME_CHIP_TEMPLATE = 'aus der Vorlage';
export const NAME_CHIP_MANUAL = 'von Hand';
export const NAME_NOTE =
  'Der Name kommt aus der Vorlage und bleibt fest; die erste Session benennt Haiku wie bisher. Der ganze Text samt Anhängen wird zur ersten Nachricht.';
export const REPLACE_TITLE = 'Text ersetzen?';
export const REPLACE_LABEL = 'Ersetzen';

export function replaceText(templateName: string): string {
  return `Im Auftrag steht schon Text. Die Vorlage „${templateName}“ ersetzt ihn. Mit „Verwerfen“ im Formular holst du ihn zurück.`;
}

/** Die erste Zeile des Vorlagentexts im Menü, ohne Kommentare. */
export function menuPreview(text: string): string {
  return (
    text
      .replace(/\{\{![\s\S]*?\}\}/g, '')
      .trim()
      .split('\n')[0] ?? ''
  );
}
