import { COMPOSER_INPUT_ID } from '@/features/background/mention';
import { useChatStore } from '@/stores/chat';

/** Leerer Entwurf → die Zeile; enthält er sie schon → unverändert; sonst Zeile, Leerzeile, alter Entwurf. */
export function setHandoffDraft(sessionId: string, line: string): void {
  const current: string = useChatStore.getState().drafts[sessionId] ?? '';
  if (current.includes(line)) {
    return;
  }
  const draft: string = current === '' ? line : `${line}\n\n${current}`;
  useChatStore.getState().setDraft(sessionId, draft);
}

/** Jede Zeile des Textes mit `> ` eingerückt, dahinter eine Leerzeile und der bisherige Entwurf. */
export function setQuoteDraft(sessionId: string, text: string): void {
  const current: string = useChatStore.getState().drafts[sessionId] ?? '';
  const quote: string = text
    .split('\n')
    .map((line: string) => `> ${line}`)
    .join('\n');
  useChatStore.getState().setDraft(sessionId, `${quote}\n\n${current}`);
}

export function focusComposer(): void {
  // Die neue Session wird gerade erst ausgewählt — das Textfeld gibt es erst im nächsten Frame.
  window.requestAnimationFrame(() => {
    document.getElementById(COMPOSER_INPUT_ID)?.focus();
  });
}
