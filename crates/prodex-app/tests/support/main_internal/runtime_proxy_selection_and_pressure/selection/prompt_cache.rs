use super::*;

#[test]
fn response_request_prompt_cache_key_is_trimmed_from_body() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body: serde_json::to_vec(&serde_json::json!({
            "prompt_cache_key": " workspace-cache:abc ",
            "input": []
        }))
        .expect("request body should serialize"),
    };

    assert_eq!(
        runtime_request_prompt_cache_key(&request).as_deref(),
        Some("workspace-cache:abc")
    );
}
