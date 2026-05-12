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
fn builds_text_response_parts() {
    let parts = build_runtime_proxy_text_response_parts(502, "upstream unavailable");

    assert_eq!(parts.status, 502);
    assert_eq!(
        runtime_buffered_response_content_type(&parts),
        Some("text/plain; charset=utf-8")
    );
    assert_eq!(parts.body.as_slice(), b"upstream unavailable");
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

#[test]
fn ignores_missing_empty_or_invalid_content_type() {
    let missing = RuntimeBufferedResponseParts {
        status: 204,
        headers: Vec::new(),
        body: Vec::new().into(),
    };
    let empty = RuntimeBufferedResponseParts {
        status: 204,
        headers: vec![("Content-Type".to_string(), b" \t ".to_vec())],
        body: Vec::new().into(),
    };
    let invalid = RuntimeBufferedResponseParts {
        status: 204,
        headers: vec![("Content-Type".to_string(), vec![0xff])],
        body: Vec::new().into(),
    };

    assert_eq!(runtime_buffered_response_content_type(&missing), None);
    assert_eq!(runtime_buffered_response_content_type(&empty), None);
    assert_eq!(runtime_buffered_response_content_type(&invalid), None);
}

#[test]
fn managed_response_body_exposes_bytes_without_transport_types() {
    let body = RuntimeManagedResponseBody::from(&b"abc"[..]);

    assert_eq!(body.as_slice(), b"abc");
    assert_eq!(&*body, &b"abc".to_vec());
    assert_eq!(body.iter().copied().collect::<Vec<_>>(), b"abc");
    assert_eq!(body.into_vec(), b"abc".to_vec());
}
