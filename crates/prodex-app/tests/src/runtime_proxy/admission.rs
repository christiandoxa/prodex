use super::*;

#[test]
fn local_overload_response_includes_retry_after_hint() {
    let response =
        runtime_proxy_response_with_retry_after(build_runtime_proxy_text_response(503, "busy"));
    let mut bytes = Vec::new();
    response
        .raw_print(&mut bytes, (1, 0).into(), &[], false, None)
        .expect("response should serialize");
    let text = String::from_utf8(bytes).expect("response should be utf8");

    assert!(text.contains("\r\nRetry-After: 1\r\n"));
}

#[test]
fn local_overload_deadline_preserves_full_backoff_across_second_boundary() {
    assert_eq!(runtime_proxy_local_overload_deadline_seconds(1_000, 1), 2);
    assert_eq!(runtime_proxy_local_overload_deadline_seconds(1_999, 1), 3);
    assert_eq!(runtime_proxy_local_overload_deadline_seconds(1_001, 3), 5);
}
