# Einstellungen gliedern und durchsuchen

Ziel: Die Einstellungsseite steht heute als ein langer Scroll mit acht Abschnitten (mit „Umgebung“ neun). Sie bekommt links eine Leiste mit vier Kategorien (Allgemein, Agent, Skills & Umgebung, App), rechts nur die Abschnitte der gewählten Kategorie und oben ein Suchfeld, das alle Kategorien auf einmal nach Zeilenname und Hilfetext durchsucht. Die Zeilen selbst bleiben unverändert. Entwurf: [artifacts/entwurf-einstellungen-gliederung.html](artifacts/entwurf-einstellungen-gliederung.html), vom User am 2026-11-04 entschieden (vier Kategorien links, Suchfeld oben; die Einstellungen sollen nicht flach weiterwachsen).

Kontext für jeden Umsetzer: [AGENTS.md](../../../AGENTS.md), [docs/code-map.md](../../code-map.md) (Zeilen „Einstellungen“, „Claude-Konto“, „Codex-Konto“, „App-Update“, „Claude-Code-Update“, „App-Rahmen“), [docs/glossary.md](../../glossary.md), [docs/conventions/](../../conventions/) (react, tailwind, typescript), [ADR 012](../../decisions/012-einstellungen-farbschema-und-listen.md) (Einstellungsseite), [Plan Umgebung](../2026-10-22_umgebung/README.md) (dessen Phase 2 setzt auf diesen Plan auf). Fehlerklassen geprüft (Vault: TypeScript, React): keine einschlägig — sie betreffen `fetch`-Lebendprüfungen und Tests, dieser Plan hat weder noch.

| # | Phase | Rating | Status |
|---|---|---|---|
| 1 | [Kategorien](phase-1-kategorien.md): Katalog aller Zeilen, Leiste links, Punkte an Kategorien; ADR 057 | standard | complete |
| 2 | [Suche](phase-2-suche.md): Suchfeld, Trefferansicht, Markierung und Fundstelle; Doku-Abschluss | standard | complete |

**Rahmen:** strikt 1 → 2. Direkt auf `main`, ein Commit pro Phase, Scope `settings`. `pnpm check` grün vor jedem Commit, `cargo` im PATH (die Prüfkette baut Rust mit, angefasst wird es nicht). Reine Oberfläche: kein Rust, keine Bindings, keine Migration, keine neuen Abhängigkeiten. Keine automatisierten Tests, keine Smoke-Checkliste. Planabschluss mit Versions-Tag (Minor-Version, [releases.md](../../conventions/releases.md)). Danach wird Phase 2 des Plans Umgebung neu geschnitten (STATE.md, „Nächster Schritt“).

**Screen-Eigentümer:** Dieser Plan besitzt die Hülle der Einstellungsseite (Kopf, Suchfeld, Leiste, Katalog). Spätere Pläne tragen neue Zeilen in den Katalog ein und bauen keinen eigenen Abschnitt hinter die Hülle. Der Entwurf ist verbindlich und ersetzt das „alles untereinander“ der Tafel `Settings` von 2026-09-28 ([canvas/Settings.dc.html](../../design/2026-09-28_hauptansichten/canvas/Settings.dc.html)); die Zeilen darin gelten weiter, wie sie im Code stehen.

## Festgelegte Entscheidungen

- **Kategorien** (Reihenfolge = Reihenfolge der Leiste; Abschnitte und Zeilen in Anzeigereihenfolge):

| id | Beschriftung | Untertitel | Abschnitt › Zeilen |
|---|---|---|---|
| `general` | Allgemein | Konto · Darstellung | Konto › Claude-Konto, Codex-Konto · Darstellung › Farbschema, Terminal |
| `agent` | Agent | Betriebsart · Modelle · Modus · Claude Code | Agent › Betriebsart, Lokales Modell, Standardmodell, Standard-Modus, Angaben ohne Fundstelle · Claude Code › Claude-Version |
| `skills` | Skills & Umgebung | Skills | Skills › Benutzer-Skills, Projekt-Skills (der Abschnitt „Umgebung“ und der Untertitel „Skills · Umgebung“ kommen mit dem Plan Umgebung) |
| `app` | App | Version · Ordner · Rechte | Agenten Verwalter › Version · Ordner › Workspaces · Rechte › Dateizugriff |

- **Der Katalog ist die einzige Stelle für Zeilenname, ⓘ-Text, Kategorie und Abschnitt jeder Zeile.** Die Zeile wird mit `<SettingRow id="…">` gezeichnet und holt Name und ⓘ-Text selbst aus dem Katalog; `SettingsView`, `AccountRow`, `CodexAccountRow`, `AppVersionRow` und `CliVersionRow` führen keine eigenen Texte mehr. Ein Zweig `renderRow` in `SettingsView` zeichnet die Bedienelemente je Katalog-Id; fehlt ein Zweig, meldet der Compiler es (`switch` über die Id mit `never` im `default`). Gründe und verworfene Wege: ADR 057.
- **Alle Zeilen aller Kategorien bleiben eingehängt**, solange die Einstellungen offen sind; ein Wechsel der Kategorie oder die Suche blendet nur aus (`hidden`). Grund: Konto, Version und Claude-Version fragen beim Einhängen Programme und das Netz ab (bis 15 s); ein Wechsel mit Aus- und Einhängen löste das jedes Mal neu aus. Es gibt also je Zeile genau eine Instanz, auch in der Suche.
- **Bedingte Zeilen** wie heute, als `isShown(settings)` im Katalog: Claude-Konto und Claude-Version nur, wenn die Betriebsart nicht `standalone` ist; Lokales Modell nur in lokalen Betriebsarten (`isLocalOperatingMode`). Ein Abschnitt ohne sichtbare Zeile entfällt.
- **Beim Öffnen** gilt die erste Kategorie in Leistenreihenfolge, die einen Punkt trägt; sonst die zuletzt in der Leiste angeklickte Kategorie dieses App-Laufs; sonst Allgemein. Gemerkt wird sie in einem Zustand-Store (`src/stores/settingsNav.ts`), nicht in der Datenbank: eine Ansichtsvorliebe, deren Verlust beim Neustart nicht weh tut.
- **Punkte an Kategorien** spiegeln den Punkt am Knopf „Einstellungen“ der Sidebar: Update des Verwalters → Kategorie „App“ (Farbe `--color-accent` wie der Sidebar-Punkt). Den Umgebungs-Punkt (Warnfarbe `--color-status-waiting`, Kategorie „Skills & Umgebung“) füttert der Plan Umgebung; die Mechanik dafür (`useCategoryAttention`, Typ `SettingsAttention`) steht schon in diesem Plan.
- **Suche:** Der Suchtext wird getrimmt und an Leerzeichen in Wörter geteilt; jedes Wort muss als Teilstring im Zeilennamen **oder** im ⓘ-Text der Zeile vorkommen (UND), Groß- und Kleinschreibung egal (`toLowerCase`). Abschnittsnamen, Pfade und Zahlen in den Bedienelementen zählen nicht. Durchsucht werden nur sichtbare Zeilen (`isShown`). Leerer Suchtext = keine Suche. Gefiltert wird bei jedem Tastendruck, ohne Verzögerung.
- **Trefferansicht:** Die Treffer sind die echten Zeilen (dieselben Instanzen wie in der Kategorie), gruppiert nach Kategorie und Abschnitt in Katalogreihenfolge, Gruppentitel „<Kategorie> › <Abschnitt>“. Die Leiste zeigt statt des gewählten Eintrags je Kategorie die Trefferzahl; Kategorien ohne Treffer sind gedimmt. Ein Klick auf eine Kategorie leert die Suche und öffnet sie.
- **Markierung und Fundstelle:** Treffer im Zeilennamen sind markiert. Steht mindestens ein Suchwort nicht im Zeilennamen, zeigt die Zeile darunter „Im Hilfetext: <Auszug>“. Auszug = erster Satz des ⓘ-Textes (Trennung nach „. “), der ein solches Wort enthält; ist der Satz länger als 140 Zeichen, 60 Zeichen vor und nach dem ersten Treffer, an Wortgrenzen geschnitten, mit „…“ an den abgeschnittenen Enden.
- **Nicht gebaut:** Verhalten bei sehr schmalem Fenster (der Entwurf lässt es offen: die Leiste bleibt 216 px breit, der Inhalt schrumpft mit `min-width: 0`, Pfade kürzen wie heute per Auslassungspunkte); Tastenkürzel oder Autofokus für die Suche; Suchverlauf; Ausblenden der Leiste.

## Finale AK (fürs Ganze; die AK je Phase stehen in den Phasen-Dateien)

1. Alle 15 Zeilen der Seite sind erreichbar: über genau eine Kategorie und über die Suche; keine Zeile ist verloren gegangen, keine doppelt gezeichnet.
2. Die Seite sieht in beiden Zuständen (Kategorie offen, Suche „ordner“) aus wie der Entwurf, mit den echten Tokens statt der Platzhalterfarben.
3. Ein Wechsel der Kategorie und jede Suche lösen keine neue Abfrage aus (Konto, Version, Claude-Version bleiben eingehängt).
4. Ein neuer Abschnitt oder eine neue Zeile ist ein Eintrag im Katalog plus ein Zweig in `renderRow`; die Doku (Code-Map, Glossar, ADR 057) sagt das.
5. `pnpm check` grün.

## Abweichungen vom Entwurf (bewusst)

- Der Entwurf zeigt nur den gelben Umgebungs-Punkt an einer Kategorie. Weil der Update-Punkt am Knopf „Einstellungen“ sonst ins Leere führte (die Zeile „Version“ läge dann in „App“ ohne Hinweis), trägt auch „App“ einen Punkt. Die Farben im Entwurf (Update blau) sind Platzhalter; die App nimmt die echten Tokens.
- Die Kategorie „Skills & Umgebung“ trägt bis zum Plan Umgebung nur den Abschnitt „Skills“ und den Untertitel „Skills“.
- Der Auszug in der Fundstelle folgt der festen Regel oben; der Entwurf zeigt Platzhaltertexte.

## Kontrakt

```ts
// src/features/settings/settingsCatalog.ts
export type SettingsCategoryId = 'general' | 'agent' | 'skills' | 'app';
export type SettingsAttention = 'update' | 'environment';
export interface SettingsCategory { id: SettingsCategoryId; label: string; summary: string }
export const SETTINGS_CATEGORIES: readonly SettingsCategory[]; // Leistenreihenfolge
interface EntryData { category: SettingsCategoryId; section: string; label: string; info: string; isShown?: (settings: Settings) => boolean }
const ENTRY_DATA = { claudeAccount: { … }, … } satisfies Record<string, EntryData>; // Schlüsselreihenfolge = Anzeigereihenfolge
export type SettingId = keyof typeof ENTRY_DATA;
export interface SettingEntry extends EntryData { id: SettingId }
export function settingEntry(id: SettingId): SettingEntry;
export function shownEntries(settings: Settings): SettingEntry[];
export interface SettingGroup { category: SettingsCategoryId; section: string; entries: SettingEntry[] }
export function groupEntries(entries: readonly SettingEntry[]): SettingGroup[]; // fasst aufeinanderfolgende Einträge mit gleicher Kategorie und gleichem Abschnitt
export function categoryLabel(id: SettingsCategoryId): string;
export function initialCategory(attention: Partial<Record<SettingsCategoryId, SettingsAttention>>, last: SettingsCategoryId | null): SettingsCategoryId;

// src/features/settings/settingsSearch.ts (Phase 2)
export interface TextPart { text: string; isHit: boolean }
export function searchTerms(query: string): string[]; // [] bei leerem Text
export function searchEntries(entries: readonly SettingEntry[], terms: readonly string[]): SettingEntry[];
export function countByCategory(entries: readonly SettingEntry[]): Record<SettingsCategoryId, number>;
export function markParts(text: string, terms: readonly string[]): TextPart[];
export function foundInInfo(label: string, info: string, terms: readonly string[]): TextPart[] | null; // null: kein Wort fehlt im Zeilennamen

// src/features/settings/SettingsSearchContext.ts (Phase 2): createContext<readonly string[]>([]) + useSettingsSearchTerms()
// src/features/settings/useCategoryAttention.ts: useCategoryAttention(): Partial<Record<SettingsCategoryId, SettingsAttention>>
// src/stores/settingsNav.ts: useSettingsNavStore { category: SettingsCategoryId | null; setCategory(category) }
```

Die Zeile: `SettingRow({ id, children })` in `SettingRow.tsx`; `AccountRowView` nimmt statt `label` und `info` eine `settingId: SettingId`.

## Wackelstellen (nur Plan, kein Prüfauftrag)

- 🟡 Die Menüs der Zeilen (`ModelMenu`, `ModeMenu`, `LocalModelMenu`) hängen an `Popover`, das per `createPortal` in `document.body` zeichnet (geprüft in `src/components/Popover.tsx`). Blendet man die Zeile aus, während ihr Menü offen ist, bliebe das Menü stehen — deshalb schließen Kategorienwechsel und jede Änderung des Suchtextes das offene Menü (`closeMenu`), Phase 1 AK 9.
- 🟡 Die Auszugsregel ist aus zwei Beispielen des Entwurfs abgeleitet; Texte mit mehreren Sätzen mit Treffer in einem späteren Satz sind nur durch die Regel, nicht durch den Entwurf belegt.
- 🟡 Schmales Fenster ungeprüft (siehe „Nicht gebaut“).

## Summary

Die Einstellungsseite hat links eine Leiste mit vier Kategorien und oben ein Suchfeld. Ein Katalog (`settingsCatalog.ts`) hält Name, ⓘ-Text, Kategorie und Abschnitt aller 15 Zeilen; alle Zeilen bleiben eingehängt und werden nur ausgeblendet. Die Suche filtert bei jedem Tastendruck nach Wörtern (UND, Teilstring) in Zeilenname und ⓘ-Text, zeigt die echten Zeilen gruppiert, markiert Treffer und nennt bei Treffern nur im Hilfetext die Fundstelle. ADR 057.

## Files touched

- `src/features/settings/`: `settingsCatalog.ts`, `settingsSearch.ts`, `SettingsSearchContext.ts`, `SettingsSearchField.tsx`/`.css`, `SettingsNav.tsx`/`.css`, `SettingRow.tsx`/`.css`, `SettingsView.tsx`/`.css`, `useCategoryAttention.ts`; `src/stores/settingsNav.ts`
- Konto-, Versions- und Claude-Versions-Zeilen (Texte aus dem Katalog)
- `docs/decisions/057-einstellungen-kategorien-und-suche.md`, `docs/code-map.md`

## Commits

- `85a4fdd` feat(settings): Einstellungen in vier Kategorien gliedern
- `c01e5ff` feat(settings): Einstellungen durchsuchen

## Deviations from plan

Keine. Die Prüffälle der AK (3 Treffer für „ordner“, 2 für „konto claude“, keiner für „xyzzy“) wurden aus dem Katalog hergeleitet, nicht in der laufenden App geprüft.

## Follow-ups

- Laufzeitbeleg in der echten App steht aus: Suche „ordner“ gegen den Entwurf, Menüs schließen beim Tippen, „Anmeldung läuft …“ überlebt Suchen.
- Schmales Fenster ungeprüft (bewusst nicht gebaut).
- Plan Umgebung Phase 2 neu schneiden (Hinweise in dessen README).
