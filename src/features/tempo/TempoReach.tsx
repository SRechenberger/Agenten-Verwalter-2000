import { useMemo, useState } from 'react';
import type { ReactElement } from 'react';
import { useUsage } from '@/features/usage/useUsage';
import { limitLabel } from '@/features/usage/usageTexts';
import { forecastText, formatPerHour } from '@/features/tempo/tempoTexts';
import type { UsageForecast } from '@/lib/bindings/UsageForecast';
import type { UsageLimit } from '@/lib/bindings/UsageLimit';
import type { UsageStatus } from '@/lib/bindings/UsageStatus';
import { severityOf } from '@/lib/severity';
import { hostUsage, OWN_USAGE } from '@/lib/usage';
import type { UsageSource } from '@/lib/usage';
import './TempoReach.css';

const FULL_PERCENT = 100;
const NOTHING_YET = 'Noch keine Prognose: ein Fenster muss erst 10 min lang gestiegen sein.';

interface TempoReachProps {
  /** Der Host, dessen Abo gilt; `null` beim eigenen. */
  hostId: string | null;
}

/** Die Reichweite je Fenster im Fenster „Tempo“ gesamt: Balken mit gestrichelter Verlängerung bis zum
 *  Stand bei der Rücksetzung (rot über 100 %) und dem Text dazu. */
export function TempoReach({ hostId }: TempoReachProps): ReactElement {
  const source: UsageSource = useMemo(
    () => (hostId === null ? OWN_USAGE : hostUsage(hostId)),
    [hostId],
  );
  const status: UsageStatus | null = useUsage(source, false);
  // Beim Öffnen festgehalten; die Uhrzeiten brauchen keine Sekundengenauigkeit.
  const [now] = useState<number>(() => Date.now());
  const limits: readonly UsageLimit[] = status?.snapshot?.limits ?? [];
  const shown: { forecast: UsageForecast; limit: UsageLimit }[] = [];
  for (const forecast of status?.forecast ?? []) {
    const limit: UsageLimit | undefined = limits.find(
      (candidate: UsageLimit) => candidate.kind === forecast.kind,
    );
    if (limit !== undefined) {
      shown.push({ forecast, limit });
    }
  }

  function renderWindow({
    forecast,
    limit,
  }: {
    forecast: UsageForecast;
    limit: UsageLimit;
  }): ReactElement {
    const atReset: number = forecast.percentAtReset ?? limit.percent;
    const isOver: boolean = atReset > FULL_PERCENT;
    return (
      <li
        key={forecast.kind}
        className={`tempo-reach__window severity severity--${severityOf(limit.percent)}`}
      >
        <div className="tempo-reach__head">
          <span>
            {limitLabel(limit)} · {formatPerHour(forecast.percentPerHour)}
          </span>
          <span>{`${String(Math.round(limit.percent))} %`}</span>
        </div>
        <div className="tempo-reach__bar">
          <span
            className={`tempo-reach__ghost${isOver ? ' tempo-reach__ghost--over' : ''}`}
            style={{
              left: `${String(Math.min(limit.percent, FULL_PERCENT))}%`,
              width: `${String(Math.max(0, Math.min(atReset, FULL_PERCENT) - Math.min(limit.percent, FULL_PERCENT)))}%`,
            }}
          />
          <span
            className="tempo-reach__fill"
            style={{ width: `${String(Math.min(limit.percent, FULL_PERCENT))}%` }}
          />
        </div>
        <span className={`tempo-reach__note${isOver ? ' tempo-reach__note--over' : ''}`}>
          {forecastText(forecast, limit.resetsAt, now)}
        </span>
      </li>
    );
  }

  return (
    <section className="tempo-reach" aria-label="Reichweite">
      <h3 className="tempo-reach__title">Reichweite</h3>
      {shown.length === 0 ? (
        <p className="tempo-reach__none">{NOTHING_YET}</p>
      ) : (
        <ul className="tempo-reach__list">{shown.map(renderWindow)}</ul>
      )}
    </section>
  );
}
