//! Nur „local“ (Z2) und die Einladung aus dem Vertrag.
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use super::*;

fn v4(a: u8, b: u8, c: u8, d: u8) -> IpAddr {
    IpAddr::V4(Ipv4Addr::new(a, b, c, d))
}

fn v6(text: &str) -> IpAddr {
    IpAddr::V6(text.parse::<Ipv6Addr>().expect("gültige IPv6-Adresse"))
}

const FINGERPRINT: &str = "3fa97c1e00112233445566778899aabbccddeeff00112233445566778899b02d";
const SECRET: &str = "AAECAwQFBgcICQoLDA0ODw";

#[test]
fn loopback_is_local() {
    assert!(is_local_peer(v4(127, 0, 0, 1)));
    assert!(is_local_peer(v4(127, 255, 0, 9)));
    assert!(is_local_peer(v6("::1")));
}

#[test]
fn rfc_1918_ranges_are_local_and_their_neighbours_are_not() {
    assert!(is_local_peer(v4(10, 0, 0, 1)));
    assert!(is_local_peer(v4(10, 255, 255, 254)));
    assert!(is_local_peer(v4(172, 16, 0, 1)));
    assert!(is_local_peer(v4(172, 31, 255, 254)));
    assert!(is_local_peer(v4(192, 168, 178, 20)));
    assert!(!is_local_peer(v4(11, 0, 0, 1)));
    assert!(!is_local_peer(v4(172, 15, 255, 255)));
    assert!(!is_local_peer(v4(172, 32, 0, 0)));
    assert!(!is_local_peer(v4(192, 169, 0, 1)));
}

#[test]
fn link_local_is_local_in_both_families() {
    assert!(is_local_peer(v4(169, 254, 0, 1)));
    assert!(is_local_peer(v4(169, 254, 255, 254)));
    assert!(!is_local_peer(v4(169, 253, 0, 1)));
    assert!(is_local_peer(v6("fe80::1")));
    assert!(is_local_peer(v6("febf::1")));
    assert!(!is_local_peer(v6("fec0::1")));
}

#[test]
fn shared_address_space_100_64_slash_10_is_local_for_vpns() {
    assert!(is_local_peer(v4(100, 64, 0, 1)));
    assert!(is_local_peer(v4(100, 92, 4, 7)));
    assert!(is_local_peer(v4(100, 127, 255, 254)));
    assert!(!is_local_peer(v4(100, 63, 255, 255)));
    assert!(!is_local_peer(v4(100, 128, 0, 0)));
}

#[test]
fn ipv6_unique_local_fc00_slash_7_is_local() {
    assert!(is_local_peer(v6("fc00::1")));
    assert!(is_local_peer(v6("fd12:3456:789a::1")));
    assert!(!is_local_peer(v6("fe00::1")));
    assert!(!is_local_peer(v6("fbff::1")));
}

#[test]
fn public_addresses_are_not_local() {
    assert!(!is_local_peer(v4(8, 8, 8, 8)));
    assert!(!is_local_peer(v4(1, 1, 1, 1)));
    assert!(!is_local_peer(v4(0, 0, 0, 0)));
    assert!(!is_local_peer(v4(255, 255, 255, 255)));
    assert!(!is_local_peer(v6("2001:db8::1")));
    assert!(!is_local_peer(v6("2a00:1450:4001::1")));
    assert!(!is_local_peer(v6("::")));
}

#[test]
fn v4_mapped_ipv6_follows_the_ipv4_rules() {
    assert!(is_local_peer(v6("::ffff:127.0.0.1")));
    assert!(is_local_peer(v6("::ffff:192.168.1.5")));
    assert!(is_local_peer(v6("::ffff:10.1.2.3")));
    assert!(is_local_peer(v6("::ffff:172.20.0.1")));
    assert!(is_local_peer(v6("::ffff:169.254.1.1")));
    assert!(is_local_peer(v6("::ffff:100.100.1.1")));
    assert!(!is_local_peer(v6("::ffff:8.8.8.8")));
    assert!(!is_local_peer(v6("::ffff:100.128.0.1")));
}

#[test]
fn an_invitation_survives_writing_and_reading() {
    let invitation = Invitation {
        host: "192.168.178.20".to_owned(),
        port: 47800,
        fingerprint: FINGERPRINT.to_owned(),
        secret: SECRET.to_owned(),
    };
    let text = invitation.to_text();
    assert_eq!(
        text,
        format!("verwalter-remote://192.168.178.20:47800?fp={FINGERPRINT}&s={SECRET}")
    );
    assert_eq!(Invitation::parse(&text), Ok(invitation));
}

#[test]
fn an_ipv6_invitation_puts_the_address_in_brackets() {
    let invitation = Invitation {
        host: "fd00::20".to_owned(),
        port: 47800,
        fingerprint: FINGERPRINT.to_owned(),
        secret: SECRET.to_owned(),
    };
    assert_eq!(invitation.address(), "[fd00::20]:47800");
    assert_eq!(Invitation::parse(&invitation.to_text()), Ok(invitation));
}

#[test]
fn reading_an_invitation_ignores_surrounding_space_and_fingerprint_case() {
    let text = format!(
        "  verwalter-remote://linux-host:47800?fp={}&s={SECRET}\n",
        FINGERPRINT.to_ascii_uppercase()
    );
    let invitation = Invitation::parse(&text).expect("lesbar");
    assert_eq!(invitation.host, "linux-host");
    assert_eq!(invitation.fingerprint, FINGERPRINT);
}

#[test]
fn an_invitation_without_scheme_address_port_fingerprint_or_secret_is_refused() {
    let read = |text: String| Invitation::parse(&text);
    assert_eq!(
        read(format!("https://h:1?fp={FINGERPRINT}&s={SECRET}")),
        Err(InvitationError::Scheme)
    );
    assert_eq!(
        read(format!(
            "verwalter-remote://:47800?fp={FINGERPRINT}&s={SECRET}"
        )),
        Err(InvitationError::Address)
    );
    assert_eq!(
        read(format!("verwalter-remote://h?fp={FINGERPRINT}&s={SECRET}")),
        Err(InvitationError::Port)
    );
    assert_eq!(
        read(format!(
            "verwalter-remote://h:0?fp={FINGERPRINT}&s={SECRET}"
        )),
        Err(InvitationError::Port)
    );
    assert_eq!(
        read(format!("verwalter-remote://h:47800?fp=abc&s={SECRET}")),
        Err(InvitationError::Fingerprint)
    );
    assert_eq!(
        read(format!("verwalter-remote://h:47800?fp={FINGERPRINT}")),
        Err(InvitationError::Secret)
    );
    assert_eq!(
        read(format!(
            "verwalter-remote://h:47800?fp={FINGERPRINT}&s=kurz"
        )),
        Err(InvitationError::Secret)
    );
}

#[test]
fn a_new_secret_has_128_bits_in_base64url() {
    let secret = new_secret().expect("Zufall");
    let bytes = URL_SAFE_NO_PAD.decode(&secret).expect("base64url");
    assert_eq!(bytes.len(), 16);
    assert_ne!(secret, new_secret().expect("Zufall"));
}
