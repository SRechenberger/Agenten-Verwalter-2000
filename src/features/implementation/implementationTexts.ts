import type { BriefChangeKind } from '@/lib/bindings/BriefChangeKind';
import type { Effort } from '@/lib/bindings/Effort';
import type { ImplementationView } from '@/lib/bindings/ImplementationView';
import type { ModelId } from '@/lib/bindings/ModelId';
import type { OnError } from '@/lib/bindings/OnError';
import type { PhaseMark } from '@/lib/bindings/PhaseMark';
import type { PredecessorsMay } from '@/lib/bindings/PredecessorsMay';
import type { StationView } from '@/lib/bindings/StationView';
import { effortLabel, modelName } from '@/lib/labels';
import type { ValueSource } from '@/features/implementation/stationAgent';
import type { ImplementationViewKind } from '@/stores/sessions';

export const VIEW_LABEL: Record<ImplementationViewKind, string> = {
  flow: 'Ablauf',
  changes: 'Changes',
};

export const REPORT_LABEL = 'Lagebericht';
export const REPORT_LOADING = 'Lagebericht wird geladen …';
export const REPORT_COPY_LABEL = 'Kopieren';
export const REPORT_COPIED = 'In die Zwischenablage kopiert.';
export const REPORT_CLOSE_LABEL = 'Schließen';
export const INQUIRY_LABEL = 'Neue Rückfrage';
export const INQUIRY_TITLE =
  'Frage zur Umsetzung in einer frischen Session — der Lagebericht hängt dran';

/** „6 von 16 fertig“; ist alles fertig, „· Umsetzung fertig“ dahinter. */
export function progressLine(implementation: ImplementationView): string {
  const done: string = `${String(implementation.stationsDone)} von ${String(implementation.stationsTotal)} fertig`;
  return implementation.finishedAt === null ? done : `${done} · Umsetzung fertig`;
}

/** „6/16“ in der Sidebar. */
export function progressShort(implementation: ImplementationView): string {
  return `${String(implementation.stationsDone)}/${String(implementation.stationsTotal)}`;
}

export const AUTO_START_LABEL = 'Automatisch starten';
export const AUTO_START_TITLE =
  'Startet jede Station, deren Vorgänger fertig sind, nach 10 Sekunden Countdown von selbst. Einzelne Stationen können abweichen.';
export const MAX_PARALLEL_LABEL = 'Parallel max.';
export const MAX_PARALLEL_TITLE =
  'Höchstens so viele Stationen laufen gleichzeitig. Weitere bereite Stationen starten selbst, sobald ein Platz frei wird – nur bei Stationen, die automatisch starten. Ein Klick auf ▶ ignoriert die Grenze.';
export const MAX_PARALLEL_LESS = 'weniger';
export const MAX_PARALLEL_MORE = 'mehr';
/** Anzeige ohne Grenze. */
export const MAX_PARALLEL_UNLIMITED = '∞';
export const MAX_PARALLEL_UNLIMITED_TITLE = 'unbegrenzt';

/** „Warteschlange: 2“ in der Leiste. */
export function queueText(count: number): string {
  return `Warteschlange: ${String(count)}`;
}

export const QUEUE_TITLE =
  'Bereite Stationen, die auf einen freien Platz warten; sie rücken selbst nach';
export const NEW_LANE_LABEL = '+ Lane';
export const NEW_LANE_TITLE = 'Neue Lane mit Stationen und Kanten anlegen';
export const STOP_AFTER_LABEL = 'Anhalten nach dieser Session';
export const STOP_AFTER_TITLE = 'Die laufende Station läuft zu Ende, danach startet nichts mehr';
export const BACK_TO_CURRENT_LABEL = 'Zur aktuellen';
export const BACK_TO_CURRENT_TITLE = 'Springt zurück zur Station, die gerade läuft oder fragt';
export const OPEN_CHAT_LABEL = 'Chat öffnen';
export const OPEN_CHAT_TITLE = 'Öffnet die Session dieser Station; Fragen beantwortest du dort';
export const START_NOW_LABEL = '▶ Jetzt starten';
export const START_NOW_TITLE = 'Startet diese Station sofort als neue Session';
export const CANCEL_COUNTDOWN_LABEL = 'Abbrechen';
export const CANCEL_COUNTDOWN_TITLE = 'Hält den Countdown an; die Station bleibt bereit';
export const DONE_BY_USER_RESULT = 'vom Benutzer erledigt';
export const DONE_BY_USER_LABEL = 'Erledigt';
export const DONE_BY_USER_TITLE = 'Meldet diese Station als fertig, damit die nächste starten darf';
export const STATION_SETTINGS_LABEL = 'Einstellungen';
export const STATION_SETTINGS_TITLE =
  'Typ, Modell, Denkaufwand, Zusatztext und Skill nur für diese Station ändern';
export const EDIT_TYPE_LABEL = 'Typ bearbeiten';
export const EDIT_TYPE_TITLE = 'Öffnet den Agententyp dieser Station zum Bearbeiten';
export const REPORT_TITLE =
  'Zeigt den Lagebericht, den auch der Agent über implementation_get liest';
export const TIMELINE_LABEL = 'Zeitleiste der Umsetzung';
export const NO_PHASES = 'Diese Umsetzung hat noch keine Lanes.';

export const PLAN_PROPOSAL_TITLE = 'Plan ändern?';
export const PLAN_CHANGED_TITLE = 'Plan geändert';
export const APPLY_CHANGE_LABEL = 'Übernehmen';
export const APPLY_CHANGE_TITLE = 'Übernimmt den Vorschlag mit den Werten, wie sie hier stehen';
export const DISCARD_CHANGE_LABEL = 'Verwerfen';
export const DISCARD_CHANGE_TITLE = 'Lässt den Plan, wie er ist';
export const SHOW_PENDING_LABEL = 'Vorschlag ansehen';
export const SHOW_PENDING_TITLE = 'Holt die Karte „Plan ändern?“ nach vorn';
export const CHANGE_MARKER_TITLE = 'Zeigt diese Plan-Änderung vorn in der Kartei';
export const PHASE_MARK_LABEL: Record<PhaseMark, string> = {
  new: 'neu',
  moved: 'vorgezogen',
};
export const PHASE_MARK_TITLE: Record<PhaseMark, string> = {
  new: 'Diese Lane kam mit der letzten Plan-Änderung dazu',
  moved: 'Diese Lane wurde mit der letzten Plan-Änderung vorgezogen',
};

// ---- Typ, Modell, Denkaufwand und Startfehler der Karte (ADR 060) ----

/** Meta der Zeitleiste und Name des Punkts, wenn der Start der Station scheiterte. */
export const START_FAILED_LABEL = 'startet nicht';
export const START_FAILED_LEAD = 'Startet nicht.';
export const START_FAILED_HINT =
  'Es ist keine Session entstanden. Behebe die Ursache; danach startet ▶ die Station.';

export const AGENT_LEAD_NEXT = 'startet als';
export const AGENT_LEAD_STARTED = 'läuft als';
export const AGENT_TYPE_TITLE = 'Typ';
export const NO_TYPE_LABEL = 'kein Typ';
export const NO_TYPE_TITLE = 'Kein Typ trifft den Namen dieser Station';
export const DELETED_TYPE_TITLE = 'Der Typ ist gelöscht';
export const MODEL_LIKE_SESSION = 'Modell: wie die Session';
export const EFFORT_LIKE_SESSION = 'Denkaufwand: wie die Session';
export const MODEL_LIKE_SESSION_TITLE = 'In dieser Betriebsart gilt das Modell des Typs nicht';
export const EFFORT_LIKE_SESSION_TITLE =
  'In dieser Betriebsart gilt der Denkaufwand des Typs nicht';
export const AGENT_TYPE_NAME_TITLE = 'Agententyp der Station';
export const SKILL_CHIP_TITLE = 'Skill der Station: steht als Befehl in der ersten Zeile';

export function deletedTypeText(name: string): string {
  return `${name} · gelöscht`;
}

/** Der kleine Zusatz hinter einem Wert („Opus 5.5 Station“, „Opus 5.5 Regel“). Ein Wert des Typs oder der Session trägt keinen. */
export function sourceSmall(source: ValueSource | null): string | null {
  switch (source) {
    case 'station':
      return 'Station';
    case 'rule':
      return 'Regel';
    case 'type':
    case null:
      return null;
  }
}

/** Tooltip der Herkunft; `ruleNumber` ist die Nummer der Session, von der eine neue erbt. */
export function sourceTitle(source: ValueSource | null, ruleNumber: number | null): string {
  switch (source) {
    case 'station':
      return 'Einstellung der Station';
    case 'type':
      return 'aus dem Typ';
    case 'rule':
      return ruleNumber === null
        ? 'allgemeine Regel: wie die Session mit der höchsten Nummer'
        : `allgemeine Regel: wie Session #${String(ruleNumber)}`;
    case null:
      return 'Wert der Session';
  }
}

/** „Zusatztext · 140 Zeichen“; der Text selbst steht im Tooltip. Die Zahl im Format der App wie der Zähler im Dialog („4.001“). */
export function promptChipText(count: number): string {
  return `Zusatztext · ${count.toLocaleString('de-DE')} Zeichen`;
}

export function promptChipTitle(text: string): string {
  return `Zusatztext zum Systemprompt: ${text}`;
}

/** „2 Lanes aktiv“ in der Kopfzeile, solange mehrere Lanes zugleich arbeiten oder fragen. */
export function lanesActiveText(count: number): string {
  return `${String(count)} Lanes aktiv`;
}

/** „Stufe 2 · parallel möglich“ über den Stationen einer Stufe; „parallel möglich“ ab zwei. */
export function stageTitle(level: number, count: number): string {
  return count > 1 ? `Stufe ${String(level)} · parallel möglich` : `Stufe ${String(level)}`;
}

/** „30.10. 14:05“ in Ortszeit. */
export function stampText(at: number): string {
  return new Date(at).toLocaleString('de-DE', {
    day: '2-digit',
    month: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  });
}

/** „aus #3 · 30.10. 14:05“; die Session gibt es nicht mehr, wenn ihre Nummer 0 ist. */
export function changeOrigin(sessionNumber: number, at: number): string {
  const origin: string =
    sessionNumber === 0 ? 'aus einer gelöschten Session' : `aus #${String(sessionNumber)}`;
  return `${origin} · ${stampText(at)}`;
}

/** Schlüssel einer Änderungsliste, „nichts“ bei leerer. */
export function keysText(keys: readonly string[]): string {
  return keys.length === 0 ? 'nichts' : keys.join(', ');
}

/** Zeile der Marke auf der Linie. */
export function changeMarkerText(sessionNumber: number): string {
  const origin: string = sessionNumber === 0 ? '' : ` · aus #${String(sessionNumber)}`;
  return `Plan geändert${origin} · von dir übernommen`;
}

const MINUTE_MS = 60_000;
const HOUR_MINUTES = 60;

/** „8 min“, „1 h 5 min“; unter einer Minute „< 1 min“. */
export function durationText(milliseconds: number): string {
  const minutes: number = Math.floor(milliseconds / MINUTE_MS);
  if (minutes < 1) {
    return '< 1 min';
  }
  if (minutes < HOUR_MINUTES) {
    return `${String(minutes)} min`;
  }
  const rest: number = minutes % HOUR_MINUTES;
  const hours: string = String(Math.floor(minutes / HOUR_MINUTES));
  return rest === 0 ? `${hours} h` : `${hours} h ${String(rest)} min`;
}

/** Zustand der Station als Satz für die große Karte („läuft seit 12 min“, „fertig · 24 min“). */
export function stateText(station: StationView, now: number): string {
  if (station.startError !== null) {
    return 'bereit';
  }
  switch (station.state) {
    case 'running':
    case 'starting':
      return `läuft seit ${durationText(runningFor(station, now))}`;
    case 'waitsForScript':
      return 'wartet auf einen Hintergrund-Befehl, etwa den Testlauf';
    case 'asking':
      return 'fragt dich';
    case 'waitsForYou':
      return 'wartet auf dich';
    case 'done':
      return `fertig · ${durationText(station.runningMs)}`;
    case 'error':
      return 'Fehler';
    case 'countdown':
      return countdownText(station, now);
    case 'ready':
    case 'pending':
      return 'wartet auf Start';
  }
}

/** Kurzfassung für die Zeitleiste: Dauer, „läuft“, „fragt“, „startet nicht“ — leer, solange nichts war. */
export function stationMeta(station: StationView, now: number): string {
  if (station.startError !== null) {
    return START_FAILED_LABEL;
  }
  switch (station.state) {
    case 'done':
      return durationText(station.runningMs);
    case 'running':
    case 'starting':
      return `läuft ${durationText(runningFor(station, now))}`;
    case 'waitsForScript':
      return 'Skript läuft';
    case 'asking':
      return 'fragt';
    case 'waitsForYou':
      return 'wartet';
    case 'error':
      return 'Fehler';
    case 'countdown':
      return countdownText(station, now);
    case 'ready':
    case 'pending':
      return '';
  }
}

function runningFor(station: StationView, now: number): number {
  if (station.startedAt === null) {
    return station.runningMs;
  }
  return Math.max(station.runningMs, now - station.startedAt);
}

function countdownText(station: StationView, now: number): string {
  if (station.startsAt === null) {
    return 'startet gleich';
  }
  const seconds: number = Math.max(0, Math.ceil((station.startsAt - now) / 1000));
  return `startet in ${String(seconds)} s`;
}

export function implementationTitle(implementation: ImplementationView): string {
  return `Umsetzung ${implementation.name}`;
}

// ---- Zeitleiste mit Kanten-Chips (ADR 061, Entwurf 2026-11-10_constraint-ablaeufe) ----

const SHORT_MODEL: Record<ModelId, string> = {
  fable: 'Fable',
  opus: 'Opus',
  opusplan: 'Opus Plan',
  sonnet: 'Sonnet',
  haiku: 'Haiku',
};

/** „Opus“ statt „Opus 5.5“: kurz für Zeile, Schaltergruppen und Zusammenfassungen. */
export function shortModel(model: ModelId): string {
  return SHORT_MODEL[model];
}

/** „hoch“ für die Zeile. */
export function shortEffort(effort: Effort): string {
  return effortLabel(effort).toLowerCase();
}

/** „Lane 2 · Review Core #6“; ohne Session ohne Nummer. */
export function rowTitle(laneKey: string, station: StationView): string {
  const number: string = station.sessionNumber === null ? '' : ` #${String(station.sessionNumber)}`;
  return `Lane ${laneKey} · ${station.stage}${number}`;
}

/** „Modell: Opus 5.5 (Typ) · Denkaufwand: hoch (Station)“, nach dem Start mit „– Werte der Session“. */
export function metaTitle(
  model: { value: ModelId; source: ValueSource | null } | null,
  effort: { value: Effort; source: ValueSource | null } | null,
  isStarted: boolean,
): string {
  const part = (label: string, text: string | null, source: ValueSource | null): string =>
    text === null
      ? `${label}: wie die Session`
      : `${label}: ${text}${source === null ? '' : ` (${SOURCE_WORD[source]})`}`;
  const both: string = `${part('Modell', model === null ? null : modelName(model.value), model?.source ?? null)} · ${part('Denkaufwand', effort === null ? null : shortEffort(effort.value), effort?.source ?? null)}`;
  return isStarted ? `${both} – Werte der Session` : both;
}

const SOURCE_WORD: Record<ValueSource, string> = {
  station: 'Station',
  type: 'Typ',
  rule: 'Standard',
};

export const BY_USER_MARK = 'durch dich';
export const BY_USER_MARK_TITLE = 'Diese Station erledigst du selbst';
export const MANUAL_MARK = '⏸ manuell';
export const MANUAL_MARK_TITLE = 'Startet nie von selbst, auch mit „Automatisch starten“';
export const AUTOMATIC_MARK = '▶ automatisch';
export const AUTOMATIC_MARK_TITLE = 'Startet von selbst, obwohl „Automatisch starten“ aus ist';
export const ROW_START_LABEL = '▶';
export const ROW_START_TITLE = 'Jetzt starten (ignoriert die Grenze „Parallel max.“)';
export const ROW_OPEN_LABEL = 'Öffnen';
export const ROW_OPEN_TITLE = 'Öffnet die Station in der Kartei';
export const ROW_OPEN_CHAT_TITLE = 'Öffnet den Chat dieser Station; die Frage beantwortest du dort';
export const ROW_TITLE = 'Zeigt diese Station in der Kartei; Doppelklick öffnet den Chat';
export const CHIP_TITLE = 'Zeigt diese Station in der Kartei';
export const START_STATION_CHIP = 'Startstation';
export const START_STATION_HINT = 'keine Vorgänger – darf sofort starten';
export const END_OF_PLAN_CHIP = 'Ende des Plans';

/** „wartet auf“, nach dem Start „wartete auf“. */
export function waitsOnLead(isBegun: boolean): string {
  return isBegun ? 'wartete auf' : 'wartet auf';
}

export const NEXT_LEAD = 'danach →';

/** Zustand rechts in der Zeile: „läuft · 12 min“, „bereit · Warteschlange 1“, „startet in 7 s“. */
export function rowStateText(station: StationView, now: number): string {
  if (station.startError !== null) {
    return START_FAILED_LABEL;
  }
  switch (station.state) {
    case 'done':
      return `fertig · ${durationText(station.runningMs)}`;
    case 'running':
    case 'starting':
      return `läuft · ${durationText(runningFor(station, now))}`;
    case 'waitsForScript':
      return 'Skript läuft';
    case 'asking':
      return 'Frage offen';
    case 'waitsForYou':
      return 'wartet auf dich';
    case 'error':
      return station.restarting ? 'Fehler · startet neu' : 'Fehler';
    case 'countdown':
      return countdownText(station, now);
    case 'ready':
      return station.queuePosition === null
        ? 'bereit'
        : `bereit · Warteschlange ${String(station.queuePosition)}`;
    case 'pending':
      return 'wartet';
  }
}

// ---- Kartei: vier Abschnitte (Ablauf, Auftrag, Agent, Automatik) ----

export const SECTION_FLOW = 'Ablauf';
export const SECTION_BRIEF = 'Auftrag';
export const SECTION_INSTRUCTION = 'Anweisung';
export const SECTION_AGENT = 'Agent';
export const SECTION_WORKER = 'Bearbeiter';
export const SECTION_AUTOMATION = 'Automatik';

/** Kopfzeile „Ablauf“ zugeklappt: „2 Vorgänger · 1 Nachfolger“. */
export function flowSummary(before: number, after: number): string {
  return `${String(before)} Vorgänger · ${String(after)} Nachfolger`;
}

/** Kopfzeile „Auftrag“: „320 Zeichen · 1 Änderung durch Vorgänger“. */
export function briefSummary(length: number, changes: number): string {
  const word: string = changes === 1 ? 'Änderung' : 'Änderungen';
  return `${String(length)} Zeichen · ${String(changes)} ${word} durch Vorgänger`;
}

export const PURPOSE_LEAD = 'Zweck:';
export const PURPOSE_BY_USER = 'Aufgabe für dich';
export const PURPOSE_BY_USER_TEXT =
  '– du bekommst die Anweisung unten und meldest „Erledigt“; erst dann geht es weiter.';
export const PURPOSE_NO_TYPE = 'ohne Typ';
export const PURPOSE_NO_TYPE_TEXT = '– keine Rolle, nur Modell und Auftrag der Station.';

export const WAITS_ON_LABEL = 'Wartet auf';
export const SUCCESSORS_LABEL = 'Nachfolger';
export const NO_PREDECESSOR_CHIP = 'keine – Startstation';
export const NO_SUCCESSOR_CHIP = 'keiner – Ende des Plans';
export const ADD_PREDECESSOR_LABEL = '+ Vorgänger';
export const ADD_SUCCESSOR_LABEL = '+ Nachfolger';
export const REMOVE_EDGE_LABEL = '×';
export const REMOVE_EDGE_TITLE = 'Kante lösen';
export const LOCKED_EDGE_TITLE = 'Gestartet: Kanten gesperrt';
export const STARTED_EDGE_REASON = 'schon gestartet – Kanten gesperrt';
export const SELF_EDGE_REASON = 'eine Station kann nicht auf sich selbst warten';
export const NO_OTHER_STATION = 'Keine weitere Station.';
export const FLOW_HINT =
  'Der Nachfolger ist abgeleitet: „+ Nachfolger“ lässt die gewählte Station auf diese warten, „×“ löst die Kante.';

/** „Zyklus: A → B → A“ mit den Namen der Stationen. */
export function cycleText(names: readonly string[]): string {
  return `Zyklus: ${names.join(' → ')}`;
}

export const BRIEF_LABEL = 'Auftrag';
export const INSTRUCTION_LABEL = 'Anweisung an dich';

/** „120/1000 Zeichen · bis zum Start änderbar“. */
export function briefCounter(length: number, max: number, isOpen: boolean): string {
  return `${String(length)}/${String(max)} Zeichen · ${isOpen ? 'bis zum Start änderbar' : 'gesendet, festgeschrieben 🔒'}`;
}

export const PREDECESSORS_MAY_LABEL = 'Vorgänger dürfen';
export const PREDECESSORS_MAY_OPTIONS: readonly { id: PredecessorsMay; label: string }[] = [
  { id: 'append', label: 'nur ergänzen' },
  { id: 'appendAndChange', label: 'ergänzen und ändern' },
  { id: 'handsOff', label: 'nicht anfassen' },
];
export const NO_BRIEF_CHANGES = 'Bisher keine Änderung durch Vorgänger.';

/** „Ergänzt von Review Core #6 (Vorgänger)“. */
export function briefChangeLead(kind: BriefChangeKind, source: string): string {
  return `${kind === 'replace' ? 'Geändert' : 'Ergänzt'} von ${source} (Vorgänger)`;
}

export const BRIEF_CHANGE_BEFORE = 'vorher:';

export function briefChangeAction(kind: BriefChangeKind): string {
  return kind === 'replace' ? 'Zurücksetzen' : 'Entfernen';
}

export const RESULT_LABEL = 'Ergebnis';
export const RESULT_PLACEHOLDER = 'Eine Zeile, geht an die Nachfolger';

export const WORKER_LABEL = 'Bearbeiter';
export const WORKER_AGENT = 'Agent';
export const WORKER_USER = 'durch dich';
export const WORKER_HINT_USER =
  'Eine Station durch dich hat keinen Typ, kein Modell und keinen System-Prompt.';
export const WORKER_FIXED_TITLE =
  'Wer die Station bearbeitet, steht im Plan fest; ändern lässt es sich nur durch eine neue Station';
export const TYPE_LABEL = 'Typ';
export const MODEL_LABEL = 'Modell';
export const EFFORT_LABEL = 'Denkaufwand';
export const SKILL_LABEL = 'Skill';
export const SKILL_PLACEHOLDER = 'z. B. implement';
export const SYSTEM_PROMPT_LABEL = 'System-Prompt';
export const NO_SYSTEM_PROMPT = '(kein Typ – kein Rollentext)';
export const EMPTY_SYSTEM_PROMPT = '(der Typ hat keinen Rollentext)';
export const ADDITION_LABEL = 'Zusatz';
export const ADDITION_PLACEHOLDER = 'Optional: Ergänzung zum Rollentext nur für diese Station';
export const SYSTEM_PROMPT_HINT =
  'Der Rollentext kommt aus dem Agententyp (Einstellungen) und wird beim Start festgeschrieben.';

/** „Standard: Opus“; ohne Wert „Standard: wie die Session“. */
export function standardOption(text: string | null): string {
  return `Standard: ${text ?? 'wie die Session'}`;
}

/** „Läuft mit Opus 5.5 · Max (Modell: Typ, Denkaufwand: Station)“; gestartet „Die Session läuft mit …“. */
export function runsWithText(
  model: { value: ModelId; source: ValueSource | null } | null,
  effort: { value: Effort; source: ValueSource | null } | null,
  state: 'next' | 'running' | 'done',
): string {
  const lead: Record<typeof state, string> = {
    next: 'Läuft mit',
    running: 'Die Session läuft mit',
    done: 'Die Session lief mit',
  };
  const modelText: string = model === null ? 'Modell wie die Session' : modelName(model.value);
  const effortText: string =
    effort === null ? 'Denkaufwand wie die Session' : effortLabel(effort.value);
  const sources: string[] = [];
  if (model?.source != null) {
    sources.push(`Modell: ${SOURCE_WORD[model.source]}`);
  }
  if (effort?.source != null) {
    sources.push(`Denkaufwand: ${SOURCE_WORD[effort.source]}`);
  }
  const suffix: string = sources.length === 0 ? '' : ` (${sources.join(', ')})`;
  return `${lead[state]} ${modelText} · ${effortText}${suffix}`;
}

export const AUTO_START_STATION_LABEL = 'Automatisch starten';

/** Schaltergruppe „Automatisch starten“ einer Station; die erste Wahl nennt den Schalter der Leiste. */
export function autoStartOptions(
  isImplementationAutomatic: boolean,
): readonly { id: boolean | null; label: string }[] {
  return [
    { id: null, label: `wie Umsetzung (${isImplementationAutomatic ? 'an' : 'aus'})` },
    { id: true, label: 'an' },
    { id: false, label: 'aus' },
  ];
}

/** „Jetzt: von selbst“ oder „Jetzt: auf ▶“. */
export function autoStartHint(isAutomatic: boolean): string {
  return `Startet die Station nach ihren Vorgängern von selbst (Countdown) oder erst auf ▶. Jetzt: ${isAutomatic ? 'von selbst' : 'auf ▶'}.`;
}

export const ON_ERROR_LABEL = 'Bei Fehler';
export const ON_ERROR_OPTIONS: readonly { id: OnError; label: string; short: string }[] = [
  { id: 'halt', label: 'anhalten', short: 'anhalten' },
  { id: 'restartOnce', label: '1× neu starten', short: '1×' },
  { id: 'restartUpToThree', label: 'bis 3× neu starten', short: 'bis 3×' },
];
export const ON_ERROR_HINT = 'Gilt bis die Station fertig ist; auch während sie läuft.';
export const BY_USER_AUTOMATION_HINT =
  'Eine Station durch dich startet nie von selbst: sie wartet auf dich, sobald ihre Vorgänger fertig sind.';

/** Kopfzeile „Automatik“: „starten: wie Umsetzung · Fehler: anhalten“. */
export function automationSummary(autoStart: boolean | null, onError: OnError): string {
  const start: string = autoStart === null ? 'wie Umsetzung' : autoStart ? 'an' : 'aus';
  const error: string =
    ON_ERROR_OPTIONS.find((option: { id: OnError }) => option.id === onError)?.short ?? onError;
  return `starten: ${start} · Fehler: ${error}`;
}

export const AUTOMATION_SUMMARY_BY_USER = 'wartet auf dich';

/** Sperren nach Zustand (Entwurf, „Was sich wann ändern lässt“). */
export function lockInfo(phase: 'open' | 'started' | 'done'): string {
  switch (phase) {
    case 'open':
      return 'Alles lässt sich ändern, bis die Station startet.';
    case 'started':
      return 'Gestartet: Auftrag, Typ, Modell, Denkaufwand und System-Prompt sind festgeschrieben 🔒 (ein Modellwechsel in der Eingabeleiste der Session bleibt möglich). Offen bleibt „Bei Fehler“.';
    case 'done':
      return 'Fertig: nur noch Ansicht, mit den Werten, mit denen die Station lief.';
  }
}

// ---- Neue Lane („+ Lane“) ----

export const NEW_LANE_TITLE_HEAD = 'Neue Lane';

/** „Schlüssel 4 (automatisch)“. */
export function laneKeyHint(key: string): string {
  return `Schlüssel ${key} (automatisch)`;
}

export const LANE_TITLE_LABEL = 'Titel';
export const LANE_TITLE_PLACEHOLDER = 'z. B. Doku';

/** „Station 1“ im Formular. */
export function newStationHead(index: number): string {
  return `Station ${String(index + 1)}`;
}

export const STATION_NAME_LABEL = 'Name der Station';
export const REMOVE_STATION_LABEL = 'entfernen';
export const ADD_STATION_LABEL = 'Station hinzufügen';
export const NO_TYPE_OPTION = 'ohne Typ';
export const BY_USER_OPTION = 'durch dich';
export const BY_USER_STAGE = 'Handtest';
export const BY_USER_BRIEF =
  'Beschreibe, was du tun sollst und woran du merkst, dass es gelungen ist.';
export const CHAIN_LABEL = 'nacheinander (jede Station wartet auf die vorige)';
export const NEW_LANE_WAITS_HINT = 'Leer = Startstation: darf sofort starten.';
export const NEW_LANE_AFTER_LABEL = 'Danach';
export const NEW_LANE_AFTER_HINT =
  'Die gewählten Stationen warten dann auch auf diese Lane. Nur Stationen, die noch nicht gestartet sind.';
export const NEW_LANE_LATER_HINT =
  'Automatik, System-Prompt und Skill stellst du nach dem Anlegen in der Kartei ein; bis dahin gelten die Standardwerte.';
export const CREATE_LANE_LABEL = 'Anlegen';
export const CANCEL_LANE_LABEL = 'Abbrechen';
export const MISSING_TITLE = 'Der Titel der Lane fehlt.';
export const NO_STATIONS = 'Mindestens eine Station.';
export const MISSING_STAGE = 'Eine Station hat keinen Namen.';

export function duplicateStage(name: string): string {
  return `Name schon vergeben: ${name}.`;
}

export function startedSuccessor(name: string): string {
  return `${name} ist schon gestartet und kann nicht warten.`;
}

export const NEW_LANE_NAME = 'neue Lane';

/** Vorschau: „Lane 4: Umsetzung → Stufe 3, Review → Stufe 4.“ */
export function previewPlaces(
  key: string,
  places: readonly { name: string; level: number }[],
  isStart: boolean,
): string {
  const where: string = places
    .map((place: { name: string; level: number }) => `${place.name} → Stufe ${String(place.level)}`)
    .join(', ');
  return `Lane ${key}: ${where}.${isStart ? ' Startstation: darf sofort starten (▶ oder Automatik).' : ''}`;
}

/** „Verschiebt: Gesamt-Review (Stufe 4 → 5), PR (Stufe 5 → 6).“ */
export function previewMoved(moved: readonly { name: string; from: number; to: number }[]): string {
  const shown: string = moved
    .slice(0, 4)
    .map(
      (item: { name: string; from: number; to: number }) =>
        `${item.name} (Stufe ${String(item.from)} → ${String(item.to)})`,
    )
    .join(', ');
  return `Verschiebt: ${shown}${moved.length > 4 ? ' …' : ''}.`;
}

// ---- Plan ändern? mit änderbaren Werten ----

/** „Session #6 schlägt vor:“. */
export function proposalLead(sessionNumber: number): string {
  return `Session #${String(sessionNumber)} schlägt vor:`;
}

/** „Neue Station Doku in Lane 4“. */
export function proposedStationHead(stage: string, lane: string): string {
  return `${stage} · Lane ${lane}`;
}

/** „Vorschlag war: Sonnet“. */
export function proposalWas(text: string): string {
  return `Vorschlag war: ${text}`;
}

export const PROPOSAL_STANDARD = 'Standard';
export const PROPOSAL_EMPTY_BRIEF = '(leer)';
export const PROPOSAL_HINT =
  'Du kannst den Vorschlag übernehmen, wie er ist, oder Typ, Modell, Denkaufwand und Auftrag vorher ändern. Alles Weitere (System-Prompt, Automatik) steht nach dem Übernehmen in der Kartei.';
export const CHANGED_LABEL = 'Geändert';
export const MAX_PARALLEL_CHANGE_LABEL = 'Parallel max.';

/** Obergrenze des Auftrags im Core (`MAX_BRIEF_CHARS`). */
export const MAX_BRIEF_CHARS = 1000;
export const BRIEF_EDIT_DROPS_CHANGES =
  'Ein geänderter Auftrag verwirft die Änderungen durch Vorgänger.';
