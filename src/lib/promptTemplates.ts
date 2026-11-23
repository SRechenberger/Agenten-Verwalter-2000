import { invoke } from '@tauri-apps/api/core';
import { listen, type Event, type UnlistenFn } from '@tauri-apps/api/event';
import type { PromptTemplate } from '@/lib/bindings/PromptTemplate';
import type { PromptTemplateDecision } from '@/lib/bindings/PromptTemplateDecision';
import type { PromptTemplateInput } from '@/lib/bindings/PromptTemplateInput';
import type { PromptTemplateSaved } from '@/lib/bindings/PromptTemplateSaved';

const PROMPT_TEMPLATES_CHANGED_EVENT = 'prompt-templates://changed';

// Die Zahlen des Core (`prompt_templates::validate`) für die Zähler im Editor; geprüft wird beim Speichern im
// Core, die Syntax zusätzlich live im Editor (`mustache.ts`).
export const MAX_NAME_CHARS = 40;
export const MAX_TEXT_UNITS = 16_000;
export const MAX_NAME_TEMPLATE_CHARS = 200;

/** Alle Vorlagen in der Reihenfolge, in der sie angelegt wurden.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `database` */
export function listPromptTemplates(): Promise<PromptTemplate[]> {
  return invoke<PromptTemplate[]>('prompt_templates_list');
}

/** Legt eine Vorlage an. Meldet die Prüfung etwas, ist nichts gespeichert und `promptTemplate` ist `null`.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `database` */
export function createPromptTemplate(input: PromptTemplateInput): Promise<PromptTemplateSaved> {
  return invoke<PromptTemplateSaved>('prompt_template_create', { input });
}

/** Ändert die Vorlage; ein anderer Name benennt sie um. Meldet die Prüfung etwas, ist nichts gespeichert.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `internal` (unbekannte Vorlage), `database` */
export function updatePromptTemplate(
  id: string,
  input: PromptTemplateInput,
): Promise<PromptTemplateSaved> {
  return invoke<PromptTemplateSaved>('prompt_template_update', { id, input });
}

/** Löscht die Vorlage.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `internal` (unbekannte Vorlage), `database` */
export async function deletePromptTemplate(id: string): Promise<void> {
  await invoke('prompt_template_delete', { id });
}

/** Beantwortet den Vorschlag eines Agenten im Kasten (ADR 075, „Per MCP“). Übernehmen speichert wie der Editor;
 *  meldet die Prüfung etwas (auch „veraltet“), bleibt der Kasten offen und `promptTemplate` ist `null`.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `internal` (Kasten nicht offen oder
 *  kein Vorschlag), `database` */
export function answerPromptTemplateProposal(
  sessionId: string,
  requestId: string,
  decision: PromptTemplateDecision,
): Promise<PromptTemplateSaved> {
  return invoke<PromptTemplateSaved>('prompt_template_proposal_answer', {
    sessionId,
    requestId,
    decision,
  });
}

/** Meldet nach jeder Änderung die ganze Liste. */
export function onPromptTemplatesChanged(
  callback: (promptTemplates: PromptTemplate[]) => void,
): Promise<UnlistenFn> {
  return listen<PromptTemplate[]>(
    PROMPT_TEMPLATES_CHANGED_EVENT,
    (event: Event<PromptTemplate[]>) => {
      callback(event.payload);
    },
  );
}
