import type { ReactElement } from 'react';
import { categoryLabel } from '@/features/context/categoryLabels';
import { ContextDonut } from '@/features/context/ContextDonut';
import { contextSeverity } from '@/features/context/contextSeverity';
import {
  formatClock,
  formatPercent,
  formatThousands,
  formatTokens,
} from '@/features/context/formatTokens';
import { useSessionContext } from '@/features/context/useSessionContext';
import type { ContextBreakdown } from '@/lib/bindings/ContextBreakdown';
import type { ContextCategory } from '@/lib/bindings/ContextCategory';
import type { ContextFile } from '@/lib/bindings/ContextFile';
import type { SessionContext } from '@/lib/bindings/SessionContext';
import type { SessionStatus } from '@/lib/bindings/SessionStatus';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import { LOCKED_TITLE } from '@/features/remote/projectAccess';
import { useSessionLocked } from '@/features/remote/useProjectAccess';
import { sendMessage } from '@/lib/chat';
import { refreshContext } from '@/lib/context';
import type { Severity } from '@/lib/severity';
import './ContextPanel.css';

const CHART_COLOR_COUNT = 7;
const PATH_MAX_LENGTH = 48;
const PERCENT_FACTOR = 100;
const DONUT_SIZE = 44;

const COMPACT_TITLE = 'Ab hier fasst Claude den bisherigen Verlauf zusammen, um Platz zu schaffen.';
const COMPACT_COMMAND = '/compact';
const COMPACT_BUTTON_TITLE =
  'Fasst den bisherigen Verlauf jetzt zusammen, um Platz im Kontext zu schaffen.';
const COMPACT_BLOCKED_TITLE =
  'Compact geht nur, wenn der Agent gerade nicht arbeitet und nichts von dir wartet.';
/** Nur hier ist der Agent untätig und wartet nicht auf eine Antwort — sonst würde „/compact“ eine Rückfrage beantworten. */
const COMPACTABLE_STATUSES: readonly SessionStatus[] = ['completed', 'paused'];

interface ContextPanelProps {
  session: SessionSummary;
  /** Schließt das umgebende Fenster, nachdem „Compact“ abgeschickt ist. */
  onClose: () => void;
}

interface CategoryRow {
  category: ContextCategory;
  /** 1 bis 7 für belegte Kategorien, `null` für den freien Platz. */
  colorIndex: number | null;
}

export function ContextPanel({ session, onClose }: ContextPanelProps): ReactElement {
  const context: SessionContext | null = useSessionContext(session.id, true);
  const isLocked: boolean = useSessionLocked(session.id);

  function refresh(): void {
    // Die Antwort kommt als `context://changed`; hier gibt es nichts abzuwarten.
    refreshContext(session.id).catch((reason: unknown) => {
      console.error('Kontext nicht anfragbar', reason);
    });
  }

  function compact(): void {
    // Wie eine getippte Nachricht: die Kommandozeile fasst zusammen, der Chat zeigt den Verlauf.
    sendMessage(session.id, COMPACT_COMMAND, [], []).catch((reason: unknown) => {
      console.error('Compact nicht sendbar', reason);
    });
    onClose();
  }

  function renderBody(): ReactElement {
    if (context === null || context.breakdown === null) {
      return renderWithoutBreakdown(context);
    }
    return renderBreakdown(context.breakdown);
  }

  function renderWithoutBreakdown(loaded: SessionContext | null): ReactElement {
    const hint: string =
      loaded === null || loaded.isAgentRunning
        ? 'Aufschlüsselung wird geladen …'
        : 'Die Aufschlüsselung kommt mit der nächsten Antwort des Agenten.';
    const usedPercent: number = shareOfWindow(session.contextUsed, session.contextWindow);
    return (
      <>
        {renderGauge(
          usedPercent,
          `${formatThousands(session.contextUsed)} von ${formatThousands(session.contextWindow)} Tokens belegt`,
        )}
        <p className="context-panel__hint">{hint}</p>
      </>
    );
  }

  function renderFoot(fetchedAt: number | null): ReactElement {
    const isRunning: boolean = context?.isAgentRunning ?? false;
    const canCompact: boolean = COMPACTABLE_STATUSES.includes(session.status) && !isLocked;
    return (
      <div className="context-panel__foot">
        <span>
          {fetchedAt !== null && `Stand ${formatClock(fetchedAt)}`}
          {fetchedAt !== null && !isRunning && ' · Agent ruht'}
        </span>
        <span className="context-panel__actions">
          {isRunning && (
            <button type="button" className="context-panel__button" onClick={refresh}>
              Aktualisieren
            </button>
          )}
          <button
            type="button"
            className="context-panel__button context-panel__button--primary"
            disabled={!canCompact}
            title={
              canCompact ? COMPACT_BUTTON_TITLE : isLocked ? LOCKED_TITLE : COMPACT_BLOCKED_TITLE
            }
            onClick={compact}
          >
            Compact
          </button>
        </span>
      </div>
    );
  }

  return (
    <section className="context-panel" aria-label="Kontext">
      <h2 className="context-panel__title">Kontext</h2>
      {renderBody()}
      {renderFoot(context?.breakdown?.fetchedAt ?? null)}
    </section>
  );
}

/** Donut links, rechts die Zahlen und die Zeile darunter (Modell); Text in der Farbe des Donuts. */
function renderGauge(usedPercent: number, summary: string, model?: string): ReactElement {
  const severity: Severity = contextSeverity(usedPercent);
  return (
    <div className={`context-panel__gauge severity severity--${severity}`}>
      <ContextDonut percent={usedPercent} size={DONUT_SIZE} />
      <div className="context-panel__gauge-text">
        <p className="context-panel__summary">{summary}</p>
        {model !== undefined && <p className="context-panel__model">{model}</p>}
      </div>
    </div>
  );
}

function renderBreakdown(breakdown: ContextBreakdown): ReactElement {
  const rows: CategoryRow[] = buildRows(breakdown.categories);
  const usedPercent: number = shareOfWindow(breakdown.totalTokens, breakdown.maxTokens);

  return (
    <>
      {renderGauge(
        usedPercent,
        `${formatTokens(breakdown.totalTokens)} / ${formatTokens(breakdown.maxTokens)} Tokens (${formatPercent(usedPercent, 0)})`,
        breakdown.model,
      )}
      {renderStack(breakdown, rows)}
      {renderTable(breakdown, rows)}
      {breakdown.autoCompactThreshold !== null && (
        <p className="context-panel__compact" title={COMPACT_TITLE}>
          <InfoIcon />
          Automatisches Zusammenfassen ab {formatTokens(breakdown.autoCompactThreshold)} Tokens
        </p>
      )}
      {breakdown.memoryFiles.length > 0 && renderMemoryFiles(breakdown.memoryFiles)}
    </>
  );
}

function renderStack(breakdown: ContextBreakdown, rows: readonly CategoryRow[]): ReactElement {
  return (
    <div className="context-panel__stack" aria-hidden="true">
      {rows.map((row: CategoryRow) => {
        if (row.colorIndex === null) {
          return null;
        }
        return (
          <span
            key={row.category.name}
            className={`context-panel__segment context-panel__chart--${String(row.colorIndex)}`}
            style={{
              width: `${String(
                Math.min(shareOfWindow(row.category.tokens, breakdown.maxTokens), PERCENT_FACTOR),
              )}%`,
            }}
          />
        );
      })}
      {breakdown.autoCompactThreshold !== null && (
        <span
          className="context-panel__threshold"
          style={{
            left: `${String(shareOfWindow(breakdown.autoCompactThreshold, breakdown.maxTokens))}%`,
          }}
        />
      )}
    </div>
  );
}

function renderTable(breakdown: ContextBreakdown, rows: readonly CategoryRow[]): ReactElement {
  return (
    <table className="context-panel__table">
      <thead>
        <tr>
          <th scope="col">Kategorie</th>
          <th scope="col" className="context-panel__number">
            Tokens
          </th>
          <th scope="col" className="context-panel__number">
            Anteil
          </th>
        </tr>
      </thead>
      <tbody>
        {rows.map((row: CategoryRow) => (
          <tr key={row.category.name}>
            <td>
              <span
                className={`context-panel__dot${
                  row.colorIndex === null
                    ? ' context-panel__dot--free'
                    : ` context-panel__chart--${String(row.colorIndex)}`
                }`}
              />
              {categoryLabel(row.category.name)}
            </td>
            <td className="context-panel__number">{formatTokens(row.category.tokens)}</td>
            <td className="context-panel__number">
              {formatPercent(shareOfWindow(row.category.tokens, breakdown.maxTokens), 1)}
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function renderMemoryFiles(files: readonly ContextFile[]): ReactElement {
  return (
    <section className="context-panel__files">
      <h3 className="context-panel__subtitle">Memory-Dateien</h3>
      <ul className="context-panel__file-list">
        {files.map((file: ContextFile) => (
          <li key={file.path} className="context-panel__file">
            <span className="context-panel__path" title={file.path}>
              {shortenPath(file.path)}
            </span>
            <span className="context-panel__number">{formatTokens(file.tokens)}</span>
          </li>
        ))}
      </ul>
    </section>
  );
}

/** Belegte Kategorien bekommen der Reihe nach eine der sieben Diagrammfarben (ab der achten von vorn). */
function buildRows(categories: readonly ContextCategory[]): CategoryRow[] {
  let usedCount = 0;
  return categories.map((category: ContextCategory): CategoryRow => {
    if (category.isFree) {
      return { category, colorIndex: null };
    }
    const colorIndex: number = (usedCount % CHART_COLOR_COUNT) + 1;
    usedCount += 1;
    return { category, colorIndex };
  });
}

function shareOfWindow(tokens: number, maxTokens: number): number {
  if (maxTokens === 0) {
    return 0;
  }
  return (tokens / maxTokens) * PERCENT_FACTOR;
}

/** Lange Pfade behalten ihr Ende: „…“ plus die letzten 47 Zeichen. */
function shortenPath(path: string): string {
  if (path.length <= PATH_MAX_LENGTH) {
    return path;
  }
  return `…${path.slice(-(PATH_MAX_LENGTH - 1))}`;
}

function InfoIcon(): ReactElement {
  return (
    <svg
      width="12"
      height="12"
      viewBox="0 0 14 14"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.3"
      strokeLinecap="round"
      aria-hidden="true"
    >
      <circle cx="7" cy="7" r="5.5" />
      <path d="M7 6.3v3.4M7 4.3v.1" />
    </svg>
  );
}
