import type { AgentType } from '@/lib/bindings/AgentType';
import type { OperatingMode } from '@/lib/bindings/OperatingMode';
import { effortLabel, modelName, operatingModeLabel } from '@/lib/labels';

// Texte der Oberfläche für Agententypen (Entwurf: docs/design/2026-11-07_agententypen/). Die Meldungen
// der Prüfung kommen vom Core und stehen nicht hier.

// ---- Liste in den Einstellungen ----

export const CREATE_LABEL = 'Agententyp anlegen';
export const LIST_INTRO =
  'Ein Typ ist eine Rolle für Stationen, etwa developer, tester oder reviewer. Die Beschreibung ist für den Planer, der Systemprompt für den Agenten selbst.';
export const LOADING_TEXT = 'Lädt …';
export const EMPTY_LEAD = 'Noch keine Agententypen.';
export const EMPTY_TEXT =
  'Ohne Typen läuft jede Station wie bisher: Modell und Denkaufwand kommen von der Session mit der höchsten Nummer.';
export const EDIT_LABEL = 'Bearbeiten';
export const DELETE_LABEL = 'Löschen';
export const PROMPT_CHIP = 'Systemprompt';
export const DEFAULT_PAIR_CHIP = 'Modell und Denkaufwand: Standard';

/** Der Hinweis über der Liste; nur die Betriebsart Claude wendet Modell und Denkaufwand der Typen an. */
export function operatingModeNote(mode: OperatingMode): string | null {
  if (mode === 'claude') {
    return null;
  }
  return `In der Betriebsart ${operatingModeLabel(mode)} gelten Modell und Denkaufwand der Typen nicht. Systemprompt, Dateien und Skills wirken auch hier.`;
}

export function editAriaLabel(agentType: AgentType): string {
  return `Agententyp ${agentType.name} bearbeiten`;
}

export function deleteAriaLabel(agentType: AgentType): string {
  return `Agententyp ${agentType.name} löschen`;
}

export function stageChipTitle(agentType: AgentType): string {
  return `Eine Station mit diesem Namen läuft als ${agentType.name}`;
}

/** „Opus 5.5 · Hoch“, oder „Standard“, wenn der Typ Modell und Denkaufwand offenlässt. */
export function pairChipText(agentType: AgentType): string {
  if (agentType.model === null || agentType.effort === null) {
    return DEFAULT_PAIR_CHIP;
  }
  return `${modelName(agentType.model)} · ${effortLabel(agentType.effort)}`;
}

export function fileCountText(count: number): string {
  return count === 1 ? '1 Datei' : `${String(count)} Dateien`;
}

export function skillCountText(count: number): string {
  return count === 1 ? '1 Skill' : `${String(count)} Skills`;
}

// ---- Dialog „Agententyp anlegen / bearbeiten“ ----

export const DIALOG_CREATE_TITLE = 'Neuer Agententyp';
export const DIALOG_EDIT_TITLE = 'Agententyp bearbeiten';
export const DIALOG_SUBTITLE =
  'Eine Rolle für Stationen. Gespeichert in den Einstellungen, gilt für alle Vorhaben.';
export const REQUIRED_LABEL = 'Pflicht';

export const NAME_LABEL = 'Name';
export const NAME_PLACEHOLDER = 'z. B. developer';
export const NAME_HINT = 'Unter diesem Namen nennt eine Station den Typ im Feld agent.';

export const DESCRIPTION_LABEL = 'Beschreibung für den Planer';
export const DESCRIPTION_PLACEHOLDER = 'Wofür taugt der Typ, wann lohnt er sich, was tut er nicht?';

export const STAGES_LABEL = 'Stationen';
export const STAGES_PLACEHOLDER = 'Stationsname, Enter';
export const STAGES_ADD_LABEL = 'Stationsname hinzufügen';
export const STAGES_HINT =
  'Eine Station mit diesem Namen läuft von selbst als dieser Typ. Groß- und Kleinschreibung und Leerzeichen am Rand zählen nicht; ein Name gehört höchstens einem Typ.';

export const PAIR_LABEL = 'Modell und Denkaufwand';
export const PAIR_DEFAULT = 'Standard';
export const PAIR_SET = 'Festlegen';
export const PAIR_HINT =
  'Beides zusammen oder „Standard“: dann gilt die allgemeine Regel. Gilt nur in der Betriebsart Claude.';
export const MODEL_MENU_TITLE = 'Modell für Stationen dieses Typs';
export const MODEL_MENU_NOTE = 'Gilt in der Betriebsart Claude. Ein Modell der Station gewinnt.';

export const PROMPT_LABEL = 'Systemprompt';
export const PROMPT_PLACEHOLDER = 'Anweisungen an den Agenten selbst';
export const PROMPT_HINT =
  'Steht hinter den Vorgaben des Verwalters, in allen vier Betriebsarten. Danach folgen die Dateien.';

export const FILES_LABEL = 'Dateien';
export const FILES_PLACEHOLDER = 'Pfad: absolut, ~/ oder relativ zu den Repositories';
export const FILES_ADD_LABEL = 'Pfad hinzufügen';
export const FILES_ADD_BUTTON = 'Hinzufügen';
export const FILES_PICK_BUTTON = 'Datei wählen …';
export const FILES_PICK_TITLE = 'Datei für den Typ wählen';
export const FILES_HINT =
  'Der Verwalter liest die Dateien beim Start der Station, ohne Frontmatter. Fehlt eine, startet die Station nicht.';

export const SKILLS_LABEL = 'Skills';
export const SKILLS_PLACEHOLDER = 'Skill, Enter';
export const SKILLS_ADD_LABEL = 'Skill hinzufügen';
export const SKILLS_SUGGESTIONS_LABEL = 'Vorschläge';
export const SKILLS_NO_SUGGESTION = 'Kein Vorschlag. Freier Text mit Enter.';
export const SKILLS_HINT =
  'Der erste Skill steht als Befehl in der ersten Zeile, die weiteren nennt die erste Nachricht. Ein Skill der Station kommt vor denen des Typs. Ob ein Skill existiert, prüft der Verwalter nicht.';

export const CANCEL_LABEL = 'Abbrechen';
export const SAVE_LABEL = 'Speichern';
export const SAVING_LABEL = 'Speichert …';

export const DISCARD_TEXT = 'Die Eingaben sind nicht gespeichert. Verwerfen?';
export const KEEP_EDITING_LABEL = 'Weiter bearbeiten';
export const DISCARD_LABEL = 'Verwerfen';

export function removeStageLabel(stage: string): string {
  return `Station ${stage} entfernen`;
}

export function removeFileLabel(path: string): string {
  return `Datei ${path} entfernen`;
}

export function removeSkillLabel(skill: string): string {
  return `Skill ${skill} entfernen`;
}

/** Die Zeile im Fuß, solange der Core Meldungen geliefert hat; `fieldCount` zählt Felder, nicht Meldungen. */
export function saveBlockedText(fieldCount: number): string {
  const rest: string =
    fieldCount === 1 ? 'Angabe fehlt oder ist ungültig.' : 'Angaben fehlen oder sind ungültig.';
  return `Speichern nicht möglich: ${String(fieldCount)} ${rest}`;
}

export function saveFailedText(reason: string): string {
  return `Nicht gespeichert: ${reason}`;
}

export function pickFailedText(reason: string): string {
  return `Dateiauswahl nicht geöffnet: ${reason}`;
}

// ---- Dialog „Typ löschen“ ----

export const DELETE_TEXT =
  'Stationen, die diesen Typ im Feld agent nennen, starten danach nicht mehr; die Karte nennt den Grund. Eine laufende Station behält ihren Systemprompt. Stationen, die nur über den Namen zugeordnet waren, laufen ohne Typ.';

export function deleteTitle(agentType: AgentType): string {
  return `„${agentType.name}“ löschen?`;
}

export function deleteFailedText(reason: string): string {
  return `Nicht gelöscht: ${reason}`;
}
