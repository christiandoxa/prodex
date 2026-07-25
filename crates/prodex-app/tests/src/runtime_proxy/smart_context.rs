use super::*;
use std::borrow::Cow;

#[path = "smart_context/budget.rs"]
mod budget;

#[path = "smart_context/rehydrate_dedupe.rs"]
mod rehydrate_dedupe;

#[path = "smart_context/prepare.rs"]
mod prepare;

#[path = "smart_context/tool_outputs/prepare_fallbacks.rs"]
mod prepare_fallbacks;

#[path = "smart_context/static_context_extra.rs"]
mod static_context_extra;

#[path = "smart_context/exactness.rs"]
mod exactness;

fn smart_context_test_request(mut body: serde_json::Value) -> RuntimeProxyRequest {
    if let Some(object) = body.as_object_mut() {
        object
            .entry("model")
            .or_insert_with(|| serde_json::Value::String("gpt-4o".to_string()));
    }
    RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/v1/responses".to_string(),
        headers: Vec::new(),
        body: serde_json::to_vec(&body).unwrap(),
    }
}

fn smart_context_observe_minimal_budget(shared: &RuntimeRotationProxyShared) {
    observe_runtime_smart_context_token_usage(
        shared,
        RuntimeTokenUsage {
            input_tokens: 24_000,
            cached_input_tokens: 0,
            output_tokens: 7_000,
            reasoning_tokens: 1_000,
        },
    );
}

fn smart_context_test_state_snapshot(shared: &RuntimeRotationProxyShared) -> String {
    let (_, state) = runtime_smart_context_proxy_state_snapshot(shared)
        .expect("smart-context state should be registered");
    format!("{state:#?}")
}

fn smart_context_test_copy_scope(
    source: &RuntimeRotationProxyShared,
    target: &RuntimeRotationProxyShared,
) {
    let source = source.runtime.lock().unwrap().clone();
    let mut target = target.runtime.lock().unwrap();
    target.paths.root = source.paths.root;
    target.upstream_base_url = source.upstream_base_url;
    target.current_profile = source.current_profile;
    target.state.profiles = source.state.profiles;
}

fn register_persistent_runtime_smart_context_test_state(
    shared: &RuntimeRotationProxyShared,
    model_context_window_tokens: Option<u64>,
) -> PathBuf {
    let artifact_path = shared.log_path.with_extension("artifacts.json");
    let _ = std::fs::remove_file(&artifact_path);
    register_runtime_smart_context_proxy_state(
        shared,
        true,
        model_context_window_tokens,
        Some(artifact_path.clone()),
    );
    artifact_path
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
    let log_path = env::temp_dir().join(format!(
        "prodex-smart-context-{name}-{}-{unique}.log",
        std::process::id()
    ));
    prepare_runtime_proxy_test_log_path(&log_path);

    RuntimeRotationProxyShared {
        smart_context_engine: std::sync::Arc::new(crate::RuntimeSmartContextEngine::default()),
        runtime_config: Arc::new(crate::RuntimeConfig::compatibility_current()),
        upstream_no_proxy: false,
        auto_redeem_enabled: false,
        compact_client: reqwest::Client::new(),
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
            profile_backoff_updated_at: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        })),
        log_path,
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
