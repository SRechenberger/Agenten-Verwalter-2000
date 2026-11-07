//! Der Zwischenspeicher des Gastes: Was der Host meldet, ersetzt oder ergänzt den Eintrag.
use serde_json::json;

use super::*;

#[test]
fn a_reported_entry_replaces_the_cached_one_with_the_same_id() {
    let mut items = vec![
        json!({ "id": "a", "status": "running" }),
        json!({ "id": "b", "status": "idle" }),
    ];
    replace_or_push(&mut items, &json!({ "id": "a", "status": "paused" }));
    assert_eq!(
        items,
        vec![
            json!({ "id": "a", "status": "paused" }),
            json!({ "id": "b", "status": "idle" }),
        ]
    );
}

#[test]
fn a_reported_entry_with_a_new_id_is_appended() {
    let mut items = vec![json!({ "id": "a" })];
    replace_or_push(&mut items, &json!({ "id": "neu", "projectId": "p" }));
    assert_eq!(
        items,
        vec![
            json!({ "id": "a" }),
            json!({ "id": "neu", "projectId": "p" })
        ]
    );
}

#[test]
fn a_reported_entry_without_an_id_leaves_the_cache_unchanged() {
    let mut items = vec![json!({ "id": "a" })];
    replace_or_push(&mut items, &json!({ "projectId": "p" }));
    assert_eq!(items, vec![json!({ "id": "a" })]);
}
