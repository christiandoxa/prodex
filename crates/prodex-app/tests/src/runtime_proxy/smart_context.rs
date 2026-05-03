use super::*;

#[test]
fn smart_context_condenses_tool_output_with_artifact_ref() {
    let mut value = serde_json::json!({
        "input": [{
            "type": "function_call_output",
            "call_id": "call_1",
            "output": (0..500).map(|index| format!("line {index}: repeated command output")).collect::<Vec<_>>().join("\n")
        }]
    });
    let mut store = RuntimeSmartContextArtifactStore::default();
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_condense_tool_outputs(
        &mut value,
        &mut store,
        7,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Minimal,
        &mut stats,
    );

    let output = value["input"][0]["output"].as_str().unwrap();
    assert!(output.contains("prodex smart context artifact"));
    assert!(output.contains("prodex-artifact:sc:"));
    assert_eq!(stats.artifacts_stored, 1);
    assert_eq!(stats.tool_outputs_condensed, 1);
}

#[test]
fn smart_context_rehydrates_known_artifact_refs() {
    let mut store = RuntimeSmartContextArtifactStore::default();
    let artifact = store.insert_text(1, "exact artifact text").unwrap();
    let mut value = serde_json::json!({
        "input": [{
            "type": "message",
            "content": format!("need prodex-artifact:{}", artifact.id)
        }]
    });
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_rehydrate_value(&mut value, &store, &mut stats);

    assert_eq!(value["input"][0]["content"], "need exact artifact text");
    assert_eq!(stats.rehydrated_refs, 1);
}

#[test]
fn smart_context_rehydrates_artifact_line_ranges() {
    let mut store = RuntimeSmartContextArtifactStore::default();
    let artifact = store
        .insert_text(1, "line one\nline two\nline three\nline four")
        .unwrap();
    let mut value = serde_json::json!({
        "input": [{
            "type": "message",
            "content": format!("need prodex-artifact:{}#L2-L3", artifact.id)
        }]
    });
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_rehydrate_value(&mut value, &store, &mut stats);

    assert_eq!(value["input"][0]["content"], "need line two\nline three");
    assert_eq!(stats.rehydrated_refs, 1);
}

#[test]
fn smart_context_dedupes_repeated_input_text() {
    let repeated = "same ".repeat(300);
    let mut value = serde_json::json!({
        "input": [
            {"type": "message", "content": repeated},
            {"type": "message", "content": repeated}
        ]
    });
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_dedupe_input_text(&mut value, &mut stats);

    assert!(
        value["input"][1]["content"]
            .as_str()
            .unwrap()
            .contains("prodex smart context duplicate")
    );
    assert_eq!(stats.duplicate_texts, 1);
}

#[test]
fn smart_context_budget_uses_runtime_token_usage_observation() {
    let shared = smart_context_test_shared("budget");
    register_runtime_smart_context_proxy_state(&shared.log_path, true);
    observe_runtime_smart_context_token_usage(
        &shared,
        RuntimeTokenUsage {
            input_tokens: 24_000,
            cached_input_tokens: 0,
            output_tokens: 7_000,
            reasoning_tokens: 1_000,
        },
    );

    let budget = runtime_smart_context_budget(&shared, 32);

    assert_eq!(
        budget.tier,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Minimal
    );
    assert_eq!(budget.observed_context_tokens, Some(32_000));
    assert_eq!(budget.token_usage_source, "runtime_usage");
}

#[test]
fn smart_context_self_check_passes_through_growth_without_rehydrate() {
    let stats = RuntimeSmartContextTransformStats {
        artifacts_stored: 1,
        tool_outputs_condensed: 1,
        duplicate_texts: 0,
        rehydrated_refs: 0,
    };

    assert_eq!(
        runtime_smart_context_rewrite_self_check(100, 101, &stats),
        "growth"
    );
    assert!(runtime_smart_context_should_pass_through_after_self_check(
        100, 101, &stats
    ));
}

fn smart_context_test_shared(name: &str) -> RuntimeRotationProxyShared {
    static NEXT_LOG_ID: AtomicU64 = AtomicU64::new(1);
    let unique = NEXT_LOG_ID.fetch_add(1, Ordering::Relaxed);
    let root = env::temp_dir().join(format!(
        "prodex-smart-context-{name}-{}-{unique}",
        std::process::id()
    ));
    let paths = AppPaths {
        state_file: root.join("state.json"),
        managed_profiles_root: root.join("profiles"),
        shared_codex_root: root.join("shared-codex"),
        legacy_shared_codex_root: root.join("shared"),
        root,
    };

    RuntimeRotationProxyShared {
        upstream_no_proxy: false,
        async_client: reqwest::Client::new(),
        async_runtime: Arc::new(
            TokioRuntimeBuilder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio runtime"),
        ),
        runtime: Arc::new(Mutex::new(RuntimeRotationState {
            paths,
            state: AppState::default(),
            upstream_base_url: "http://127.0.0.1".to_string(),
            include_code_review: false,
            current_profile: "main".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::new(),
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        })),
        log_path: env::temp_dir().join(format!(
            "prodex-smart-context-{name}-{}-{unique}.log",
            std::process::id()
        )),
        request_sequence: Arc::new(AtomicU64::new(1)),
        state_save_revision: Arc::new(AtomicU64::new(0)),
        local_overload_backoff_until: Arc::new(AtomicU64::new(0)),
        active_request_count: Arc::new(AtomicUsize::new(0)),
        active_request_limit: 8,
        runtime_state_lock_wait_counters:
            RuntimeRotationProxyShared::new_runtime_state_lock_wait_counters(),
        lane_admission: RuntimeProxyLaneAdmission::new(RuntimeProxyLaneLimits {
            responses: 8,
            compact: 8,
            websocket: 8,
            standard: 8,
        }),
    }
}
