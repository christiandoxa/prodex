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
