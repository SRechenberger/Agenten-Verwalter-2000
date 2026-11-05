import { create } from 'zustand';
import type { SettingsCategoryId } from '@/features/settings/settingsCatalog';

interface SettingsNavState {
  /** `null`, solange in diesem App-Lauf noch keine Kategorie angeklickt wurde. */
  category: SettingsCategoryId | null;
  setCategory: (category: SettingsCategoryId) => void;
}

/** Die zuletzt in der Leiste der Einstellungen gewählte Kategorie; bewusst nicht in der Datenbank. */
export const useSettingsNavStore = create<SettingsNavState>((set) => ({
  category: null,
  setCategory: (category: SettingsCategoryId): void => {
    set({ category });
  },
}));
