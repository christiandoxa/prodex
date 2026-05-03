use super::*;

#[test]
fn anthropic_compat_json_response_plans_static_routes() {
    let root =
        runtime_proxy_anthropic_compat_json_response("GET", "/?ready=true", "0.1.2").unwrap();
    assert_eq!(root.status, 200);
    assert_eq!(
        root.body.get("version").and_then(serde_json::Value::as_str),
        Some("0.1.2")
    );

    let health = runtime_proxy_anthropic_compat_json_response("HEAD", "/health", "0.1.2").unwrap();
    assert_eq!(
        health
            .body
            .get("status")
            .and_then(serde_json::Value::as_str),
        Some("ok")
    );

    assert!(runtime_proxy_anthropic_compat_json_response("POST", "/", "0.1.2").is_none());
}
