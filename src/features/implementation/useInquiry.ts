import { useCallback } from 'react';
import { focusComposer } from '@/features/chat/composerDraft';
import type { InquiryCreated } from '@/lib/bindings/InquiryCreated';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import { createInquiry } from '@/lib/implementation';
import { useAttachmentsStore } from '@/stores/attachments';

/** „Neue Rückfrage“ (ADR 048): legt die Session an, hängt den Lagebericht in ihren Entwurf, öffnet sie und
 *  setzt den Fokus ins Eingabefeld. Gesendet wird erst, wenn der User auf Senden klickt. */
export function useInquiry(
  onSessionCreated: (created: SessionSummary) => void,
): (implementationId: string) => Promise<void> {
  return useCallback(
    async (implementationId: string): Promise<void> => {
      const created: InquiryCreated = await createInquiry(implementationId);
      useAttachmentsStore.getState().add(created.session.id, [created.attachment]);
      onSessionCreated(created.session);
      focusComposer();
    },
    [onSessionCreated],
  );
}
