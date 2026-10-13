//! Isolated runtime fixture shared by Presidio policy regression tests.
use super::{
    AppPaths, AppState, Arc, AtomicU64, AtomicUsize, Mutex, Ordering, RuntimeConfig,
    RuntimeContinuationStatuses, RuntimeProxyLaneAdmission, RuntimeProxyLaneLimits,
    RuntimeRotationProxyShared, RuntimeRotationState, TokioRuntimeBuilder,
};

pub(super) fn presidio_test_shared(
    name: &str,
    governance: prodex_config::GovernanceConfig,
) -> RuntimeRotationProxyShared {
    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "prodex-presidio-test-{name}-{}-{id}",
        std::process::id()
    ));
    let paths = AppPaths {
        state_file: root.join("state.json"),
        managed_profiles_root: root.join("profiles"),
        shared_codex_root: root.join("shared-codex"),
        legacy_shared_codex_root: root.join("shared"),
        root,
    };
    let log_path = std::env::temp_dir().join(format!(
        "prodex-presidio-test-{name}-{}-{id}.log",
        std::process::id()
    ));
    crate::runtime_core_shared::prepare_runtime_proxy_test_log_path(&log_path);
    let mut runtime_config = RuntimeConfig::compatibility_current();
    runtime_config.governance = governance;
    runtime_config.tenant_detector_patterns = Default::default();

    RuntimeRotationProxyShared {
        precommit_deadline: None,
        smart_context_engine: Arc::new(crate::RuntimeSmartContextEngine::default()),
        runtime_config: Arc::new(runtime_config),
        upstream_no_proxy: false,
        auto_redeem_enabled: false,
        async_client: reqwest::Client::new(),
        compact_client: reqwest::Client::new(),
        // `await_runtime_proxy_async_task` synchronously receives work spawned on this
        // runtime, so the fixture needs its own worker just like the production runtime.
        async_runtime: Arc::new(
            TokioRuntimeBuilder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
                .unwrap(),
        ),
        runtime: Arc::new(Mutex::new(RuntimeRotationState {
            paths,
            state: AppState::default(),
            upstream_base_url: "http://127.0.0.1".to_string(),
            include_code_review: false,
            current_profile: "test".to_string(),
            profile_usage_auth: Default::default(),
            turn_state_bindings: Default::default(),
            session_id_bindings: Default::default(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: Default::default(),
            profile_usage_snapshots: Default::default(),
            profile_retry_backoff_until: Default::default(),
            profile_transport_backoff_until: Default::default(),
            profile_route_circuit_open_until: Default::default(),
            profile_backoff_updated_at: Default::default(),
            profile_health: Default::default(),
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
