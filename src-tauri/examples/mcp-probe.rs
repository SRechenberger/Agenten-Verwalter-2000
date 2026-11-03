//! Prüfprogramm für den MCP-Zugang des Verwalters (ADR 046) gegen die laufende App.
//!
//! Aufruf:
//!   mcp-probe <url> <schlüssel> [list | call <name> <json>] [--negative]
//!   mcp-probe <konfigurationsdatei> [list | call <name> <json>] [--negative]
//!
//! Sendet `initialize`, `notifications/initialized` und dann `tools/list` (Standard) oder
//! `tools/call`, und druckt je Anfrage Status, Köpfe und Körper. `--negative` schickt zusätzlich
//! die Fehlfälle (ohne Schlüssel, falscher Schlüssel, falscher Host, GET, OPTIONS, text/plain,
//! 2 MiB) und vergleicht den Status mit dem erwarteten. Die Konfigurationsdatei ist die, die der
//! Verwalter beim Agenten-Start schreibt (`<Benutzerordner>\.verwalter\mcp\<session-id>.json`).
//!
//! Spricht rohes HTTP über `std::net`, damit auch `Host` und Methode frei wählbar sind.
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::ExitCode;
use std::time::Duration;

use serde_json::{Value, json};

const TIMEOUT: Duration = Duration::from_secs(30);
const LARGE_BODY_BYTES: usize = 2 * 1024 * 1024;
const PROTOCOL_VERSION: &str = "2025-11-25";

/// Bezeichnung, erwarteter Status, `Host`, Methode, Köpfe, Körper.
type NegativeCase<'a> = (
    &'a str,
    u16,
    String,
    &'a str,
    Vec<(&'a str, String)>,
    &'a [u8],
);

struct Target {
    /// `127.0.0.1:<port>`
    address: String,
    path: String,
    key: String,
}

struct Answer {
    status: u16,
    head: String,
    body: String,
}

impl Answer {
    fn header(&self, name: &str) -> Option<&str> {
        self.head
            .split("\r\n")
            .filter_map(|line: &str| line.split_once(':'))
            .find(|(key, _): &(&str, &str)| key.trim().eq_ignore_ascii_case(name))
            .map(|(_, value): (&str, &str)| value.trim())
    }

    fn has_cors(&self) -> bool {
        self.head
            .split("\r\n")
            .any(|line: &str| line.to_ascii_lowercase().starts_with("access-control-"))
    }
}

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let negative = args.iter().any(|arg: &String| arg == "--negative");
    args.retain(|arg: &String| arg != "--negative");
    let (target, rest) = match parse_target(&args) {
        Ok(found) => found,
        Err(message) => {
            eprintln!("{message}");
            eprintln!(
                "Aufruf: mcp-probe <url> <schlüssel> | <konfigurationsdatei> \
                 [list | call <name> <json>] [--negative]"
            );
            return ExitCode::FAILURE;
        }
    };
    let action = match rest {
        [] => json!({ "method": "tools/list", "params": {} }),
        [list] if list == "list" => json!({ "method": "tools/list", "params": {} }),
        [call, name, arguments] if call == "call" => {
            let Ok(arguments) = serde_json::from_str::<Value>(arguments) else {
                eprintln!("Argumente sind kein JSON: {arguments}");
                return ExitCode::FAILURE;
            };
            json!({ "method": "tools/call", "params": { "name": name, "arguments": arguments } })
        }
        _ => {
            eprintln!("Unbekannte Aktion: {}", rest.join(" "));
            return ExitCode::FAILURE;
        }
    };
    let mut failed = !run_normal(&target, action);
    if negative {
        failed |= !run_negative(&target);
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn parse_target(args: &[String]) -> Result<(Target, &[String]), String> {
    let first = args.first().ok_or("Ziel fehlt")?;
    if first.to_ascii_lowercase().ends_with(".json") {
        let text = fs::read_to_string(first).map_err(|error| format!("{first}: {error}"))?;
        let config: Value = serde_json::from_str(&text).map_err(|error| error.to_string())?;
        let server = config
            .pointer("/mcpServers/verwalter")
            .ok_or("Kein Eintrag mcpServers.verwalter")?;
        let url = server
            .get("url")
            .and_then(Value::as_str)
            .ok_or("url fehlt")?;
        let key = server
            .pointer("/headers/Authorization")
            .and_then(Value::as_str)
            .and_then(|value: &str| value.strip_prefix("Bearer "))
            .ok_or("Authorization fehlt")?;
        return Ok((target_of(url, key)?, &args[1..]));
    }
    let key = args.get(1).ok_or("Schlüssel fehlt")?;
    Ok((target_of(first, key)?, &args[2..]))
}

fn target_of(url: &str, key: &str) -> Result<Target, String> {
    let rest = url
        .strip_prefix("http://")
        .ok_or("Nur http://127.0.0.1:<port>/mcp")?;
    let (address, path) = rest.split_once('/').unwrap_or((rest, ""));
    Ok(Target {
        address: address.to_owned(),
        path: format!("/{path}"),
        key: key.to_owned(),
    })
}

/// Handschlag, Benachrichtigung, dann die gewünschte Anfrage; `false`, wenn eine scheitert.
fn run_normal(target: &Target, action: Value) -> bool {
    let initialize = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": { "name": "mcp-probe", "version": "1" },
        },
    });
    let Some(answer) = post_json(target, &initialize, None) else {
        return false;
    };
    print_answer("initialize", &answer);
    let session = answer.header("Mcp-Session-Id").map(str::to_owned);
    let initialized = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
    let Some(answer) = post_json(target, &initialized, session.as_deref()) else {
        return false;
    };
    print_answer("notifications/initialized", &answer);
    let mut request = action;
    request["jsonrpc"] = json!("2.0");
    request["id"] = json!(2);
    let label = request["method"].as_str().unwrap_or_default().to_owned();
    let Some(answer) = post_json(target, &request, session.as_deref()) else {
        return false;
    };
    print_answer(&label, &answer);
    answer.status == 200
}

fn post_json(target: &Target, message: &Value, session: Option<&str>) -> Option<Answer> {
    let mut headers: Vec<(&str, String)> = vec![
        ("Authorization", format!("Bearer {}", target.key)),
        ("Content-Type", "application/json".to_owned()),
        ("Accept", "application/json, text/event-stream".to_owned()),
    ];
    if let Some(session) = session {
        headers.push(("Mcp-Session-Id", session.to_owned()));
    }
    let body = message.to_string().into_bytes();
    match send(target, &target.address, "POST", &headers, &body) {
        Ok(answer) => Some(answer),
        Err(error) => {
            eprintln!("FEHLER {}: {error}", message["method"]);
            None
        }
    }
}

/// Jeder Fehlfall mit erwartetem Status; `false`, wenn einer abweicht oder CORS-Köpfe trägt.
fn run_negative(target: &Target) -> bool {
    let ping = json!({ "jsonrpc": "2.0", "id": 9, "method": "ping" })
        .to_string()
        .into_bytes();
    let bearer = format!("Bearer {}", target.key);
    let json_type = ("Content-Type", "application/json".to_owned());
    let large = vec![b' '; LARGE_BODY_BYTES];
    let cases: Vec<NegativeCase> = vec![
        (
            "ohne Schlüssel",
            401,
            target.address.clone(),
            "POST",
            vec![json_type.clone()],
            &ping,
        ),
        (
            "falscher Schlüssel",
            401,
            target.address.clone(),
            "POST",
            vec![
                ("Authorization", "Bearer 0000".to_owned()),
                json_type.clone(),
            ],
            &ping,
        ),
        (
            "falscher Host",
            403,
            format!("evil.example:{}", port_of(&target.address)),
            "POST",
            vec![("Authorization", bearer.clone()), json_type.clone()],
            &ping,
        ),
        (
            "GET",
            405,
            target.address.clone(),
            "GET",
            vec![("Authorization", bearer.clone())],
            &[],
        ),
        (
            "OPTIONS",
            405,
            target.address.clone(),
            "OPTIONS",
            vec![
                ("Origin", "http://127.0.0.1".to_owned()),
                ("Access-Control-Request-Method", "POST".to_owned()),
            ],
            &[],
        ),
        (
            "text/plain",
            415,
            target.address.clone(),
            "POST",
            vec![
                ("Authorization", bearer.clone()),
                ("Content-Type", "text/plain".to_owned()),
            ],
            &ping,
        ),
        (
            "2 MiB",
            413,
            target.address.clone(),
            "POST",
            vec![("Authorization", bearer.clone()), json_type.clone()],
            &large,
        ),
    ];
    let mut all_ok = true;
    println!("--- Fehlfälle");
    for (label, expected, host, method, headers, body) in cases {
        match send(target, &host, method, &headers, body) {
            Ok(answer) => {
                let ok = answer.status == expected && !answer.has_cors();
                all_ok &= ok;
                println!(
                    "{} {label}: {} (erwartet {expected}){}",
                    if ok { "OK    " } else { "FEHLER" },
                    answer.status,
                    if answer.has_cors() {
                        ", trägt Access-Control-*"
                    } else {
                        ""
                    }
                );
            }
            Err(error) => {
                all_ok = false;
                println!("FEHLER {label}: {error}");
            }
        }
    }
    all_ok
}

fn port_of(address: &str) -> &str {
    address.rsplit_once(':').map_or("", |(_, port)| port)
}

/// Eine Anfrage mit frei gewähltem `Host`; liest die Antwort bis zum Verbindungsende.
fn send(
    target: &Target,
    host: &str,
    method: &str,
    headers: &[(&str, String)],
    body: &[u8],
) -> Result<Answer, String> {
    let mut stream =
        TcpStream::connect(&target.address).map_err(|error| format!("Verbindung: {error}"))?;
    let _ = stream.set_read_timeout(Some(TIMEOUT));
    let mut head = format!("{method} {} HTTP/1.1\r\nHost: {host}\r\n", target.path);
    for (name, value) in headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    if method == "POST" {
        head.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    head.push_str("Connection: close\r\n\r\n");
    stream
        .write_all(head.as_bytes())
        .map_err(|error| format!("Senden: {error}"))?;
    // Lehnt der Server vor dem Körper ab, darf das Schreiben scheitern; die Antwort zählt.
    let _ = stream.write_all(body);
    let mut raw: Vec<u8> = Vec::new();
    stream
        .read_to_end(&mut raw)
        .map_err(|error| format!("Lesen: {error}"))?;
    let text = String::from_utf8_lossy(&raw).into_owned();
    let (head, body) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
    let status = head
        .split(' ')
        .nth(1)
        .and_then(|code: &str| code.parse().ok())
        .ok_or_else(|| format!("Keine Statuszeile: {head}"))?;
    Ok(Answer {
        status,
        head: head.to_owned(),
        body: body.to_owned(),
    })
}

fn print_answer(label: &str, answer: &Answer) {
    println!("--- {label}");
    println!("{}", answer.head);
    println!();
    if !answer.body.is_empty() {
        let pretty = serde_json::from_str::<Value>(&answer.body)
            .ok()
            .and_then(|value: Value| serde_json::to_string_pretty(&value).ok())
            .unwrap_or_else(|| answer.body.clone());
        println!("{pretty}");
    }
}
