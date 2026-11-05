//! Je Zeile der Tabelle „Steuerrecht“ aus ADR 059 mindestens ein Test.
use super::*;

const PROJECT: &str = "project";
const OTHER_PROJECT: &str = "other-project";

fn device(id: &str) -> DeviceRef {
    DeviceRef {
        id: id.to_owned(),
        name: format!("Gerät {id}"),
    }
}

fn shared_control() -> Control {
    let mut control = Control::default();
    control.share(PROJECT);
    control
}

fn held_by_a() -> Control {
    let mut control = shared_control();
    control.take(PROJECT, device("a")).expect("freigegeben");
    control
}

fn is_host_again(control: &Control, project_id: &str) -> bool {
    control.is_shared(project_id) && control.holder(project_id).is_none()
}

#[test]
fn share_offers_the_project_without_a_holder() {
    let control = shared_control();
    assert!(is_host_again(&control, PROJECT));
    assert_eq!(
        control.access(PROJECT),
        ProjectAccess::Shared { holder: None }
    );
}

#[test]
fn unshare_clears_shared_and_holder_at_once() {
    let mut control = held_by_a();
    control.unshare(PROJECT);
    assert!(!control.is_shared(PROJECT));
    assert!(control.holder(PROJECT).is_none());
    assert_eq!(control.access(PROJECT), ProjectAccess::Own);
}

#[test]
fn take_sets_the_holder_when_shared_and_free() {
    let control = held_by_a();
    assert_eq!(
        control.holder(PROJECT).map(|holder| holder.link),
        Some(Link::Connected)
    );
    assert_eq!(
        control.access(PROJECT),
        ProjectAccess::Shared {
            holder: Some(device("a"))
        }
    );
}

#[test]
fn take_is_refused_when_the_project_is_not_shared() {
    let mut control = Control::default();
    assert_eq!(control.take(PROJECT, device("a")), Err(Refused::NotShared));
    assert!(control.holder(PROJECT).is_none());
}

#[test]
fn two_devices_are_never_holder_at_the_same_time() {
    let mut control = held_by_a();
    assert_eq!(
        control.take(PROJECT, device("b")),
        Err(Refused::HeldBy(device("a")))
    );
    assert_eq!(
        control.holder(PROJECT).map(|holder| holder.device.clone()),
        Some(device("a"))
    );
}

#[test]
fn a_second_device_cannot_take_while_the_holder_is_only_disconnected() {
    let mut control = held_by_a();
    control.connection_lost("a", 1000.0);
    assert_eq!(
        control.take(PROJECT, device("b")),
        Err(Refused::HeldBy(device("a")))
    );
}

#[test]
fn the_same_device_may_take_again_and_is_connected_afterwards() {
    let mut control = held_by_a();
    control.connection_lost("a", 1000.0);
    assert_eq!(control.take(PROJECT, device("a")), Ok(()));
    assert_eq!(
        control.holder(PROJECT).map(|holder| holder.link),
        Some(Link::Connected)
    );
}

#[test]
fn take_is_refused_for_a_revoked_device() {
    let mut control = shared_control();
    control.revoke_device("a");
    assert_eq!(
        control.take(PROJECT, device("a")),
        Err(Refused::DeviceRevoked)
    );
}

#[test]
fn give_back_frees_the_project_and_keeps_it_shared() {
    let mut control = held_by_a();
    assert_eq!(control.give_back(PROJECT, "a"), Ok(()));
    assert!(is_host_again(&control, PROJECT));
}

#[test]
fn give_back_is_refused_for_anyone_but_the_holder() {
    let mut control = held_by_a();
    assert_eq!(control.give_back(PROJECT, "b"), Err(Refused::NotHolder));
    assert!(control.holder(PROJECT).is_some());
}

#[test]
fn losing_the_connection_marks_the_link_and_keeps_the_holder() {
    let mut control = held_by_a();
    assert_eq!(control.connection_lost("a", 1000.0), vec![PROJECT]);
    assert_eq!(
        control.holder(PROJECT).map(|holder| holder.link),
        Some(Link::Lost { since: 1000.0 })
    );
}

#[test]
fn a_repeated_loss_does_not_push_the_deadline_back() {
    let mut control = held_by_a();
    control.connection_lost("a", 1000.0);
    control.connection_lost("a", 2000.0);
    assert_eq!(
        control.holder(PROJECT).map(|holder| holder.link),
        Some(Link::Lost { since: 1000.0 })
    );
}

#[test]
fn reconnecting_restores_the_link_without_a_new_take() {
    let mut control = held_by_a();
    control.connection_lost("a", 1000.0);
    assert_eq!(control.reconnected("a"), vec![PROJECT]);
    assert_eq!(
        control.holder(PROJECT).map(|holder| holder.link),
        Some(Link::Connected)
    );
}

#[test]
fn reconnecting_another_device_changes_nothing() {
    let mut control = held_by_a();
    control.connection_lost("a", 1000.0);
    assert!(control.reconnected("b").is_empty());
    assert_eq!(
        control.holder(PROJECT).map(|holder| holder.link),
        Some(Link::Lost { since: 1000.0 })
    );
}

#[test]
fn five_minutes_lost_free_the_project_and_keep_it_shared() {
    let mut control = held_by_a();
    control.connection_lost("a", 1000.0);
    assert_eq!(control.expire(1000.0 + LOST_LIMIT_MS), vec![PROJECT]);
    assert!(is_host_again(&control, PROJECT));
}

#[test]
fn just_under_five_minutes_lost_keep_the_holder() {
    let mut control = held_by_a();
    control.connection_lost("a", 1000.0);
    assert!(control.expire(1000.0 + LOST_LIMIT_MS - 1.0).is_empty());
    assert!(control.holder(PROJECT).is_some());
}

#[test]
fn a_connected_holder_never_expires() {
    let mut control = held_by_a();
    assert!(control.expire(1000.0 * LOST_LIMIT_MS).is_empty());
    assert!(control.holder(PROJECT).is_some());
}

#[test]
fn another_device_may_take_after_the_deadline() {
    let mut control = held_by_a();
    control.connection_lost("a", 1000.0);
    control.expire(1000.0 + LOST_LIMIT_MS);
    assert_eq!(control.take(PROJECT, device("b")), Ok(()));
}

#[test]
fn a_restart_of_the_host_keeps_shared_and_has_no_holder() {
    let mut control = Control::restore([PROJECT.to_owned()], ["a".to_owned()]);
    assert!(is_host_again(&control, PROJECT));
    assert!(!control.has_holder());
    assert_eq!(
        control.take(PROJECT, device("a")),
        Err(Refused::DeviceRevoked)
    );
    assert_eq!(control.take(PROJECT, device("b")), Ok(()));
}

#[test]
fn server_off_frees_every_project_and_keeps_them_shared() {
    let mut control = held_by_a();
    control.share(OTHER_PROJECT);
    control
        .take(OTHER_PROJECT, device("b"))
        .expect("freigegeben");
    assert_eq!(control.server_off(), vec![OTHER_PROJECT, PROJECT]);
    assert!(is_host_again(&control, PROJECT));
    assert!(is_host_again(&control, OTHER_PROJECT));
}

#[test]
fn revoking_a_device_frees_only_its_projects() {
    let mut control = held_by_a();
    control.share(OTHER_PROJECT);
    control
        .take(OTHER_PROJECT, device("b"))
        .expect("freigegeben");
    assert_eq!(control.revoke_device("a"), vec![PROJECT]);
    assert!(is_host_again(&control, PROJECT));
    assert!(control.holder(OTHER_PROJECT).is_some());
    assert!(control.is_shared(PROJECT));
}

#[test]
fn archiving_without_a_holder_ends_the_sharing_only_with_the_step_after_the_database() {
    let mut control = shared_control();
    assert_eq!(control.may_archive(PROJECT), Ok(()));
    assert!(control.is_shared(PROJECT));
    control.unshare(PROJECT);
    assert!(!control.is_shared(PROJECT));
}

#[test]
fn archiving_is_refused_while_a_guest_is_holder() {
    let control = held_by_a();
    assert_eq!(
        control.may_archive(PROJECT),
        Err(Refused::HeldBy(device("a")))
    );
    assert!(control.is_shared(PROJECT));
}

#[test]
fn forgetting_a_deleted_project_leaves_nothing_behind() {
    let mut control = held_by_a();
    control.forget(PROJECT);
    assert!(!control.is_shared(PROJECT));
    assert!(!control.has_holder());
}

#[test]
fn a_holder_always_implies_shared_whatever_the_order() {
    let mut control = held_by_a();
    control.unshare(PROJECT);
    assert_eq!(control.give_back(PROJECT, "a"), Err(Refused::NotHolder));
    assert_eq!(control.take(PROJECT, device("a")), Err(Refused::NotShared));
    assert!(!control.has_holder());
}
