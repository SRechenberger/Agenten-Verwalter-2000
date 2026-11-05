import type { Settings } from '@/lib/bindings/Settings';
import { isLocalOperatingMode } from '@/lib/labels';

// Alle Zeilen der Einstellungsseite: Name, ⓘ-Text, Kategorie, Abschnitt (ADR 057).
// Neue Zeile = Eintrag hier + Zweig in `renderRow`.

export type SettingsCategoryId = 'general' | 'agent' | 'skills' | 'app';

/** Wofür ein Punkt an einer Kategorie in der Leiste steht. */
export type SettingsAttention = 'update' | 'environment';

export interface SettingsCategory {
  id: SettingsCategoryId;
  label: string;
  summary: string;
}

/** In der Reihenfolge der Leiste. */
export const SETTINGS_CATEGORIES: readonly SettingsCategory[] = [
  { id: 'general', label: 'Allgemein', summary: 'Konto · Darstellung' },
  {
    id: 'agent',
    label: 'Agent',
    summary: 'Betriebsart · Modelle · Modus · Claude Code · Selbstfreigabe',
  },
  { id: 'skills', label: 'Skills & Umgebung', summary: 'Skills · Umgebung' },
  { id: 'app', label: 'App', summary: 'Version · Ordner · Rechte' },
];

interface EntryData {
  category: SettingsCategoryId;
  section: string;
  label: string;
  /** Die Erklärung hinter dem ⓘ. */
  info: string;
  /** Fehlt `isShown`, steht die Zeile immer da. */
  isShown?: (settings: Settings) => boolean;
}

function isNotStandalone(settings: Settings): boolean {
  return settings.operatingMode !== 'standalone';
}

function isLocalMode(settings: Settings): boolean {
  return isLocalOperatingMode(settings.operatingMode);
}

function hasMasterDir(settings: Settings): boolean {
  return settings.masterDir !== null;
}

const CHAIN_BRAKE_INFO =
  'Nach 6 selbst freigegebenen Aufrufen in Folge fragt der Kasten trotzdem nach.';

// Die Schlüsselreihenfolge ist die Anzeigereihenfolge.
const ENTRY_DATA = {
  claudeAccount: {
    category: 'general',
    section: 'Konto',
    label: 'Claude-Konto',
    info: 'Das Konto, mit dem die Claude-Kommandozeile angemeldet ist. „Konto wechseln“ startet deren Anmeldung (claude auth login) in einem eigenen Fenster und im Browser; Schließen des Fensters bricht ab. Gilt für jeden Agenten, der danach startet.',
    isShown: isNotStandalone,
  },
  codexAccount: {
    category: 'general',
    section: 'Konto',
    label: 'Codex-Konto',
    info: 'Meldet Codex mit deinem ChatGPT-Konto an; gilt für alle Codex-Sessions ab der nächsten Nachricht.',
  },
  colorScheme: {
    category: 'general',
    section: 'Darstellung',
    label: 'Farbschema',
    info: 'System folgt der Windows-Einstellung für hell oder dunkel.',
  },
  terminalPlacement: {
    category: 'general',
    section: 'Darstellung',
    label: 'Terminal',
    info: 'Wo das Terminal neben dem Chat erscheint: unten unter der Ansicht oder als Spalte rechts. Ein- und ausblenden mit dem Knopf >_ oder Strg+Ö.',
  },
  operatingMode: {
    category: 'agent',
    section: 'Agent',
    label: 'Betriebsart',
    info: 'Claude: Modellanfragen gehen an dein Claude-Abo. Claude Code + LM Studio: dieselbe Claude-Kommandozeile mit Werkzeugen, Skills und Anweisungen, aber das Modell läuft lokal in LM Studio — für die Zeit, in der das Kontingent aufgebraucht ist. Autark: ohne Claude-Kommandozeile und ohne Anthropic — ein eigener, kleinerer Agent des Verwalters mit dem lokalen Modell. Codex: Sessions arbeiten mit OpenAI Codex und deinem ChatGPT-Konto. Gilt ab der nächsten Nachricht jeder Session.',
  },
  localModel: {
    category: 'agent',
    section: 'Agent',
    label: 'Lokales Modell',
    info: 'Ein in LM Studio geladenes Modell. Die Kontextlänge, mit der es dort geladen ist, gilt als Kontextfenster der Sessions. Adresse des Servers: Umgebungsvariable VERWALTER_LMSTUDIO_URL, sonst http://localhost:1234.',
    isShown: isLocalMode,
  },
  defaultModel: {
    category: 'agent',
    section: 'Agent',
    label: 'Standardmodell',
    info: 'Mit diesem Modell startet die erste Session eines neuen Vorhabens; weitere Sessions im Vorhaben übernehmen den Stand der letzten. In der Session jederzeit über die Eingabeleiste änderbar.',
  },
  defaultMode: {
    category: 'agent',
    section: 'Agent',
    label: 'Standard-Modus',
    info: 'Wie selbstständig Claude arbeitet und wie gründlich es nachdenkt. Gilt für neue Vorhaben; in der Session mit Umschalt + Tab bzw. über die Eingabeleiste wechselbar.',
  },
  markUnsourced: {
    category: 'agent',
    section: 'Agent',
    label: 'Angaben ohne Fundstelle',
    info: 'Unterstreicht Pfade, Datei:Zeile und Zahlen ab zwei Ziffern, die in keiner Werkzeug-Ausgabe derselben Antwort vorkommen.',
  },
  cliVersion: {
    category: 'agent',
    section: 'Claude Code',
    label: 'Claude-Version',
    info: 'Die Version der Claude-Kommandozeile, die der Verwalter für seine Agenten startet. „Aktualisieren“ führt claude update aus. Laufende Agenten behalten ihre alte Version bis zu ihrem nächsten Start; ist die Datei dabei gesperrt, schlägt die Aktualisierung fehl — dann die Sessions pausieren und es erneut versuchen.',
    isShown: isNotStandalone,
  },
  selfReleaseMessages: {
    category: 'agent',
    section: 'Selbstfreigabe',
    label: 'Selbstfreigabe für Nachrichten',
    info: `Ein Agent schickt Nachrichten an andere Sessions des Vorhabens ohne Kasten. ${CHAIN_BRAKE_INFO}`,
  },
  selfReleaseTldr: {
    category: 'agent',
    section: 'Selbstfreigabe',
    label: 'Selbstfreigabe für TL;DR',
    info: 'Ein Agent liest das TL;DR einer anderen Session des Vorhabens ohne Kasten.',
  },
  selfReleaseOpenSession: {
    category: 'agent',
    section: 'Selbstfreigabe',
    label: 'Selbstfreigabe für neue Sessions',
    info: `Ein Agent öffnet neue Sessions im Vorhaben ohne Kasten. ${CHAIN_BRAKE_INFO}`,
  },
  selfReleaseTldrModel: {
    category: 'agent',
    section: 'Selbstfreigabe',
    label: 'Modell des TL;DR',
    info: 'Mit diesem Modell entsteht das TL;DR, das ein Agent über das Session-Werkzeug abruft.',
  },
  userSkills: {
    category: 'skills',
    section: 'Skills',
    label: 'Benutzer-Skills',
    info: 'Skills aus deinem Benutzerordner stehen in jeder Session zur Verfügung.',
  },
  projectSkills: {
    category: 'skills',
    section: 'Skills',
    label: 'Projekt-Skills',
    info: 'Skills aus den .claude\\skills-Ordnern der Repositories einer Session. Welche gelten, hängt davon ab, welche Repositories die Session umfasst.',
  },
  environmentLinks: {
    category: 'skills',
    section: 'Umgebung',
    label: 'Verknüpfungen',
    info: 'Junctions und symbolische Links direkt unter ~\\.claude und direkt in dessen Unterordnern, deren Ziel nicht erreichbar ist.',
  },
  masterDir: {
    category: 'skills',
    section: 'Umgebung',
    label: 'Master-Ordner',
    info: 'Ordner mit den Originalen deiner Skills, Befehle, Ausgabestile und der Datei CLAUDE.md, z. B. auf einem synchronisierten Laufwerk. Erwartet mindestens einen der Unterordner skills, commands, output-styles oder die Datei CLAUDE.md.',
  },
  environmentMirror: {
    category: 'skills',
    section: 'Umgebung',
    label: 'Abgleich',
    info: 'Vergleicht Skills, Befehle, Ausgabestile und CLAUDE.md im Master-Ordner mit ~\\.claude nach Inhalt. Die Änderungszeit entscheidet nur, welche Seite neuer ist. „Spiegeln“ kopiert, was im Master neuer ist oder lokal fehlt, und löscht nie.',
    isShown: hasMasterDir,
  },
  appVersion: {
    category: 'app',
    section: 'Agenten Verwalter',
    label: 'Version',
    info: 'Die Version dieses Verwalters. „Erneut prüfen“ fragt die GitHub-Releases ab. Ein Update lädt die neue Datei, prüft ihre Prüfsumme und startet den Verwalter neu; laufende Agenten werden dabei unterbrochen.',
  },
  workspaces: {
    category: 'app',
    section: 'Ordner',
    label: 'Workspaces',
    info: 'Hier legt die App den Workspace jedes Vorhabens an: Arbeitsordner seiner Sessions und Ablage der Anhänge.',
  },
  fileAccess: {
    category: 'app',
    section: 'Rechte',
    label: 'Dateizugriff',
    info: 'Ein Agent arbeitet im Workspace seines Vorhabens, in den Haupt-Checkouts von dessen Repositories und in deren Ticket-Worktrees — nicht im übrigen Benutzerverzeichnis.',
  },
} satisfies Record<string, EntryData>;

export type SettingId = keyof typeof ENTRY_DATA;

export interface SettingEntry extends EntryData {
  id: SettingId;
}

export interface SettingGroup {
  category: SettingsCategoryId;
  section: string;
  entries: SettingEntry[];
}

const ENTRY_IDS = Object.keys(ENTRY_DATA) as SettingId[];

export function settingEntry(id: SettingId): SettingEntry {
  const data: EntryData = ENTRY_DATA[id];
  return { id, ...data };
}

/** Die Zeilen, die für diese Einstellungen gelten, in Anzeigereihenfolge. */
export function shownEntries(settings: Settings): SettingEntry[] {
  return ENTRY_IDS.map((id: SettingId) => settingEntry(id)).filter(
    (entry: SettingEntry) => entry.isShown === undefined || entry.isShown(settings),
  );
}

/** Fasst aufeinanderfolgende Einträge mit gleicher Kategorie und gleichem Abschnitt zusammen. */
export function groupEntries(entries: readonly SettingEntry[]): SettingGroup[] {
  const groups: SettingGroup[] = [];
  for (const entry of entries) {
    const last: SettingGroup | undefined = groups[groups.length - 1];
    if (last !== undefined && last.category === entry.category && last.section === entry.section) {
      last.entries.push(entry);
    } else {
      groups.push({ category: entry.category, section: entry.section, entries: [entry] });
    }
  }
  return groups;
}

export function categoryLabel(id: SettingsCategoryId): string {
  const category: SettingsCategory | undefined = SETTINGS_CATEGORIES.find(
    (candidate: SettingsCategory) => candidate.id === id,
  );
  return category?.label ?? id;
}

/** Beim Öffnen: die erste Kategorie mit Punkt, sonst die zuletzt gewählte, sonst Allgemein. */
export function initialCategory(
  attention: Partial<Record<SettingsCategoryId, SettingsAttention>>,
  last: SettingsCategoryId | null,
): SettingsCategoryId {
  const flagged: SettingsCategory | undefined = SETTINGS_CATEGORIES.find(
    (category: SettingsCategory) => attention[category.id] !== undefined,
  );
  if (flagged !== undefined) {
    return flagged.id;
  }
  return last ?? 'general';
}
