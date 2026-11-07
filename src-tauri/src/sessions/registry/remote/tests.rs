//! Die gezeigte Session je Oberfläche (ADR 059, Klasse B).
use super::*;

const GUEST: Option<&str> = Some("gast");

fn viewed(entries: &[(Option<&str>, &str)]) -> Viewed {
    entries
        .iter()
        .map(|(surface, session)| (surface.map(str::to_owned), (*session).to_owned()))
        .collect()
}

#[test]
fn a_guest_switching_sessions_leaves_the_host_surfaces_session_viewed() {
    let mut shown = viewed(&[(None, "host"), (GUEST, "alt")]);
    assert_eq!(
        unviewed_after(&mut shown, GUEST, Some("neu")),
        Some("alt".to_owned())
    );
    assert_eq!(shown, viewed(&[(None, "host")]));
}

#[test]
fn a_session_another_surface_still_shows_stays_viewed() {
    let mut shown = viewed(&[(None, "beide"), (GUEST, "beide")]);
    assert_eq!(unviewed_after(&mut shown, GUEST, None), None);
    assert_eq!(shown, viewed(&[(None, "beide")]));
}

#[test]
fn showing_the_same_session_again_does_not_unview_it() {
    let mut shown = viewed(&[(None, "a")]);
    assert_eq!(unviewed_after(&mut shown, None, Some("a")), None);
}

#[test]
fn a_surface_without_a_shown_session_unviews_nothing() {
    let mut shown = viewed(&[(None, "a")]);
    assert_eq!(unviewed_after(&mut shown, GUEST, Some("b")), None);
    assert_eq!(shown, viewed(&[(None, "a")]));
}
