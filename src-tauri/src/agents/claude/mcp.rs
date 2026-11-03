//! Liest die Antworten der Kommandozeile auf `mcp_status` und `mcp_authenticate`. Das Format ist
//! nicht als stabil dokumentiert: jedes Feld ist optional, ein unlesbares Feld bleibt leer, ein
//! unlesbarer Server fällt weg.
use serde_json::Value;

use crate::mcp::model::{McpServer, McpServerStatus};
use crate::mcp_access::SERVER_NAME;

/// Antwort auf `mcp_authenticate`.
pub struct McpAuthAnswer {
    /// Fehlt, wenn schon ein gültiges Token vorliegt.
    pub auth_url: Option<String>,
    pub callback_expected: bool,
}

/// `None`, wenn `requiresUserAction` fehlt (dann ist es keine Antwort auf `mcp_authenticate`).
pub fn mcp_auth(body: &Value) -> Option<McpAuthAnswer> {
    body.get("requiresUserAction")?.as_bool()?;
    let auth_url = text_field(body, "authUrl").filter(|url: &String| !url.is_empty());
    let callback_expected = body
        .get("callbackExpected")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    Some(McpAuthAnswer {
        auth_url,
        callback_expected,
    })
}

/// `None`, wenn `body` keine Serverliste ist (`mcpServers` fehlt oder ist kein Array).
pub fn mcp_servers(body: &Value) -> Option<Vec<McpServer>> {
    let servers = body.get("mcpServers")?.as_array()?;
    Some(servers.iter().filter_map(server).collect())
}

/// Ein Eintrag ohne `name` fällt weg, der Server des Verwalters selbst auch: Er ist kein Server des
/// Nutzers und lässt sich dort weder anmelden noch abschalten.
fn server(raw: &Value) -> Option<McpServer> {
    let name = raw.get("name")?.as_str()?.to_owned();
    if name == SERVER_NAME {
        return None;
    }
    let status = raw
        .get("status")
        .and_then(Value::as_str)
        .map_or(McpServerStatus::Unknown, McpServerStatus::from_cli);
    Some(McpServer {
        name,
        status,
        scope: text_field(raw, "scope").unwrap_or_default(),
        connection: raw.get("config").map(connection).unwrap_or_default(),
        tools: tool_names(raw),
        error: text_field(raw, "error"),
    })
}

fn text_field(raw: &Value, field: &str) -> Option<String> {
    raw.get(field).and_then(Value::as_str).map(str::to_owned)
}

/// Werkzeuge ohne `name` fallen weg.
fn tool_names(raw: &Value) -> Vec<String> {
    let Some(tools) = raw.get("tools").and_then(Value::as_array) else {
        return Vec::new();
    };
    tools
        .iter()
        .filter_map(|tool: &Value| tool.get("name").and_then(Value::as_str))
        .map(str::to_owned)
        .collect()
}

/// Kurzform der Verbindung: Adresse (`HTTPS · <Host>`), sonst Programm (`stdio · <Dateiname>`),
/// sonst der gemeldete `type`; leer, wenn nichts davon da ist.
fn connection(config: &Value) -> String {
    if let Some(url) = config.get("url").and_then(Value::as_str) {
        if let Some(rest) = url.strip_prefix("https://") {
            return format!("HTTPS · {}", host_of(rest));
        }
        if let Some(rest) = url.strip_prefix("http://") {
            return format!("HTTP · {}", host_of(rest));
        }
    }
    if let Some(command) = config.get("command").and_then(Value::as_str) {
        let file_name = command.rsplit(['\\', '/']).next().unwrap_or(command);
        return format!("stdio · {file_name}");
    }
    config
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

/// Der Teil einer Adresse nach `://` bis zum ersten `/`, `?` oder `#`; ein Port bleibt dran.
fn host_of(after_scheme: &str) -> &str {
    after_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or(after_scheme)
}
