use super::super::copilot_instructions::runtime_copilot_init_current_workspace_custom_instructions;
use super::deepseek_rewrite::RuntimeDeepSeekConversationStore;
mod background_workers;
mod context;
mod listener_worker;

pub(super) use self::background_workers::{
    RuntimeLocalRewriteWorkers, spawn_runtime_local_rewrite_workers,
};
use self::background_workers::{runtime_local_rewrite_log_path, runtime_local_rewrite_server};
pub(super) use self::context::{
    RuntimeLocalRewriteProcessServices, RuntimeLocalRewriteProxyShared,
    RuntimeLocalRewriteRequestContext,
};
use self::listener_worker::spawn_runtime_local_rewrite_listener_worker;
#[cfg(test)]
pub(crate) use super::local_rewrite_constraints::start_runtime_local_rewrite_proxy;
pub(crate) use super::local_rewrite_constraints::start_runtime_local_rewrite_proxy;
use super::local_rewrite_copilot::runtime_copilot_oauth_pool_from_provider;
use super::local_rewrite_gemini::runtime_gemini_oauth_pool_from_provider;
pub(super) use super::local_rewrite_model_memory::{
    RuntimeLocalRewriteModelMemoryState, runtime_local_rewrite_model_selection,
};
pub(crate) use super::local_rewrite_options::{
    RuntimeLocalRewriteProviderOptions, RuntimeLocalRewriteProxyStartOptions,
};
use super::local_rewrite_pipeline::run_runtime_local_rewrite_pipeline;
use super::local_rewrite_request::RuntimeLocalRewriteRequest;
pub(super) use super::local_rewrite_upstream::{
    RuntimeLocalRewriteContinuationReader, RuntimeLocalRewriteLiveResponse,
    RuntimeLocalRewriteSsePrefetch, RuntimeLocalRewriteUpstreamResponse,
    RuntimeLocalRewriteUpstreamResult,
};
use super::provider_bridge::runtime_provider_label;
use crate::presidio_runtime::runtime_governed_presidio_redaction_config;
use crate::proxy_config::{
    build_runtime_upstream_async_http_client, build_runtime_upstream_async_http_compact_client,
    runtime_upstream_proxy_mode_label,
};
use crate::quota_support::validate_credential_free_http_url;
use crate::runtime_background::{
    RuntimeProxyMarkerGuard, initialize_runtime_probe_refresh_queue,
    register_runtime_proxy_persistence_mode,
};
use crate::runtime_config::RuntimeConfig;
use crate::runtime_core_shared::runtime_proxy_log_to_path;
use crate::runtime_proxy::{
    build_runtime_proxy_json_error_response, register_runtime_presidio_redaction_proxy_state,
    register_runtime_smart_context_proxy_state,
};
use crate::runtime_state_shared::{
    RuntimeContinuationStatuses, RuntimeRotationProxyShared, RuntimeRotationState,
};
use crate::{RuntimeRotationProxy, runtime_proxy_request_sequence_seed};
use anyhow::{Context, Result};
use prodex_provider_core::provider_adapter;
use prodex_runtime_state::{RuntimeProxyLaneAdmission, RuntimeProxyLaneLimits};
use runtime_proxy_crate::RuntimeProxyRequest;
use std::collections::BTreeMap;
use std::io::{self, Read};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize};
use std::sync::{Arc, Mutex};
use tokio::runtime::Builder as TokioRuntimeBuilder;

pub(crate) const RUNTIME_LOCAL_REWRITE_PROXY_MOUNT_PATH: &str = "/v1";
pub(super) const RUNTIME_LOCAL_REWRITE_PROFILE: &str = "local";
pub(super) const RUNTIME_GATEWAY_CONVERSATION_NAMESPACE_HEADER: &str =
    "x-prodex-internal-conversation-namespace";

pub(super) struct RuntimeLocalRewriteAsyncResponse {
    pub(super) response: Option<reqwest::Response>,
    status: reqwest::StatusCode,
    headers: reqwest::header::HeaderMap,
    pub(super) async_runtime: Arc<tokio::runtime::Runtime>,
    pub(super) stream_idle_timeout_ms: u64,
    pub(super) pending: Vec<u8>,
    pub(super) reader: Option<RuntimeLocalRewriteContinuationReader>,
}

impl RuntimeLocalRewriteAsyncResponse {
    pub(super) fn new(
        response: reqwest::Response,
        async_runtime: Arc<tokio::runtime::Runtime>,
        stream_idle_timeout_ms: u64,
    ) -> Self {
        let status = response.status();
        let headers = response.headers().clone();
        Self {
            response: Some(response),
            status,
            headers,
            async_runtime,
            stream_idle_timeout_ms,
            pending: Vec::new(),
            reader: None,
        }
    }

    pub(super) fn status(&self) -> reqwest::StatusCode {
        self.status
    }

    pub(super) fn headers(&self) -> &reqwest::header::HeaderMap {
        &self.headers
    }

    pub(super) fn into_reader(mut self) -> Box<dyn Read + Send> {
        if let Some(reader) = self.reader.take() {
            return Box::new(reader);
        }
        Box::new(RuntimeLocalRewriteSsePrefetch::spawn(self, None).into_reader())
    }
}

impl Read for RuntimeLocalRewriteAsyncResponse {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        if self.reader.is_none() {
            let response = self.response.take().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "runtime upstream stream reader was already handed off",
                )
            })?;
            let prefetch = RuntimeLocalRewriteSsePrefetch::spawn_parts(
                response,
                Arc::clone(&self.async_runtime),
                self.stream_idle_timeout_ms,
                std::mem::take(&mut self.pending),
                None,
            );
            self.reader = Some(prefetch.into_reader());
        }
        self.reader
            .as_mut()
            .expect("runtime local rewrite stream reader should be present")
            .read(buffer)
    }
}

impl RuntimeLocalRewriteRequestContext {
    fn conversation_store_for_request(
        &self,
        request: &RuntimeProxyRequest,
        store: &RuntimeDeepSeekConversationStore,
    ) -> RuntimeDeepSeekConversationStore {
        if self.allow_local_file_access {
            return store.clone();
        }
        let namespace = runtime_proxy_crate::runtime_request_session_id(request)
            .or_else(|| {
                request
                    .headers
                    .iter()
                    .find(|(name, _)| {
                        name.eq_ignore_ascii_case(RUNTIME_GATEWAY_CONVERSATION_NAMESPACE_HEADER)
                    })
                    .map(|(_, value)| value.clone())
            })
            .unwrap_or_else(|| "gateway".to_string());
        store.scoped(&namespace)
    }

    pub(super) fn deepseek_conversations_for_request(
        &self,
        request: &RuntimeProxyRequest,
    ) -> RuntimeDeepSeekConversationStore {
        self.conversation_store_for_request(request, &self.deepseek_conversations)
    }

    pub(super) fn gemini_conversations_for_request(
        &self,
        request: &RuntimeProxyRequest,
    ) -> RuntimeDeepSeekConversationStore {
        self.conversation_store_for_request(request, &self.gemini_conversations)
    }
}

pub(super) type RuntimeLocalRewriteModelMemory = Arc<Mutex<RuntimeLocalRewriteModelMemoryState>>;

pub(super) fn start_runtime_local_rewrite_proxy_with_file_access(
    options: RuntimeLocalRewriteProxyStartOptions<'_>,
    runtime_config: Arc<RuntimeConfig>,
    allow_local_file_access: bool,
) -> Result<RuntimeRotationProxy> {
    validate_credential_free_http_url(&options.upstream_base_url, "runtime upstream base URL")?;
    let (server, listen_addr) = runtime_local_rewrite_server(options.preferred_listen_addr)?;
    let prepared = prepare_runtime_local_rewrite_application(
        options,
        runtime_config,
        allow_local_file_access,
        ("loopback", Some(listen_addr)),
    )?;
    let RuntimeLocalRewritePrepared {
        runtime_config,
        shared,
        shutdown,
        draining,
        worker_count,
        log_path,
        marker_guard,
    } = prepared;
    #[cfg(test)]
    let (listener_ready_tx, listener_ready_rx) = std::sync::mpsc::channel();
    let RuntimeLocalRewriteWorkers {
        worker_threads,
        gemini_live_sidecar_addr,
    } = spawn_runtime_local_rewrite_workers(
        &shared,
        Some(&server),
        &shutdown,
        worker_count,
        #[cfg(test)]
        Some(listener_ready_tx),
        false,
    )?;
    #[cfg(test)]
    for _ in 0..worker_count {
        listener_ready_rx
            .recv()
            .expect("runtime local rewrite listener should start");
    }
    Ok(RuntimeRotationProxy {
        runtime_config: Arc::clone(&runtime_config),
        server,
        draining,
        shutdown,
        worker_threads,
        accept_worker_count: worker_count,
        listen_addr,
        realtime_ws_sidecar_addr: gemini_live_sidecar_addr,
        realtime_ws_model: None,
        log_path,
        active_request_count: Arc::clone(&shared.runtime_shared.active_request_count),
        #[cfg(test)]
        request_sequence: Arc::clone(&shared.runtime_shared.request_sequence),
        #[cfg(test)]
        lane_admission: shared.runtime_shared.lane_admission.clone(),
        #[cfg(test)]
        gateway_route_load: None,
        #[cfg(test)]
        gateway_usage: None,
        #[cfg(test)]
        gateway_side_effect_snapshot: None,
        owner_lock: None,
        _live_log_source: None,
        _marker_guard: marker_guard,
    })
}

pub(super) struct RuntimeLocalRewritePrepared {
    pub(super) runtime_config: Arc<RuntimeConfig>,
    pub(super) shared: RuntimeLocalRewriteProxyShared,
    pub(super) shutdown: Arc<AtomicBool>,
    pub(super) draining: Arc<AtomicBool>,
    pub(super) worker_count: usize,
    pub(super) log_path: PathBuf,
    pub(super) marker_guard: RuntimeProxyMarkerGuard,
}

pub(super) fn prepare_runtime_local_rewrite_application(
    options: RuntimeLocalRewriteProxyStartOptions<'_>,
    runtime_config: Arc<RuntimeConfig>,
    allow_local_file_access: bool,
    transport: (&str, Option<std::net::SocketAddr>),
) -> Result<RuntimeLocalRewritePrepared> {
    let (transport, listen_addr) = transport;
    let RuntimeLocalRewriteProxyStartOptions {
        paths,
        state,
        upstream_base_url,
        provider,
        upstream_no_proxy,
        smart_context_enabled,
        presidio_redaction_enabled,
        model_context_window_tokens,
        preferred_listen_addr: _,
    } = options;
    validate_credential_free_http_url(&upstream_base_url, "runtime upstream base URL")?;
    let log_path = runtime_local_rewrite_log_path(&runtime_config)?;
    initialize_runtime_probe_refresh_queue(runtime_config.tuning.probe_refresh_worker_count);
    let worker_count = runtime_config.tuning.worker_count;
    let active_request_limit = runtime_config.tuning.active_request_limit;
    let lane_admission = RuntimeProxyLaneAdmission::new(RuntimeProxyLaneLimits {
        responses: runtime_config.tuning.lane_limits.responses,
        compact: runtime_config.tuning.lane_limits.compact,
        websocket: runtime_config.tuning.lane_limits.websocket,
        standard: runtime_config.tuning.lane_limits.standard,
    });
    let async_runtime = Arc::new(
        TokioRuntimeBuilder::new_multi_thread()
            .worker_threads(runtime_config.tuning.async_worker_count)
            .enable_all()
            .build()
            .context("failed to build runtime local rewrite async runtime")?,
    );
    let runtime_shared = RuntimeRotationProxyShared {
        smart_context_engine: Arc::new(crate::RuntimeSmartContextEngine::default()),
        runtime_config: Arc::clone(&runtime_config),
        upstream_no_proxy,
        auto_redeem_enabled: false,
        async_client: build_runtime_upstream_async_http_client(true, &runtime_config)?,
        compact_client: build_runtime_upstream_async_http_compact_client(
            upstream_no_proxy,
            &runtime_config,
        )?,
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
        lane_admission,
        runtime: Arc::new(Mutex::new(RuntimeRotationState {
            paths: paths.clone(),
            state: state.clone(),
            upstream_base_url: upstream_base_url.clone(),
            include_code_review: false,
            current_profile: RUNTIME_LOCAL_REWRITE_PROFILE.to_string(),
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
    };
    let marker_guard = RuntimeProxyMarkerGuard::new(&log_path);
    register_runtime_proxy_persistence_mode(&log_path, true);
    register_runtime_smart_context_proxy_state(
        &runtime_shared,
        smart_context_enabled,
        model_context_window_tokens,
        Some(paths.root.join("runtime-smart-context-artifacts.json")),
    );
    register_runtime_presidio_redaction_proxy_state(
        &log_path,
        if presidio_redaction_enabled {
            Some(runtime_governed_presidio_redaction_config(
                paths,
                &runtime_config,
            )?)
        } else {
            None
        },
    )?;
    let bridge_kind = provider.bridge_kind();
    let contract = provider_adapter(bridge_kind.provider_id());
    runtime_proxy_log_to_path(
        &log_path,
        &format!(
            "runtime local rewrite started transport={transport} listen_addr={} smart_context_enabled={smart_context_enabled} presidio_redaction_enabled={presidio_redaction_enabled} upstream_base_url={upstream_base_url} upstream_proxy_mode={} provider={} client_format={} upstream_format={} response_format={} endpoint={}",
            listen_addr.map_or_else(|| "-".to_string(), |addr| addr.to_string()),
            runtime_upstream_proxy_mode_label(true),
            runtime_provider_label(bridge_kind),
            contract.client_request_format().label(),
            contract.upstream_request_format().label(),
            contract.response_format().label(),
            contract.canonical_client_endpoint(),
        ),
    );
    let gemini_oauth_pool = runtime_gemini_oauth_pool_from_provider(&provider);
    let copilot_oauth_pool =
        runtime_copilot_oauth_pool_from_provider(&provider, Arc::clone(&runtime_shared.runtime));
    if matches!(
        &provider,
        RuntimeLocalRewriteProviderOptions::Copilot { .. }
    ) {
        runtime_copilot_init_current_workspace_custom_instructions();
    }
    let process = Arc::new(RuntimeLocalRewriteProcessServices {
        runtime_shared: runtime_shared.clone(),
        mount_path: RUNTIME_LOCAL_REWRITE_PROXY_MOUNT_PATH.to_string(),
        deepseek_conversations: RuntimeDeepSeekConversationStore::default(),
        gemini_conversations: RuntimeDeepSeekConversationStore::default(),
        gemini_oauth_pool,
        copilot_oauth_pool,
        model_memory: Arc::new(Mutex::new(RuntimeLocalRewriteModelMemoryState::default())),
        api_key_cursor: Arc::new(AtomicUsize::new(0)),
        provider_sse_prefetch_slots: Arc::new(tokio::sync::Semaphore::new(
            active_request_limit.max(1),
        )),
        allow_local_file_access,
    });
    let shared = RuntimeLocalRewriteRequestContext {
        process,
        upstream_base_url,
        provider: Arc::new(provider),
    };
    Ok(RuntimeLocalRewritePrepared {
        runtime_config,
        shared,
        shutdown: Arc::new(AtomicBool::new(false)),
        draining: Arc::new(AtomicBool::new(false)),
        worker_count,
        log_path,
        marker_guard,
    })
}

fn handle_runtime_local_rewrite_proxy_request(
    request: tiny_http::Request,
    shared: &RuntimeLocalRewriteProxyShared,
) {
    let request = RuntimeLocalRewriteRequest::tiny(request);
    if !super::local_rewrite_request::runtime_local_rewrite_request_target_valid(request.url()) {
        let _ = request.respond(build_runtime_proxy_json_error_response(
            400,
            "invalid_request_target",
            "request target is invalid",
        ));
        return;
    }
    let target = request.url().to_string();
    run_runtime_local_rewrite_pipeline(request, target, shared);
}
