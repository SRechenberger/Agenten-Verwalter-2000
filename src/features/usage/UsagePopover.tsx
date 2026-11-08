import { useState } from 'react';
import type { ReactElement } from 'react';
import { Popover } from '@/components/Popover';
import { SeverityIcon } from '@/components/SeverityIcon';
import { formatClock, formatPercent } from '@/features/context/formatTokens';
import { behaviorText, formatResetAt, limitLabel } from '@/features/usage/usageTexts';
import type { UsageBreakdown } from '@/lib/bindings/UsageBreakdown';
import type { UsageLimit } from '@/lib/bindings/UsageLimit';
import type { UsageShare } from '@/lib/bindings/UsageShare';
import type { UsageSnapshot } from '@/lib/bindings/UsageSnapshot';
import type { UsageStatus } from '@/lib/bindings/UsageStatus';
import { planLabel } from '@/lib/labels';
import { severityOf } from '@/lib/severity';
import type { Severity } from '@/lib/severity';
import type { UsageSource } from '@/lib/usage';
import './UsagePopover.css';

const USAGE_POPOVER_WIDTH = 380;
const FULL_PERCENT = 100;

const NUMBER_FORMAT = new Intl.NumberFormat('de-DE');

const PERIODS = ['day', 'week'] as const;
type Period = (typeof PERIODS)[number];
const PERIOD_LABEL: Readonly<Record<Period, string>> = { day: 'Tag', week: 'Woche' };

const SKILL_EXPLANATION =
  'Aufwendige Skills lassen sich eingrenzen oder per Frontmatter auf ein günstigeres Modell legen.';
const NO_LIMITS_TEXT =
  'Für dieses Konto meldet Claude kein Kontingent (zum Beispiel bei Anmeldung per API-Schlüssel).';
const NO_CODEX_LIMITS_TEXT = 'Für dieses Konto meldet Codex kein Kontingent.';

interface UsagePopoverProps {
  status: UsageStatus | null;
  /** Woher das Kontingent kommt; das Fenster fragt dort neu an. */
  source: UsageSource;
  /** Der Host, dessen Kontingent das Fenster zeigt; `null` beim eigenen. */
  hostName: string | null;
  /** Ob das Abo zu Codex gehört (nur der Text ohne Kontingent ändert sich). */
  isCodex: boolean;
  onClose: () => void;
}

export function UsagePopover({
  status,
  source,
  hostName,
  isCodex,
  onClose,
}: UsagePopoverProps): ReactElement {
  const [period, setPeriod] = useState<Period>('day');
  // Das Fenster wird bei jedem Öffnen neu eingehängt; so bleibt der Reset-Zeitpunkt beim Rendern stabil.
  const [now] = useState<number>(() => Date.now());
  const snapshot: UsageSnapshot | null = status?.snapshot ?? null;
  const hasBreakdown: boolean =
    snapshot !== null && (snapshot.day !== null || snapshot.week !== null);

  function refresh(): void {
    source.refresh(true).catch((reason: unknown) => {
      console.error('Kontingent nicht abrufbar', reason);
    });
  }

  function renderLimits(loaded: UsageSnapshot): ReactElement {
    if (loaded.limits.length === 0) {
      return (
        <p className="usage-popover__hint">{isCodex ? NO_CODEX_LIMITS_TEXT : NO_LIMITS_TEXT}</p>
      );
    }
    return (
      <ul className="usage-popover__limits">
        {loaded.limits.map((limit: UsageLimit) => renderLimit(limit, now))}
      </ul>
    );
  }

  function renderBody(): ReactElement | null {
    if (snapshot === null) {
      return null;
    }
    return (
      <>
        {renderLimits(snapshot)}
        {hasBreakdown && (
          <section className="usage-popover__drivers">
            <div className="usage-popover__drivers-head">
              <h3 className="usage-popover__subtitle">Was treibt den Verbrauch?</h3>
              <div className="usage-popover__periods">
                {PERIODS.map((candidate: Period) => (
                  <button
                    key={candidate}
                    type="button"
                    className="usage-popover__period"
                    aria-pressed={candidate === period}
                    onClick={(): void => {
                      setPeriod(candidate);
                    }}
                  >
                    {PERIOD_LABEL[candidate]}
                  </button>
                ))}
              </div>
            </div>
            <p className="usage-popover__note">
              Näherung aus den Sessions auf {hostName === null ? 'diesem Rechner' : hostName} — ohne
              andere Geräte und claude.ai
            </p>
            {renderDrivers(snapshot[period])}
          </section>
        )}
      </>
    );
  }

  return (
    <Popover
      label="Kontingent"
      placement="above"
      align="start"
      width={USAGE_POPOVER_WIDTH}
      onClose={onClose}
    >
      <div className="usage-popover">
        <div className="usage-popover__head">
          <h2 className="usage-popover__title">
            Kontingent
            {hostName !== null && <span className="usage-popover__host"> · {hostName}</span>}
          </h2>
          {snapshot?.plan != null && (
            <span className="usage-popover__plan">Abo: {planLabel(snapshot.plan)}</span>
          )}
        </div>
        {renderBody()}
        {status?.error != null && (
          <p className="usage-popover__error">Kontingent nicht abrufbar: {status.error}</p>
        )}
        <div className="usage-popover__foot">
          <span>{snapshot !== null && `Stand ${formatClock(snapshot.fetchedAt)}`}</span>
          <button
            type="button"
            className="usage-popover__refresh"
            disabled={status?.isLoading === true}
            onClick={refresh}
          >
            {status?.isLoading === true ? 'Lädt …' : 'Aktualisieren'}
          </button>
        </div>
      </div>
    </Popover>
  );
}

function renderLimit(limit: UsageLimit, now: number): ReactElement {
  const resetAt: string | null =
    limit.resetsAt === null ? null : formatResetAt(limit.resetsAt, now);
  const severity: Severity = severityOf(limit.percent);
  return (
    <li key={limit.kind} className={`usage-popover__limit severity severity--${severity}`}>
      <div className="usage-popover__limit-head">
        <span className="usage-popover__limit-name">
          <SeverityIcon severity={severity} />
          {limitLabel(limit.kind, limit.windowMinutes)}
        </span>
        <span className="usage-popover__percent">{formatPercent(limit.percent, 0)}</span>
      </div>
      <div className="usage-popover__bar">
        <span
          className="usage-popover__fill"
          style={{ width: `${String(Math.min(limit.percent, FULL_PERCENT))}%` }}
        />
      </div>
      {resetAt !== null && <span className="usage-popover__reset">Zurückgesetzt {resetAt}</span>}
    </li>
  );
}

function renderDrivers(breakdown: UsageBreakdown | null): ReactElement {
  if (breakdown === null) {
    return <p className="usage-popover__hint">Dafür liegen keine Daten vor.</p>;
  }
  const topSkill: UsageShare | undefined = breakdown.skills[0];
  return (
    <>
      {breakdown.behaviors.map((behavior: UsageShare) => {
        const text = behaviorText(behavior.key, behavior.percent);
        return (
          <div key={behavior.key} className="usage-popover__driver">
            <strong>{text.headline}</strong>
            {text.explanation !== null && (
              <span className="usage-popover__explanation">{text.explanation}</span>
            )}
          </div>
        );
      })}
      {topSkill !== undefined && (
        <>
          <div className="usage-popover__driver">
            <strong>
              {formatPercent(topSkill.percent, 0)} kam aus /{topSkill.key}
            </strong>
            <span className="usage-popover__explanation">{SKILL_EXPLANATION}</span>
          </div>
          <table className="usage-popover__skills">
            <thead>
              <tr>
                <th scope="col">Skills</th>
                <th scope="col" className="usage-popover__percent">
                  Anteil
                </th>
              </tr>
            </thead>
            <tbody>
              {breakdown.skills.map((skill: UsageShare) => (
                <tr key={skill.key}>
                  <td>/{skill.key}</td>
                  <td className="usage-popover__percent">{formatPercent(skill.percent, 0)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      )}
      <p className="usage-popover__note">
        {NUMBER_FORMAT.format(breakdown.requestCount)} Anfragen ·{' '}
        {NUMBER_FORMAT.format(breakdown.sessionCount)} Sessions
      </p>
    </>
  );
}
