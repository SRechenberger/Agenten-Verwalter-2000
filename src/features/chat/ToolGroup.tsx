import type { ReactElement } from 'react';
import type { ToolEntry } from '@/features/chat/buildBlocks';
import { SessionLink } from '@/features/chat/SessionLink';
import { ToolDetail } from '@/features/chat/ToolDetail';
import type { ToolLink } from '@/features/chat/sessionLinks';
import { groupDot, groupSummary, groupTitle } from '@/features/chat/toolSummary';
import { useToolLinks } from '@/features/chat/useToolLinks';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import './ToolGroup.css';

const TOOL_TOGGLE_HINT = 'Eingabe und Ausgabe dieses Aufrufs ein-/ausblenden';

interface ToolGroupProps {
  sessionId: string;
  tools: readonly ToolEntry[];
  /** Liste der Sessions: Nur wer noch darin steht, ist ein Sprungziel. */
  sessions: readonly SessionSummary[];
  isExpanded: boolean;
  onToggle: () => void;
  isToolExpanded: (toolUseId: string) => boolean;
  onToggleTool: (toolUseId: string) => void;
}

interface ChevronProps {
  isOpen: boolean;
  className: string;
}

function Chevron({ isOpen, className }: ChevronProps): ReactElement {
  return (
    <svg
      className={className}
      width="11"
      height="11"
      viewBox="0 0 14 14"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.6"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d={isOpen ? 'M3.5 5.5 7 9l3.5-3.5' : 'M5.5 3.5 9 7l-3.5 3.5'} />
    </svg>
  );
}

interface Resolved {
  link: ToolLink;
  known: SessionSummary | undefined;
}

interface ToolLineProps {
  sessionId: string;
  tool: ToolEntry;
  sessions: readonly SessionSummary[];
  isOpen: boolean;
  onToggle: () => void;
}

/** Eine Zeile der Gruppe. Kennt die Ausgabe Empfänger oder ein Ziel, stehen sie als Links neben dem Knopf zum
 *  Aufklappen — ein Knopf in einem Knopf wäre kein gültiges HTML. Sonst bleibt die Zeile ein einziger Knopf. */
function ToolLine({ sessionId, tool, sessions, isOpen, onToggle }: ToolLineProps): ReactElement {
  // Ein Ziel, das weder in der Liste steht noch einen Titel nennt, hat nichts zu zeigen.
  const links: Resolved[] = useToolLinks(sessionId, tool).flatMap((link: ToolLink): Resolved[] => {
    const known: SessionSummary | undefined = sessions.find(
      (session: SessionSummary) => session.id === link.id,
    );
    return known !== undefined || link.title !== '' ? [{ link, known }] : [];
  });
  const lineClass = `tool-group__line tool-group__line--${tool.state}`;
  const label: ReactElement = (
    <>
      <span className="tool-group__branch">⎿</span>
      <span className="tool-group__tool">{tool.tool}</span>
      <span className="tool-group__target">{tool.target}</span>
      {tool.state === 'interrupted' && (
        <span className="tool-group__interrupted">unterbrochen</span>
      )}
    </>
  );
  if (links.length === 0) {
    return (
      <button
        type="button"
        className={lineClass}
        aria-expanded={isOpen}
        title={TOOL_TOGGLE_HINT}
        onClick={onToggle}
      >
        {label}
        <Chevron isOpen={isOpen} className="tool-group__chevron" />
      </button>
    );
  }
  return (
    <div className="tool-group__split">
      <button
        type="button"
        className={lineClass}
        aria-expanded={isOpen}
        title={TOOL_TOGGLE_HINT}
        onClick={onToggle}
      >
        {label}
      </button>
      <span className="tool-group__links">
        {links.map(({ link, known }: Resolved) => {
          const title: string = known?.name ?? link.title;
          return (
            <SessionLink
              key={link.id}
              id={link.id}
              number={known?.number ?? 0}
              title={title}
              canJump={known !== undefined}
              state={link.state?.text}
            >
              <span className="session-link__title">
                {known === undefined ? title : `Session ${String(known.number)} „${title}“`}
              </span>
              {link.state !== null && (
                <span
                  className={`session-link__state${
                    link.state.tone === null ? '' : ` session-link__state--${link.state.tone}`
                  }`}
                >
                  {link.state.text}
                </span>
              )}
            </SessionLink>
          );
        })}
      </span>
      <button
        type="button"
        className={`${lineClass} tool-group__chevron-button`}
        aria-expanded={isOpen}
        aria-label={TOOL_TOGGLE_HINT}
        title={TOOL_TOGGLE_HINT}
        onClick={onToggle}
      >
        <Chevron isOpen={isOpen} className="tool-group__chevron" />
      </button>
    </div>
  );
}

export function ToolGroup({
  sessionId,
  tools,
  sessions,
  isExpanded,
  onToggle,
  isToolExpanded,
  onToggleTool,
}: ToolGroupProps): ReactElement {
  return (
    <div className="tool-group">
      <button
        type="button"
        className="tool-group__toggle"
        aria-expanded={isExpanded}
        onClick={onToggle}
      >
        <span className={`tool-group__dot tool-group__dot--${groupDot(tools)}`} />
        <span className="tool-group__title">{groupTitle(tools)}</span>
        <span className="tool-group__summary">{groupSummary(tools)}</span>
        <Chevron isOpen={isExpanded} className="tool-group__chevron" />
      </button>
      {isExpanded && (
        <div className="tool-group__lines">
          {tools.map((tool: ToolEntry) => {
            const isOpen: boolean = isToolExpanded(tool.toolUseId);
            return (
              <div key={tool.toolUseId} className="tool-group__item">
                <ToolLine
                  sessionId={sessionId}
                  tool={tool}
                  sessions={sessions}
                  isOpen={isOpen}
                  onToggle={(): void => {
                    onToggleTool(tool.toolUseId);
                  }}
                />
                {isOpen && <ToolDetail sessionId={sessionId} tool={tool} />}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
