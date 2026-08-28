use super::*;

#[test]
fn previous_response_message_detects_invalid_request_id_shape() {
    let payload = serde_json::json!({
        "type": "error",
        "status": 400,
        "error": {
            "type": "invalid_request_error",
            "message": "Invalid `previous_response_id`."
        }
    });

    assert_eq!(
        extract_runtime_proxy_previous_response_message_from_value(&payload),
        Some("Invalid `previous_response_id`.".to_string())
    );
    assert_eq!(
        extract_runtime_proxy_previous_response_message(payload.to_string().as_bytes()),
        Some("Invalid `previous_response_id`.".to_string())
    );
    assert!(runtime_proxy_value_is_invalid_previous_response_id(
        &payload
    ));
    assert!(runtime_proxy_body_is_invalid_previous_response_id(
        payload.to_string().as_bytes()
    ));
}

#[test]
fn previous_response_message_does_not_classify_unrelated_invalid_request() {
    let payload = serde_json::json!({
        "type": "error",
        "status": 400,
        "error": {
            "type": "invalid_request_error",
            "message": "Invalid input."
        }
    });

    assert_eq!(
        extract_runtime_proxy_previous_response_message_from_value(&payload),
        None
    );
}

#[test]
fn previous_response_message_requires_the_exact_invalid_id_error() {
    for message in [
        "The previous_response_id is invalid for another reason.",
        "Invalid previous_response_id.",
        "Invalid `previous_response_id` value.",
    ] {
        let payload = serde_json::json!({
            "type": "error",
            "status": 400,
            "error": {
                "type": "invalid_request_error",
                "message": message
            }
        });

        assert!(!runtime_proxy_value_is_invalid_previous_response_id(
            &payload
        ));
    }
}

#[test]
fn exact_websocket_invalid_id_is_normalized_for_codex_full_replay() {
    let original = serde_json::json!({
        "type": "error",
        "status": 400,
        "error": {
            "type": "invalid_request_error",
            "message": "Invalid `previous_response_id`."
        }
    })
    .to_string();

    let RuntimeWebsocketErrorPayload::Text(translated) =
        runtime_translate_invalid_previous_response_websocket_error(
            RuntimeWebsocketErrorPayload::Text(original),
        )
    else {
        panic!("text payload should remain text");
    };
    let translated: serde_json::Value = serde_json::from_str(&translated).expect("valid JSON");

    assert_eq!(translated["error"]["code"], "previous_response_not_found");
    assert_eq!(
        translated["error"]["message"],
        "Invalid `previous_response_id`."
    );
}

#[test]
fn websocket_invalid_id_normalization_does_not_rewrite_near_matches() {
    let original = RuntimeWebsocketErrorPayload::Text(
        serde_json::json!({
            "type": "error",
            "status": 400,
            "error": {
                "type": "invalid_request_error",
                "message": "Invalid `previous_response_id` value."
            }
        })
        .to_string(),
    );

    assert_eq!(
        runtime_translate_invalid_previous_response_websocket_error(original.clone()),
        original
    );
}

#[test]
fn inspect_sse_buffer_commits_before_later_retryable_failure() {
    let body = concat!(
        "event: response.output_text.delta\r\n",
        "data: {\"type\":\"response.output_text.delta\",\"response\":{\"id\":\"resp-output\"},\"delta\":\"once\"}\r\n",
        "\r\n",
        "event: response.failed\r\n",
        "data: {\"type\":\"response.failed\",\"response\":{\"error\":{\"code\":\"insufficient_quota\"}}}\r\n",
        "\r\n",
    );

    assert_eq!(
        inspect_runtime_sse_buffer(body.as_bytes()),
        RuntimeSseInspectionProgress::Commit {
            response_ids: vec!["resp-output".to_string()],
            turn_state: None,
        }
    );
}

#[test]
fn inspect_sse_profile_unavailable_is_retryable_without_quota_classification() {
    let body = concat!(
        "event: response.failed\n",
        "data: {\"type\":\"response.failed\",\"error\":{\"code\":\"deactivated_workspace\"}}\n",
        "\n",
    );

    assert_eq!(
        inspect_runtime_sse_buffer(body.as_bytes()),
        RuntimeSseInspectionProgress::Overloaded
    );
}

#[test]
fn inspect_sse_buffer_drops_invalid_utf8_event_and_recovers() {
    let body = &b"data: {\"type\":\"response.created\",\"response_id\":\"resp-\xff\"}\n\n\
data: {\"type\":\"response.completed\",\"response_id\":\"resp-2\"}\n\n"[..];

    assert_eq!(
        inspect_runtime_sse_buffer(body),
        RuntimeSseInspectionProgress::Commit {
            response_ids: vec!["resp-2".to_string()],
            turn_state: None,
        }
    );
}

#[test]
fn inspect_sse_buffer_does_not_turn_a_prefix_budget_into_eof() {
    let body = b"data: {\"type\":\"response.completed\",\"response_id\":\"resp-prefix\"}\n";

    assert_eq!(
        inspect_runtime_sse_buffer(body),
        RuntimeSseInspectionProgress::Hold {
            response_ids: Vec::new(),
            turn_state: None,
        }
    );
    assert_eq!(
        inspect_runtime_sse_buffer_at_eof(body),
        RuntimeSseInspectionProgress::Commit {
            response_ids: vec!["resp-prefix".to_string()],
            turn_state: None,
        }
    );
}

#[test]
fn body_snippet_normalizes_whitespace_and_truncates() {
    assert_eq!(
        runtime_proxy_body_snippet(b" one\n two\tthree ", 7),
        "one two..."
    );
    assert_eq!(runtime_proxy_body_snippet(b"  \n\t  ", 7), "-");
    assert_eq!(
        runtime_proxy_body_snippet(b"bad-\xff bytes", 64),
        "bad-\u{fffd} bytes"
    );
}

#[test]
fn response_ids_from_payload_matches_body_bytes_and_ignores_invalid_json() {
    let payload = r#"{
        "response": {"id": "resp-a"},
        "response_id": "resp-b",
        "object": "response",
        "id": "resp-c"
    }"#;

    assert_eq!(
        extract_runtime_response_ids_from_payload(payload),
        extract_runtime_response_ids_from_body_bytes(payload.as_bytes())
    );
    assert_eq!(
        extract_runtime_response_ids_from_payload(payload),
        vec![
            "resp-a".to_string(),
            "resp-b".to_string(),
            "resp-c".to_string()
        ]
    );
    assert!(extract_runtime_response_ids_from_payload("{").is_empty());
}
