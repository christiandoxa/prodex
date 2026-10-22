use super::{OUTPUT_VERIFY_MAX_LINE_BYTES, exact_user_message_line, output_line_plan};
use prodex_mojo_core::session_cli_policy::SessionPromptWriteOutputLineAction::{
    Malformed, Oversized, Process,
};
use serde_json::json;

#[test]
fn decoded_exact_delivery_keeps_serde_escaping_duplicates_and_part_precedence() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) }
    let mut record = json!({"type":"response_item", "payload":{
        "type":"message", "role":"user", "content":[
            {"text":"  東京🧭\0\"\\ ", "content":"ignored"}, {"content":"\nend "}
        ]
    }});
    let expected = "  東京🧭\0\"\\ \n\nend ";
    assert!(exact_user_message_line(&serde_json::to_vec(&record).unwrap(), expected).unwrap());
    assert!(
        !exact_user_message_line(&serde_json::to_vec(&record).unwrap(), expected.trim()).unwrap()
    );
    record["payload"]["content"][0]["text"] = serde_json::Value::Null;
    assert!(
        !exact_user_message_line(&serde_json::to_vec(&record).unwrap(), "ignored\n\nend ").unwrap()
    );
    record["payload"]["content"] = json!([{"text":""}, {"text":""}]);
    assert!(exact_user_message_line(&serde_json::to_vec(&record).unwrap(), "\n").unwrap());
    record["payload"]["content"] = json!([]);
    assert!(!exact_user_message_line(&serde_json::to_vec(&record).unwrap(), "").unwrap());
    for (line, expected) in [
        (
            r#"{"type":"event_msg","payload":{"type":"user_message","message":"old","message":"last"}}"#,
            true,
        ),
        (
            r#"{"type":"event_msg","payload":{"type":"user_message","message":"last","message":null}}"#,
            false,
        ),
        (
            r#"{"type":"event_msg","payload":{"type":"user_message","message":"last"},"unused":1e400}"#,
            false,
        ),
        (
            r#"{"type":"event_msg","payload":{"type":"user_message","message":"last"},"unused":1e30}"#,
            true,
        ),
    ] {
        assert_eq!(
            exact_user_message_line(line.as_bytes(), "last").unwrap(),
            expected
        );
    }
    assert!(!exact_user_message_line(b"\xff\n", "").unwrap());
    assert!(!exact_user_message_line(b"{", "").unwrap());
}

#[test]
fn decoded_visible_user_exemption_keeps_metadata_types_and_ignores_content_shape() {
    let mut record = json!({"type":"response_item","payload":{
        "type":"message", "role":"user", "padding":"x".repeat(70_000)
    }});
    assert_eq!(
        output_line_plan(&serde_json::to_vec(&record).unwrap(), false),
        Ok(Process)
    );
    assert!(!exact_user_message_line(&serde_json::to_vec(&record).unwrap(), "").unwrap());
    for (kinds, expected) in [
        (json!(["user.text"]), Process),
        (json!([]), Oversized),
        (json!(["user.text", 1]), Oversized),
        (json!(["internal"]), Oversized),
        (json!(null), Process),
        (json!({}), Process),
        (json!("internal"), Process),
    ] {
        record["payload"]["internal_chat_message_metadata_passthrough"] =
            json!({"content_item_kinds":kinds});
        assert_eq!(
            output_line_plan(&serde_json::to_vec(&record).unwrap(), false),
            Ok(expected)
        );
    }
    record["payload"]["role"] = json!("assistant");
    assert_eq!(
        output_line_plan(&serde_json::to_vec(&record).unwrap(), false),
        Ok(Oversized)
    );
    record["type"] = json!("event_msg");
    record["payload"]["type"] = json!("user_message");
    record["payload"]["message"] = json!("visible");
    record["payload"]["internal_chat_message_metadata_passthrough"] =
        json!({"content_item_kinds":[]});
    assert_eq!(
        output_line_plan(&serde_json::to_vec(&record).unwrap(), false),
        Ok(Process)
    );
}

#[test]
fn decoded_record_shape_keeps_missing_null_scalar_and_known_record_contracts() {
    for (record, expected) in [
        (
            json!({"type":"event_msg", "payload":{"type":"unknown"}}),
            Process,
        ),
        (json!({"type":"response_item", "payload":{}}), Malformed),
        (json!({"type":"session_meta", "payload":{}}), Process),
        (json!({"type":"turn_context", "payload":[]}), Malformed),
        (json!({"type":"unknown", "payload":null}), Process),
        (json!({"type":1, "payload":false}), Process),
        (json!({"payload":[]}), Process),
        (json!({"type":"unknown"}), Malformed),
        (json!(null), Malformed),
    ] {
        assert_eq!(
            output_line_plan(&serde_json::to_vec(&record).unwrap(), false),
            Ok(expected)
        );
    }
    assert_eq!(output_line_plan(b"{", false), Ok(Malformed));
}

#[test]
fn decoded_exact_delivery_preserves_raw_line_bound_including_newline() {
    let expected = "\t".repeat(65_536);
    let mut line = serde_json::to_vec(&json!({"type":"event_msg", "payload":{
        "type":"user_message", "message":expected
    }}))
    .unwrap();
    line.resize(OUTPUT_VERIFY_MAX_LINE_BYTES - 1, b' ');
    line.push(b'\n');
    assert!(exact_user_message_line(&line, &expected).unwrap());
    line.push(b' ');
    assert!(!exact_user_message_line(&line, &expected).unwrap());
}
