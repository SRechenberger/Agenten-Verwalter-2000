import { createContext } from 'react';
import type { RefSession } from '@/lib/sessionRefs';

export interface SessionRefs {
  /** Alle Sessions, die die App kennt. */
  sessions: readonly RefSession[];
  /** Das Vorhaben der angezeigten Session; eine Nummer („Session 3“) gilt dort. */
  projectId: string;
  /** Name eines Vorhabens, für den Tooltip bei einer Session eines anderen; `null`, wenn unbekannt. */
  projectName: (projectId: string) => string | null;
}

/** Woraus „Session N“ und Session-IDs im Chattext zu Links werden; `null` = keine Links, der Text bleibt. */
export const SessionRefsContext = createContext<SessionRefs | null>(null);
