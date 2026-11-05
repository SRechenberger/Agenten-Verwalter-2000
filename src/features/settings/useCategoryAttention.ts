import { useMemo } from 'react';
import { needsAttention } from '@/features/environment/environmentTexts';
import type { SettingsAttention, SettingsCategoryId } from '@/features/settings/settingsCatalog';
import { useAppUpdateStore } from '@/stores/appUpdate';
import { useEnvironmentStore } from '@/stores/environment';

type CategoryAttention = Partial<Record<SettingsCategoryId, SettingsAttention>>;

/** Welche Kategorien einen Punkt tragen: `app` bei neuer Version, `skills` wenn die Umgebung Aufmerksamkeit braucht. */
export function useCategoryAttention(): CategoryAttention {
  const hasAppUpdate: boolean = useAppUpdateStore((state) => state.status?.hasUpdate === true);
  const isEnvironmentFlagged: boolean = useEnvironmentStore((state) =>
    needsAttention(state.status),
  );
  // Stabiles Objekt, solange sich die beiden Werte nicht ändern: `initialCategory` und `SettingsNav` lesen es.
  return useMemo((): CategoryAttention => {
    const attention: CategoryAttention = {};
    if (hasAppUpdate) {
      attention.app = 'update';
    }
    if (isEnvironmentFlagged) {
      attention.skills = 'environment';
    }
    return attention;
  }, [hasAppUpdate, isEnvironmentFlagged]);
}
