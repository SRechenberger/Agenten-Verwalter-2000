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
export const TEXT_OVER_REASON = 'Der Text ist länger als 16 000 Zeichen.';
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

// ---- Vorschlag eines Agenten im Chat (ADR 075, „Per MCP“) ----
export const PROPOSAL_ARIA_LABEL = 'Vorschlag einer Prompt-Vorlage';
export const PROPOSAL_NEW_TITLE = 'Claude schlägt eine neue Prompt-Vorlage vor';
export const PROPOSAL_WAITING = 'wartet auf dich';
export const PROPOSAL_FORM_LABEL = 'Formular';
export const PROPOSAL_NO_NAME_TEMPLATE = 'keine';
export const PROPOSAL_VIEW_LABEL = 'Ansicht';
export const PROPOSAL_VIEW_DIFF = 'Diff';
export const PROPOSAL_VIEW_BEFORE = 'Vorher';
export const PROPOSAL_VIEW_AFTER = 'Nachher';
export const PROPOSAL_BEFORE_NOTE = 'So ist die Vorlage heute gespeichert.';
export const PROPOSAL_AFTER_NOTE = 'So wäre die Vorlage nach „Übernehmen“.';
export const PROPOSAL_SHOW_ALL = 'Ganz zeigen';
export const PROPOSAL_COLLAPSE = 'Einklappen';
export const PROPOSAL_ACCEPT = 'Übernehmen';
export const PROPOSAL_ACCEPTING = 'Übernimmt …';
export const PROPOSAL_EDIT = 'Bearbeiten …';
export const PROPOSAL_REJECT = 'Verwerfen';
export const PROPOSAL_BACK = 'Zurück';
export const PROPOSAL_REASON_LABEL = 'Begründung für Claude (optional)';
/** Freier Text im Eingabefeld verwirft einen offenen Vorschlag; der Kasten sagt es. */
export const PROPOSAL_FREE_TEXT_HINT =
  'Text im Eingabefeld unten verwirft den Vorschlag mit dieser Begründung.';
export const PROPOSAL_CANCELLED_NOTE = 'Der Agent wurde gestoppt; nichts gespeichert.';
export const PROPOSAL_SHOW = 'Vorschlag zeigen';
export const PROPOSAL_HIDE = 'Vorschlag ausblenden';
export const PROPOSAL_OPEN_SETTINGS = 'In den Einstellungen öffnen';
export const PROPOSAL_DIALOG_TITLE = 'Vorschlag bearbeiten';

export function proposalUpdateTitle(name: string): string {
  return `Claude schlägt eine Änderung an der Vorlage „${name}“ vor`;
}

export function proposalAnsweredTitle(name: string): string {
  return `Prompt-Vorlage „${name}“`;
}

/** „Unverändert: Name.“ */
export function proposalUnchangedText(labels: readonly string[]): string {
  return `Unverändert: ${labels.join(', ')}.`;
}

/** Wörtlich wie `db::prompt_templates::update_if_unchanged`. */
export function proposalStaleText(name: string): string {
  return `Die Vorlage „${name}“ wurde seit dem Vorschlag geändert oder gelöscht. Verwirf den Vorschlag.`;
}

/** Der Zähler hinter „Ganz zeigen“, gezählt wie im Editor. */
export function proposalTextCount(count: string, max: string): string {
  return `· ${count} / ${max}`;
}

export function proposalDialogSubtitle(sessionNumber: number): string {
  return `Vorbefüllt mit dem Vorschlag von Session ${String(sessionNumber)}. „Übernehmen“ speichert diesen Stand und meldet ihn an Claude.`;
}

export function proposalFailedText(reason: string): string {
  return `Nicht beantwortet: ${reason}`;
}

// ---- Neues Vorhaben ----
export const PICK_TITLE = 'Vorlage wählen';
export const MENU_HEADING = 'Vorlage';
export const MENU_SETTINGS_LINK = 'Vorlagen in den Einstellungen bearbeiten …';
export const BAR_HINT = 'Mit / startest du mit einem Skill, mit ▤ aus einer Vorlage.';

// Während des Diktats sind `/` und ▤ gesperrt (Taiga #219): Der Tooltip nennt den Grund, die Zeile in der Leiste
// ersetzt den Hinweis; ▤ steht nur in „Neues Vorhaben“ und nur mit Vorlagen.
const LOCKED_BY_DICTATION = 'während des Diktats gesperrt';
export const COMMAND_TITLE_DICTATING = `Befehle und Skills – ${LOCKED_BY_DICTATION}`;
export const PICK_TITLE_DICTATING = `${PICK_TITLE} – ${LOCKED_BY_DICTATION}`;
export const DICTATION_HINT_WITH_TEMPLATES = 'Diktat läuft – / und ▤ sind gesperrt';
export const DICTATION_HINT = 'Diktat läuft – / ist gesperrt';
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

/** Der Vorlagentext in einer Zeile für Menü und Liste: ohne Kommentare, Zeilenumbrüche als Leerzeichen. */
export function textPreview(text: string): string {
  return text
    .replace(/\{\{![\s\S]*?\}\}/g, '')
    .replace(/\s+/g, ' ')
    .trim();
}
