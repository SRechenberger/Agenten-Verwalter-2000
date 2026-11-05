//! Wer sich verbinden darf und wie die Einladung aussieht (ADR 059). Kein Tauri, kein Netz außer
//! `local_addresses`.
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, UdpSocket};
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;

pub const SCHEME: &str = "verwalter-remote://";
/// Eine Einladung gilt zehn Minuten und einmal.
pub const INVITATION_TTL: Duration = Duration::from_secs(10 * 60);
/// Nach so vielen falschen Geheimnissen ist eine offene Einladung ungültig.
pub const INVITATION_ATTEMPTS: u32 = 5;
/// Bytes des Einmal-Geheimnisses: 128 Bit.
pub const SECRET_BYTES: usize = 16;

/// Nur „local“: Loopback, RFC 1918, Link-Local, 100.64.0.0/10 (VPN wie Tailscale) und IPv6 ULA,
/// auch als v4-mapped IPv6-Adresse. Alles andere trennt der Host vor dem Handschlag.
pub fn is_local_peer(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(v4) => is_local_v4(v4),
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => is_local_v4(v4),
            None => is_local_v6(v6),
        },
    }
}

fn is_local_v4(address: Ipv4Addr) -> bool {
    let [first, second, ..] = address.octets();
    address.is_loopback()
        || address.is_private()
        || address.is_link_local()
        || (first == 100 && (second & 0b1100_0000) == 0b0100_0000)
}

fn is_local_v6(address: Ipv6Addr) -> bool {
    let first = address.segments()[0];
    address.is_loopback()
        // fc00::/7, Unique Local
        || (first & 0xfe00) == 0xfc00
        // fe80::/10, Link-Local
        || (first & 0xffc0) == 0xfe80
}

/// Die Adressen, unter denen der Host im LAN oder VPN erreichbar ist, ohne Loopback. Ein UDP-Socket
/// mit `connect` sendet nichts, er wählt nur die Route; so braucht es keine Liste der Schnittstellen.
pub fn local_addresses() -> Vec<IpAddr> {
    let targets: [(IpAddr, IpAddr); 6] = [
        (
            Ipv4Addr::UNSPECIFIED.into(),
            Ipv4Addr::new(192, 168, 255, 255).into(),
        ),
        (
            Ipv4Addr::UNSPECIFIED.into(),
            Ipv4Addr::new(10, 255, 255, 255).into(),
        ),
        (
            Ipv4Addr::UNSPECIFIED.into(),
            Ipv4Addr::new(172, 31, 255, 255).into(),
        ),
        (
            Ipv4Addr::UNSPECIFIED.into(),
            Ipv4Addr::new(100, 127, 255, 255).into(),
        ),
        (
            Ipv4Addr::UNSPECIFIED.into(),
            Ipv4Addr::new(192, 0, 2, 1).into(),
        ),
        (
            Ipv6Addr::UNSPECIFIED.into(),
            Ipv6Addr::new(0xfd00, 0, 0, 0, 0, 0, 0, 1).into(),
        ),
    ];
    let mut found: Vec<IpAddr> = Vec::new();
    for (bind, target) in targets {
        let local = UdpSocket::bind((bind, 0))
            .and_then(|socket: UdpSocket| socket.connect((target, 9)).map(|()| socket))
            .and_then(|socket: UdpSocket| socket.local_addr());
        if let Ok(local) = local {
            let ip = local.ip();
            if is_local_peer(ip) && !ip.is_loopback() && !found.contains(&ip) {
                found.push(ip);
            }
        }
    }
    found
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InvitationError {
    #[error("Das ist keine Einladung des Verwalters. Sie beginnt mit {SCHEME}")]
    Scheme,
    #[error("In der Einladung fehlt die Adresse des Hosts.")]
    Address,
    #[error("In der Einladung fehlt der Port.")]
    Port,
    #[error("In der Einladung fehlt der Fingerabdruck.")]
    Fingerprint,
    #[error("In der Einladung fehlt das Geheimnis.")]
    Secret,
}

/// `verwalter-remote://<Adresse>:<Port>?fp=<SHA-256 hex>&s=<128 Bit base64url>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invitation {
    /// Name oder IP ohne Klammern; am Gast änderbar.
    pub host: String,
    pub port: u16,
    /// Kleinbuchstaben.
    pub fingerprint: String,
    pub secret: String,
}

impl Invitation {
    /// `<Adresse>:<Port>`, eine IPv6-Adresse in Klammern.
    pub fn address(&self) -> String {
        if self.host.contains(':') {
            format!("[{}]:{}", self.host, self.port)
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }

    pub fn to_text(&self) -> String {
        format!(
            "{SCHEME}{}?fp={}&s={}",
            self.address(),
            self.fingerprint,
            self.secret
        )
    }

    pub fn parse(text: &str) -> Result<Invitation, InvitationError> {
        let rest = text
            .trim()
            .strip_prefix(SCHEME)
            .ok_or(InvitationError::Scheme)?;
        let (authority, query) = rest.split_once('?').unwrap_or((rest, ""));
        let authority = authority.trim_end_matches('/');
        let (host, port) = match authority.strip_prefix('[') {
            Some(bracketed) => {
                let (host, after) = bracketed.split_once(']').ok_or(InvitationError::Address)?;
                (host, after.strip_prefix(':').ok_or(InvitationError::Port)?)
            }
            None => authority.rsplit_once(':').ok_or(InvitationError::Port)?,
        };
        if host.is_empty()
            || host
                .chars()
                .any(|character: char| character.is_whitespace() || "/?#@".contains(character))
        {
            return Err(InvitationError::Address);
        }
        let port = port
            .parse::<u16>()
            .ok()
            .filter(|port: &u16| *port != 0)
            .ok_or(InvitationError::Port)?;
        let mut fingerprint: Option<String> = None;
        let mut secret: Option<String> = None;
        for pair in query.split('&') {
            match pair.split_once('=') {
                Some(("fp", value)) => fingerprint = Some(value.to_ascii_lowercase()),
                Some(("s", value)) => secret = Some(value.to_owned()),
                _ => {}
            }
        }
        let fingerprint = fingerprint
            .filter(|value: &String| {
                value.len() == 64 && value.bytes().all(|byte: u8| byte.is_ascii_hexdigit())
            })
            .ok_or(InvitationError::Fingerprint)?;
        let secret = secret
            .filter(|value: &String| {
                URL_SAFE_NO_PAD
                    .decode(value)
                    .is_ok_and(|bytes: Vec<u8>| bytes.len() == SECRET_BYTES)
            })
            .ok_or(InvitationError::Secret)?;
        Ok(Invitation {
            host: host.to_owned(),
            port,
            fingerprint,
            secret,
        })
    }
}

/// Ein neues Einmal-Geheimnis, base64url ohne Auffüllung.
pub fn new_secret() -> std::io::Result<String> {
    Ok(URL_SAFE_NO_PAD.encode(super::tls::random_bytes::<SECRET_BYTES>()?))
}

#[cfg(test)]
mod tests;
