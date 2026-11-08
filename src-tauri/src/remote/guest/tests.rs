//! Was `chat_send` am Gast mit den Anhängen tut, bevor es zum Host geht (ADR 059).
use super::*;

fn upload_known(local: &str) -> Result<Option<String>, CommandError> {
    Ok(match local {
        "lokal-1" => Some("host-1".to_owned()),
        "lokal-2" => Some("host-2".to_owned()),
        _ => None,
    })
}

#[test]
fn local_attachments_are_replaced_by_the_ids_the_host_gave() {
    let args = json!({ "sessionId": "s", "attachmentIds": ["lokal-1", "lokal-2"] });
    let (args, uploaded) = replace_attachment_ids(args, upload_known).expect("ersetzt");
    assert_eq!(args["attachmentIds"], json!(["host-1", "host-2"]));
    assert_eq!(uploaded, vec!["lokal-1".to_owned(), "lokal-2".to_owned()]);
}

#[test]
fn an_attachment_id_unknown_here_goes_through_unchanged() {
    let args = json!({ "attachmentIds": ["lokal-1", "lagebericht-am-host", "lokal-2"] });
    let (args, uploaded) = replace_attachment_ids(args, upload_known).expect("ersetzt");
    assert_eq!(
        args["attachmentIds"],
        json!(["host-1", "lagebericht-am-host", "host-2"])
    );
    assert_eq!(uploaded, vec!["lokal-1".to_owned(), "lokal-2".to_owned()]);
}

#[test]
fn a_message_without_attachments_is_not_touched() {
    let args = json!({ "sessionId": "s", "text": "Hallo", "attachmentIds": [] });
    let (after, uploaded) = replace_attachment_ids(args.clone(), upload_known).expect("ersetzt");
    assert_eq!(after, args);
    assert!(uploaded.is_empty());
}

#[test]
fn a_call_without_the_attachment_field_is_not_touched() {
    let args = json!({ "sessionId": "s" });
    let (after, uploaded) = replace_attachment_ids(args.clone(), upload_known).expect("ersetzt");
    assert_eq!(after, args);
    assert!(uploaded.is_empty());
}

#[test]
fn a_failed_upload_stops_the_send_and_deletes_nothing() {
    let args = json!({ "attachmentIds": ["lokal-1", "kaputt"] });
    let result = replace_attachment_ids(args, |local: &str| {
        if local == "kaputt" {
            Err(CommandError::Io("zu groß".to_owned()))
        } else {
            upload_known(local)
        }
    });
    assert!(matches!(result, Err(CommandError::Io(_))));
}
