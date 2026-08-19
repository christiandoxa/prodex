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
