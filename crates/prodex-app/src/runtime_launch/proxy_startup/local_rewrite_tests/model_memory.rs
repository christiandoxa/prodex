use super::super::local_rewrite::RuntimeLocalRewriteModelMemoryState;
use super::super::local_rewrite_model_memory::{
    RUNTIME_LOCAL_REWRITE_MODEL_MEMORY_LIMIT, runtime_local_rewrite_model_allows_session_memory,
    runtime_local_rewrite_model_scope,
};
use super::super::provider_bridge::RuntimeProviderBridgeKind;
use crate::RuntimeProxyRequest;

#[test]
fn local_rewrite_model_memory_is_scoped_by_provider_and_session() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/responses".to_string(),
        headers: vec![("session_id".to_string(), "sess-a".to_string())],
        body: Vec::new(),
    };
    let body = serde_json::to_vec(&serde_json::json!({"model": "auto"})).unwrap();
    let gemini_scope =
        runtime_local_rewrite_model_scope(RuntimeProviderBridgeKind::Gemini, &request, &body)
            .unwrap();
    let deepseek_scope =
        runtime_local_rewrite_model_scope(RuntimeProviderBridgeKind::DeepSeek, &request, &body)
            .unwrap();
    let mut memory = RuntimeLocalRewriteModelMemoryState::default();

    memory.remember_selected_model(&gemini_scope, "gemini-3.1-pro-preview");

    assert_eq!(
        memory.selected_model(&gemini_scope).as_deref(),
        Some("gemini-3.1-pro-preview")
    );
    assert_eq!(memory.selected_model(&deepseek_scope), None);
}

#[test]
fn local_rewrite_model_memory_only_overrides_auto_like_models() {
    assert!(runtime_local_rewrite_model_allows_session_memory("auto"));
    assert!(runtime_local_rewrite_model_allows_session_memory("default"));
    assert!(runtime_local_rewrite_model_allows_session_memory(""));
    assert!(!runtime_local_rewrite_model_allows_session_memory(
        "claude-sonnet-4-6"
    ));
}

#[test]
fn local_rewrite_model_memory_is_bounded() {
    let mut memory = RuntimeLocalRewriteModelMemoryState::default();
    for index in 0..=RUNTIME_LOCAL_REWRITE_MODEL_MEMORY_LIMIT {
        memory.remember_selected_model(&format!("session-{index:04}"), "model");
    }

    assert_eq!(memory.selected_model("session-0000"), None);
    assert_eq!(
        memory
            .selected_model(&format!(
                "session-{RUNTIME_LOCAL_REWRITE_MODEL_MEMORY_LIMIT:04}"
            ))
            .as_deref(),
        Some("model")
    );
}
