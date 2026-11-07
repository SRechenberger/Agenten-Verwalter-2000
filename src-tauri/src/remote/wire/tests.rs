//! Die Rahmen genau so, wie der Vertrag sie schreibt, und die Grenzen von Zeile und Warteschlange.
use serde_json::json;

use super::*;

fn round_trip(frame: Frame, expected: Value) {
    let line = encode(&frame);
    assert_eq!(line.last(), Some(&b'\n'));
    let written: Value = serde_json::from_slice(&line).expect("JSON");
    assert_eq!(written, expected);
    assert_eq!(decode(&line[..line.len() - 1]).expect("lesbar"), frame);
}

#[test]
fn every_frame_has_the_shape_of_the_contract() {
    round_trip(
        Frame::Pair {
            protocol: PROTOCOL,
            secret: "s".to_owned(),
            device_name: "laptop".to_owned(),
        },
        json!({"type": "pair", "protocol": PROTOCOL, "secret": "s", "deviceName": "laptop"}),
    );
    round_trip(
        Frame::Hello {
            protocol: PROTOCOL,
            device_id: "d".to_owned(),
            token: "t".to_owned(),
        },
        json!({"type": "hello", "protocol": PROTOCOL, "deviceId": "d", "token": "t"}),
    );
    round_trip(
        Frame::Paired {
            device_id: "d".to_owned(),
            token: "t".to_owned(),
        },
        json!({"type": "paired", "deviceId": "d", "token": "t"}),
    );
    round_trip(
        Frame::Welcome {
            host_id: "h".to_owned(),
            host_name: "linux-host".to_owned(),
        },
        json!({"type": "welcome", "hostId": "h", "hostName": "linux-host"}),
    );
    round_trip(
        Frame::Refused {
            reason: RefuseReason::Revoked,
            message: "Satz".to_owned(),
            protocol: PROTOCOL,
        },
        json!({"type": "refused", "reason": "revoked", "message": "Satz", "protocol": PROTOCOL}),
    );
    round_trip(
        Frame::Call {
            id: 7,
            command: "session_list".to_owned(),
            args: json!({}),
        },
        json!({"type": "call", "id": 7, "command": "session_list", "args": {}}),
    );
    round_trip(
        Frame::Event {
            name: "chat://entry".to_owned(),
            payload: json!({"sessionId": "s"}),
        },
        json!({"type": "event", "name": "chat://entry", "payload": {"sessionId": "s"}}),
    );
    round_trip(
        Frame::Access {
            project_id: "p".to_owned(),
        },
        json!({"type": "access", "projectId": "p"}),
    );
    round_trip(
        Frame::Terminal {
            id: "t".to_owned(),
            output: json!({"kind": "data"}),
        },
        json!({"type": "terminal", "id": "t", "output": {"kind": "data"}}),
    );
    round_trip(Frame::Ping, json!({"type": "ping"}));
}

#[test]
fn a_result_carries_either_ok_or_err_next_to_its_id() {
    round_trip(
        Frame::Result {
            id: 3,
            outcome: Outcome::Ok(Value::Null),
        },
        json!({"type": "result", "id": 3, "ok": null}),
    );
    round_trip(
        Frame::Result {
            id: 4,
            outcome: Outcome::Err(json!({"kind": "hostUnreachable", "message": "h"})),
        },
        json!({"type": "result", "id": 4, "err": {"kind": "hostUnreachable", "message": "h"}}),
    );
}

#[test]
fn every_refuse_reason_is_written_as_the_contract_names_it() {
    for (reason, text) in [
        (RefuseReason::Protocol, "protocol"),
        (RefuseReason::Invitation, "invitation"),
        (RefuseReason::Token, "token"),
        (RefuseReason::Revoked, "revoked"),
    ] {
        assert_eq!(serde_json::to_value(reason).expect("JSON"), json!(text));
    }
}

#[test]
fn lines_come_out_one_by_one_even_when_split_across_reads() {
    let mut lines = LineBuffer::new(MAX_LINE);
    lines.push(b"{\"type\":");
    assert!(lines.next_line().expect("kein Fehler").is_none());
    lines.push(b"\"ping\"}\n{\"type\":\"ping\"}\nRest");
    assert_eq!(
        lines.next_line().expect("kein Fehler"),
        Some(b"{\"type\":\"ping\"}".to_vec())
    );
    assert_eq!(
        lines.next_line().expect("kein Fehler"),
        Some(b"{\"type\":\"ping\"}".to_vec())
    );
    assert!(lines.next_line().expect("kein Fehler").is_none());
}

#[test]
fn a_line_at_the_limit_passes_and_one_byte_more_is_refused_before_its_end() {
    let mut lines = LineBuffer::new(8);
    lines.push(b"12345678\n");
    assert_eq!(
        lines.next_line().expect("genau an der Grenze"),
        Some(b"12345678".to_vec())
    );
    lines.push(b"123456789");
    assert!(matches!(lines.next_line(), Err(WireError::TooLong)));
}

#[test]
fn the_contract_limits_are_64_mib_per_line_and_16_mib_per_queue() {
    assert_eq!(MAX_LINE, 64 * 1024 * 1024);
    assert_eq!(MAX_QUEUE, 16 * 1024 * 1024);
    assert_eq!(Timing::CONTRACT.ping_interval, Duration::from_secs(15));
    assert_eq!(Timing::CONTRACT.idle_limit, Duration::from_secs(45));
}

#[test]
fn the_queue_holds_at_most_its_limit_even_for_a_single_line() {
    let outbox = Outbox::new(10);
    assert_eq!(outbox.push(vec![0; 11]), Err(SendError::Full));
    assert_eq!(outbox.push(vec![0; 10]), Ok(()));
    assert_eq!(outbox.push(vec![0; 1]), Err(SendError::Full));
    assert!(matches!(outbox.next(Instant::now()), Next::Line(line) if line.len() == 10));
    assert_eq!(outbox.push(vec![0; 6]), Ok(()));
    assert_eq!(outbox.push(vec![0; 4]), Ok(()));
    assert_eq!(outbox.push(vec![0; 1]), Err(SendError::Full));
    outbox.close();
    assert_eq!(outbox.push(vec![0; 1]), Err(SendError::Closed));
}
