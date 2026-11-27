import { isStartFailure, stationDisplay } from '@/features/implementation/commandStation';
import type { StationRef } from '@/features/implementation/implementationOrder';
import { clockText, liveText } from '@/features/implementation/stationLive';
import { stationLabel as stateNameOf } from '@/features/implementation/stationState';
import type { BriefChangeKind } from '@/lib/bindings/BriefChangeKind';
import type { Effort } from '@/lib/bindings/Effort';
import type { ImplementationView } from '@/lib/bindings/ImplementationView';
import type { ModelId } from '@/lib/bindings/ModelId';
import type { OnError } from '@/lib/bindings/OnError';
import type { PhaseMark } from '@/lib/bindings/PhaseMark';
import type { PredecessorsMay } from '@/lib/bindings/PredecessorsMay';
import type { StationPatch } from '@/lib/bindings/StationPatch';
import type { StationView } from '@/lib/bindings/StationView';
import { effortLabel, modelName } from '@/lib/labels';
import { QUOTA_WAIT_TEXT, readyStationText } from '@/features/queue/queueTexts';
import type { QuotaWaiting } from '@/features/implementation/quotaWaiting';
import type { ValueSource } from '@/features/implementation/stationAgent';
import type { ImplementationViewKind } from '@/stores/sessions';

export const VIEW_LABEL: Record<ImplementationViewKind, string> = {
  flow: 'Ablauf',
  changes: 'Changes',
  artifacts: 'Artefakte',
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
/** Marke „Plan ändern?“ an der Zeile der Umsetzung in der Leiste (#132). */
export const PLAN_CHANGE_MARK = '✎ Plan?';
export const PLAN_CHANGE_MARK_LABEL = 'Plan ändern? Vorschlag ansehen';
export const PLAN_CHANGE_MARK_TITLE = 'Plan ändern? Zeigt den Vorschlag';
export const START_IMPLEMENTATION_LABEL = '▶ Umsetzung starten';
export const START_IMPLEMENTATION_TITLE =
  'Schaltet „Automatisch starten“ ein und startet die bereiten Stationen bis zur Grenze „Gleichzeitige Agenten max.“; der Rest wartet in der Warteschlange.';
export const AUTO_RESUME_LABEL = 'Automatisch fortsetzen';
export const AUTO_RESUME_GLYPH = '↻';
export const AUTO_RESUME_TITLE =
  'Setzt jede Station nach leerem Kontingent 30 Sekunden nach dem Zurücksetzen selbst fort. Schaltet alle Stationen, die noch nicht fertig sind; ↻ in der Zeile oder die Kartei schalten nur eine (die letzte Aktion gewinnt).';

function stations(count: number, one: string, many: string): string {
  return `${String(count)} ${count === 1 ? one : many}`;
}

/** Warnleiste: „Kontingent aufgebraucht.“ mit den Zahlen dahinter, nach der Rücksetzzeit „Kontingent
 *  zurückgesetzt.“ mit denen, die noch warten (Entwurf 2026-11-15). */
export function quotaBarText(waiting: QuotaWaiting): { headline: string; detail: string } {
  if (waiting.isReset) {
    const rest: number = waiting.waiting - waiting.resuming;
    const parts: string[] = [];
    if (waiting.resuming > 0) {
      parts.push(`${stations(waiting.resuming, 'Station setzt', 'Stationen setzen')} gleich fort`);
    }
    if (rest > 0) {
      parts.push(`${stations(rest, 'wartet', 'warten')} noch auf dich`);
    }
    return { headline: 'Kontingent zurückgesetzt.', detail: `${parts.join(' · ')}.` };
  }
  const parts: string[] = [stations(waiting.waiting, 'Station wartet', 'Stationen warten')];
  if (waiting.resuming > 0) {
    parts.push(
      `${stations(waiting.resuming, 'setzt', 'setzen')} ${waiting.resumesAt === null ? 'gleich' : `um ${clockText(waiting.resumesAt)} automatisch`} fort`,
    );
  }
  if (waiting.waitsForYou > 0) {
    parts.push(`${stations(waiting.waitsForYou, 'wartet', 'warten')} auf dich (Schalter aus)`);
  }
  if (waiting.unknownReset > 0) {
    parts.push(
      `${String(waiting.unknownReset)} ohne bekannte Rücksetzzeit (${waiting.unknownReset === 1 ? 'setzt' : 'setzen'} nicht fort)`,
    );
  }
  return { headline: 'Kontingent aufgebraucht.', detail: `${parts.join(' · ')}.` };
}

export const NEW_LANE_LABEL = '+ Lane';
export const NEW_LANE_TITLE = 'Neue Lane mit Stationen und Kanten anlegen';
export const STOP_AFTER_LABEL = 'Anhalten nach dieser Session';
export const STOP_AFTER_TITLE = 'Die laufende Station läuft zu Ende, danach startet nichts mehr';
export const BACK_TO_CURRENT_LABEL = 'Zur aktuellen';
export const BACK_TO_CURRENT_TITLE = 'Springt zurück zur Station, die gerade läuft oder fragt';
export const OPEN_CHAT_LABEL = 'Chat öffnen';
export const OPEN_CHAT_TITLE = 'Öffnet die Session dieser Station; Fragen beantwortest du dort';
export const COMMAND_FAILED_LEAD = 'Fehler.';
export const PURPOSE_COMMAND = 'Befehl des Verwalters';
export const PURPOSE_COMMAND_TEXT =
  '– führt die Schritte nacheinander aus, ohne Agent und ohne Modell.';
export const PURPOSE_MESSAGE = 'Nachricht des Verwalters';
export const PURPOSE_MESSAGE_TEXT =
  '– füllt die Vorlage und stellt sie zu, ohne Agent und ohne Modell; die Station ist danach fertig.';
export const START_NOW_MESSAGE_TITLE = 'Stellt die Nachricht dieser Station sofort zu';
export const RESTART_MESSAGE_TITLE = 'Stellt die Nachricht noch einmal zu';
export const START_NOW_LABEL = '▶ Jetzt starten';
export const START_NOW_TITLE = 'Startet diese Station sofort als neue Session';
export const RESTART_COMMAND_TITLE = 'Startet die Schritte von vorn';
export const START_NOW_COMMAND_TITLE = 'Startet die Befehle dieser Station sofort';
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
/** Marke im Kopf der Karte „Plan ändern?“ (#132). */
export const PLAN_PROPOSAL_BADGE = 'wartet auf dich';
export const PLAN_CHANGED_TITLE = 'Plan geändert';
export const APPLY_CHANGE_LABEL = 'Übernehmen';
export const APPLY_CHANGE_TITLE = 'Übernimmt den Vorschlag mit den Werten, wie sie hier stehen';
export const DISCARD_CHANGE_LABEL = 'Verwerfen';
export const DISCARD_CHANGE_TITLE = 'Lässt den Plan, wie er ist';
export const CANCEL_LABEL = 'Abbrechen…';
export const CANCEL_TITLE =
  'Beendet die Umsetzung und stoppt laufende Stationen; die Sessions bleiben, eine neue lässt sich anlegen';
export const DELETE_LABEL = 'Löschen…';
export const DELETE_TITLE =
  'Entfernt die Umsetzung samt Lanes, Stationen und Verlauf; Sessions und Worktrees bleiben';
export const CONFIRM_CANCEL_LABEL = 'Abbrechen';
export const CONFIRM_DELETE_LABEL = 'Löschen';
export const CONFIRM_BACK_LABEL = 'Zurück';

/** Rückfrage in der Leiste vor dem Abbrechen (ADR 066). */
export function cancelQuestion(name: string): string {
  return `Umsetzung „${name}“ abbrechen? Laufende Stationen werden gestoppt, die Sessions bleiben, danach lässt sich eine neue Umsetzung anlegen.`;
}

/** Rückfrage in der Leiste vor dem Löschen (ADR 066). */
export function deleteQuestion(name: string): string {
  return `Umsetzung „${name}“ endgültig löschen? Lanes, Stationen und Verlauf gehen verloren, Sessions und Worktrees bleiben.`;
}

/** „Abgebrochen am 09.11. 14:05“ statt der Schalter in der Leiste. */
export function cancelledText(at: number): string {
  return `Abgebrochen am ${stampText(at)}`;
}

/** Warum „Löschen…“ gesperrt ist, solange eine Station arbeitet (wie im Core, ADR 066). */
export function deleteBlockedText(station: StationView): string {
  return `Station „${stationDisplay(station)}“ läuft noch. Erst stoppen oder abwarten, dann löschen.`;
}

export const CANCEL_REQUEST_TITLE = 'Umsetzung abbrechen?';
export const DELETE_REQUEST_TITLE = 'Umsetzung löschen?';
export const CANCEL_REQUEST_REASON = 'Grund';
export const CANCEL_REQUEST_HINT =
  'Bis du bestätigst oder verwirfst, startet keine Station. Bestätigen stoppt laufende Stationen; Sessions und Worktrees bleiben.';
export const CONFIRM_REQUEST_LABEL = 'Bestätigen';
export const CONFIRM_REQUEST_TITLE = 'Führt die Anfrage aus';
export const DISCARD_REQUEST_LABEL = 'Verwerfen';
export const DISCARD_REQUEST_TITLE =
  'Lässt die Umsetzung, wie sie ist, und plant die Stationen neu';
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

export const TRAIL_BACK_TITLE = 'Zurück zur Umsetzung';

/** „Vorige aktive Station: C-5 · Review · läuft“ am ‹ bzw. › der Rückweg-Leiste; ohne Ziel „Keine vorige aktive Station“. */
export function trailTargetTitle(
  direction: 'previous' | 'next',
  target: StationRef | undefined,
): string {
  const noun: string = direction === 'previous' ? 'vorige' : 'nächste';
  if (target === undefined) {
    return `Keine ${noun} aktive Station`;
  }
  const lead: string = direction === 'previous' ? 'Vorige' : 'Nächste';
  return `${lead} aktive Station: ${stationDisplay(target.station)} · ${stateNameOf(target.station)}`;
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
    case 'waitsForQuota':
      return 'setzt automatisch fort';
    case 'asking':
      return 'fragt dich';
    case 'waitsForYou':
      return 'wartet auf dich';
    case 'reported':
      return 'gemeldet, der Master entscheidet';
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
  if (isStartFailure(station)) {
    return START_FAILED_LABEL;
  }
  const live: string | null = liveText(station);
  if (live !== null) {
    return live;
  }
  switch (station.state) {
    case 'done':
      return durationText(station.runningMs);
    case 'running':
    case 'starting':
      return `läuft ${durationText(runningFor(station, now))}`;
    case 'waitsForScript':
      return 'Skript läuft';
    case 'waitsForQuota':
      return 'setzt automatisch fort';
    case 'asking':
      return 'fragt';
    case 'waitsForYou':
      return 'wartet';
    case 'reported':
      return 'gemeldet';
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
export const ROW_START_TITLE = 'Jetzt starten (ignoriert die Grenze „Gleichzeitige Agenten max.“)';
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

/** Tooltip des ↻ in der Zeile. */
export function rowResumeTitle(station: StationView): string {
  if (station.state === 'done') {
    return 'Fertig: nur Ansicht.';
  }
  const source: string = station.autoResume === null ? 'wie die Umsetzung' : 'nur diese Station';
  return `${AUTO_RESUME_LABEL}: ${station.resumesAutomatically ? 'an' : 'aus'} (${source}). Ein Klick schaltet nur diese Station.`;
}

/** Zustand rechts in der Zeile: „läuft · 12 min“, „bereit · Warteschlange 1“, „startet in 7 s“; `isQuotaHeld`:
 *  Die Station wartet wegen des Kontingents („wartet auf Kontingent“). */
export function rowStateText(station: StationView, now: number, isQuotaHeld = false): string {
  if (isStartFailure(station)) {
    return START_FAILED_LABEL;
  }
  const live: string | null = liveText(station);
  if (live !== null) {
    return live;
  }
  switch (station.state) {
    case 'done':
      return `fertig · ${durationText(station.runningMs)}`;
    case 'running':
    case 'starting':
      return `läuft · ${durationText(runningFor(station, now))}`;
    case 'waitsForScript':
      return 'Skript läuft';
    case 'waitsForQuota':
      return 'setzt automatisch fort';
    case 'asking':
      return 'Frage offen';
    case 'waitsForYou':
      return 'wartet auf dich';
    case 'reported':
      return 'gemeldet';
    case 'error':
      if (!station.restarting) {
        return 'Fehler';
      }
      return isQuotaHeld ? `Fehler · ${QUOTA_WAIT_TEXT}` : 'Fehler · startet neu';
    case 'countdown':
      return countdownText(station, now);
    case 'ready':
      return readyStationText(station.queuePosition, isQuotaHeld);
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

/** Grund, den „Speichern“ bzw. „Übernehmen“ als `title` nennt, solange ein Feld über seiner Grenze liegt. */
export function overLimitReason(label: string, max: number): string {
  return `${label} ist länger als ${String(max)} Zeichen.`;
}

/** Zeile unter dem Zähler, solange ein Feld über der Grenze liegt und deshalb nichts gespeichert wird. */
export function notSavedLine(reason: string): string {
  return `Nicht gespeichert: ${reason}`;
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
export const BRIEF_COUNTER_OPEN = 'bis zum Start änderbar';
export const BRIEF_COUNTER_SENT = 'gesendet, festgeschrieben 🔒';

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

/** „Läuft mit: an (von der Umsetzung).“ unter der Wahl „Automatisch fortsetzen“ der Kartei. */
export function autoResumeHint(resumes: boolean, isOwn: boolean): string {
  return `Läuft mit: ${resumes ? 'an' : 'aus'} (${isOwn ? 'nur diese Station' : 'von der Umsetzung'}). Setzt nach leerem Kontingent von selbst fort; änderbar, bis die Station fertig ist, auch während sie läuft.`;
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

function inheritedText(value: boolean | null): string {
  return value === null ? 'wie Umsetzung' : value ? 'an' : 'aus';
}

/** Kopfzeile „Automatik“: „starten: wie Umsetzung · fortsetzen: an · Fehler: anhalten“. */
export function automationSummary(
  autoStart: boolean | null,
  autoResume: boolean | null,
  onError: OnError,
): string {
  const error: string =
    ON_ERROR_OPTIONS.find((option: { id: OnError }) => option.id === onError)?.short ?? onError;
  return `starten: ${inheritedText(autoStart)} · fortsetzen: ${inheritedText(autoResume)} · Fehler: ${error}`;
}

export const AUTOMATION_SUMMARY_BY_USER = 'wartet auf dich';

/** Sperren nach Zustand (Entwurf, „Was sich wann ändern lässt“). */
export function lockInfo(phase: 'open' | 'started' | 'done'): string {
  switch (phase) {
    case 'open':
      return 'Alles lässt sich ändern, bis die Station startet.';
    case 'started':
      return 'Gestartet: Auftrag, Typ, Modell, Denkaufwand und System-Prompt sind festgeschrieben 🔒 (ein Modellwechsel in der Eingabeleiste der Session bleibt möglich). Offen bleiben „Automatisch fortsetzen“ und „Bei Fehler“.';
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

/** Eine neue Station eines Vorschlags, noch ohne Kennung: „Doku · Lane 4“. */
export function proposedStationHead(stage: string, lane: string): string {
  return `${stage} · Lane ${lane}`;
}

/** „Vorschlag war: Sonnet“. */
export function proposalWas(text: string): string {
  return `Vorschlag war: ${text}`;
}

export const PROPOSAL_STANDARD = 'Standard';

/** „Ändert: Wartet auf → c2, u1 · Modell · Auftrag“: was eine Änderung an einer bestehenden Station setzt. */
export function patchSetsText(patch: StationPatch): string {
  const parts: string[] = [];
  if (patch.stage !== undefined) {
    parts.push(`Name → ${patch.stage}`);
  }
  if (patch.after !== undefined) {
    parts.push(`${WAITS_ON_LABEL} → ${keysText(patch.after)}`);
  }
  if (patch.priority !== undefined) {
    parts.push(`Priorität → ${String(patch.priority)}`);
  }
  if (patch.agent !== undefined) {
    parts.push(TYPE_LABEL);
  }
  if (patch.model !== undefined) {
    parts.push(MODEL_LABEL);
  }
  if (patch.effort !== undefined) {
    parts.push(EFFORT_LABEL);
  }
  if (patch.brief !== undefined) {
    parts.push(BRIEF_LABEL);
  }
  if (patch.prompt !== undefined) {
    parts.push(ADDITION_LABEL);
  }
  if (patch.skill !== undefined) {
    parts.push(`${SKILL_LABEL} → ${patch.skill}`);
  }
  if (patch.command !== undefined) {
    parts.push('Befehl');
  }
  if (patch.cwd !== undefined) {
    parts.push('Ordner');
  }
  return `Ändert: ${parts.length === 0 ? 'nichts' : parts.join(' · ')}`;
}
/** Was eine neue Station im Plan tut: ohne Vorgänger startet sie sofort, auch neben der Station davor. */
export function proposedWaitsText(after: readonly string[]): string {
  return after.length === 0
    ? 'Startstation: beginnt sofort, auch neben anderen'
    : `${WAITS_ON_LABEL}: ${keysText(after)}`;
}
export const PROPOSAL_EMPTY_BRIEF = '(leer)';
export const PROPOSAL_HINT =
  'Du kannst den Vorschlag übernehmen, wie er ist, oder Typ, Modell, Denkaufwand und Auftrag vorher ändern. Alles Weitere (System-Prompt, Automatik) steht nach dem Übernehmen in der Kartei.';
export const CHANGED_LABEL = 'Geändert';

/** Obergrenze des Auftrags im Core (`MAX_BRIEF_CHARS`). */
export const MAX_BRIEF_CHARS = 1000;
export const BRIEF_EDIT_DROPS_CHANGES =
  'Ein geänderter Auftrag verwirft die Änderungen durch Vorgänger.';

// ---- Fehlermeldungen der Befehle ----

export const START_FAILED = 'Start fehlgeschlagen';
export const APPLY_FAILED = 'Übernehmen fehlgeschlagen';
/** Bestätigen mit Löschen: abgebrochen ist schon, gelöscht noch nicht (ADR 066). */
export const CANCELLED_NOT_DELETED = 'Abgebrochen, Löschen noch nicht möglich';
export const NOT_SAVED = 'Nicht gespeichert';
export const BRIEF_NOT_SAVED = 'Auftrag nicht gespeichert';
export const NOT_REMOVED = 'Nicht entfernt';
export const EDGE_NOT_CHANGED = 'Kante nicht geändert';
export const LANE_NOT_CREATED = 'Lane nicht angelegt';

/** Der Vorschlag ist übernommen, aber nicht jede Änderung des Benutzers daran ging durch. */
export function proposalAppliedWithFailures(failures: readonly string[]): string {
  return `Der Vorschlag ist übernommen, aber nicht alle deine Änderungen: ${failures.join('; ')}`;
}

/** Eine Station des übernommenen Vorschlags fehlt im Plan danach. */
export function stationNotFound(stage: string, lane: string): string {
  return `Station „${stage}“ in Lane ${lane} nicht gefunden`;
}

// „Als fertig markieren“ (ADR 048).
export const MARK_DONE_BUTTON = 'Als fertig markieren …';
export const MARK_DONE_TITLE =
  'Die Station gilt als erledigt; die Nachfolger starten wie nach einer Annahme';
export const MARK_DONE_CONFIRM = 'Als fertig markieren';
export const MARK_DONE_HEADING = 'Station als fertig markieren?';
export const MARK_DONE_CANCEL = 'Abbrechen';
export const MARK_DONE_TEXT =
  'gilt dann als erledigt. Die Nachfolger starten wie nach einer Annahme; eine Fix-Runde gibt es nicht. Die Session bleibt, wie sie ist.';
/** Eine Befehls-Station hat keine Session (ADR 073). */
export const MARK_DONE_TEXT_COMMAND =
  'gilt dann als erledigt. Die Nachfolger starten wie nach einer Annahme; eine Fix-Runde gibt es nicht.';
