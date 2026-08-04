#![cfg(test)]

use super::super::copilot_instructions::{
    RUNTIME_COPILOT_CUSTOM_INSTRUCTIONS_HEADER, runtime_copilot_workspace_custom_instructions,
};
use super::super::deepseek_rewrite::{
    RuntimeDeepSeekConversationStore, RuntimeDeepSeekTranslatedRequest,
};
use super::super::local_rewrite::RuntimeLocalRewriteProviderOptions;
use super::state::{RuntimeCopilotOAuthPoolState, RuntimeCopilotSelectedAuth};
use super::*;
use prodex_state::ResponseProfileBinding;
use std::collections::BTreeMap;
use std::io::{Cursor, Read};

fn copilot_runtime_shared() -> crate::RuntimeRotationProxyShared {
    let root = std::env::temp_dir().join(format!("prodex-copilot-runtime-{}", std::process::id()));
    let paths = crate::AppPaths {
        state_file: root.join("state.json"),
        managed_profiles_root: root.join("profiles"),
        shared_codex_root: root.join("shared-codex"),
        legacy_shared_codex_root: root.join("shared"),
        root,
    };
    crate::RuntimeRotationProxyShared {
        smart_context_engine: Arc::new(crate::RuntimeSmartContextEngine::default()),
        runtime_config: Arc::new(crate::RuntimeConfig::compatibility_current()),
        upstream_no_proxy: false,
        auto_redeem_enabled: false,
        compact_client: reqwest::Client::new(),
        async_client: reqwest::Client::new(),
        async_runtime: Arc::new(
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap(),
        ),
        runtime: Arc::new(Mutex::new(crate::RuntimeRotationState {
            paths,
            state: crate::AppState::default(),
            upstream_base_url: "https://api.example.test/v1".to_string(),
            include_code_review: false,
            current_profile: "main".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: crate::RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::new(),
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_backoff_updated_at: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        })),
        log_path: std::env::temp_dir()
            .join(format!("prodex-copilot-runtime-{}.log", std::process::id())),
        request_sequence: Arc::new(std::sync::atomic::AtomicU64::new(1)),
        state_save_revision: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        local_overload_backoff_until: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        active_request_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        active_request_limit: 8,
        runtime_state_lock_wait_counters:
            crate::RuntimeRotationProxyShared::new_runtime_state_lock_wait_counters(),
        lane_admission: crate::RuntimeProxyLaneAdmission::new(crate::RuntimeProxyLaneLimits {
            responses: 8,
            compact: 8,
            websocket: 8,
            standard: 8,
        }),
    }
}

fn copilot_profile(profile_name: &str) -> RuntimeCopilotProfileAuth {
    RuntimeCopilotProfileAuth {
        profile_name: profile_name.to_string(),
        api_key: format!("token-{profile_name}"),
        api_url: format!("https://api.{profile_name}.githubcopilot.test"),
        model_catalog: Vec::new(),
    }
}

fn copilot_pool(profile_names: &[&str]) -> RuntimeCopilotOAuthPool {
    let shared = copilot_runtime_shared();
    RuntimeCopilotOAuthPool {
        state: Arc::new(Mutex::new(RuntimeCopilotOAuthPoolState {
            profiles: profile_names
                .iter()
                .map(|profile_name| copilot_profile(profile_name))
                .collect(),
            next_index: 0,
        })),
        runtime: Arc::clone(&shared.runtime),
    }
}

fn copilot_pool_with_shared(
    profile_names: &[&str],
) -> (crate::RuntimeRotationProxyShared, RuntimeCopilotOAuthPool) {
    let shared = copilot_runtime_shared();
    let pool = RuntimeCopilotOAuthPool {
        state: Arc::new(Mutex::new(RuntimeCopilotOAuthPoolState {
            profiles: profile_names
                .iter()
                .map(|profile_name| copilot_profile(profile_name))
                .collect(),
            next_index: 0,
        })),
        runtime: Arc::clone(&shared.runtime),
    };
    (shared, pool)
}

fn conversation_store() -> RuntimeDeepSeekConversationStore {
    RuntimeDeepSeekConversationStore::default()
}

fn selected_auth(
    profile_name: &str,
    api_key: &str,
    api_url: Option<&str>,
) -> RuntimeCopilotSelectedAuth {
    RuntimeCopilotSelectedAuth {
        profile_name: profile_name.to_string(),
        api_key: api_key.to_string(),
        api_url: api_url.map(str::to_string),
        hard_affinity: false,
        projected: false,
    }
}

#[test]
fn copilot_profile_pool_debug_output_redacts_sensitive_fields() {
    let profile = RuntimeCopilotProfileAuth {
        profile_name: "copilot-profile-secret".to_string(),
        api_key: "copilot-api-key-secret".to_string(),
        api_url: "https://api.copilot-secret.example".to_string(),
        model_catalog: vec![serde_json::json!({
            "id": "copilot-model-secret",
            "name": "Copilot Secret Model"
        })],
    };
    let rendered = format!("{profile:?}");

    assert!(rendered.contains("RuntimeCopilotProfileAuth"));
    assert!(rendered.contains("<redacted>"));
    assert!(rendered.contains("<redacted:1>"));
    for raw in [
        "copilot-profile-secret",
        "copilot-api-key-secret",
        "https://api.copilot-secret.example",
        "copilot-model-secret",
        "Copilot Secret Model",
    ] {
        assert!(!rendered.contains(raw), "{rendered}");
    }

    let state = RuntimeCopilotOAuthPoolState {
        profiles: vec![profile],
        next_index: 7,
    };
    let rendered = format!("{state:?}");

    assert!(rendered.contains("RuntimeCopilotOAuthPoolState"));
    assert!(rendered.contains("<redacted>"));
    assert!(rendered.matches("<redacted:1>").count() >= 1);
    for raw in ["copilot-profile-secret", "copilot-api-key-secret"] {
        assert!(!rendered.contains(raw), "{rendered}");
    }
}

fn temp_copilot_instruction_root(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "prodex-copilot-instructions-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

#[test]
fn copilot_workspace_custom_instructions_reads_github_files_only() {
    let root = temp_copilot_instruction_root("github-only");
    std::fs::create_dir_all(root.join(".github/instructions/nested")).unwrap();
    std::fs::write(
        root.join(".github/copilot-instructions.md"),
        "Prefer concise answers.",
    )
    .unwrap();
    std::fs::write(
        root.join(".github/instructions/nested/review.instructions.md"),
        "Review risky diffs first.",
    )
    .unwrap();
    std::fs::write(
        root.join("AGENTS.md"),
        "@/home/test-user/.prodex/private/RTK.md",
    )
    .unwrap();

    let instructions = runtime_copilot_workspace_custom_instructions(&root)
        .unwrap()
        .unwrap();

    assert!(instructions.contains("## .github/copilot-instructions.md"));
    assert!(instructions.contains("Prefer concise answers."));
    assert!(instructions.contains("## .github/instructions/nested/review.instructions.md"));
    assert!(instructions.contains("Review risky diffs first."));
    assert!(!instructions.contains("AGENTS.md"));
    assert!(!instructions.contains(".prodex/private"));
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn copilot_workspace_custom_instructions_do_not_follow_symlinks() {
    let root = temp_copilot_instruction_root("symlink");
    let outside = root.with_file_name(format!(
        "{}-outside",
        root.file_name().unwrap().to_string_lossy()
    ));
    let _ = std::fs::remove_dir_all(&outside);
    std::fs::create_dir_all(root.join(".github")).unwrap();
    std::fs::create_dir_all(outside.join("instructions")).unwrap();
    std::fs::write(
        outside.join("copilot-instructions.md"),
        "outside global secret",
    )
    .unwrap();
    std::fs::write(
        outside.join("instructions").join("leak.instructions.md"),
        "outside scoped secret",
    )
    .unwrap();
    std::os::unix::fs::symlink(
        outside.join("copilot-instructions.md"),
        root.join(".github").join("copilot-instructions.md"),
    )
    .unwrap();
    std::os::unix::fs::symlink(
        outside.join("instructions"),
        root.join(".github").join("instructions"),
    )
    .unwrap();

    let instructions = runtime_copilot_workspace_custom_instructions(&root).unwrap();

    assert_eq!(instructions, None);
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(outside);
}

#[test]
fn copilot_custom_instructions_merge_into_chat_body() {
    let mut translated = RuntimeDeepSeekTranslatedRequest {
        body: serde_json::to_vec(&serde_json::json!({
            "model": "gpt-5.1-codex",
            "stream": true,
            "messages": [
                {"role": "system", "content": "Existing system."},
                {"role": "user", "content": "Hi"}
            ]
        }))
        .unwrap(),
        messages: vec![
            serde_json::json!({"role": "system", "content": "Existing system."}),
            serde_json::json!({"role": "user", "content": "Hi"}),
        ],
        response_metadata: None,
    };

    runtime_copilot_apply_custom_instructions(&mut translated, "Prefer Rust tests.").unwrap();

    let body: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
    let system = body["messages"][0]["content"].as_str().unwrap();
    assert!(system.contains("Existing system."));
    assert!(system.contains(RUNTIME_COPILOT_CUSTOM_INSTRUCTIONS_HEADER));
    assert!(system.contains("Prefer Rust tests."));
    assert_eq!(
        translated.messages,
        body["messages"].as_array().unwrap().clone()
    );
}

#[test]
fn copilot_responses_bridge_maps_mcp_optional_tools_to_chat_functions() {
    let conversations = conversation_store();
    let request = serde_json::json!({
        "model": "codex",
        "stream": true,
        "input": "compress and inspect the workspace",
        "tools": [
            {
                "type": "mcp_tool",
                "name": "mcp__prodex_sqz__sqz_read_file",
                "description": "Read a file through SQZ.",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"}
                    },
                    "required": ["path"]
                }
            },
            {
                "type": "mcp_toolset",
                "mcp_server_name": "prodex-sqz",
                "default_config": {"enabled": false},
                "configs": {
                    "compress": {"enabled": true},
                    "sqz_read_file": {"enabled": false}
                }
            }
        ],
        "tool_choice": {
            "type": "mcp_tool",
            "name": "mcp__prodex_sqz__sqz_read_file"
        }
    });

    let translated = runtime_copilot_responses_chat_request_body(
        &serde_json::to_vec(&request).unwrap(),
        &conversations,
    )
    .expect("Copilot Responses request should translate to chat");
    let body: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
    let tools = body["tools"].as_array().unwrap();

    assert_eq!(body["model"], prodex_cli::SUPER_COPILOT_DEFAULT_MODEL);
    assert_eq!(tools.len(), 2);
    assert_eq!(
        tools[0]["function"]["name"],
        "mcp__prodex_sqz__sqz_read_file"
    );
    assert_eq!(tools[0]["function"]["parameters"]["required"][0], "path");
    assert_eq!(tools[1]["function"]["name"], "mcp__prodex_sqz__compress");
    assert_eq!(
        body["tool_choice"]["function"]["name"],
        "mcp__prodex_sqz__sqz_read_file"
    );
}

#[test]
fn copilot_bridge_stream_records_response_id_and_restores_mcp_namespace() {
    let captured = Arc::new(Mutex::new(Vec::<String>::new()));
    let captured_for_recorder = Arc::clone(&captured);
    let recorder: RuntimeCopilotBindingRecorder = Arc::new(move |response_id| {
        captured_for_recorder.lock().unwrap().push(response_id);
    });
    let chat_stream = concat!(
        "data: {\"id\":\"chatcmpl_copilot_1\",\"model\":\"gpt-5.1-codex\",\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_sqz_1\",\"type\":\"function\",\"function\":{\"name\":\"mcp__prodex_sqz__compress\",\"arguments\":\"{}\"}}]}}]}\n\n",
        "data: {\"id\":\"chatcmpl_copilot_1\",\"choices\":[{\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let chat_reader = super::super::deepseek_rewrite::RuntimeDeepSeekChatSseReader::new(
        Cursor::new(chat_stream),
        11,
        Vec::new(),
        None,
        conversation_store(),
    );
    let mut reader = RuntimeCopilotResponsesSseBindingReader::new(chat_reader, Some(recorder));
    let mut output = String::new();

    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains("\"namespace\":\"mcp__prodex_sqz\""));
    assert!(output.contains("\"name\":\"compress\""));
    assert_eq!(captured.lock().unwrap().as_slice(), ["chatcmpl_copilot_1"]);
}

#[test]
fn copilot_oauth_pool_rotates_fresh_requests() {
    let pool = copilot_pool(&["alpha", "beta"]);
    let body = serde_json::to_vec(&serde_json::json!({"input": "hi"})).unwrap();

    let first = pool.select_attempts(&body, &[]).unwrap();
    let second = pool.select_attempts(&body, &[]).unwrap();

    assert_eq!(first[0].profile_name, "alpha");
    assert_eq!(
        first[0].api_url.as_deref(),
        Some("https://api.alpha.githubcopilot.test")
    );
    assert_eq!(first[1].profile_name, "beta");
    assert!(!first[0].hard_affinity);
    assert_eq!(second[0].profile_name, "beta");
    assert_eq!(second[1].profile_name, "alpha");
}

#[test]
fn copilot_oauth_pool_preserves_previous_response_affinity() {
    let (shared, pool) = copilot_pool_with_shared(&["alpha", "beta"]);
    shared
        .runtime
        .lock()
        .unwrap()
        .state
        .response_profile_bindings
        .insert(
            "resp_1".to_string(),
            ResponseProfileBinding {
                profile_name: "beta".to_string(),
                bound_at: 1,
                binding_identity: None,
            },
        );
    let body = serde_json::to_vec(&serde_json::json!({"previous_response_id": "resp_1"})).unwrap();

    let attempts = pool.select_attempts(&body, &[]).unwrap();

    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0].profile_name, "beta");
    assert_eq!(
        attempts[0].api_url.as_deref(),
        Some("https://api.beta.githubcopilot.test")
    );
    assert!(attempts[0].hard_affinity);
}

#[test]
fn copilot_oauth_affinity_uses_fallback_profile_credentials_exactly() {
    let (shared, pool) = copilot_pool_with_shared(&[]);
    let fallback_profiles = vec![copilot_profile("alpha"), copilot_profile("beta")];
    shared
        .runtime
        .lock()
        .unwrap()
        .state
        .response_profile_bindings
        .insert(
            "resp_fallback".to_string(),
            ResponseProfileBinding {
                profile_name: "beta".to_string(),
                bound_at: 1,
                binding_identity: None,
            },
        );
    let body = serde_json::to_vec(&serde_json::json!({
        "previous_response_id": "resp_fallback"
    }))
    .unwrap();

    let attempts = pool.select_attempts(&body, &fallback_profiles).unwrap();

    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0].profile_name, "beta");
    assert_eq!(attempts[0].api_key, "token-beta");
    assert_eq!(
        attempts[0].api_url.as_deref(),
        Some("https://api.beta.githubcopilot.test")
    );
    assert!(attempts[0].hard_affinity);
}

#[test]
fn copilot_raw_api_keys_use_bounded_rotating_order() {
    let provider = RuntimeLocalRewriteProviderOptions::Copilot {
        auth: RuntimeCopilotProviderAuth::ApiKeys {
            api_keys: vec!["key-a".to_string(), "key-b".to_string()],
        },
    };
    let (_shared, pool) = copilot_pool_with_shared(&["api-key-1", "api-key-2"]);
    let pool =
        runtime_copilot_oauth_pool_from_provider(&provider, Arc::clone(&pool.runtime)).unwrap();
    let body = serde_json::to_vec(&serde_json::json!({"input": "hi"})).unwrap();

    let first = pool
        .select_attempts_with_identity(&body, &[], None, None, "https://api.example.test")
        .unwrap();
    let second = pool
        .select_attempts_with_identity(&body, &[], None, None, "https://api.example.test")
        .unwrap();

    assert_eq!(
        first
            .iter()
            .map(|attempt| (attempt.profile_name.as_str(), attempt.api_key.as_str()))
            .collect::<Vec<_>>(),
        [("api-key-1", "key-a"), ("api-key-2", "key-b")]
    );
    assert_eq!(second[0].profile_name, "api-key-2");
    assert_eq!(second[1].profile_name, "api-key-1");
    assert!(first.iter().all(|attempt| !attempt.hard_affinity));
}

#[test]
fn copilot_fresh_candidate_does_not_create_a_binding_before_acceptance() {
    let provider = RuntimeLocalRewriteProviderOptions::Copilot {
        auth: RuntimeCopilotProviderAuth::ApiKeys {
            api_keys: vec!["key-a".to_string(), "key-b".to_string()],
        },
    };
    let shared = copilot_runtime_shared();
    let pool =
        runtime_copilot_oauth_pool_from_provider(&provider, Arc::clone(&shared.runtime)).unwrap();
    let body = serde_json::to_vec(&serde_json::json!({"input": "hi"})).unwrap();

    let attempts = pool
        .select_attempts_with_identity(
            &body,
            &[],
            Some("turn-pending"),
            Some("session-pending"),
            "https://api.example.test",
        )
        .unwrap();

    assert_eq!(attempts.len(), 2);
    let state = shared.runtime.lock().unwrap();
    assert!(state.state.response_profile_bindings.is_empty());
    assert!(state.turn_state_bindings.is_empty());
    assert!(state.session_id_bindings.is_empty());
}

#[test]
fn copilot_accepted_identity_pins_raw_key_and_public_endpoint() {
    let provider = RuntimeLocalRewriteProviderOptions::Copilot {
        auth: RuntimeCopilotProviderAuth::ApiKeys {
            api_keys: vec!["key-a".to_string(), "key-b".to_string()],
        },
    };
    let shared = copilot_runtime_shared();
    let pool =
        runtime_copilot_oauth_pool_from_provider(&provider, Arc::clone(&shared.runtime)).unwrap();
    let recorder = runtime_copilot_binding_recorder(
        &pool,
        &shared,
        selected_auth("api-key-2", "key-b", None),
        "https://user:secret@api.example.test/v1?token=secret".to_string(),
        Some("turn-1".to_string()),
        Some("session-1".to_string()),
    );
    recorder("resp-1".to_string());
    let state = shared.runtime.lock().unwrap();
    let binding = state.state.response_profile_bindings.get("resp-1").unwrap();
    let expected = prodex_provider_core::RuntimeProviderBindingIdentity::from_raw_key(
        prodex_provider_core::ProviderId::Copilot,
        "key-b",
        "https://api.example.test/v1",
        Some("api-key-2"),
    )
    .unwrap();
    assert_eq!(binding.binding_identity.as_ref(), Some(&expected));
    let encoded = serde_json::to_string(binding).unwrap();
    assert!(!encoded.contains("api.example.test"));
    assert!(!encoded.contains("key-b"));
    drop(state);

    let body = serde_json::to_vec(&serde_json::json!({
        "previous_response_id": "resp-1"
    }))
    .unwrap();
    let attempts = pool
        .select_attempts_with_identity(
            &body,
            &[],
            Some("turn-1"),
            Some("session-1"),
            "https://api.example.test/v1",
        )
        .unwrap();

    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0].profile_name, "api-key-2");
    assert_eq!(attempts[0].api_key, "key-b");
    assert_eq!(attempts[0].api_url, None);
    assert!(attempts[0].hard_affinity);
}

#[test]
fn copilot_conflicting_identity_keys_fail_closed_before_selection() {
    let (shared, pool) = copilot_pool_with_shared(&["alpha", "beta"]);
    pool.remember_accepted_identity(
        &shared,
        &selected_auth(
            "alpha",
            "token-alpha",
            Some("https://api.alpha.githubcopilot.test"),
        ),
        "https://api.alpha.githubcopilot.test",
        Some("resp-conflict"),
        Some("turn-conflict"),
        None,
    );
    pool.remember_accepted_identity(
        &shared,
        &selected_auth(
            "beta",
            "token-beta",
            Some("https://api.beta.githubcopilot.test"),
        ),
        "https://api.beta.githubcopilot.test",
        None,
        Some("turn-conflict"),
        None,
    );
    let body = serde_json::to_vec(&serde_json::json!({
        "previous_response_id": "resp-conflict"
    }))
    .unwrap();

    let error = pool
        .select_attempts_with_identity(
            &body,
            &[],
            Some("turn-conflict"),
            None,
            "https://fallback.example.test",
        )
        .err()
        .expect("conflicting Copilot identity should fail closed");
    let rendered = error.to_string();

    assert!(rendered.contains("conflicting"));
    for raw in [
        "resp-conflict",
        "turn-conflict",
        "api.alpha.githubcopilot.test",
        "api.beta.githubcopilot.test",
    ] {
        assert!(!rendered.contains(raw), "{rendered}");
    }
}

#[test]
fn copilot_bound_endpoint_mismatch_fails_closed() {
    let (shared, pool) = copilot_pool_with_shared(&["alpha"]);
    pool.remember_accepted_identity(
        &shared,
        &selected_auth("alpha", "token-alpha", None),
        "https://api.other.example.test/v1?credential=secret",
        Some("resp-endpoint"),
        None,
        None,
    );
    let body = serde_json::to_vec(&serde_json::json!({
        "previous_response_id": "resp-endpoint"
    }))
    .unwrap();

    assert!(pool
        .select_attempts_with_identity(
            &body,
            &[],
            None,
            None,
            "https://fallback.example.test",
        )
        .is_err());
}

#[test]
fn copilot_generic_429_is_not_precommit_retryable() {
    assert!(!runtime_copilot_provider_retry_precommit(
        ProviderRetryCause::RotateCredential,
        RuntimeProviderErrorClass::RateLimit,
        429,
        b"too many requests",
        0,
        2,
    ));
    assert!(runtime_copilot_provider_retry_precommit(
        ProviderRetryCause::RotateCredential,
        RuntimeProviderErrorClass::RateLimit,
        429,
        br#"{"error":{"code":"rate_limit_exceeded"}}"#,
        0,
        2,
    ));
}

#[test]
fn copilot_sse_binding_reader_preserves_bytes_and_records_response_id() {
    let captured = Arc::new(Mutex::new(Vec::<String>::new()));
    let captured_for_recorder = Arc::clone(&captured);
    let recorder: RuntimeCopilotBindingRecorder = Arc::new(move |response_id| {
        captured_for_recorder.lock().unwrap().push(response_id);
    });
    let stream = concat!(
        "event: response.created\n",
        "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_1\"}}\n\n",
        "event: response.output_text.delta\n",
        "data: {\"type\":\"response.output_text.delta\",\"response_id\":\"resp_1\",\"delta\":\"hi\"}\n\n",
        "data: [DONE]\n\n",
    );
    let mut reader =
        RuntimeCopilotResponsesSseBindingReader::new(Cursor::new(stream), Some(recorder));
    let mut output = String::new();

    reader.read_to_string(&mut output).unwrap();

    assert_eq!(output, stream);
    assert_eq!(captured.lock().unwrap().as_slice(), ["resp_1"]);
}

#[test]
fn copilot_binding_recorder_reads_buffered_responses_body() {
    let captured = Arc::new(Mutex::new(None::<String>));
    let captured_for_recorder = Arc::clone(&captured);
    let recorder: RuntimeCopilotBindingRecorder = Arc::new(move |response_id| {
        *captured_for_recorder.lock().unwrap() = Some(response_id);
    });
    let body = serde_json::to_vec(&serde_json::json!({"id": "resp_1"})).unwrap();

    runtime_copilot_remember_bindings_from_responses_body(Some(&recorder), &body);

    assert_eq!(captured.lock().unwrap().as_deref(), Some("resp_1"));
}
