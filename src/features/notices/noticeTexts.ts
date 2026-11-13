import { formatThousands } from '@/features/context/formatTokens';
import { formatResetAt } from '@/features/usage/usageTexts';
import type { Mode } from '@/lib/bindings/Mode';
import type { ModelId } from '@/lib/bindings/ModelId';
import { modeOption, modelName } from '@/lib/labels';

const TRIGGER_MANUAL = 'manual';
const TRIGGER_AUTO = 'auto';
const DIRECTION_REVERT = 'revert';
const MIN_SHOWN_COST_USD = 0.01;

export const COMPACT_LEAD = 'Verlauf zusammengefasst';
export const COMPACT_CACHE_REBUILD = 'Cache wird neu aufgebaut';
export const SUMMARY_SHOW = 'Zusammenfassung ansehen';
export const SUMMARY_HIDE = 'Zusammenfassung ausblenden';
export const MODEL_SWITCH_LEAD = 'Modell gewechselt:';
export const FALLBACK_LEAD = 'Sicherheitsfilter hat eine Nachricht markiert.';
export const FALLBACK_REVERT_LEAD = 'Zurück beim ursprünglichen Modell.';
export const INFO_LEAD = 'Hinweis von Claude Code:';
export const MODE_CHANGED_LEAD = 'Modus geändert:';
export const QUOTA_LEAD = 'Kontingent aufgebraucht.';
export const QUOTA_RESUME_LABEL = 'Automatisch fortsetzen';
export const QUOTA_RESUME_TITLE =
  'Schickt dem Agenten kurz nach dem Zurücksetzen die Aufforderung weiterzumachen. Gilt nur, solange die App läuft.';
export const REPORT_UPDATED_LEAD = 'Lagebericht aktualisiert angehängt';
export const PLAN_MISSING_LEAD = 'Kein Plan im Umsetzungstool.';
export const PLAN_MISSING_TEXT =
  'Der Agent hat auch nach zwei Erinnerungen keine Umsetzung angelegt.';
export const PLAN_MISSING_SETTLED = 'Erledigt.';
export const PLAN_RETRY_LABEL = 'Erneut';
export const PLAN_RETRY_TITLE = 'Startet einen neuen Planungszug mit demselben Auftrag.';
export const PLAN_CONTINUE_LABEL = 'Trotzdem weiter';
export const PLAN_CONTINUE_TITLE = 'Beendet die Prüfung für diesen Zug.';

/** „Stand 14:05 · Phase 2 · Test fertig“. */
export function reportUpdatedText(at: number, summary: string): string {
  const time: string = new Date(at).toLocaleTimeString('de-DE', {
    hour: '2-digit',
    minute: '2-digit',
  });
  return `· Stand ${time} · ${summary}`;
}
export const COST_TITLE =
  'Rechenwert der Claude-Kommandozeile zum Listenpreis; beim Abo kein echter Betrag.';

const COST_FORMAT = new Intl.NumberFormat('de-DE', {
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
});

/** „von Hand“ bzw. „automatisch“; ein unbekannter oder fehlender Auslöser wird weggelassen. */
export function compactTriggerText(trigger: string | null): string | null {
  if (trigger === TRIGGER_MANUAL) {
    return 'von Hand';
  }
  if (trigger === TRIGGER_AUTO) {
    return 'automatisch';
  }
  return null;
}

/** „vorher 64k Token, jetzt 4k“; ohne die Angaben (Agent „Autark“) `null`. */
export function compactTokensText(
  preTokens: number | null,
  postTokens: number | null,
): string | null {
  if (preTokens === null) {
    return null;
  }
  if (postTokens === null) {
    return `vorher ${formatThousands(preTokens)} Token`;
  }
  return `vorher ${formatThousands(preTokens)} Token, jetzt ${formatThousands(postTokens)}`;
}

export function modelSwitchedText(from: ModelId, to: ModelId, recacheTokens: number): string {
  return `${modelName(from)} → ${modelName(to)}. Die nächste Nachricht schreibt den Cache neu (rund ${formatThousands(recacheTokens)} Token).`;
}

export function isFallbackRevert(direction: string): boolean {
  return direction === DIRECTION_REVERT;
}

export function modelFallbackLead(direction: string): string {
  return isFallbackRevert(direction) ? FALLBACK_REVERT_LEAD : FALLBACK_LEAD;
}

export function modelFallbackText(
  direction: string,
  originalName: string,
  fallbackName: string,
): string {
  if (isFallbackRevert(direction)) {
    return `Claude antwortet wieder mit ${originalName}.`;
  }
  return `Claude antwortet ab hier mit ${fallbackName} statt ${originalName}.`;
}

export function modelFallbackActionLabel(originalName: string): string {
  return `Mit ${originalName} weiter`;
}

export function modeChangedText(from: Mode, to: Mode): string {
  return `von ${modeOption(from).label} auf ${modeOption(to).label}.`;
}

/** „Es setzt sich um 15:30 Uhr zurück.“; `null`, wenn die Zeit unbekannt, unlesbar oder vorbei ist. */
export function quotaResetText(resetsAt: number | null, now: number): string | null {
  // `toISOString` wirft bei einem Wert außerhalb des Datumsbereichs; die Zahl kommt von der Kommandozeile.
  if (resetsAt === null || Number.isNaN(new Date(resetsAt).getTime())) {
    return null;
  }
  const resetText: string | null = formatResetAt(new Date(resetsAt).toISOString(), now);
  return resetText === null ? null : `Es setzt sich ${resetText} zurück.`;
}

export function deniedCallsText(count: number): string {
  return count === 1 ? '1 Aufruf abgelehnt' : `${String(count)} Aufrufe abgelehnt`;
}

export function unsourcedText(count: number): string {
  return count === 1 ? '1 Angabe ohne Fundstelle' : `${String(count)} Angaben ohne Fundstelle`;
}

/** Fehlt ein markierter Text im geladenen Verlauf, lässt sich die Zahl der Angaben nicht summieren. */
export function unsourcedPartsText(count: number): string {
  return count === 1
    ? '1 Antwortteil mit Angaben ohne Fundstelle'
    : `${String(count)} Antwortteile mit Angaben ohne Fundstelle`;
}

/** „≈ 0,08 $ Rechenwert“; unter einem Cent „< 0,01 $ Rechenwert“. */
export function costText(costUsd: number): string {
  if (costUsd < MIN_SHOWN_COST_USD) {
    return `< ${COST_FORMAT.format(MIN_SHOWN_COST_USD)} $ Rechenwert`;
  }
  return `≈ ${COST_FORMAT.format(costUsd)} $ Rechenwert`;
}
