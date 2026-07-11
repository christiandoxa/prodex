use super::*;

mod anthropic_rewrite;
mod chat_compatible_request;
mod chat_compatible_rewrite;
mod deepseek_reasoning;
pub(super) mod deepseek_rewrite;
mod deepseek_sse;
mod deepseek_sse_reader;
#[cfg(test)]
mod gemini_openai_compat_tests;
mod gemini_rewrite;
mod gemini_sse;
mod gemini_thought_signatures;
mod local_rewrite;
mod local_rewrite_application_boundary;
mod local_rewrite_application_data_plane;
mod local_rewrite_constraints;
mod local_rewrite_copilot;
mod local_rewrite_copilot_bindings;
mod local_rewrite_deepseek;
mod local_rewrite_gateway_admin_audit;
mod local_rewrite_gateway_admin_auth;
mod local_rewrite_gateway_admin_dispatch;
mod local_rewrite_gateway_admin_fields;
mod local_rewrite_gateway_admin_keys;
mod local_rewrite_gateway_admin_ledger;
mod local_rewrite_gateway_admin_payloads;
mod local_rewrite_gateway_admin_response;
mod local_rewrite_gateway_admin_route_explain;
mod local_rewrite_gateway_admin_router;
mod local_rewrite_gateway_admin_scim;
mod local_rewrite_gateway_admin_store_mutation;
mod local_rewrite_gateway_backend_connection;
mod local_rewrite_gateway_billing_csv;
mod local_rewrite_gateway_billing_summary;
mod local_rewrite_gateway_budget;
mod local_rewrite_gateway_config;
mod local_rewrite_gateway_credentials;
mod local_rewrite_gateway_dashboard;
mod local_rewrite_gateway_data_plane_audit;
mod local_rewrite_gateway_distributed_rate_limit;
mod local_rewrite_gateway_file_ledger;
mod local_rewrite_gateway_guardrail_webhook;
mod local_rewrite_gateway_key_patch;
mod local_rewrite_gateway_key_payloads;
mod local_rewrite_gateway_key_store_backend;
mod local_rewrite_gateway_keys;
mod local_rewrite_gateway_ledger;
mod local_rewrite_gateway_ledger_types;
mod local_rewrite_gateway_metrics;
mod local_rewrite_gateway_openapi;
mod local_rewrite_gateway_reconciliation_audit;
mod local_rewrite_gateway_reconciliation_runtime;
mod local_rewrite_gateway_redis_ledger;
mod local_rewrite_gateway_reservation;
mod local_rewrite_gateway_route_load;
mod local_rewrite_gateway_scim;
mod local_rewrite_gateway_scope;
#[cfg(test)]
mod local_rewrite_gateway_side_effect_snapshot;
#[cfg(test)]
use local_rewrite_gateway_side_effect_snapshot::gateway_snapshot_handle;
mod local_rewrite_gateway_sql_ledger;
mod local_rewrite_gateway_sqlite_utils;
mod local_rewrite_gateway_store_file;
mod local_rewrite_gateway_store_types;
mod local_rewrite_gateway_usage;
mod local_rewrite_gateway_usage_backend;
mod local_rewrite_gateway_util;
mod local_rewrite_gemini;
mod local_rewrite_gemini_bindings;
mod local_rewrite_gemini_compact;
mod local_rewrite_gemini_live;
mod local_rewrite_gemini_models;
mod local_rewrite_gemini_quota;
mod local_rewrite_gemini_thought_signatures;
mod local_rewrite_kiro;
mod local_rewrite_model_memory;
mod local_rewrite_options;
mod local_rewrite_pipeline;
mod local_rewrite_rate_limits;
mod local_rewrite_response;
mod local_rewrite_response_guardrails;
mod local_rewrite_response_spend;
mod local_rewrite_search_fallback;
#[cfg(test)]
mod local_rewrite_tests;
mod local_rewrite_transport;
mod local_rewrite_transport_copilot;
mod local_rewrite_upstream;
#[cfg(test)]
mod openai_responses_contract_tests;
mod provider_bridge;
mod provider_bridge_spend;
mod provider_models;
mod provider_sse_events;
mod provider_sse_reader;
mod provider_tools;
mod workers;
pub(crate) use anthropic_rewrite::{
    RuntimeAnthropicOAuthProfileAuth, RuntimeAnthropicProviderAuth,
};
pub(crate) use deepseek_rewrite::RuntimeDeepSeekWebSearchMode;
pub(crate) use gemini_rewrite::{RuntimeGeminiOAuthProfileAuth, RuntimeGeminiProviderAuth};
pub(crate) use local_rewrite::{
    RUNTIME_LOCAL_REWRITE_PROXY_MOUNT_PATH, RuntimeGatewayAdminRole, RuntimeGatewayAdminToken,
    RuntimeGatewayGuardrailWebhookConfig, RuntimeGatewayObservabilityConfig,
    RuntimeGatewayOidcConfig, RuntimeGatewaySsoConfig, RuntimeGatewayStateStore,
    RuntimeLocalRewriteProviderOptions, RuntimeLocalRewriteProxyStartOptions,
    RuntimeProjectedProviderCredential, start_runtime_gateway_rewrite_proxy_with_runtime_config,
    start_runtime_local_rewrite_proxy,
};
pub(crate) use local_rewrite_copilot::{RuntimeCopilotProfileAuth, RuntimeCopilotProviderAuth};
pub(crate) use local_rewrite_gateway_backend_connection::{
    runtime_gateway_postgres_migrate_compatibility_state,
    runtime_gateway_sqlite_migrate_compatibility_state,
};
pub(crate) use local_rewrite_gateway_credentials::{
    RuntimeGatewayCredentialRefreshCandidate, RuntimeGatewayCredentialRefreshPlan,
};
pub(crate) use local_rewrite_kiro::RuntimeKiroProfileAuth;
use workers::spawn_runtime_rotation_proxy_workers;

#[cfg(test)]
pub(crate) fn start_runtime_rotation_proxy(
    paths: &AppPaths,
    state: &AppState,
    current_profile: &str,
    upstream_base_url: String,
    include_code_review: bool,
) -> Result<RuntimeRotationProxy> {
    start_runtime_rotation_proxy_with_listen_addr(
        paths,
        state,
        current_profile,
        upstream_base_url,
        include_code_review,
        false,
        None,
    )
}

#[cfg(test)]
pub(crate) fn start_runtime_rotation_proxy_with_listen_addr(
    paths: &AppPaths,
    state: &AppState,
    current_profile: &str,
    upstream_base_url: String,
    include_code_review: bool,
    upstream_no_proxy: bool,
    preferred_listen_addr: Option<&str>,
) -> Result<RuntimeRotationProxy> {
    start_runtime_rotation_proxy_with_options(RuntimeRotationProxyStartOptions {
        paths,
        state,
        current_profile,
        upstream_base_url,
        include_code_review,
        upstream_no_proxy,
        auto_redeem: false,
        smart_context_enabled: false,
        presidio_redaction_enabled: false,
        model_context_window_tokens: None,
        preferred_listen_addr,
    })
}

pub(crate) struct RuntimeRotationProxyStartOptions<'a> {
    pub(crate) paths: &'a AppPaths,
    pub(crate) state: &'a AppState,
    pub(crate) current_profile: &'a str,
    pub(crate) upstream_base_url: String,
    pub(crate) include_code_review: bool,
    pub(crate) upstream_no_proxy: bool,
    pub(crate) auto_redeem: bool,
    pub(crate) smart_context_enabled: bool,
    pub(crate) presidio_redaction_enabled: bool,
    pub(crate) model_context_window_tokens: Option<u64>,
    pub(crate) preferred_listen_addr: Option<&'a str>,
}

pub(crate) fn start_runtime_rotation_proxy_with_options(
    options: RuntimeRotationProxyStartOptions<'_>,
) -> Result<RuntimeRotationProxy> {
    let RuntimeRotationProxyStartOptions {
        paths,
        state,
        current_profile,
        upstream_base_url,
        include_code_review,
        upstream_no_proxy,
        auto_redeem,
        smart_context_enabled,
        presidio_redaction_enabled,
        model_context_window_tokens,
        preferred_listen_addr,
    } = options;
    let runtime_config = Arc::new(RuntimeConfig::from_env_policy_and_cli(paths)?);
    let log_path = initialize_runtime_proxy_log_path_from_config(&runtime_config);
    for key in runtime_config.compatibility_defaults() {
        runtime_proxy_log_to_path(
            &log_path,
            &runtime_proxy_structured_log_message(
                "runtime_config_compatibility_default",
                [runtime_proxy_log_field("key", *key)],
            ),
        );
    }
    let (server, listen_addr) = match preferred_listen_addr {
        Some(preferred) => match TinyServer::http(preferred) {
            Ok(server) => {
                let server = Arc::new(server);
                let listen_addr = server.server_addr().to_ip().with_context(|| {
                    format!(
                        "runtime auto-rotate proxy did not expose a TCP listen address after binding {preferred}"
                    )
                })?;
                (server, listen_addr)
            }
            Err(err) => {
                runtime_proxy_log_to_path(
                    &log_path,
                    &format!(
                        "runtime proxy preferred_listen_addr_unavailable requested={preferred} error={err}"
                    ),
                );
                let server = Arc::new(TinyServer::http("127.0.0.1:0").map_err(|fallback_err| {
                    anyhow::anyhow!(
                        "failed to bind runtime auto-rotate proxy on {preferred}: {err}; fallback bind also failed: {fallback_err}"
                    )
                })?);
                let listen_addr = server.server_addr().to_ip().context(
                    "runtime auto-rotate proxy did not expose a TCP listen address after fallback bind",
                )?;
                (server, listen_addr)
            }
        },
        None => {
            let server = Arc::new(TinyServer::http("127.0.0.1:0").map_err(|err| {
                anyhow::anyhow!("failed to bind runtime auto-rotate proxy: {err}")
            })?);
            let listen_addr = server
                .server_addr()
                .to_ip()
                .context("runtime auto-rotate proxy did not expose a TCP listen address")?;
            (server, listen_addr)
        }
    };
    initialize_runtime_probe_refresh_queue(runtime_config.tuning.probe_refresh_worker_count);
    let owner_lock = try_acquire_runtime_owner_lock(paths)?;
    let persistence_enabled = owner_lock.is_some();
    let async_worker_count = runtime_config.tuning.async_worker_count;
    let async_runtime = Arc::new(
        TokioRuntimeBuilder::new_multi_thread()
            .worker_threads(async_worker_count)
            .enable_all()
            .build()
            .context("failed to build runtime auto-rotate async runtime")?,
    );
    let worker_count = runtime_config.tuning.worker_count;
    let long_lived_worker_count = runtime_config.tuning.long_lived_worker_count;
    let long_lived_queue_capacity = runtime_config.tuning.long_lived_queue_capacity;
    let active_request_limit = runtime_config.tuning.active_request_limit;
    let lane_admission = RuntimeProxyLaneAdmission::new(RuntimeProxyLaneLimits {
        responses: runtime_config.tuning.lane_limits.responses,
        compact: runtime_config.tuning.lane_limits.compact,
        websocket: runtime_config.tuning.lane_limits.websocket,
        standard: runtime_config.tuning.lane_limits.standard,
    });
    let persisted_state = AppState::load_with_recovery(paths).unwrap_or(RecoveredLoad {
        value: state.clone(),
        recovered_from_backup: false,
    });
    let mut restored_state = merge_runtime_state_snapshot(state.clone(), &persisted_state.value);
    let persisted_continuations =
        load_runtime_continuations_with_recovery(paths, &restored_state.profiles).unwrap_or(
            RecoveredLoad {
                value: RuntimeContinuationStore::default(),
                recovered_from_backup: false,
            },
        );
    let continuation_journal =
        load_runtime_continuation_journal_with_recovery(paths, &restored_state.profiles).unwrap_or(
            RecoveredLoad {
                value: RuntimeContinuationJournal::default(),
                recovered_from_backup: false,
            },
        );
    let fallback_continuations = runtime_continuation_store_from_app_state(&restored_state);
    let restored_continuations = merge_runtime_continuation_store(
        &merge_runtime_continuation_store(
            &fallback_continuations,
            &persisted_continuations.value,
            &restored_state.profiles,
        ),
        &continuation_journal.value.continuations,
        &restored_state.profiles,
    );
    let continuation_sidecar_present = runtime_continuations_file_path(paths).exists()
        || runtime_continuations_last_good_file_path(paths).exists();
    let continuation_migration_needed = !continuation_sidecar_present
        && (restored_continuations != RuntimeContinuationStore::default());
    let restored_session_id_bindings = merge_profile_bindings(
        &restored_continuations.session_profile_bindings,
        &runtime_external_session_id_bindings(&restored_continuations.session_id_bindings),
        &restored_state.profiles,
    );
    let restored_runtime_session_id_bindings = merge_profile_bindings(
        &restored_continuations.session_id_bindings,
        &restored_continuations.session_profile_bindings,
        &restored_state.profiles,
    );
    restored_state.response_profile_bindings =
        restored_continuations.response_profile_bindings.clone();
    restored_state.session_profile_bindings = restored_session_id_bindings.clone();
    let persisted_profile_scores =
        load_runtime_profile_scores_with_recovery(paths, &restored_state.profiles).unwrap_or(
            RecoveredLoad {
                value: BTreeMap::new(),
                recovered_from_backup: false,
            },
        );
    let persisted_usage_snapshots =
        load_runtime_usage_snapshots_with_recovery(paths, &restored_state.profiles).unwrap_or(
            RecoveredLoad {
                value: BTreeMap::new(),
                recovered_from_backup: false,
            },
        );
    let mut persisted_backoffs =
        load_runtime_profile_backoffs_with_recovery(paths, &restored_state.profiles).unwrap_or(
            RecoveredLoad {
                value: RuntimeProfileBackoffs::default(),
                recovered_from_backup: false,
            },
        );
    let startup_now = Local::now().timestamp();
    let persisted_backoffs_softened = runtime_soften_persisted_backoffs_for_startup(
        &mut persisted_backoffs.value,
        &persisted_profile_scores.value,
        startup_now,
    );
    let persisted_profile_scores_count = persisted_profile_scores.value.len();
    let persisted_usage_snapshots_count = persisted_usage_snapshots.value.len();
    let persisted_response_binding_count = runtime_external_response_profile_bindings(
        &restored_continuations.response_profile_bindings,
    )
    .len();
    let persisted_session_binding_count = restored_continuations.session_profile_bindings.len();
    let persisted_turn_state_binding_count = restored_continuations.turn_state_bindings.len();
    let persisted_session_id_binding_count = restored_runtime_session_id_bindings.len();
    let persisted_retry_backoffs_count = persisted_backoffs.value.retry_backoff_until.len();
    let persisted_transport_backoffs_count = persisted_backoffs.value.transport_backoff_until.len();
    let persisted_route_circuit_count = persisted_backoffs.value.route_circuit_open_until.len();
    let expired_usage_snapshot_count = persisted_usage_snapshots
        .value
        .values()
        .filter(|snapshot| !runtime_usage_snapshot_is_usable(snapshot, startup_now))
        .count();
    let restored_global_scores_count = persisted_profile_scores
        .value
        .keys()
        .filter(|key| !key.starts_with("__route_"))
        .count();
    let restored_route_scores_count = persisted_profile_scores
        .value
        .keys()
        .filter(|key| key.starts_with("__route_health__"))
        .count();
    let restored_bad_pairing_count = persisted_profile_scores
        .value
        .keys()
        .filter(|key| key.starts_with("__route_bad_pairing__"))
        .count();
    let restored_success_streak_count = persisted_profile_scores
        .value
        .keys()
        .filter(|key| key.starts_with("__route_success__"))
        .count();
    let shared = RuntimeRotationProxyShared {
        runtime_config: Arc::clone(&runtime_config),
        upstream_no_proxy,
        auto_redeem_enabled: auto_redeem,
        async_client: build_runtime_upstream_async_http_client(upstream_no_proxy, &runtime_config)?,
        async_runtime,
        log_path: log_path.clone(),
        request_sequence: Arc::new(AtomicU64::new(runtime_proxy_request_sequence_seed(
            &log_path,
        ))),
        state_save_revision: Arc::new(AtomicU64::new(0)),
        local_overload_backoff_until: Arc::new(AtomicU64::new(0)),
        active_request_count: Arc::new(AtomicUsize::new(0)),
        active_request_limit,
        runtime_state_lock_wait_counters:
            RuntimeRotationProxyShared::new_runtime_state_lock_wait_counters(),
        lane_admission: lane_admission.clone(),
        runtime: Arc::new(Mutex::new(RuntimeRotationState {
            paths: paths.clone(),
            state: restored_state.clone(),
            upstream_base_url: upstream_base_url.clone(),
            include_code_review,
            current_profile: current_profile.to_string(),
            profile_usage_auth: load_runtime_profile_usage_auth_cache(&restored_state),
            turn_state_bindings: restored_continuations.turn_state_bindings.clone(),
            session_id_bindings: restored_runtime_session_id_bindings,
            continuation_statuses: restored_continuations.statuses.clone(),
            profile_probe_cache: BTreeMap::new(),
            profile_usage_snapshots: persisted_usage_snapshots.value,
            profile_retry_backoff_until: persisted_backoffs.value.retry_backoff_until,
            profile_transport_backoff_until: persisted_backoffs.value.transport_backoff_until,
            profile_route_circuit_open_until: persisted_backoffs.value.route_circuit_open_until,
            profile_inflight: BTreeMap::new(),
            profile_health: persisted_profile_scores.value,
        })),
    };
    register_runtime_proxy_persistence_mode(&log_path, persistence_enabled);
    register_runtime_smart_context_proxy_state(
        &log_path,
        smart_context_enabled,
        model_context_window_tokens,
        Some(paths.root.join("runtime-smart-context-artifacts.json")),
    );
    register_runtime_presidio_redaction_proxy_state(
        &log_path,
        if presidio_redaction_enabled {
            Some(runtime_presidio_redaction_config(paths)?)
        } else {
            None
        },
    );
    runtime_proxy_log_to_path(
        &log_path,
        &format!(
            "runtime proxy started listen_addr={listen_addr} current_profile={current_profile} include_code_review={include_code_review} smart_context_enabled={smart_context_enabled} presidio_redaction_enabled={presidio_redaction_enabled} upstream_base_url={upstream_base_url} persistence_mode={}",
            if persistence_enabled {
                "owner"
            } else {
                "follower"
            }
        ),
    );
    runtime_proxy_log_to_path(
        &log_path,
        &format!(
            "runtime_proxy_upstream_proxy_mode mode={}",
            runtime_upstream_proxy_mode_label(upstream_no_proxy)
        ),
    );
    runtime_proxy_log_to_path(
        &log_path,
        &format!(
            "runtime_proxy_restore_counts persisted_scores={} persisted_usage_snapshots={} expired_usage_snapshots={} response_bindings={} session_bindings={} turn_state_bindings={} session_id_bindings={} retry_backoffs={} transport_backoffs={} route_circuits={} global_scores={} route_scores={} bad_pairing_scores={} success_streak_scores={} recovered_state={} recovered_continuations={} recovered_scores={} recovered_usage_snapshots={} recovered_backoffs={} recovered_continuation_journal={}",
            persisted_profile_scores_count,
            persisted_usage_snapshots_count,
            expired_usage_snapshot_count,
            persisted_response_binding_count,
            persisted_session_binding_count,
            persisted_turn_state_binding_count,
            persisted_session_id_binding_count,
            persisted_retry_backoffs_count,
            persisted_transport_backoffs_count,
            persisted_route_circuit_count,
            restored_global_scores_count,
            restored_route_scores_count,
            restored_bad_pairing_count,
            restored_success_streak_count,
            persisted_state.recovered_from_backup,
            persisted_continuations.recovered_from_backup,
            persisted_profile_scores.recovered_from_backup,
            persisted_usage_snapshots.recovered_from_backup,
            persisted_backoffs.recovered_from_backup,
            continuation_journal.recovered_from_backup,
        ),
    );
    audit_runtime_proxy_startup_state(&shared);
    schedule_runtime_startup_probe_warmup(&shared);
    if persisted_backoffs_softened && let Ok(runtime) = shared.runtime.lock() {
        schedule_runtime_state_save_from_runtime(&shared, &runtime, "startup_backoff_soften");
    }
    if continuation_migration_needed && let Ok(runtime) = shared.runtime.lock() {
        schedule_runtime_state_save_from_runtime(
            &shared,
            &runtime,
            "startup_continuation_migration",
        );
    }
    let shutdown = Arc::new(AtomicBool::new(false));
    runtime_proxy_log_to_path(
        &log_path,
        &format!(
            "runtime proxy worker_count={worker_count} async_worker_count={async_worker_count} long_lived_worker_count={long_lived_worker_count} long_lived_queue_capacity={long_lived_queue_capacity} active_request_limit={active_request_limit} lane_limits=responses:{} compact:{} websocket:{} standard:{}",
            lane_admission.limits.responses,
            lane_admission.limits.compact,
            lane_admission.limits.websocket,
            lane_admission.limits.standard
        ),
    );
    let worker_threads = spawn_runtime_rotation_proxy_workers(
        &server,
        &shutdown,
        &shared,
        worker_count,
        long_lived_worker_count,
        long_lived_queue_capacity,
    );

    Ok(RuntimeRotationProxy {
        runtime_config: Arc::clone(&runtime_config),
        server,
        shutdown,
        worker_threads,
        accept_worker_count: worker_count,
        listen_addr,
        gemini_live_sidecar_addr: None,
        gemini_live_sidecar_model: None,
        log_path,
        active_request_count: Arc::clone(&shared.active_request_count),
        #[cfg(test)]
        request_sequence: Arc::clone(&shared.request_sequence),
        #[cfg(test)]
        lane_admission: shared.lane_admission.clone(),
        #[cfg(test)]
        gateway_route_load: None,
        #[cfg(test)]
        gateway_usage: None,
        #[cfg(test)]
        gateway_side_effect_snapshot: None,
        owner_lock,
    })
}
