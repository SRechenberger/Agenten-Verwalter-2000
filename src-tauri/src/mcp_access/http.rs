//! Das Stück HTTP, das der MCP-Zugang braucht: Kopf lesen, danach genau `Content-Length` Bytes
//! Körper, Antwort schreiben. Nie `Access-Control-*`: keine Seite im Browser soll eine Antwort
//! lesen dürfen (ADR 046).
use std::io::{self, Read, Write};
use std::net::{Shutdown, TcpStream};

pub const MAX_BODY_BYTES: usize = 1024 * 1024;
const MAX_HEAD_BYTES: usize = 16 * 1024;
/// Obergrenze beim Verwerfen eines abgelehnten Körpers.
const MAX_DRAIN_BYTES: usize = 8 * MAX_BODY_BYTES;
const HEAD_END: &[u8] = b"\r\n\r\n";

pub struct Head {
    pub method: String,
    pub path: String,
    headers: Vec<(String, String)>,
    /// Was hinter dem Kopf schon mitgelesen wurde: der Anfang des Körpers.
    rest: Vec<u8>,
}

impl Head {
    /// Wert des ersten Kopfes mit diesem Namen, Groß-/Kleinschreibung egal.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _): &&(String, String)| key.eq_ignore_ascii_case(name))
            .map(|(_, value): &(String, String)| value.as_str())
    }
}

pub enum BodyError {
    LengthRequired,
    TooLarge,
    Incomplete,
}

/// Liest bis zum Ende der Kopfzeilen; `None` bei kaputter, zu großer oder abgebrochener Anfrage.
pub fn read_head(stream: &mut TcpStream) -> Option<Head> {
    let mut buffer: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 4096];
    let end = loop {
        if let Some(index) = buffer
            .windows(HEAD_END.len())
            .position(|window: &[u8]| window == HEAD_END)
        {
            break index;
        }
        if buffer.len() > MAX_HEAD_BYTES {
            return None;
        }
        let read = stream.read(&mut chunk).ok()?;
        if read == 0 {
            return None;
        }
        buffer.extend_from_slice(&chunk[..read]);
    };
    let rest = buffer.split_off(end + HEAD_END.len());
    let text = String::from_utf8(buffer).ok()?;
    let mut lines = text.split("\r\n");
    let mut request_line = lines.next()?.split(' ');
    let method = request_line.next()?.to_owned();
    let path = request_line.next()?.to_owned();
    let headers = lines
        .filter_map(|line: &str| line.split_once(':'))
        .map(|(name, value): (&str, &str)| (name.trim().to_owned(), value.trim().to_owned()))
        .collect();
    Some(Head {
        method,
        path,
        headers,
        rest,
    })
}

/// Genau `Content-Length` Bytes Körper; die Grenze wird vor dem Lesen geprüft.
pub fn read_body(stream: &mut TcpStream, head: Head) -> Result<Vec<u8>, BodyError> {
    let length: usize = head
        .header("content-length")
        .and_then(|value: &str| value.parse().ok())
        .ok_or(BodyError::LengthRequired)?;
    if length > MAX_BODY_BYTES {
        return Err(BodyError::TooLarge);
    }
    let mut body = head.rest;
    if body.len() > length {
        return Err(BodyError::Incomplete);
    }
    let missing = length - body.len();
    let start = body.len();
    body.resize(length, 0);
    if missing > 0 {
        stream
            .read_exact(&mut body[start..])
            .map_err(|_| BodyError::Incomplete)?;
    }
    Ok(body)
}

pub struct Reply {
    pub status: u16,
    pub reason: &'static str,
    pub content_type: &'static str,
    pub body: Vec<u8>,
    /// Kopf `Mcp-Session-Id`, nur auf `initialize`.
    pub session_id: Option<String>,
}

pub fn write_reply(stream: &mut TcpStream, reply: &Reply) -> io::Result<()> {
    let mut head = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\n\
         X-Content-Type-Options: nosniff\r\nConnection: close\r\n",
        reply.status,
        reply.reason,
        reply.content_type,
        reply.body.len()
    );
    if let Some(session_id) = &reply.session_id {
        head.push_str(&format!("Mcp-Session-Id: {session_id}\r\n"));
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes())?;
    stream.write_all(&reply.body)?;
    stream.flush()
}

/// Nach einer Ablehnung vor dem Körper: ungelesene Daten im Empfangspuffer ließen Windows die
/// Verbindung beim Schließen hart zurücksetzen, und der Client sähe statt der Antwort einen Fehler.
pub fn drain(stream: &mut TcpStream) {
    let _ = stream.shutdown(Shutdown::Write);
    let mut chunk = [0u8; 16 * 1024];
    let mut total: usize = 0;
    while total < MAX_DRAIN_BYTES {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(read) => total += read,
        }
    }
}
