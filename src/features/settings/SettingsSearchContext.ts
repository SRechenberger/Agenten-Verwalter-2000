import { createContext, useContext } from 'react';

/** Die Suchwörter der Einstellungsseite; leer = keine Suche. */
export const SettingsSearchContext = createContext<readonly string[]>([]);

export function useSettingsSearchTerms(): readonly string[] {
  return useContext(SettingsSearchContext);
}
