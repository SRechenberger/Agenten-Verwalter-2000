import { useState } from 'react';
import type { ChangeEvent, ReactElement } from 'react';
import { Dialog } from '@/components/Dialog';
import { ExternalLink } from '@/components/ExternalLink';
import {
  actionErrorText,
  AGENT_IDLE_TEXT,
  authWaitText,
  FOOT_NOTE,
  groupServers,
  LOADING_TEXT,
  NEW_SESSION_TEXT,
  NO_SERVERS_TEXT,
  RECONNECT_LABEL,
  REOPEN_LABEL,
  summaryText,
} from '@/features/mcp/mcpTexts';
import type { McpGroup } from '@/features/mcp/mcpTexts';
import { McpServerRow } from '@/features/mcp/McpServerRow';
import { useSessionMcp } from '@/features/mcp/useSessionMcp';
import type { McpAuthWait } from '@/lib/bindings/McpAuthWait';
import type { McpServer } from '@/lib/bindings/McpServer';
import type { SessionMcp } from '@/lib/bindings/SessionMcp';
import type { SessionStatus } from '@/lib/bindings/SessionStatus';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import { commandErrorText } from '@/lib/errors';
import { useSessionIsRemote } from '@/features/remote/useProjectAccess';
import { authenticateMcpServer, reconnectMcpServer, toggleMcpServer } from '@/lib/mcp';
import './McpDialog.css';

const TITLE_ID = 'mcp-dialog-title';

interface McpDialogProps {
  session: SessionSummary;
  onClose: () => void;
}

/** Dialog „MCP-Server“: die Server der Session mit Status, Herkunft und Werkzeugen; Anmelden, Neu verbinden und Ein-/Ausschalten. */
export function McpDialog({ session, onClose }: McpDialogProps): ReactElement {
  const [filter, setFilter] = useState<string>('');
  const [expanded, setExpanded] = useState<string | null>(null);
  // Ein abgelehnter Command (die Aktion kam gar nicht beim Agenten an); Fehler der Aktion selbst stehen in `mcp.error`.
  const [actionFailure, setActionFailure] = useState<string | null>(null);
  const mcp: SessionMcp | null = useSessionMcp(session.id, true);
  const isRemote: boolean = useSessionIsRemote(session.id);
  const servers: McpServer[] | null = mcp?.servers ?? null;

  function reconnect(name: string): void {
    // Die Antwort kommt als `mcp://changed`; hier gibt es nichts abzuwarten.
    setActionFailure(null);
    reconnectMcpServer(session.id, name).catch((reason: unknown) => {
      setActionFailure(commandErrorText(reason));
    });
  }

  function authenticate(name: string): void {
    // Die Anmeldeseite öffnet der Core, sobald die Antwort da ist; Beginn und Ende melden `mcp://changed`.
    setActionFailure(null);
    authenticateMcpServer(session.id, name).catch((reason: unknown) => {
      setActionFailure(commandErrorText(reason));
    });
  }

  function setEnabled(name: string, enabled: boolean): void {
    setActionFailure(null);
    toggleMcpServer(session.id, name, enabled).catch((reason: unknown) => {
      setActionFailure(commandErrorText(reason));
    });
  }

  function toggleExpanded(name: string): void {
    if (expanded === name) {
      setExpanded(null);
    } else {
      setExpanded(name);
    }
  }

  function failureText(loaded: SessionMcp): string | null {
    if (actionFailure !== null) {
      return actionFailure;
    }
    if (loaded.error !== null) {
      return actionErrorText(loaded.error);
    }
    return null;
  }

  function renderBody(): ReactElement {
    if (mcp === null || servers === null || servers.length === 0) {
      return renderEmpty();
    }
    return renderList(mcp, servers);
  }

  function renderEmpty(): ReactElement {
    return <p className="mcp-dialog__empty">{emptyText(mcp, session.status)}</p>;
  }

  function renderList(loaded: SessionMcp, loadedServers: readonly McpServer[]): ReactElement {
    const groups: McpGroup[] = groupServers(loadedServers, filter);
    const failure: string | null = failureText(loaded);
    return (
      <>
        <div className="mcp-dialog__filter">
          <label className="mcp-dialog__filter-label">
            <span className="mcp-dialog__filter-icon">
              <SearchIcon />
            </span>
            <span className="mcp-dialog__filter-text">Server filtern</span>
            <input
              type="text"
              className="mcp-dialog__filter-input"
              placeholder="Server filtern …"
              value={filter}
              onChange={(event: ChangeEvent<HTMLInputElement>) => {
                setFilter(event.target.value);
              }}
            />
          </label>
        </div>
        {loaded.auth !== null && renderAuthWait(loaded, loaded.auth)}
        {failure !== null && (
          <p className="mcp-dialog__error" role="alert">
            {failure}
          </p>
        )}
        <div className="mcp-dialog__list">
          {groups.map((group: McpGroup) => renderGroup(loaded, group))}
          {groups.length === 0 && (
            <p className="mcp-dialog__no-match">Kein Server passt zu „{filter.trim()}“.</p>
          )}
        </div>
      </>
    );
  }

  function renderAuthWait(loaded: SessionMcp, auth: McpAuthWait): ReactElement {
    return (
      <div className="mcp-dialog__auth" role="status">
        <p className="mcp-dialog__auth-text">{authWaitText(auth)}</p>
        {!auth.callbackExpected && (
          <button
            type="button"
            className="mcp-dialog__auth-reconnect"
            disabled={!loaded.isAgentRunning || loaded.busy.includes(auth.server)}
            onClick={() => {
              reconnect(auth.server);
            }}
          >
            {RECONNECT_LABEL}
          </button>
        )}
        <span className="mcp-dialog__auth-reopen">
          <ExternalLink href={auth.url}>{REOPEN_LABEL}</ExternalLink>
        </span>
      </div>
    );
  }

  function renderGroup(loaded: SessionMcp, group: McpGroup): ReactElement {
    return (
      <section key={group.scope}>
        <h3 className="mcp-dialog__group">{group.title}</h3>
        <ul className="mcp-dialog__servers">
          {group.servers.map((server: McpServer) => (
            <McpServerRow
              key={server.name}
              server={server}
              isExpanded={expanded === server.name}
              isBusy={loaded.busy.includes(server.name)}
              canAct={loaded.isAgentRunning}
              onToggleExpanded={() => {
                toggleExpanded(server.name);
              }}
              onReconnect={() => {
                reconnect(server.name);
              }}
              onAuthenticate={() => {
                authenticate(server.name);
              }}
              canLogin={!isRemote}
              onSetEnabled={(enabled: boolean) => {
                setEnabled(server.name, enabled);
              }}
            />
          ))}
        </ul>
      </section>
    );
  }

  function renderFootText(loadedServers: readonly McpServer[] | null): ReactElement {
    if (loadedServers === null || loadedServers.length === 0) {
      return <span className="mcp-dialog__foot-text" />;
    }
    return (
      <span className="mcp-dialog__foot-text">
        <span>{summaryText(loadedServers)}</span>
        <span>{FOOT_NOTE}</span>
      </span>
    );
  }

  return (
    <Dialog labelledBy={TITLE_ID} className="mcp-dialog" onClose={onClose}>
      <div className="mcp-dialog__head">
        <div className="mcp-dialog__heading">
          <h2 id={TITLE_ID} className="mcp-dialog__title">
            MCP-Server
          </h2>
          <span className="mcp-dialog__subtitle">
            Session #{session.number} {session.name}
          </span>
        </div>
        <button
          type="button"
          className="mcp-dialog__close"
          aria-label="Schließen"
          onClick={onClose}
        >
          <CloseIcon />
        </button>
      </div>
      {renderBody()}
      <div className="mcp-dialog__foot">
        {renderFootText(servers)}
        <button type="button" className="mcp-dialog__done" onClick={onClose}>
          Schließen
        </button>
      </div>
    </Dialog>
  );
}

/** Was statt der Liste steht: wartet die Liste noch auf den Agenten, ist er nie gestartet oder ruht er, oder gibt es keine Server. */
function emptyText(mcp: SessionMcp | null, status: SessionStatus): string {
  if (mcp === null) {
    return LOADING_TEXT;
  }
  if (mcp.servers !== null) {
    return NO_SERVERS_TEXT;
  }
  if (mcp.isAgentRunning) {
    return LOADING_TEXT;
  }
  if (status === 'new') {
    return NEW_SESSION_TEXT;
  }
  return AGENT_IDLE_TEXT;
}

function CloseIcon(): ReactElement {
  return (
    <svg
      width="16"
      height="16"
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      aria-hidden="true"
    >
      <path d="M4 4l8 8" />
      <path d="M12 4l-8 8" />
    </svg>
  );
}

function SearchIcon(): ReactElement {
  return (
    <svg
      width="14"
      height="14"
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      aria-hidden="true"
    >
      <circle cx="7" cy="7" r="4.5" />
      <path d="M10.5 10.5L14 14" />
    </svg>
  );
}
