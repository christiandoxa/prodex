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
