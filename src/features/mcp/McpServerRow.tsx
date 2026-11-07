import type { ReactElement } from 'react';
import { Switch } from '@/components/Switch';
import {
  detailNote,
  ERROR_BOX_TITLE,
  LOGIN_LABEL,
  LOGIN_TITLE,
  NO_ERROR_REASON_TEXT,
  NO_TOOLS_TEXT,
  originText,
  RECONNECT_TITLE,
  statusText,
  SWITCH_TITLE,
  SWITCH_TITLE_OFF,
  toolsLabel,
  toolsTitle,
} from '@/features/mcp/mcpTexts';
import type { McpServer } from '@/lib/bindings/McpServer';
import './McpServerRow.css';

interface McpServerRowProps {
  server: McpServer;
  isExpanded: boolean;
  /** Für den Server läuft gerade eine Aktion; Neu verbinden und Schalter sind dann gesperrt. */
  isBusy: boolean;
  /** Der Agent läuft und kann Steueranfragen annehmen. */
  canAct: boolean;
  onToggleExpanded: () => void;
  onReconnect: () => void;
  onAuthenticate: () => void;
  /** `false` an einem entfernten Vorhaben: Die Anmeldung öffnete einen Browser am Host. */
  canLogin?: boolean;
  onSetEnabled: (enabled: boolean) => void;
}

const NO_CONNECTION_TEXT = '–';

export function McpServerRow({
  server,
  isExpanded,
  isBusy,
  canAct,
  onToggleExpanded,
  onReconnect,
  onAuthenticate,
  canLogin = true,
  onSetEnabled,
}: McpServerRowProps): ReactElement {
  const isOff: boolean = server.status === 'disabled';
  const classNames: string = [
    'mcp-server',
    isExpanded ? 'mcp-server--expanded' : '',
    isOff ? 'mcp-server--off' : '',
  ]
    .filter(Boolean)
    .join(' ');
  const subText: string = buildSubText(server);

  function renderDetails(): ReactElement {
    return (
      <div className="mcp-server__details">
        <dl className="mcp-server__facts">
          <dt className="mcp-server__fact-term">Herkunft</dt>
          <dd className="mcp-server__fact-value">{originText(server.scope)}</dd>
          <dt className="mcp-server__fact-term">Verbindung</dt>
          <dd className="mcp-server__fact-value mcp-server__fact-value--mono">
            {server.connection === '' ? NO_CONNECTION_TEXT : server.connection}
          </dd>
        </dl>
        {renderStatusDetails()}
      </div>
    );
  }

  function renderStatusDetails(): ReactElement | null {
    if (server.status === 'connected') {
      return renderTools();
    }
    if (server.status === 'failed') {
      return renderError();
    }
    const note: string | null = detailNote(server.status);
    if (note === null) {
      return null;
    }
    return <p className="mcp-server__note">{note}</p>;
  }

  function renderTools(): ReactElement {
    if (server.tools.length === 0) {
      return <p className="mcp-server__note">{NO_TOOLS_TEXT}</p>;
    }
    return (
      <div className="mcp-server__tools-block">
        <span className="mcp-server__tools-title">{toolsTitle(server.tools.length)}</span>
        <ul className="mcp-server__tools">
          {server.tools.map((tool: string) => (
            <li key={tool} className="mcp-server__tool">
              {tool}
            </li>
          ))}
        </ul>
      </div>
    );
  }

  function renderError(): ReactElement {
    const reason: string =
      server.error === null || server.error === '' ? NO_ERROR_REASON_TEXT : server.error;
    return (
      <div className="mcp-server__error">
        <span className="mcp-server__error-title">{ERROR_BOX_TITLE}</span>
        <span className="mcp-server__error-text">{reason}</span>
      </div>
    );
  }

  return (
    <li className={classNames}>
      <div className="mcp-server__head">
        <button
          type="button"
          className="mcp-server__toggle"
          aria-expanded={isExpanded}
          onClick={onToggleExpanded}
        >
          <span className="mcp-server__chevron">
            <ChevronIcon />
          </span>
          <span className={`mcp-server__dot mcp-server__dot--${server.status}`} />
          <span className="mcp-server__text">
            <span className="mcp-server__name">{server.name}</span>
            {subText !== '' && <span className="mcp-server__sub">{subText}</span>}
          </span>
        </button>
        <span className={`mcp-server__status mcp-server__status--${server.status}`}>
          {statusText(server.status)}
        </span>
        {server.status === 'needsAuth' && canLogin && (
          <button
            type="button"
            className="mcp-server__login"
            title={LOGIN_TITLE}
            disabled={!canAct || isBusy}
            onClick={onAuthenticate}
          >
            {LOGIN_LABEL}
          </button>
        )}
        <button
          type="button"
          className="mcp-server__reconnect"
          aria-label={`${server.name} neu verbinden`}
          title={RECONNECT_TITLE}
          disabled={!canAct || isBusy || isOff}
          onClick={onReconnect}
        >
          <ReconnectIcon isSpinning={isBusy} />
        </button>
        <span className="mcp-server__switch">
          <Switch
            checked={!isOff}
            label={`${server.name} ${isOff ? 'einschalten' : 'ausschalten'}`}
            title={isOff ? SWITCH_TITLE_OFF : SWITCH_TITLE}
            disabled={!canAct || isBusy}
            onChange={onSetEnabled}
          />
        </span>
      </div>
      {isExpanded && renderDetails()}
    </li>
  );
}

/** Zweite Zeile unter dem Namen: Verbindung, bei verbundenen Servern dahinter die Werkzeuganzahl. */
function buildSubText(server: McpServer): string {
  const parts: string[] = [];
  if (server.connection !== '') {
    parts.push(server.connection);
  }
  if (server.status === 'connected') {
    parts.push(toolsLabel(server.tools.length));
  }
  return parts.join(' · ');
}

function ChevronIcon(): ReactElement {
  return (
    <svg
      width="14"
      height="14"
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.6"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d="M6 4l4 4-4 4" />
    </svg>
  );
}

function ReconnectIcon({ isSpinning }: { isSpinning: boolean }): ReactElement {
  return (
    <svg
      className={`mcp-server__reconnect-icon${isSpinning ? ' mcp-server__reconnect-icon--spinning' : ''}`}
      width="14"
      height="14"
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.6"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d="M13.5 8a5.5 5.5 0 1 1-1.6-3.9" />
      <path d="M13.5 2.5v3h-3" />
    </svg>
  );
}
