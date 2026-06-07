use super::{
    RuntimeGeminiBindingRecorder, RuntimeGeminiOAuthPool, RuntimeGeminiOAuthPoolState,
    RuntimeGeminiOAuthProfileAuth, RuntimeGeminiPrecommitDecision, RuntimeGeminiPrecommitProbe,
    runtime_gemini_initial_oauth_pool_index, runtime_gemini_now_ms,
    runtime_gemini_precommit_decision_for_data_lines,
    runtime_gemini_remember_bindings_from_responses_body,
    runtime_gemini_should_inline_rate_limit_retry,
    runtime_gemini_should_rotate_after_quota_response,
};
use crate::{GeminiOAuthSecret, gemini_code_assist_endpoint};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

fn gemini_profile(profile_name: &str) -> RuntimeGeminiOAuthProfileAuth {
    RuntimeGeminiOAuthProfileAuth {
        profile_name: profile_name.to_string(),
        codex_home: std::env::temp_dir().join(format!("prodex-gemini-{profile_name}")),
        email: Some(format!("{profile_name}@example.com")),
        access_token: format!("token-{profile_name}"),
        project_id: Some(format!("project-{profile_name}")),
    }
}

fn gemini_pool(profile_names: &[&str]) -> RuntimeGeminiOAuthPool {
    RuntimeGeminiOAuthPool {
        state: Arc::new(Mutex::new(RuntimeGeminiOAuthPoolState {
            profiles: profile_names
                .iter()
                .map(|profile_name| gemini_profile(profile_name))
                .collect(),
            next_index: 0,
            response_profile_bindings: BTreeMap::new(),
            tool_call_profile_bindings: BTreeMap::new(),
            quota_headers: BTreeMap::new(),
            model_cooldowns_until: BTreeMap::new(),
            model_unavailable_until: BTreeMap::new(),
        })),
    }
}

#[test]
fn gemini_oauth_pool_rotates_fresh_requests() {
    let pool = gemini_pool(&["alpha", "beta"]);
    let body = serde_json::to_vec(&serde_json::json!({"input": "hi"})).unwrap();

    let first = pool.select_attempts(&body, &[]).unwrap();
    let second = pool.select_attempts(&body, &[]).unwrap();

    assert_eq!(first[0].profile_name, "alpha");
    assert_eq!(first[1].profile_name, "beta");
    assert!(!first[0].hard_affinity);
    assert_eq!(second[0].profile_name, "beta");
    assert_eq!(second[1].profile_name, "alpha");
}

#[test]
fn gemini_oauth_pool_initial_index_is_bounded() {
    assert_eq!(runtime_gemini_initial_oauth_pool_index(0), 0);
    assert_eq!(runtime_gemini_initial_oauth_pool_index(1), 0);
    assert!(runtime_gemini_initial_oauth_pool_index(6) < 6);
}

#[test]
fn gemini_oauth_pool_preserves_previous_response_affinity() {
    let pool = gemini_pool(&["alpha", "beta"]);
    pool.state
        .lock()
        .unwrap()
        .remember_bindings("beta", "resp_1", &[]);
    let body = serde_json::to_vec(&serde_json::json!({"previous_response_id": "resp_1"})).unwrap();

    let attempts = pool.select_attempts(&body, &[]).unwrap();

    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0].profile_name, "beta");
    assert!(attempts[0].hard_affinity);
}

#[test]
fn gemini_oauth_pool_preserves_tool_output_affinity() {
    let pool = gemini_pool(&["alpha", "beta"]);
    pool.state
        .lock()
        .unwrap()
        .remember_bindings("beta", "resp_1", &["call_1".to_string()]);
    let body = serde_json::to_vec(&serde_json::json!({
        "input": [{
            "type": "function_call_output",
            "call_id": "call_1",
            "output": "done"
        }]
    }))
    .unwrap();

    let attempts = pool.select_attempts(&body, &[]).unwrap();

    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0].profile_name, "beta");
    assert!(attempts[0].hard_affinity);
}

#[test]
fn gemini_oauth_pool_preserves_custom_tool_output_affinity() {
    let pool = gemini_pool(&["alpha", "beta"]);
    pool.state
        .lock()
        .unwrap()
        .remember_bindings("beta", "resp_1", &["call_patch_1".to_string()]);
    let body = serde_json::to_vec(&serde_json::json!({
        "input": [{
            "type": "custom_tool_call_output",
            "call_id": "call_patch_1",
            "output": "patched"
        }]
    }))
    .unwrap();

    let attempts = pool.select_attempts(&body, &[]).unwrap();

    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0].profile_name, "beta");
    assert!(attempts[0].hard_affinity);
}

#[test]
fn gemini_oauth_pool_skips_model_scoped_cooldown_for_fresh_requests() {
    let pool = gemini_pool(&["alpha", "beta"]);
    pool.state.lock().unwrap().remember_model_cooldown_until(
        "alpha",
        "gemini-2.5-pro",
        runtime_gemini_now_ms() + 60_000,
    );
    let body = serde_json::to_vec(&serde_json::json!({
        "model": "gemini-2.5-pro",
        "input": "hi"
    }))
    .unwrap();

    let attempts = pool.select_attempts(&body, &[]).unwrap();

    assert_eq!(attempts[0].profile_name, "beta");
    assert_eq!(attempts.len(), 1);
}

#[test]
fn gemini_oauth_pool_preserves_affinity_despite_model_cooldown() {
    let pool = gemini_pool(&["alpha", "beta"]);
    {
        let mut state = pool.state.lock().unwrap();
        state.remember_bindings("alpha", "resp_1", &[]);
        state.remember_model_cooldown_until(
            "alpha",
            "gemini-2.5-pro",
            runtime_gemini_now_ms() + 60_000,
        );
    }
    let body = serde_json::to_vec(&serde_json::json!({
        "model": "gemini-2.5-pro",
        "previous_response_id": "resp_1"
    }))
    .unwrap();

    let attempts = pool.select_attempts(&body, &[]).unwrap();

    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0].profile_name, "alpha");
    assert!(attempts[0].hard_affinity);
}

#[test]
fn gemini_oauth_pool_model_cooldown_is_model_scoped() {
    let pool = gemini_pool(&["alpha", "beta"]);
    pool.state.lock().unwrap().remember_model_cooldown_until(
        "alpha",
        "gemini-2.5-pro",
        runtime_gemini_now_ms() + 60_000,
    );
    let body = serde_json::to_vec(&serde_json::json!({
        "model": "gemini-2.5-flash",
        "input": "hi"
    }))
    .unwrap();

    let attempts = pool.select_attempts(&body, &[]).unwrap();

    assert_eq!(attempts[0].profile_name, "alpha");
    assert_eq!(attempts[1].profile_name, "beta");
}

#[test]
fn gemini_oauth_pool_skips_endpoint_scoped_unavailable_model_for_fresh_requests() {
    let pool = gemini_pool(&["alpha", "beta"]);
    pool.state.lock().unwrap().remember_model_unavailable_until(
        "alpha",
        &gemini_code_assist_endpoint(),
        "gemini-2.5-pro",
        runtime_gemini_now_ms() + 60_000,
    );
    let body = serde_json::to_vec(&serde_json::json!({
        "model": "gemini-2.5-pro",
        "input": "hi"
    }))
    .unwrap();

    let attempts = pool.select_attempts(&body, &[]).unwrap();

    assert_eq!(attempts[0].profile_name, "beta");
    assert_eq!(attempts.len(), 1);
}

#[test]
fn gemini_oauth_pool_model_unavailable_cache_is_endpoint_scoped() {
    let pool = gemini_pool(&["alpha"]);
    let endpoint = gemini_code_assist_endpoint();
    let other_endpoint = "https://generativelanguage.googleapis.com/v1beta";
    pool.state.lock().unwrap().remember_model_unavailable_until(
        "alpha",
        other_endpoint,
        "gemini-2.5-pro",
        runtime_gemini_now_ms() + 60_000,
    );
    let models = vec!["gemini-2.5-pro".to_string(), "gemini-2.5-flash".to_string()];

    let available = pool.available_model_chain_for_profile("alpha", &endpoint, &models);

    assert_eq!(available, models);
}

#[test]
fn gemini_oauth_pool_updates_refreshed_auth() {
    let pool = gemini_pool(&["alpha"]);
    let refreshed = GeminiOAuthSecret {
        auth_mode: "gemini_oauth".to_string(),
        access_token: "token-refreshed".to_string(),
        refresh_token: Some("refresh-alpha".to_string()),
        token_type: Some("Bearer".to_string()),
        scope: None,
        expiry_date: None,
        email: "alpha-refreshed@example.com".to_string(),
        project_id: Some("project-refreshed".to_string()),
    };

    let selected = pool
        .remember_refreshed_auth("alpha", refreshed, false)
        .unwrap()
        .unwrap();

    assert_eq!(selected.profile_name, "alpha");
    assert!(!selected.hard_affinity);
    let state = pool.state.lock().unwrap();
    let profile = state.profile_by_name("alpha").unwrap();
    assert_eq!(profile.access_token, "token-refreshed");
    assert_eq!(
        profile.email.as_deref(),
        Some("alpha-refreshed@example.com")
    );
    assert_eq!(profile.project_id.as_deref(), Some("project-refreshed"));
}

#[test]
fn gemini_binding_recorder_reads_responses_body() {
    let captured = Arc::new(Mutex::new(None::<(String, Vec<String>)>));
    let captured_for_recorder = Arc::clone(&captured);
    let recorder: RuntimeGeminiBindingRecorder = Arc::new(move |response_id, call_ids| {
        *captured_for_recorder.lock().unwrap() = Some((response_id, call_ids));
    });
    let body = serde_json::to_vec(&serde_json::json!({
        "id": "resp_1",
        "output": [{
            "type": "function_call",
            "call_id": "call_1",
            "name": "shell",
            "arguments": "{}"
        }]
    }))
    .unwrap();

    runtime_gemini_remember_bindings_from_responses_body(Some(&recorder), &body);

    let (response_id, call_ids) = captured.lock().unwrap().clone().unwrap();
    assert_eq!(response_id, "resp_1");
    assert_eq!(call_ids, vec!["call_1"]);
}

#[test]
fn gemini_binding_recorder_reads_custom_tool_calls() {
    let captured = Arc::new(Mutex::new(None::<(String, Vec<String>)>));
    let captured_for_recorder = Arc::clone(&captured);
    let recorder: RuntimeGeminiBindingRecorder = Arc::new(move |response_id, call_ids| {
        *captured_for_recorder.lock().unwrap() = Some((response_id, call_ids));
    });
    let body = serde_json::to_vec(&serde_json::json!({
        "id": "resp_patch_1",
        "output": [{
            "type": "custom_tool_call",
            "call_id": "call_patch_1",
            "name": "apply_patch",
            "input": "*** Begin Patch\n*** End Patch"
        }]
    }))
    .unwrap();

    runtime_gemini_remember_bindings_from_responses_body(Some(&recorder), &body);

    let (response_id, call_ids) = captured.lock().unwrap().clone().unwrap();
    assert_eq!(response_id, "resp_patch_1");
    assert_eq!(call_ids, vec!["call_patch_1"]);
}

#[test]
fn gemini_quota_rotation_predicate_respects_affinity_and_attempt_budget() {
    assert!(runtime_gemini_should_rotate_after_quota_response(
        429, false, 0, 2
    ));
    assert!(!runtime_gemini_should_rotate_after_quota_response(
        429, false, 0, 1
    ));
    assert!(!runtime_gemini_should_rotate_after_quota_response(
        429, true, 0, 2
    ));
    assert!(!runtime_gemini_should_rotate_after_quota_response(
        429, false, 1, 2
    ));
    assert!(!runtime_gemini_should_rotate_after_quota_response(
        500, false, 0, 2
    ));
}

#[test]
fn gemini_rate_limit_inline_retry_is_bounded() {
    assert!(!runtime_gemini_should_inline_rate_limit_retry(0));
    assert!(runtime_gemini_should_inline_rate_limit_retry(10_000));
    assert!(!runtime_gemini_should_inline_rate_limit_retry(10_001));
    assert!(!runtime_gemini_should_inline_rate_limit_retry(60_000));
}

#[test]
fn gemini_precommit_retries_malformed_stream_before_visible_output() {
    let data = vec![
        serde_json::json!({
            "responseId": "resp_bad",
            "candidates": [{
                "content": {"parts": []},
                "finishReason": "MALFORMED_FUNCTION_CALL"
            }]
        })
        .to_string(),
    ];
    let mut probe = RuntimeGeminiPrecommitProbe::default();

    let decision = runtime_gemini_precommit_decision_for_data_lines(&data, &mut probe);

    assert_eq!(
        decision,
        RuntimeGeminiPrecommitDecision::RetryableInvalid("MALFORMED_FUNCTION_CALL".to_string())
    );
}

#[test]
fn gemini_precommit_commits_once_visible_output_exists() {
    let data = vec![
        serde_json::json!({
            "responseId": "resp_text",
            "candidates": [{
                "content": {"parts": [{"text": "hi"}]}
            }]
        })
        .to_string(),
    ];
    let mut probe = RuntimeGeminiPrecommitProbe::default();

    let decision = runtime_gemini_precommit_decision_for_data_lines(&data, &mut probe);

    assert_eq!(decision, RuntimeGeminiPrecommitDecision::Commit);
}

#[test]
fn gemini_precommit_commits_native_code_execution_output() {
    let data = vec![
        serde_json::json!({
            "responseId": "resp_code",
            "candidates": [{
                "content": {"parts": [
                    {"executableCode": {"language": "PYTHON", "code": "print(4)"}},
                    {"codeExecutionResult": {"outcome": "OUTCOME_OK", "output": "4"}}
                ]},
                "finishReason": "STOP"
            }]
        })
        .to_string(),
    ];
    let mut probe = RuntimeGeminiPrecommitProbe::default();

    let decision = runtime_gemini_precommit_decision_for_data_lines(&data, &mut probe);

    assert_eq!(decision, RuntimeGeminiPrecommitDecision::Commit);
}

#[test]
fn gemini_precommit_commits_thought_only_stop_as_reasoning_output() {
    let data = vec![
        serde_json::json!({
            "responseId": "resp_thought",
            "candidates": [{
                "content": {"parts": [{"text": "reasoning", "thought": true}]},
                "finishReason": "STOP"
            }]
        })
        .to_string(),
    ];
    let mut probe = RuntimeGeminiPrecommitProbe::default();

    let decision = runtime_gemini_precommit_decision_for_data_lines(&data, &mut probe);

    assert_eq!(decision, RuntimeGeminiPrecommitDecision::Commit);
}

#[test]
fn gemini_precommit_commits_max_tokens_for_codex_incomplete_event() {
    let data = vec![
        serde_json::json!({
            "responseId": "resp_truncated",
            "candidates": [{
                "content": {"parts": []},
                "finishReason": "MAX_TOKENS"
            }]
        })
        .to_string(),
    ];
    let mut probe = RuntimeGeminiPrecommitProbe::default();

    let decision = runtime_gemini_precommit_decision_for_data_lines(&data, &mut probe);

    assert_eq!(decision, RuntimeGeminiPrecommitDecision::Commit);
}

#[test]
fn gemini_precommit_retries_empty_stop_before_commit() {
    let data = vec![
        serde_json::json!({
            "responseId": "resp_empty",
            "candidates": [{
                "content": {"parts": []},
                "finishReason": "STOP"
            }]
        })
        .to_string(),
    ];
    let mut probe = RuntimeGeminiPrecommitProbe::default();

    let decision = runtime_gemini_precommit_decision_for_data_lines(&data, &mut probe);

    assert_eq!(
        decision,
        RuntimeGeminiPrecommitDecision::RetryableInvalid("gemini_empty_response".to_string())
    );
}
