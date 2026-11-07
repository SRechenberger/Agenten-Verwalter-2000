import { revealItemInDir } from '@tauri-apps/plugin-opener';
import { useMemo, useRef, useState } from 'react';
import type { ReactElement } from 'react';
import { ModeMenu } from '@/components/ModeMenu';
import { ModelMenu } from '@/components/ModelMenu';
import { Switch } from '@/components/Switch';
import { AccountRow } from '@/features/account/AccountRow';
import { runningSessionCount } from '@/features/appupdate/appUpdateTexts';
import { AppVersionRow } from '@/features/appupdate/AppVersionRow';
import { CodexAccountRow } from '@/features/codexaccount/CodexAccountRow';
import { CliVersionRow } from '@/features/cliupdate/CliVersionRow';
import {
  codexEfforts,
  codexModelLabel,
  codexModelOptions,
  selectedCodexModel,
} from '@/features/codex/codexModels';
import { useCodexModels } from '@/features/codex/useCodexModels';
import { EnvironmentLinksRow } from '@/features/environment/EnvironmentLinksRow';
import { EnvironmentMirrorRow } from '@/features/environment/EnvironmentMirrorRow';
import { MasterDirRow } from '@/features/environment/MasterDirRow';
import { useEnvironmentCheck } from '@/features/environment/useEnvironment';
import { RemoteConnectRow } from '@/features/remote/RemoteConnectRow';
import { RemoteDevicesRow } from '@/features/remote/RemoteDevicesRow';
import { RemoteEnabledRow } from '@/features/remote/RemoteEnabledRow';
import { RemoteHostsRow } from '@/features/remote/RemoteHostsRow';
import { RemotePortRow } from '@/features/remote/RemotePortRow';
import { RemoteStatusRow } from '@/features/remote/RemoteStatusRow';
import { useKnownRepositories } from '@/features/repositories/useKnownRepositories';
import { ColorSchemeSegment } from '@/features/settings/ColorSchemeSegment';
import { TerminalPlacementSegment } from '@/features/settings/TerminalPlacementSegment';
import { LocalModelMenu } from '@/features/settings/LocalModelMenu';
import { OperatingModeSegment } from '@/features/settings/OperatingModeSegment';
import { SelectButton } from '@/features/settings/SelectButton';
import { SelfReleaseRow } from '@/features/settings/SelfReleaseSettings';
import type { SelfReleaseRowId } from '@/features/settings/SelfReleaseSettings';
import { SettingRow } from '@/features/settings/SettingRow';
import { SettingsNav } from '@/features/settings/SettingsNav';
import { SettingsSearchContext } from '@/features/settings/SettingsSearchContext';
import { SettingsSearchField } from '@/features/settings/SettingsSearchField';
import {
  categoryLabel,
  groupEntries,
  initialCategory,
  shownEntries,
} from '@/features/settings/settingsCatalog';
import { countByCategory, searchEntries, searchTerms } from '@/features/settings/settingsSearch';
import type {
  SettingEntry,
  SettingGroup,
  SettingId,
  SettingsCategoryId,
  ShownFacts,
} from '@/features/settings/settingsCatalog';
import { useCategoryAttention } from '@/features/settings/useCategoryAttention';
import { useLocalModels } from '@/features/settings/useLocalModels';
import { useSettingsUpdate } from '@/features/settings/useSettingsUpdate';
import type { CodexModel } from '@/lib/bindings/CodexModel';
import type { Effort } from '@/lib/bindings/Effort';
import type { KnownRepository } from '@/lib/bindings/KnownRepository';
import type { Mode } from '@/lib/bindings/Mode';
import type { ModelId } from '@/lib/bindings/ModelId';
import type { OperatingMode } from '@/lib/bindings/OperatingMode';
import type { SelfRelease } from '@/lib/bindings/SelfRelease';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import type { Settings } from '@/lib/bindings/Settings';
import type { SettingsOverview } from '@/lib/bindings/SettingsOverview';
import { commandErrorText } from '@/lib/errors';
import {
  effortLabel,
  isLocalOperatingMode,
  localModelLabel,
  MODEL_OPTIONS,
  modeOption,
  modelName,
} from '@/lib/labels';
import { useRemoteStore } from '@/stores/remote';
import { useSettingsStore } from '@/stores/settings';
import { useSettingsNavStore } from '@/stores/settingsNav';
import './SettingsView.css';

const NO_CODEX_MODELS: readonly CodexModel[] = [];

type OpenMenu = 'model' | 'mode' | 'localModel' | 'selfReleaseModel' | null;

interface SettingsViewProps {
  /** Alle Sessions; ein Update des Verwalters unterbricht die laufenden. */
  sessions: readonly SessionSummary[];
  overview: SettingsOverview | null;
  loadError: string | null;
  onReload: () => void;
}

export function SettingsView({
  sessions,
  overview,
  loadError,
  onReload,
}: SettingsViewProps): ReactElement {
  const [openMenu, setOpenMenu] = useState<OpenMenu>(null);
  const [revealError, setRevealError] = useState<string | null>(null);
  const [isSavingSelfRelease, setIsSavingSelfRelease] = useState<boolean>(false);
  const storedSettings: Settings | null = useSettingsStore((state) => state.settings);
  const storedCategory: SettingsCategoryId | null = useSettingsNavStore((state) => state.category);
  const setStoredCategory = useSettingsNavStore((state) => state.setCategory);
  // Die Seite wird bei jedem Öffnen der Einstellungen eingehängt: das ist das Prüfen beim Öffnen.
  useEnvironmentCheck();
  const attention = useCategoryAttention();
  const [category, setCategory] = useState<SettingsCategoryId>(() =>
    initialCategory(attention, storedCategory),
  );
  const [query, setQuery] = useState<string>('');
  const contentRef = useRef<HTMLDivElement>(null);
  const { repositories, isLoading: areRepositoriesLoading } = useKnownRepositories();
  const { save, error: saveError } = useSettingsUpdate();
  const remoteError: string | null = useRemoteStore((state) => state.syncError);
  const remoteServer = useRemoteStore((state) => state.server);
  const hasRemoteHosts: boolean = useRemoteStore((state) => state.hosts.length > 0);
  const facts: ShownFacts = {
    remoteEnabled: remoteServer?.enabled === true,
    remoteListening: remoteServer?.listening === true,
    hasRemoteHosts,
  };

  const settings: Settings | null = storedSettings ?? overview?.settings ?? null;
  const isLocalOperation: boolean =
    settings !== null && isLocalOperatingMode(settings.operatingMode);
  const localModels = useLocalModels(isLocalOperation);
  const codexModels = useCodexModels(settings?.operatingMode === 'codex');
  const terms: string[] = useMemo(() => searchTerms(query), [query]);
  const isSearching: boolean = terms.length > 0;
  const hits: SettingEntry[] = searchEntries(
    settings === null ? [] : shownEntries(settings, facts),
    terms,
  );
  const hitIds: ReadonlySet<SettingId> = new Set(hits.map((entry: SettingEntry) => entry.id));
  // Vor dem Laden gibt es keine Zeilen, die ein Ergebnis tragen könnten.
  const isShowingHits: boolean = isSearching && settings !== null && overview !== null;

  function toggleMenu(menu: Exclude<OpenMenu, null>): void {
    setOpenMenu(openMenu === menu ? null : menu);
  }

  function closeMenu(): void {
    setOpenMenu(null);
  }

  function changeQuery(value: string): void {
    closeMenu();
    setQuery(value);
    contentRef.current?.scrollTo({ top: 0 });
  }

  function selectCategory(next: SettingsCategoryId): void {
    // Die Menüs zeichnen per Portal in den body und blieben sonst über einer ausgeblendeten Zeile stehen.
    closeMenu();
    setQuery('');
    setCategory(next);
    setStoredCategory(next);
    contentRef.current?.scrollTo({ top: 0 });
  }

  function reveal(path: string): void {
    setRevealError(null);
    revealItemInDir(path).catch((reason: unknown) => {
      console.error('Explorer nicht geöffnet', reason);
      setRevealError(`Explorer nicht geöffnet: ${commandErrorText(reason)}`);
    });
  }

  function saveOperatingMode(operatingMode: OperatingMode): void {
    closeMenu();
    save({ kind: 'operatingMode', value: operatingMode }).catch(() => undefined);
  }

  function saveLocalModel(id: string): void {
    closeMenu();
    save({ kind: 'localModel', value: id }).catch(() => undefined);
  }

  function saveDefaultModel(model: ModelId): void {
    save({ kind: 'defaultModel', value: model }).catch(() => undefined);
  }

  function saveDefaultCodexModel(model: string): void {
    save({ kind: 'defaultCodexModel', value: model }).catch(() => undefined);
  }

  function saveDefaultMode(mode: Mode, effort: Effort): void {
    save({ kind: 'defaultMode', mode, effort }).catch(() => undefined);
  }

  function saveDefaultSelfRelease(next: SelfRelease): void {
    // Jede Zeile schreibt den ganzen Wert: Bis das Speichern durch ist, sind alle vier gesperrt.
    setIsSavingSelfRelease(true);
    save({ kind: 'defaultSelfRelease', value: next })
      .catch(() => undefined)
      .finally(() => {
        setIsSavingSelfRelease(false);
      });
  }

  function renderSelfReleaseRow(id: SelfReleaseRowId, current: Settings): ReactElement {
    return (
      <SelfReleaseRow
        id={id}
        value={current.defaultSelfRelease}
        disabled={isSavingSelfRelease}
        onChange={saveDefaultSelfRelease}
        isModelMenuOpen={openMenu === 'selfReleaseModel'}
        onToggleModelMenu={(): void => {
          toggleMenu('selfReleaseModel');
        }}
        onCloseModelMenu={closeMenu}
        modelNote="Gilt für neue Vorhaben."
      />
    );
  }

  function projectSkillsText(): string {
    if (areRepositoriesLoading) {
      return '…';
    }
    const withSkills: KnownRepository[] = repositories.filter(
      (repository: KnownRepository) => !repository.isMissing && repository.skillCount > 0,
    );
    if (withSkills.length === 0) {
      return 'Keine Repository-Skills';
    }
    return withSkills
      .map((repository: KnownRepository) => `${repository.name} ${String(repository.skillCount)}`)
      .join(' · ');
  }

  function renderRevealButton(path: string): ReactElement {
    return (
      <button
        type="button"
        className="settings-view__button"
        onClick={(): void => {
          reveal(path);
        }}
      >
        Im Explorer zeigen
      </button>
    );
  }

  function renderErrors(): ReactElement | null {
    const sentences: string[] = [];
    if (loadError !== null) {
      sentences.push(`Einstellungen nicht ladbar: ${loadError}`);
    }
    if (saveError !== null) {
      sentences.push(`Nicht gespeichert: ${saveError}`);
    }
    if (remoteError !== null) {
      sentences.push(remoteError);
    }
    if (revealError !== null) {
      sentences.push(revealError);
    }
    if (sentences.length === 0) {
      return null;
    }
    return (
      <div className="settings-view__errors" role="alert">
        {sentences.map((sentence: string) => (
          <p key={sentence} className="settings-view__error">
            {sentence}
          </p>
        ))}
        {loadError !== null && (
          <button type="button" className="settings-view__button" onClick={onReload}>
            Erneut laden
          </button>
        )}
      </div>
    );
  }

  /** Das Bedienelement einer Zeile; Name und ⓘ-Text stehen im Katalog. */
  function renderRow(id: SettingId, current: Settings, loaded: SettingsOverview): ReactElement {
    const currentMode = modeOption(current.defaultMode);
    const isCodex: boolean = current.operatingMode === 'codex';
    const codexList: readonly CodexModel[] = codexModels.models?.models ?? NO_CODEX_MODELS;
    const defaultCodexModel: CodexModel | null = selectedCodexModel(
      codexList,
      current.defaultCodexModel,
    );
    const codexEmptyText: string =
      codexModels.models === null
        ? 'Modelle werden geladen …'
        : (codexModels.models.error ?? 'Keine Modelle verfügbar.');
    switch (id) {
      case 'claudeAccount':
        return <AccountRow />;
      case 'codexAccount':
        return <CodexAccountRow />;
      case 'colorScheme':
        return (
          <SettingRow id={id}>
            <ColorSchemeSegment
              value={current.colorScheme}
              onChange={(scheme): void => {
                save({ kind: 'colorScheme', value: scheme }).catch(() => undefined);
              }}
            />
          </SettingRow>
        );
      case 'terminalPlacement':
        return (
          <SettingRow id={id}>
            <TerminalPlacementSegment
              value={current.terminalPlacement}
              onChange={(placement): void => {
                save({ kind: 'terminalPlacement', value: placement }).catch(() => undefined);
              }}
            />
          </SettingRow>
        );
      case 'operatingMode':
        return (
          <SettingRow id={id}>
            <OperatingModeSegment value={current.operatingMode} onChange={saveOperatingMode} />
          </SettingRow>
        );
      case 'localModel':
        return (
          <SettingRow id={id}>
            <SelectButton
              label="Lokales Modell"
              value={localModelLabel(current.localModel)}
              isOpen={openMenu === 'localModel'}
              onToggle={(): void => {
                toggleMenu('localModel');
              }}
            >
              <LocalModelMenu
                value={current.localModel}
                models={localModels.models}
                isLoading={localModels.isLoading}
                onChange={saveLocalModel}
                onReload={localModels.reload}
                onClose={closeMenu}
              />
            </SelectButton>
          </SettingRow>
        );
      case 'defaultModel':
        return (
          <SettingRow id={id}>
            <SelectButton
              label="Standardmodell"
              value={
                isCodex
                  ? codexModelLabel(codexList, current.defaultCodexModel)
                  : modelName(current.defaultModel)
              }
              isOpen={openMenu === 'model'}
              onToggle={(): void => {
                toggleMenu('model');
              }}
            >
              {isCodex ? (
                <ModelMenu
                  options={codexModelOptions(codexList)}
                  value={defaultCodexModel?.id ?? ''}
                  onChange={saveDefaultCodexModel}
                  onClose={closeMenu}
                  placement="below"
                  title="Modell für neue Vorhaben"
                  note="Gilt ab dem nächsten neuen Vorhaben."
                  emptyText={codexEmptyText}
                />
              ) : (
                <ModelMenu
                  options={MODEL_OPTIONS}
                  value={current.defaultModel}
                  onChange={saveDefaultModel}
                  onClose={closeMenu}
                  placement="below"
                  title="Modell für neue Vorhaben"
                  note="Gilt ab dem nächsten neuen Vorhaben."
                />
              )}
            </SelectButton>
          </SettingRow>
        );
      case 'defaultMode':
        return (
          <SettingRow id={id}>
            <SelectButton
              label="Standard-Modus"
              value={`${currentMode.label} · Denkaufwand ${effortLabel(current.defaultEffort)}`}
              isOpen={openMenu === 'mode'}
              onToggle={(): void => {
                toggleMenu('mode');
              }}
            >
              <ModeMenu
                mode={current.defaultMode}
                effort={current.defaultEffort}
                onModeChange={(mode: Mode): void => {
                  saveDefaultMode(mode, current.defaultEffort);
                }}
                onEffortChange={(effort: Effort): void => {
                  saveDefaultMode(current.defaultMode, effort);
                }}
                onClose={closeMenu}
                placement="below"
                align="start"
                efforts={isCodex ? codexEfforts(defaultCodexModel) : undefined}
              />
            </SelectButton>
          </SettingRow>
        );
      case 'markUnsourced':
        return (
          <SettingRow id={id}>
            <Switch
              checked={current.markUnsourced}
              label="markieren"
              onChange={(checked: boolean): void => {
                save({ kind: 'markUnsourced', value: checked }).catch(() => undefined);
              }}
            />
          </SettingRow>
        );
      case 'cliVersion':
        return <CliVersionRow />;
      case 'selfReleaseMessages':
        return (
          <>
            <p className="settings-view__note">
              Diese Werte gelten für neue Vorhaben; bestehende behalten ihre eigenen in der
              Übersicht.
            </p>
            {renderSelfReleaseRow(id, current)}
          </>
        );
      case 'selfReleaseTldr':
      case 'selfReleaseOpenSession':
      case 'selfReleaseTldrModel':
        return renderSelfReleaseRow(id, current);
      case 'userSkills':
        return (
          <SettingRow id={id}>
            <span className="settings-view__path-text">
              <span className="settings-view__path" title={loaded.userSkillsDir}>
                {loaded.userSkillsDir}
              </span>
              <span className="settings-view__extra">{skillCountLabel(loaded.userSkillCount)}</span>
            </span>
            {renderRevealButton(loaded.userSkillsDir)}
          </SettingRow>
        );
      case 'projectSkills':
        return (
          <SettingRow id={id}>
            <span className="settings-view__path-text">
              <span className="settings-view__plain">{projectSkillsText()}</span>
              {!areRepositoriesLoading && hasProjectSkills(repositories) && (
                <span className="settings-view__extra">je Repository</span>
              )}
            </span>
          </SettingRow>
        );
      case 'environmentLinks':
        return <EnvironmentLinksRow />;
      case 'masterDir':
        return (
          <MasterDirRow
            masterDir={current.masterDir}
            onSave={(value: string | null): Promise<boolean> => save({ kind: 'masterDir', value })}
          />
        );
      case 'environmentMirror':
        return <EnvironmentMirrorRow />;
      case 'appVersion':
        return <AppVersionRow runningSessions={runningSessionCount(sessions)} />;
      case 'workspaces':
        return (
          <SettingRow id={id}>
            <span className="settings-view__path-text">
              <span className="settings-view__path" title={loaded.workspacesDir}>
                {loaded.workspacesDir}
              </span>
            </span>
            {renderRevealButton(loaded.workspacesDir)}
          </SettingRow>
        );
      case 'fileAccess':
        return (
          <SettingRow id={id}>
            <span className="settings-view__plain">Workspace und Repositories des Vorhabens</span>
          </SettingRow>
        );
      case 'remoteEnabled':
        return <RemoteEnabledRow />;
      case 'remotePort':
        return <RemotePortRow />;
      case 'remoteStatus':
        return <RemoteStatusRow />;
      case 'remoteDevices':
        return <RemoteDevicesRow />;
      case 'remoteHosts':
        return <RemoteHostsRow />;
      case 'remoteConnect':
        return <RemoteConnectRow />;
      default:
        return assertNever(id);
    }
  }

  function renderSections(
    current: Settings,
    loaded: SettingsOverview,
    hitIds: ReadonlySet<SettingId>,
  ): ReactElement[] {
    const groups: SettingGroup[] = groupEntries(shownEntries(current, facts));
    // Alle Kategorien bleiben eingehängt (Konto, Version und Claude-Version fragen beim
    // Einhängen Programme und das Netz ab); die nicht gewählten bzw. nicht getroffenen
    // sind nur ausgeblendet.
    return groups.map((group: SettingGroup) => {
      const hasHit: boolean = group.entries.some((entry: SettingEntry) => hitIds.has(entry.id));
      return (
        <section
          key={`${group.category}/${group.section}`}
          className="settings-view__section"
          hidden={isSearching ? !hasHit : group.category !== category}
        >
          <h2 className="settings-view__section-title">
            {isSearching ? (
              <>
                {categoryLabel(group.category)}{' '}
                <span className="settings-view__section-path">
                  › <b>{group.section}</b>
                </span>
              </>
            ) : (
              group.section
            )}
          </h2>
          {group.entries.map((entry: SettingEntry) => (
            <div key={entry.id} hidden={isSearching && !hitIds.has(entry.id)}>
              {renderRow(entry.id, current, loaded)}
            </div>
          ))}
        </section>
      );
    });
  }

  function renderSearchSummary(hitCount: number): ReactElement {
    const text: string = query.trim();
    if (hitCount === 0) {
      return <p className="settings-view__empty">{`Nichts gefunden für „${text}“.`}</p>;
    }
    return <p className="settings-view__count">{`${String(hitCount)} Treffer für „${text}“`}</p>;
  }

  function renderBody(): ReactElement | null {
    if (settings === null || overview === null) {
      if (loadError !== null) {
        return null;
      }
      return <p className="settings-view__loading">Lädt …</p>;
    }
    return (
      <>
        {isSearching && renderSearchSummary(hits.length)}
        {renderSections(settings, overview, hitIds)}
      </>
    );
  }

  return (
    <div className="settings-view">
      <header className="settings-view__header">
        <h1 className="settings-view__title">Einstellungen</h1>
        <SettingsSearchField value={query} onChange={changeQuery} />
      </header>
      <div className="settings-view__split">
        <SettingsNav
          active={isShowingHits ? null : category}
          attention={attention}
          counts={isShowingHits ? countByCategory(hits) : null}
          onSelect={selectCategory}
        />
        <div ref={contentRef} className="settings-view__scroll">
          <SettingsSearchContext.Provider value={terms}>
            <div className="settings-view__content">
              {renderErrors()}
              {renderBody()}
            </div>
          </SettingsSearchContext.Provider>
        </div>
      </div>
    </div>
  );
}

function assertNever(value: never): never {
  throw new Error(`Zeile ohne Zweig in renderRow: ${String(value)}`);
}

function skillCountLabel(count: number): string {
  return count === 1 ? '1 Skill' : `${String(count)} Skills`;
}

function hasProjectSkills(repositories: readonly KnownRepository[]): boolean {
  return repositories.some(
    (repository: KnownRepository) => !repository.isMissing && repository.skillCount > 0,
  );
}
