use super::*;

#[test]
fn builds_json_error_parts() {
    let parts = build_runtime_proxy_json_error_parts(503, "service_unavailable", "retry");

    assert_eq!(parts.status, 503);
    assert_eq!(
        runtime_buffered_response_content_type(&parts),
        Some("application/json")
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&parts.body).unwrap(),
        serde_json::json!({
            "error": {
                "code": "service_unavailable",
                "message": "retry"
            }
        })
    );
}

#[test]
fn finds_content_type_case_insensitively() {
    let parts = RuntimeBufferedResponseParts {
        status: 200,
        headers: vec![("content-type".to_string(), b" text/event-stream ".to_vec())],
        body: Vec::new().into(),
    };

    assert_eq!(
        runtime_buffered_response_content_type(&parts),
        Some("text/event-stream")
    );
}
