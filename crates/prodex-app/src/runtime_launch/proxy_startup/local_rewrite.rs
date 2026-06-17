use super::super::copilot_instructions::runtime_copilot_init_current_workspace_custom_instructions;
use super::deepseek_rewrite::*;
use super::gemini_rewrite::*;
use super::local_rewrite_copilot::{
    RuntimeCopilotOAuthPool, RuntimeCopilotRequestContext,
    runtime_copilot_oauth_pool_from_provider, send_runtime_copilot_upstream_request,
};
use super::local_rewrite_deepseek::send_runtime_deepseek_upstream_request;
use super::local_rewrite_gemini::{
    RuntimeGeminiOAuthPool, RuntimeGeminiRequestContext, runtime_gemini_oauth_pool_from_provider,
    send_runtime_gemini_upstream_request,
};
use super::local_rewrite_gemini_compact::{
    runtime_gemini_local_compact_response_parts, runtime_gemini_semantic_compact_request_body,
    runtime_gemini_semantic_compact_response_parts,
};
use super::local_rewrite_gemini_live::{
    handle_runtime_gemini_live_websocket_request, spawn_runtime_gemini_live_sidecar,
};
use super::local_rewrite_response::{
    respond_runtime_local_rewrite_proxy_request,
    runtime_local_rewrite_buffered_response_from_response,
    runtime_local_rewrite_response_with_call_id,
};
use super::local_rewrite_search_fallback::{
    RuntimeLocalRewritePreparedSendResult, RuntimeLocalRewriteSearchFallbackRequest,
    send_runtime_local_rewrite_prepared_request_with_chat_search_fallback,
};
use super::local_rewrite_transport::{
    RuntimeLocalRewritePreparedAuth, runtime_local_rewrite_anthropic_auth_attempts,
    runtime_local_rewrite_api_key_attempts, runtime_local_rewrite_upstream_url,
    runtime_openai_standard_provider_upstream_url, send_runtime_local_rewrite_prepared_request,
};
use super::provider_bridge::{
    RuntimeProviderBridgeKind, RuntimeProviderGatewaySpendEvent, runtime_provider_error_class,
    runtime_provider_gateway_cost_for_request, runtime_provider_model_fallback_chain,
    runtime_provider_model_from_body, runtime_provider_models_buffered_response,
    runtime_provider_openai_contract, runtime_provider_request_body_with_model,
    runtime_provider_request_ledger_message, runtime_provider_should_retry_with_next_model,
    runtime_provider_should_rotate_auth_after_response,
};
use super::*;
use base64::Engine;
use fs2::FileExt;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header, jwk::JwkSet};
use postgres::{Client as PostgresClient, GenericClient, NoTls};
use prodex_provider_core::{
    calculate_cost_microusd, estimate_request_input_tokens, microusd_to_usd,
};
use redis::Commands;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::OpenOptions;

pub(crate) const RUNTIME_LOCAL_REWRITE_PROXY_MOUNT_PATH: &str = "/v1";
pub(super) const RUNTIME_LOCAL_REWRITE_PROFILE: &str = "local";
const RUNTIME_GATEWAY_REDIS_KEY_STORE_KEY: &str = "prodex:gateway:virtual_keys";
const RUNTIME_GATEWAY_REDIS_KEY_STORE_LOCK: &str = "prodex:gateway:virtual_keys:lock";
const RUNTIME_GATEWAY_REDIS_USAGE_KEY: &str = "prodex:gateway:virtual_key_usage";
const RUNTIME_GATEWAY_REDIS_USAGE_LOCK: &str = "prodex:gateway:virtual_key_usage:lock";
const RUNTIME_GATEWAY_REDIS_LEDGER_KEY: &str = "prodex:gateway:billing_ledger";
const RUNTIME_GATEWAY_REDIS_LEDGER_LOCK: &str = "prodex:gateway:billing_ledger:lock";
const RUNTIME_GATEWAY_SCIM_USER_SCHEMA: &str = "urn:ietf:params:scim:schemas:core:2.0:User";
const RUNTIME_GATEWAY_SCIM_LIST_SCHEMA: &str = "urn:ietf:params:scim:api:messages:2.0:ListResponse";
const RUNTIME_GATEWAY_SCIM_PRODEX_SCHEMA: &str = "urn:prodex:params:scim:schemas:gateway:2.0:User";

#[derive(Clone)]
pub(super) struct RuntimeLocalRewriteProxyShared {
    pub(super) runtime_shared: RuntimeRotationProxyShared,
    pub(super) upstream_base_url: String,
    pub(super) mount_path: String,
    pub(super) provider: RuntimeLocalRewriteProviderOptions,
    pub(super) deepseek_conversations: RuntimeDeepSeekConversationStore,
    pub(super) deepseek_pending_messages: RuntimeDeepSeekPendingMessages,
    pub(super) gemini_conversations: RuntimeDeepSeekConversationStore,
    pub(super) gemini_oauth_pool: Option<RuntimeGeminiOAuthPool>,
    pub(super) copilot_oauth_pool: Option<RuntimeCopilotOAuthPool>,
    pub(super) model_memory: RuntimeLocalRewriteModelMemory,
    pub(super) api_key_cursor: Arc<AtomicUsize>,
    pub(super) client: reqwest::blocking::Client,
    pub(super) gateway_auth_token_hash: Option<runtime_proxy_crate::LocalBridgeBearerTokenHash>,
    pub(super) gateway_admin_tokens: Vec<RuntimeGatewayAdminToken>,
    pub(super) gateway_sso: RuntimeGatewaySsoConfig,
    pub(super) gateway_state_store: RuntimeGatewayStateStore,
    pub(super) gateway_virtual_keys: Arc<Mutex<Vec<RuntimeGatewayVirtualKeyEntry>>>,
    pub(super) gateway_virtual_key_store_path: PathBuf,
    pub(super) gateway_virtual_key_usage:
        Arc<Mutex<BTreeMap<String, runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage>>>,
    pub(super) gateway_virtual_key_usage_path: Option<PathBuf>,
    gateway_virtual_key_usage_save_in_flight: Arc<AtomicBool>,
    gateway_virtual_key_usage_save_dirty: Arc<AtomicBool>,
    gateway_virtual_key_usage_pending_deltas: Arc<Mutex<Vec<RuntimeGatewayVirtualKeyUsageDelta>>>,
    gateway_virtual_key_request_ids: Arc<Mutex<BTreeSet<u64>>>,
    pub(super) gateway_route_aliases: Vec<runtime_proxy_crate::RuntimeGatewayRouteAlias>,
    pub(super) gateway_route_load: RuntimeGatewayRouteLoadState,
    pub(super) gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig,
    pub(super) gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig,
    pub(super) gateway_call_id_header: Option<String>,
    pub(super) gateway_observability: RuntimeGatewayObservabilityConfig,
}

pub(super) type RuntimeLocalRewriteModelMemory = Arc<Mutex<RuntimeLocalRewriteModelMemoryState>>;
pub(super) type RuntimeGatewayRouteLoadState =
    Arc<Mutex<BTreeMap<String, runtime_proxy_crate::RuntimeGatewayRouteModelState>>>;

#[derive(Clone, Debug)]
pub(crate) struct RuntimeGatewayAdminToken {
    pub(crate) name: String,
    pub(crate) token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash,
    pub(crate) role: RuntimeGatewayAdminRole,
    pub(crate) tenant_id: Option<String>,
    pub(crate) allowed_key_prefixes: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct RuntimeGatewaySsoConfig {
    pub(crate) proxy_token_hash: Option<runtime_proxy_crate::LocalBridgeBearerTokenHash>,
    pub(crate) token_header: String,
    pub(crate) user_header: String,
    pub(crate) role_header: String,
    pub(crate) tenant_header: String,
    pub(crate) key_prefixes_header: String,
    pub(crate) oidc: Option<RuntimeGatewayOidcConfig>,
    pub(crate) default_role: RuntimeGatewayAdminRole,
}

#[derive(Clone, Debug)]
pub(crate) struct RuntimeGatewayOidcConfig {
    pub(crate) issuer: String,
    pub(crate) audience: String,
    pub(crate) jwks_url: Option<String>,
    pub(crate) user_claim: String,
    pub(crate) role_claim: String,
    pub(crate) tenant_claim: String,
    pub(crate) key_prefixes_claim: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum RuntimeGatewayAdminRole {
    #[default]
    Admin,
    Viewer,
}

impl RuntimeGatewayAdminRole {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "admin" | "write" | "writer" => Some(Self::Admin),
            "viewer" | "read" | "readonly" | "read-only" => Some(Self::Viewer),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::Viewer => "viewer",
        }
    }

    fn can_write(self) -> bool {
        matches!(self, Self::Admin)
    }
}

#[derive(Clone, Debug)]
pub(crate) enum RuntimeGatewayStateStore {
    File {
        key_store_path: PathBuf,
        usage_path: PathBuf,
        ledger_path: PathBuf,
    },
    Sqlite {
        path: PathBuf,
    },
    Postgres {
        url: String,
        state_path: PathBuf,
    },
    Redis {
        url: String,
        state_path: PathBuf,
    },
}

impl RuntimeGatewayStateStore {
    pub(crate) fn file(paths: &AppPaths) -> Self {
        Self::File {
            key_store_path: paths.root.join("gateway-virtual-keys.json"),
            usage_path: paths.root.join("gateway-virtual-key-usage.json"),
            ledger_path: paths.root.join("gateway-billing-ledger.jsonl"),
        }
    }

    pub(crate) fn sqlite(path: PathBuf) -> Self {
        Self::Sqlite { path }
    }

    pub(crate) fn postgres(url_env: String, url: String) -> Self {
        Self::Postgres {
            state_path: PathBuf::from(format!("postgres:{url_env}")),
            url,
        }
    }

    pub(crate) fn redis(url_env: String, url: String) -> Self {
        Self::Redis {
            state_path: PathBuf::from(format!("redis:{url_env}")),
            url,
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Self::File { .. } => "file",
            Self::Sqlite { .. } => "sqlite",
            Self::Postgres { .. } => "postgres",
            Self::Redis { .. } => "redis",
        }
    }

    fn key_store_path(&self) -> &Path {
        match self {
            Self::File { key_store_path, .. } => key_store_path,
            Self::Sqlite { path } => path,
            Self::Postgres { state_path, .. } => state_path,
            Self::Redis { state_path, .. } => state_path,
        }
    }

    fn usage_path(&self) -> &Path {
        match self {
            Self::File { usage_path, .. } => usage_path,
            Self::Sqlite { path } => path,
            Self::Postgres { state_path, .. } => state_path,
            Self::Redis { state_path, .. } => state_path,
        }
    }

    fn ledger_path(&self) -> &Path {
        match self {
            Self::File { ledger_path, .. } => ledger_path,
            Self::Sqlite { path } => path,
            Self::Postgres { state_path, .. } => state_path,
            Self::Redis { state_path, .. } => state_path,
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct RuntimeGatewayVirtualKeyEntry {
    key: runtime_proxy_crate::RuntimeGatewayVirtualKey,
    source: RuntimeGatewayVirtualKeySource,
    tenant_id: Option<String>,
    created_at_epoch: Option<u64>,
    updated_at_epoch: Option<u64>,
    disabled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RuntimeGatewayVirtualKeySource {
    Policy,
    Admin,
}

impl RuntimeGatewayVirtualKeySource {
    fn as_str(self) -> &'static str {
        match self {
            Self::Policy => "policy",
            Self::Admin => "admin",
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct RuntimeGatewayVirtualKeyUsageDelta {
    pub(super) request_id: u64,
    pub(super) key_name: String,
    pub(super) model: String,
    pub(super) minute_epoch: u64,
    pub(super) input_tokens: u64,
    pub(super) estimated_cost_microusd: Option<u64>,
    pub(super) created_at_epoch: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RuntimeGatewayBillingLedgerEntry {
    object: String,
    phase: String,
    request: u64,
    call_id: String,
    key_name: String,
    model: String,
    minute_epoch: u64,
    input_tokens: u64,
    estimated_cost_microusd: Option<u64>,
    estimated_cost_usd: Option<f64>,
    created_at_epoch: u64,
    #[serde(default)]
    response_status: Option<u16>,
    #[serde(default)]
    response_bytes: Option<u64>,
    #[serde(default)]
    output_tokens: Option<u64>,
    #[serde(default)]
    final_cost_microusd: Option<u64>,
    #[serde(default)]
    final_cost_usd: Option<f64>,
    #[serde(default)]
    reconciled_at_epoch: Option<u64>,
}

#[derive(Clone, Debug, Default, Serialize)]
struct RuntimeGatewayBillingSummaryBucket {
    key_name: Option<String>,
    model: Option<String>,
    requests: u64,
    successful_requests: u64,
    failed_requests: u64,
    unreconciled_requests: u64,
    input_tokens: u64,
    output_tokens: u64,
    response_bytes: u64,
    estimated_cost_microusd: u64,
    estimated_cost_usd: f64,
    final_cost_microusd: u64,
    final_cost_usd: f64,
    first_created_at_epoch: Option<u64>,
    last_created_at_epoch: Option<u64>,
    last_reconciled_at_epoch: Option<u64>,
}

impl RuntimeGatewayBillingSummaryBucket {
    fn with_key_model(key_name: Option<String>, model: Option<String>) -> Self {
        Self {
            key_name,
            model,
            ..Self::default()
        }
    }

    fn record(&mut self, entry: &RuntimeGatewayBillingLedgerEntry) {
        self.requests = self.requests.saturating_add(1);
        match entry.response_status {
            Some(status) if (200..300).contains(&status) => {
                self.successful_requests = self.successful_requests.saturating_add(1);
            }
            Some(_) => {
                self.failed_requests = self.failed_requests.saturating_add(1);
            }
            None => {
                self.unreconciled_requests = self.unreconciled_requests.saturating_add(1);
            }
        }
        self.input_tokens = self.input_tokens.saturating_add(entry.input_tokens);
        self.output_tokens = self
            .output_tokens
            .saturating_add(entry.output_tokens.unwrap_or_default());
        self.response_bytes = self
            .response_bytes
            .saturating_add(entry.response_bytes.unwrap_or_default());
        self.estimated_cost_microusd = self
            .estimated_cost_microusd
            .saturating_add(entry.estimated_cost_microusd.unwrap_or_default());
        self.final_cost_microusd = self
            .final_cost_microusd
            .saturating_add(entry.final_cost_microusd.unwrap_or_default());
        self.estimated_cost_usd = microusd_to_usd(self.estimated_cost_microusd);
        self.final_cost_usd = microusd_to_usd(self.final_cost_microusd);
        self.first_created_at_epoch = Some(
            self.first_created_at_epoch
                .map(|current| current.min(entry.created_at_epoch))
                .unwrap_or(entry.created_at_epoch),
        );
        self.last_created_at_epoch = Some(
            self.last_created_at_epoch
                .map(|current| current.max(entry.created_at_epoch))
                .unwrap_or(entry.created_at_epoch),
        );
        if let Some(reconciled_at_epoch) = entry.reconciled_at_epoch {
            self.last_reconciled_at_epoch = Some(
                self.last_reconciled_at_epoch
                    .map(|current| current.max(reconciled_at_epoch))
                    .unwrap_or(reconciled_at_epoch),
            );
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct RuntimeLocalRewriteModelMemoryState {
    selected_models: BTreeMap<String, String>,
}

pub(super) struct RuntimeLocalRewriteModelSelection {
    pub(super) model: String,
    pub(super) body: Vec<u8>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct RuntimeGatewayObservabilityConfig {
    pub(crate) sinks: Vec<String>,
    pub(crate) jsonl_path: Option<PathBuf>,
    pub(crate) http_endpoint: Option<String>,
    pub(crate) http_schema: String,
    pub(crate) http_bearer_token: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct RuntimeGatewayGuardrailWebhookConfig {
    pub(crate) url: Option<String>,
    pub(crate) phases: Vec<String>,
    pub(crate) bearer_token: Option<String>,
    pub(crate) fail_closed: bool,
}

impl RuntimeGatewayGuardrailWebhookConfig {
    pub(super) fn enabled_for(&self, phase: &str) -> bool {
        self.url.is_some()
            && (self.phases.is_empty()
                || self
                    .phases
                    .iter()
                    .any(|configured| configured.eq_ignore_ascii_case(phase)))
    }
}

impl RuntimeGatewayObservabilityConfig {
    pub(super) fn sink_enabled(&self, sink: &str) -> bool {
        self.sinks
            .iter()
            .any(|configured| configured.eq_ignore_ascii_case(sink))
    }
}

#[derive(Clone)]
pub(crate) enum RuntimeLocalRewriteProviderOptions {
    Anthropic {
        auth: RuntimeAnthropicProviderAuth,
    },
    Copilot {
        auth: RuntimeCopilotProviderAuth,
    },
    OpenAiResponses {
        api_keys: Vec<String>,
    },
    DeepSeek {
        api_keys: Vec<String>,
    },
    Gemini {
        auth: RuntimeGeminiProviderAuth,
        thinking_budget_tokens: Option<u64>,
        model_resolution: crate::RuntimeGeminiModelResolution,
    },
}

impl RuntimeLocalRewriteProviderOptions {
    pub(super) fn bridge_kind(&self) -> RuntimeProviderBridgeKind {
        match self {
            RuntimeLocalRewriteProviderOptions::Anthropic { .. } => {
                RuntimeProviderBridgeKind::Anthropic
            }
            RuntimeLocalRewriteProviderOptions::Copilot { .. } => {
                RuntimeProviderBridgeKind::Copilot
            }
            RuntimeLocalRewriteProviderOptions::OpenAiResponses { .. } => {
                RuntimeProviderBridgeKind::OpenAiResponses
            }
            RuntimeLocalRewriteProviderOptions::DeepSeek { .. } => {
                RuntimeProviderBridgeKind::DeepSeek
            }
            RuntimeLocalRewriteProviderOptions::Gemini { .. } => RuntimeProviderBridgeKind::Gemini,
        }
    }
}

pub(crate) struct RuntimeLocalRewriteProxyStartOptions<'a> {
    pub(crate) paths: &'a AppPaths,
    pub(crate) state: &'a AppState,
    pub(crate) upstream_base_url: String,
    pub(crate) provider: RuntimeLocalRewriteProviderOptions,
    pub(crate) upstream_no_proxy: bool,
    pub(crate) smart_context_enabled: bool,
    pub(crate) presidio_redaction_enabled: bool,
    pub(crate) model_context_window_tokens: Option<u64>,
    pub(crate) preferred_listen_addr: Option<&'a str>,
    pub(crate) gateway_auth_token_hash: Option<runtime_proxy_crate::LocalBridgeBearerTokenHash>,
    pub(crate) gateway_admin_tokens: Vec<RuntimeGatewayAdminToken>,
    pub(crate) gateway_sso: RuntimeGatewaySsoConfig,
    pub(crate) gateway_state_store: RuntimeGatewayStateStore,
    pub(crate) gateway_virtual_keys: Vec<runtime_proxy_crate::RuntimeGatewayVirtualKey>,
    pub(crate) gateway_route_aliases: Vec<runtime_proxy_crate::RuntimeGatewayRouteAlias>,
    pub(crate) gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig,
    pub(crate) gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig,
    pub(crate) gateway_call_id_header: Option<String>,
    pub(crate) gateway_observability: RuntimeGatewayObservabilityConfig,
}

pub(crate) fn start_runtime_local_rewrite_proxy(
    options: RuntimeLocalRewriteProxyStartOptions<'_>,
) -> Result<RuntimeRotationProxy> {
    let RuntimeLocalRewriteProxyStartOptions {
        paths,
        state,
        upstream_base_url,
        provider,
        upstream_no_proxy,
        smart_context_enabled,
        presidio_redaction_enabled,
        model_context_window_tokens,
        preferred_listen_addr,
        gateway_auth_token_hash,
        gateway_admin_tokens,
        gateway_sso,
        gateway_state_store,
        gateway_virtual_keys,
        gateway_route_aliases,
        gateway_guardrails,
        gateway_guardrail_webhook,
        gateway_call_id_header,
        gateway_observability,
    } = options;
    let log_path = initialize_runtime_proxy_log_path();
    let bind_addr = preferred_listen_addr.unwrap_or("127.0.0.1:0");
    let server = Arc::new(TinyServer::http(bind_addr).map_err(|err| {
        anyhow::anyhow!("failed to bind runtime local rewrite proxy on {bind_addr}: {err}")
    })?);
    let listen_addr = server
        .server_addr()
        .to_ip()
        .context("runtime local rewrite proxy did not expose a TCP listen address")?;
    let worker_count = runtime_proxy_worker_count();
    let long_lived_worker_count = runtime_proxy_long_lived_worker_count();
    let active_request_limit =
        runtime_proxy_active_request_limit(worker_count, long_lived_worker_count);
    let lane_admission = RuntimeProxyLaneAdmission::new(runtime_proxy_lane_limits(
        active_request_limit,
        worker_count,
        long_lived_worker_count,
    ));
    let async_runtime = Arc::new(
        TokioRuntimeBuilder::new_multi_thread()
            .worker_threads(runtime_proxy_async_worker_count())
            .enable_all()
            .build()
            .context("failed to build runtime local rewrite async runtime")?,
    );
    let runtime_shared = RuntimeRotationProxyShared {
        upstream_no_proxy,
        async_client: build_runtime_upstream_async_http_client(true)?,
        async_runtime,
        log_path: log_path.clone(),
        request_sequence: Arc::new(AtomicU64::new(1)),
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
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        })),
    };
    register_runtime_proxy_persistence_mode(&log_path, true);
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
    let bridge_kind = provider.bridge_kind();
    let openai_contract = runtime_provider_openai_contract(bridge_kind);
    let gateway_virtual_key_store_path = gateway_state_store.key_store_path().to_path_buf();
    let gateway_virtual_key_entries = runtime_gateway_virtual_key_entries_from_sources(
        gateway_virtual_keys,
        &gateway_state_store,
        &log_path,
    );
    let gateway_virtual_key_usage_path = Some(gateway_state_store.usage_path().to_path_buf());
    let gateway_virtual_key_usage =
        runtime_gateway_virtual_key_usage_load(&gateway_state_store, &log_path);
    let gateway_auth_required =
        gateway_auth_token_hash.is_some() || !gateway_virtual_key_entries.is_empty();
    runtime_proxy_log_to_path(
        &log_path,
        &format!(
            "runtime local rewrite proxy started listen_addr={listen_addr} smart_context_enabled={smart_context_enabled} presidio_redaction_enabled={presidio_redaction_enabled} upstream_base_url={upstream_base_url} upstream_proxy_mode={} provider={} client_format={} upstream_format={} response_format={} endpoint={} auth_required={} virtual_keys={} route_aliases={} guardrail_blocked_keywords={} guardrail_blocked_output_keywords={} guardrail_allowed_models={} observability_sinks={}",
            runtime_upstream_proxy_mode_label(true),
            super::provider_bridge::runtime_provider_label(bridge_kind),
            openai_contract.client_request_format.label(),
            openai_contract.upstream_request_format.label(),
            openai_contract.response_format.label(),
            openai_contract.canonical_client_endpoint,
            gateway_auth_required,
            gateway_virtual_key_entries.len(),
            gateway_route_aliases.len(),
            gateway_guardrails.blocked_keywords.len(),
            gateway_guardrails.blocked_output_keywords.len(),
            gateway_guardrails.allowed_models.len(),
            if gateway_observability.sinks.is_empty() {
                "-".to_string()
            } else {
                gateway_observability.sinks.join(",")
            }
        ),
    );
    let gemini_oauth_pool = runtime_gemini_oauth_pool_from_provider(&provider);
    let copilot_oauth_pool = runtime_copilot_oauth_pool_from_provider(&provider);
    if matches!(
        &provider,
        RuntimeLocalRewriteProviderOptions::Copilot { .. }
    ) {
        runtime_copilot_init_current_workspace_custom_instructions();
    }
    let shared = RuntimeLocalRewriteProxyShared {
        runtime_shared: runtime_shared.clone(),
        upstream_base_url,
        mount_path: RUNTIME_LOCAL_REWRITE_PROXY_MOUNT_PATH.to_string(),
        provider,
        deepseek_conversations: Arc::new(Mutex::new(BTreeMap::new())),
        deepseek_pending_messages: Arc::new(Mutex::new(BTreeMap::new())),
        gemini_conversations: Arc::new(Mutex::new(BTreeMap::new())),
        gemini_oauth_pool,
        copilot_oauth_pool,
        model_memory: Arc::new(Mutex::new(RuntimeLocalRewriteModelMemoryState::default())),
        api_key_cursor: Arc::new(AtomicUsize::new(0)),
        client: build_runtime_local_rewrite_http_client()?,
        gateway_auth_token_hash,
        gateway_admin_tokens,
        gateway_sso,
        gateway_state_store,
        gateway_virtual_keys: Arc::new(Mutex::new(gateway_virtual_key_entries)),
        gateway_virtual_key_store_path,
        gateway_virtual_key_usage: Arc::new(Mutex::new(gateway_virtual_key_usage)),
        gateway_virtual_key_usage_path,
        gateway_virtual_key_usage_save_in_flight: Arc::new(AtomicBool::new(false)),
        gateway_virtual_key_usage_save_dirty: Arc::new(AtomicBool::new(false)),
        gateway_virtual_key_usage_pending_deltas: Arc::new(Mutex::new(Vec::new())),
        gateway_virtual_key_request_ids: Arc::new(Mutex::new(BTreeSet::new())),
        gateway_route_aliases,
        gateway_route_load: Arc::new(Mutex::new(BTreeMap::new())),
        gateway_guardrails,
        gateway_guardrail_webhook,
        gateway_call_id_header,
        gateway_observability,
    };
    if let Some(pool) = shared.gemini_oauth_pool.as_ref() {
        pool.spawn_quota_refresh(shared.runtime_shared.log_path.clone());
    }
    let shutdown = Arc::new(AtomicBool::new(false));
    let mut worker_threads = Vec::new();
    let gemini_live_sidecar_addr = if matches!(
        &shared.provider,
        RuntimeLocalRewriteProviderOptions::Gemini { .. }
    ) {
        Some(spawn_runtime_gemini_live_sidecar(
            shared.clone(),
            Arc::clone(&shutdown),
            &mut worker_threads,
        )?)
    } else {
        None
    };
    for _ in 0..worker_count {
        let server: Arc<TinyServer> = Arc::clone(&server);
        let shutdown = Arc::clone(&shutdown);
        let shared = shared.clone();
        worker_threads.push(thread::spawn(move || {
            loop {
                match server.recv() {
                    Ok(request) => {
                        let result = crate::runtime_panic::catch_runtime_unwind_silently(|| {
                            handle_runtime_local_rewrite_proxy_request(request, &shared);
                        });
                        if let Err(panic) = result {
                            runtime_proxy_log(
                                &shared.runtime_shared,
                                format!(
                                    "runtime_proxy_worker_panic lane=local_rewrite panic={}",
                                    crate::runtime_panic::runtime_panic_payload_label(
                                        panic.as_ref()
                                    )
                                ),
                            );
                        }
                    }
                    Err(_) if shutdown.load(Ordering::SeqCst) => break,
                    Err(_) => {}
                }
                if shutdown.load(Ordering::SeqCst) {
                    break;
                }
            }
        }));
    }

    Ok(RuntimeRotationProxy {
        server,
        shutdown,
        worker_threads,
        accept_worker_count: worker_count,
        listen_addr,
        gemini_live_sidecar_addr,
        gemini_live_sidecar_model: gemini_live_sidecar_addr.map(|_| {
            super::local_rewrite_gemini_live::runtime_gemini_live_default_model().to_string()
        }),
        log_path,
        active_request_count: Arc::clone(&runtime_shared.active_request_count),
        owner_lock: None,
    })
}

fn build_runtime_local_rewrite_http_client() -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_millis(
            runtime_proxy_http_connect_timeout_ms(),
        ))
        .no_proxy()
        .build()
        .context("failed to build runtime local rewrite HTTP client")
}

pub(super) fn runtime_local_rewrite_model_selection(
    shared: &RuntimeLocalRewriteProxyShared,
    provider_kind: RuntimeProviderBridgeKind,
    request: &RuntimeProxyRequest,
    body: &[u8],
    default_model: &str,
) -> RuntimeLocalRewriteModelSelection {
    let body_model = runtime_provider_model_from_body(body);
    let original_model = body_model
        .clone()
        .unwrap_or_else(|| default_model.to_string());
    let scope = runtime_local_rewrite_model_scope(provider_kind, request, body);
    let remembered_model = scope
        .as_deref()
        .and_then(|scope| shared.model_memory.lock().ok()?.selected_model(scope));
    let model = remembered_model
        .filter(|_| runtime_local_rewrite_model_allows_session_memory(&original_model))
        .unwrap_or_else(|| original_model.clone());
    if let (Some(scope), Some(explicit_model)) = (scope.as_deref(), body_model.as_deref())
        && !runtime_local_rewrite_model_allows_session_memory(explicit_model)
        && let Ok(mut memory) = shared.model_memory.lock()
    {
        memory.remember_selected_model(scope, explicit_model);
    }
    let body = if model != original_model {
        runtime_provider_request_body_with_model(body, &model)
    } else {
        body.to_vec()
    };
    RuntimeLocalRewriteModelSelection { model, body }
}

pub(super) fn runtime_local_rewrite_model_allows_session_memory(model: &str) -> bool {
    matches!(
        model.trim().to_ascii_lowercase().as_str(),
        "" | "auto" | "default"
    )
}

pub(super) fn runtime_local_rewrite_model_scope(
    provider_kind: RuntimeProviderBridgeKind,
    request: &RuntimeProxyRequest,
    body: &[u8],
) -> Option<String> {
    runtime_proxy_crate::runtime_request_explicit_session_id(request)
        .map(runtime_proxy_crate::RuntimeExplicitSessionId::into_string)
        .or_else(|| runtime_proxy_crate::runtime_request_session_id_from_turn_metadata(request))
        .or_else(|| {
            serde_json::from_slice::<serde_json::Value>(body)
                .ok()
                .and_then(|value| {
                    runtime_proxy_crate::runtime_request_session_id_from_value(&value)
                })
        })
        .map(|session_id| {
            format!(
                "{}:session:{session_id}",
                super::provider_bridge::runtime_provider_label(provider_kind)
            )
        })
}

impl RuntimeLocalRewriteModelMemoryState {
    pub(super) fn remember_selected_model(&mut self, scope: &str, model: &str) {
        self.selected_models
            .insert(scope.to_string(), model.trim().to_string());
    }

    pub(super) fn selected_model(&self, scope: &str) -> Option<String> {
        self.selected_models.get(scope).cloned()
    }
}

fn handle_runtime_local_rewrite_proxy_request(
    request: tiny_http::Request,
    shared: &RuntimeLocalRewriteProxyShared,
) {
    let request_path = request.url().to_string();
    let runtime_shared = &shared.runtime_shared;
    let request_id = runtime_proxy_next_request_id(runtime_shared);
    if runtime_gateway_virtual_key_entries_is_empty(shared)
        && let Some(auth_token_hash) = shared.gateway_auth_token_hash.as_ref()
        && !runtime_gateway_request_path_is_admin(&request_path, shared)
        && !runtime_local_rewrite_request_is_authorized(&request, auth_token_hash)
    {
        runtime_proxy_log(
            runtime_shared,
            runtime_proxy_structured_log_message(
                "gateway_auth_rejected",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "http"),
                    runtime_proxy_log_field("path", request_path.as_str()),
                ],
            ),
        );
        let _ = request.respond(build_runtime_proxy_text_response(
            401,
            "missing or invalid gateway bearer token",
        ));
        return;
    }
    let websocket = is_tiny_http_websocket_upgrade(&request);
    let request_transport = if websocket { "websocket" } else { "http" };
    let _active_request_guard = match acquire_runtime_proxy_active_request_slot_with_wait(
        runtime_shared,
        request_transport,
        &request_path,
    ) {
        Ok(guard) => guard,
        Err(RuntimeProxyAdmissionRejection::GlobalLimit) => {
            mark_runtime_proxy_local_overload(runtime_shared, "active_request_limit");
            reject_runtime_proxy_overloaded_request(
                request,
                runtime_shared,
                "active_request_limit",
            );
            return;
        }
        Err(RuntimeProxyAdmissionRejection::LaneLimit(lane)) => {
            let reason = format!("lane_limit:{}", runtime_route_kind_label(lane));
            reject_runtime_proxy_overloaded_request(request, runtime_shared, &reason);
            return;
        }
    };

    if websocket {
        if matches!(
            &shared.provider,
            RuntimeLocalRewriteProviderOptions::Gemini { .. }
        ) && is_runtime_realtime_websocket_path(&request_path)
        {
            handle_runtime_gemini_live_websocket_request(request_id, request, shared);
            return;
        }
        runtime_proxy_log(
            runtime_shared,
            runtime_proxy_structured_log_message(
                "local_rewrite_websocket_rejected",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "websocket"),
                    runtime_proxy_log_field("path", request_path),
                ],
            ),
        );
        let _ = request.respond(build_runtime_proxy_text_response(
            501,
            "runtime local rewrite proxy does not support websocket upstreams",
        ));
        return;
    }

    let mut request = request;
    let mut captured = match capture_runtime_proxy_request(&mut request) {
        Ok(captured) => captured,
        Err(err) => {
            runtime_proxy_log(
                runtime_shared,
                runtime_proxy_structured_log_message(
                    "local_rewrite_capture_error",
                    [
                        runtime_proxy_log_field("request", request_id.to_string()),
                        runtime_proxy_log_field("transport", "http"),
                        runtime_proxy_log_field("error", format!("{err:#}")),
                    ],
                ),
            );
            let _ = request.respond(build_runtime_proxy_text_response(502, &err.to_string()));
            return;
        }
    };
    if let Err(err) =
        apply_runtime_presidio_redaction_to_request(request_id, &mut captured, runtime_shared)
    {
        runtime_proxy_log(
            runtime_shared,
            runtime_proxy_structured_log_message(
                "local_rewrite_presidio_redaction_failed",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "http"),
                    runtime_proxy_log_field("error", format!("{err:#}")),
                ],
            ),
        );
        let _ = request.respond(build_runtime_proxy_text_response(502, &err.to_string()));
        return;
    }
    if let Some(response) = runtime_gateway_admin_response(request_id, &captured, shared) {
        let _ = request.respond(response);
        return;
    }
    if let Some(rejection) = runtime_gateway_virtual_key_rejection(request_id, &captured, shared) {
        runtime_proxy_log(
            runtime_shared,
            runtime_proxy_structured_log_message(
                "gateway_virtual_key_rejected",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "http"),
                    runtime_proxy_log_field("reason", rejection.code()),
                    runtime_proxy_log_field("path", captured.path_and_query.as_str()),
                ],
            ),
        );
        let _ = request.respond(build_runtime_proxy_json_error_response(
            rejection.status(),
            rejection.code(),
            "gateway virtual key policy rejected this request",
        ));
        return;
    }
    if let Some(block) =
        runtime_gateway_guardrail_webhook_block("pre", request_id, &captured.body, shared)
    {
        runtime_proxy_log(
            runtime_shared,
            runtime_proxy_structured_log_message(
                "gateway_guardrail_webhook_blocked",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "http"),
                    runtime_proxy_log_field("phase", "pre"),
                    runtime_proxy_log_field("reason", block.reason.as_str()),
                    runtime_proxy_log_field("value", block.value.as_str()),
                    runtime_proxy_log_field("path", captured.path_and_query.as_str()),
                ],
            ),
        );
        let _ = request.respond(build_runtime_proxy_json_error_response(
            403,
            "policy_violation",
            "gateway guardrail webhook blocked this request",
        ));
        return;
    }
    if let Some(block) = runtime_proxy_crate::runtime_gateway_guardrail_block(
        &captured.body,
        &shared.gateway_guardrails,
    ) {
        runtime_proxy_log(
            runtime_shared,
            runtime_proxy_structured_log_message(
                "gateway_guardrail_blocked",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "http"),
                    runtime_proxy_log_field("reason", block.kind.as_str()),
                    runtime_proxy_log_field("value", block.value.as_str()),
                    runtime_proxy_log_field("path", captured.path_and_query.as_str()),
                ],
            ),
        );
        let _ = request.respond(build_runtime_proxy_json_error_response(
            403,
            "policy_violation",
            "gateway guardrail blocked this request",
        ));
        return;
    }
    let route_load = shared
        .gateway_route_load
        .lock()
        .map(|load| load.clone())
        .unwrap_or_default();
    let mut route_load_guard = None;
    if let Some(rewrite) = runtime_proxy_crate::runtime_gateway_rewrite_route_alias_with_state(
        &captured.body,
        &shared.gateway_route_aliases,
        request_id,
        &route_load,
    ) {
        route_load_guard = Some(RuntimeGatewayRouteLoadGuard::enter(
            Arc::clone(&shared.gateway_route_load),
            rewrite.model.as_str(),
            &captured.body,
        ));
        runtime_proxy_log(
            runtime_shared,
            runtime_proxy_structured_log_message(
                "gateway_route_alias_rewrite",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "http"),
                    runtime_proxy_log_field("alias", rewrite.alias.as_str()),
                    runtime_proxy_log_field("strategy", rewrite.strategy.as_str()),
                    runtime_proxy_log_field("model", rewrite.model.as_str()),
                    runtime_proxy_log_field("path", captured.path_and_query.as_str()),
                ],
            ),
        );
        captured.body = rewrite.body;
    }
    if path_without_query(&captured.path_and_query).ends_with("/responses/compact") {
        if let RuntimeLocalRewriteProviderOptions::Gemini { auth, .. } = &shared.provider {
            respond_runtime_gemini_compact_request(request_id, request, &captured, shared, auth);
            return;
        }
        let provider_name = match &shared.provider {
            RuntimeLocalRewriteProviderOptions::Anthropic { .. } => "Anthropic",
            RuntimeLocalRewriteProviderOptions::Copilot { .. } => "GitHub Copilot",
            RuntimeLocalRewriteProviderOptions::Gemini { .. } => "Gemini",
            _ => "DeepSeek",
        };
        let _ = request.respond(build_runtime_proxy_text_response(
            501,
            &format!("{provider_name} provider does not support Codex remote compact yet"),
        ));
        return;
    }
    if let Some(parts) = runtime_provider_models_buffered_response(
        shared.provider.bridge_kind(),
        &captured.method,
        &captured.path_and_query,
    ) {
        runtime_proxy_log(
            runtime_shared,
            runtime_provider_request_ledger_message(
                request_id,
                shared.provider.bridge_kind(),
                &captured.path_and_query,
                None,
                parts.status,
                0,
                captured.body.len(),
            ),
        );
        let _ = request.respond(runtime_local_rewrite_response_with_call_id(
            parts, request_id, shared,
        ));
        return;
    }
    let response = match send_runtime_local_rewrite_upstream_request(request_id, &captured, shared)
    {
        Ok(response) => response,
        Err(err) => {
            runtime_proxy_log(
                runtime_shared,
                runtime_proxy_structured_log_message(
                    "local_rewrite_upstream_error",
                    [
                        runtime_proxy_log_field("request", request_id.to_string()),
                        runtime_proxy_log_field("transport", "http"),
                        runtime_proxy_log_field("error", format!("{err:#}")),
                    ],
                ),
            );
            let _ = request.respond(build_runtime_proxy_text_response(502, &err.to_string()));
            return;
        }
    };
    respond_runtime_local_rewrite_proxy_request(request_id, request, response, &captured, shared);
    drop(route_load_guard);
}

fn runtime_gateway_request_path_is_admin(
    request_path: &str,
    shared: &RuntimeLocalRewriteProxyShared,
) -> bool {
    let path = path_without_query(request_path);
    let admin_prefix = format!("{}/prodex/gateway", shared.mount_path.trim_end_matches('/'));
    path == format!("{admin_prefix}/admin")
        || path == format!("{admin_prefix}/openapi.json")
        || path == format!("{admin_prefix}/metrics")
        || path == format!("{admin_prefix}/usage")
        || path == format!("{admin_prefix}/keys")
        || path.starts_with(&format!("{admin_prefix}/keys/"))
        || path == format!("{admin_prefix}/ledger")
        || path == format!("{admin_prefix}/ledger.csv")
        || path == format!("{admin_prefix}/ledger/summary")
        || path == format!("{admin_prefix}/ledger/summary.csv")
        || path == format!("{admin_prefix}/scim/v2/Users")
        || path.starts_with(&format!("{admin_prefix}/scim/v2/Users/"))
}

struct RuntimeGatewayRouteLoadGuard {
    load: RuntimeGatewayRouteLoadState,
    model: String,
    started_at: Instant,
}

impl RuntimeGatewayRouteLoadGuard {
    fn enter(load: RuntimeGatewayRouteLoadState, model: &str, body: &[u8]) -> Self {
        let minute_epoch = runtime_gateway_route_minute_epoch();
        let estimated_tokens = estimate_request_input_tokens(body);
        if let Ok(mut load_map) = load.lock() {
            let entry = load_map.entry(model.to_string()).or_default();
            if entry.minute_epoch != minute_epoch {
                entry.minute_epoch = minute_epoch;
                entry.requests_this_minute = 0;
                entry.tokens_this_minute = 0;
            }
            entry.in_flight = entry.in_flight.saturating_add(1);
            entry.requests_this_minute = entry.requests_this_minute.saturating_add(1);
            entry.tokens_this_minute = entry.tokens_this_minute.saturating_add(estimated_tokens);
        }
        Self {
            load,
            model: model.to_string(),
            started_at: Instant::now(),
        }
    }
}

impl Drop for RuntimeGatewayRouteLoadGuard {
    fn drop(&mut self) {
        if let Ok(mut load_map) = self.load.lock()
            && let Some(state) = load_map.get_mut(&self.model)
        {
            state.in_flight = state.in_flight.saturating_sub(1);
            let elapsed_ms = self
                .started_at
                .elapsed()
                .as_millis()
                .min(u128::from(u64::MAX)) as u64;
            state.latency_ms_ewma = Some(match state.latency_ms_ewma {
                Some(previous) => previous.saturating_mul(7).saturating_add(elapsed_ms) / 8,
                None => elapsed_ms,
            });
            if state.in_flight == 0
                && state.requests_this_minute == 0
                && state.tokens_this_minute == 0
            {
                load_map.remove(&self.model);
            }
        }
    }
}

fn runtime_gateway_route_minute_epoch() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() / 60)
        .unwrap_or_default()
}

fn runtime_gateway_virtual_key_entries_is_empty(shared: &RuntimeLocalRewriteProxyShared) -> bool {
    shared
        .gateway_virtual_keys
        .lock()
        .map(|entries| entries.iter().all(|entry| entry.disabled))
        .unwrap_or(true)
}

fn runtime_gateway_active_virtual_keys(
    shared: &RuntimeLocalRewriteProxyShared,
) -> Vec<runtime_proxy_crate::RuntimeGatewayVirtualKey> {
    shared
        .gateway_virtual_keys
        .lock()
        .map(|entries| {
            entries
                .iter()
                .filter(|entry| !entry.disabled)
                .map(|entry| entry.key.clone())
                .collect()
        })
        .unwrap_or_default()
}

fn runtime_gateway_virtual_key_entries_from_sources(
    policy_keys: Vec<runtime_proxy_crate::RuntimeGatewayVirtualKey>,
    state_store: &RuntimeGatewayStateStore,
    log_path: &Path,
) -> Vec<RuntimeGatewayVirtualKeyEntry> {
    let mut entries = policy_keys
        .into_iter()
        .map(|key| RuntimeGatewayVirtualKeyEntry {
            tenant_id: key.tenant_id.clone(),
            key,
            source: RuntimeGatewayVirtualKeySource::Policy,
            created_at_epoch: None,
            updated_at_epoch: None,
            disabled: false,
        })
        .collect::<Vec<_>>();
    let mut seen = entries
        .iter()
        .map(|entry| entry.key.name.to_ascii_lowercase())
        .collect::<Vec<_>>();
    for record in runtime_gateway_virtual_key_store_load(state_store, log_path).keys {
        let key_name = record.name.trim().to_string();
        if key_name.is_empty() {
            continue;
        }
        let normalized = key_name.to_ascii_lowercase();
        if seen.iter().any(|seen| seen == &normalized) {
            runtime_proxy_log_to_path(
                log_path,
                &runtime_proxy_structured_log_message(
                    "gateway_virtual_key_store_duplicate_ignored",
                    [
                        runtime_proxy_log_field(
                            "path",
                            state_store.key_store_path().display().to_string(),
                        ),
                        runtime_proxy_log_field("key", key_name),
                    ],
                ),
            );
            continue;
        }
        let Some(entry) = runtime_gateway_virtual_key_entry_from_stored(&record) else {
            runtime_proxy_log_to_path(
                log_path,
                &runtime_proxy_structured_log_message(
                    "gateway_virtual_key_store_invalid_hash",
                    [
                        runtime_proxy_log_field(
                            "path",
                            state_store.key_store_path().display().to_string(),
                        ),
                        runtime_proxy_log_field("key", key_name),
                    ],
                ),
            );
            continue;
        };
        seen.push(normalized);
        entries.push(entry);
    }
    entries
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct RuntimeGatewayVirtualKeyStoreFile {
    #[serde(default = "runtime_gateway_virtual_key_store_version")]
    version: u32,
    #[serde(default)]
    keys: Vec<RuntimeGatewayStoredVirtualKey>,
    #[serde(default)]
    scim_users: Vec<RuntimeGatewayScimUser>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RuntimeGatewayScimUser {
    id: String,
    user_name: String,
    #[serde(default)]
    external_id: Option<String>,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default = "runtime_gateway_scim_user_active_default")]
    active: bool,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    tenant_id: Option<String>,
    #[serde(default)]
    allowed_key_prefixes: Vec<String>,
    created_at_epoch: u64,
    updated_at_epoch: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RuntimeGatewayStoredVirtualKey {
    name: String,
    token_hash_base64: String,
    #[serde(default)]
    tenant_id: Option<String>,
    #[serde(default)]
    allowed_models: Vec<String>,
    #[serde(default)]
    budget_microusd: Option<u64>,
    #[serde(default)]
    request_budget: Option<u64>,
    #[serde(default)]
    rpm_limit: Option<u64>,
    #[serde(default)]
    tpm_limit: Option<u64>,
    #[serde(default)]
    disabled: Option<bool>,
    created_at_epoch: u64,
    updated_at_epoch: u64,
}

fn runtime_gateway_virtual_key_entry_from_stored(
    record: &RuntimeGatewayStoredVirtualKey,
) -> Option<RuntimeGatewayVirtualKeyEntry> {
    let token_hash = runtime_proxy_crate::LocalBridgeBearerTokenHash::from_hash_base64(
        &record.token_hash_base64,
    )?;
    Some(RuntimeGatewayVirtualKeyEntry {
        key: runtime_proxy_crate::RuntimeGatewayVirtualKey {
            name: record.name.trim().to_string(),
            tenant_id: record.tenant_id.clone(),
            token_hash,
            allowed_models: record.allowed_models.clone(),
            budget_microusd: record.budget_microusd,
            request_budget: record.request_budget,
            rpm_limit: record.rpm_limit,
            tpm_limit: record.tpm_limit,
        },
        source: RuntimeGatewayVirtualKeySource::Admin,
        tenant_id: record.tenant_id.clone(),
        created_at_epoch: Some(record.created_at_epoch),
        updated_at_epoch: Some(record.updated_at_epoch),
        disabled: record.disabled.unwrap_or(false),
    })
}

fn runtime_gateway_virtual_key_store_version() -> u32 {
    1
}

fn runtime_gateway_scim_user_active_default() -> bool {
    true
}

fn runtime_gateway_virtual_key_store_load(
    state_store: &RuntimeGatewayStateStore,
    log_path: &Path,
) -> RuntimeGatewayVirtualKeyStoreFile {
    let path = state_store.key_store_path();
    match state_store {
        RuntimeGatewayStateStore::Sqlite { path } => {
            return match runtime_gateway_sqlite_load_key_store(path) {
                Ok(mut store) => {
                    store.keys.sort_by(|left, right| left.name.cmp(&right.name));
                    store
                        .scim_users
                        .sort_by(|left, right| left.user_name.cmp(&right.user_name));
                    store
                }
                Err(err) => {
                    runtime_proxy_log_to_path(
                        log_path,
                        &runtime_proxy_structured_log_message(
                            "gateway_virtual_key_store_load_failed",
                            [
                                runtime_proxy_log_field("backend", state_store.label()),
                                runtime_proxy_log_field("path", path.display().to_string()),
                                runtime_proxy_log_field("error", err.to_string()),
                            ],
                        ),
                    );
                    RuntimeGatewayVirtualKeyStoreFile::default()
                }
            };
        }
        RuntimeGatewayStateStore::Postgres {
            url, state_path, ..
        } => {
            return match runtime_gateway_postgres_load_key_store(url) {
                Ok(mut store) => {
                    store.keys.sort_by(|left, right| left.name.cmp(&right.name));
                    store
                        .scim_users
                        .sort_by(|left, right| left.user_name.cmp(&right.user_name));
                    store
                }
                Err(err) => {
                    runtime_proxy_log_to_path(
                        log_path,
                        &runtime_proxy_structured_log_message(
                            "gateway_virtual_key_store_load_failed",
                            [
                                runtime_proxy_log_field("backend", state_store.label()),
                                runtime_proxy_log_field("path", state_path.display().to_string()),
                                runtime_proxy_log_field("error", err.to_string()),
                            ],
                        ),
                    );
                    RuntimeGatewayVirtualKeyStoreFile::default()
                }
            };
        }
        RuntimeGatewayStateStore::Redis { url, state_path } => {
            return match runtime_gateway_redis_load_key_store(url) {
                Ok(mut store) => {
                    store.keys.sort_by(|left, right| left.name.cmp(&right.name));
                    store
                        .scim_users
                        .sort_by(|left, right| left.user_name.cmp(&right.user_name));
                    store
                }
                Err(err) => {
                    runtime_proxy_log_to_path(
                        log_path,
                        &runtime_proxy_structured_log_message(
                            "gateway_virtual_key_store_load_failed",
                            [
                                runtime_proxy_log_field("backend", state_store.label()),
                                runtime_proxy_log_field("path", state_path.display().to_string()),
                                runtime_proxy_log_field("error", err.to_string()),
                            ],
                        ),
                    );
                    RuntimeGatewayVirtualKeyStoreFile::default()
                }
            };
        }
        RuntimeGatewayStateStore::File { .. } => {}
    }
    match std::fs::read(path) {
        Ok(bytes) => match serde_json::from_slice::<RuntimeGatewayVirtualKeyStoreFile>(&bytes) {
            Ok(mut store) => {
                store.keys.sort_by(|left, right| left.name.cmp(&right.name));
                store
                    .scim_users
                    .sort_by(|left, right| left.user_name.cmp(&right.user_name));
                store
            }
            Err(err) => {
                runtime_proxy_log_to_path(
                    log_path,
                    &runtime_proxy_structured_log_message(
                        "gateway_virtual_key_store_load_failed",
                        [
                            runtime_proxy_log_field("path", path.display().to_string()),
                            runtime_proxy_log_field("error", err.to_string()),
                        ],
                    ),
                );
                RuntimeGatewayVirtualKeyStoreFile::default()
            }
        },
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            RuntimeGatewayVirtualKeyStoreFile::default()
        }
        Err(err) => {
            runtime_proxy_log_to_path(
                log_path,
                &runtime_proxy_structured_log_message(
                    "gateway_virtual_key_store_load_failed",
                    [
                        runtime_proxy_log_field("path", path.display().to_string()),
                        runtime_proxy_log_field("error", err.to_string()),
                    ],
                ),
            );
            RuntimeGatewayVirtualKeyStoreFile::default()
        }
    }
}

fn runtime_gateway_virtual_key_store_save(
    path: &Path,
    store: &RuntimeGatewayVirtualKeyStoreFile,
) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let payload = serde_json::to_vec_pretty(store).map_err(std::io::Error::other)?;
    let tmp_path = path.with_extension("json.tmp");
    std::fs::write(&tmp_path, payload)?;
    std::fs::rename(tmp_path, path)?;
    Ok(())
}

fn runtime_gateway_sqlite_open(path: &Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let conn = Connection::open(path)
        .with_context(|| format!("failed to open gateway sqlite state {}", path.display()))?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    let _ = conn.pragma_update(None, "journal_mode", "WAL");
    conn.execute_batch(
        r#"
        PRAGMA foreign_keys = ON;
        CREATE TABLE IF NOT EXISTS prodex_gateway_schema_migrations (
            version INTEGER PRIMARY KEY,
            applied_at_epoch INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS prodex_gateway_virtual_keys (
            name TEXT PRIMARY KEY COLLATE NOCASE,
            tenant_id TEXT,
            token_hash_base64 TEXT NOT NULL,
            allowed_models_json TEXT NOT NULL DEFAULT '[]',
            budget_microusd INTEGER,
            request_budget INTEGER,
            rpm_limit INTEGER,
            tpm_limit INTEGER,
            disabled INTEGER NOT NULL DEFAULT 0,
            created_at_epoch INTEGER NOT NULL,
            updated_at_epoch INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS prodex_gateway_scim_users (
            id TEXT PRIMARY KEY,
            user_name TEXT NOT NULL UNIQUE COLLATE NOCASE,
            tenant_id TEXT,
            external_id TEXT,
            display_name TEXT,
            active INTEGER NOT NULL DEFAULT 1,
            role TEXT,
            allowed_key_prefixes_json TEXT NOT NULL DEFAULT '[]',
            created_at_epoch INTEGER NOT NULL,
            updated_at_epoch INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS prodex_gateway_virtual_key_usage (
            key_name TEXT PRIMARY KEY COLLATE NOCASE,
            minute_epoch INTEGER NOT NULL DEFAULT 0,
            requests_this_minute INTEGER NOT NULL DEFAULT 0,
            tokens_this_minute INTEGER NOT NULL DEFAULT 0,
            requests_total INTEGER NOT NULL DEFAULT 0,
            spend_microusd INTEGER NOT NULL DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS prodex_gateway_billing_ledger (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            phase TEXT NOT NULL,
            request_id INTEGER NOT NULL,
            call_id TEXT NOT NULL,
            key_name TEXT NOT NULL COLLATE NOCASE,
            model TEXT NOT NULL,
            minute_epoch INTEGER NOT NULL,
            input_tokens INTEGER NOT NULL,
            estimated_cost_microusd INTEGER,
            created_at_epoch INTEGER NOT NULL,
            UNIQUE(request_id, key_name, phase)
        );
        INSERT OR IGNORE INTO prodex_gateway_schema_migrations (version, applied_at_epoch)
        VALUES (1, strftime('%s', 'now'));
        "#,
    )?;
    for column in [
        "ALTER TABLE prodex_gateway_billing_ledger ADD COLUMN response_status INTEGER",
        "ALTER TABLE prodex_gateway_billing_ledger ADD COLUMN response_bytes INTEGER",
        "ALTER TABLE prodex_gateway_billing_ledger ADD COLUMN output_tokens INTEGER",
        "ALTER TABLE prodex_gateway_billing_ledger ADD COLUMN final_cost_microusd INTEGER",
        "ALTER TABLE prodex_gateway_billing_ledger ADD COLUMN final_cost_usd REAL",
        "ALTER TABLE prodex_gateway_billing_ledger ADD COLUMN reconciled_at_epoch INTEGER",
        "ALTER TABLE prodex_gateway_virtual_keys ADD COLUMN tenant_id TEXT",
        "ALTER TABLE prodex_gateway_scim_users ADD COLUMN tenant_id TEXT",
    ] {
        match conn.execute(column, []) {
            Ok(_) => {}
            Err(err) if runtime_gateway_sqlite_duplicate_column_error(&err) => {}
            Err(err) => return Err(err.into()),
        }
    }
    Ok(conn)
}

fn runtime_gateway_postgres_open(url: &str) -> Result<PostgresClient> {
    let mut client = PostgresClient::connect(url, NoTls)
        .context("failed to connect to gateway postgres state")?;
    client.batch_execute(
        r#"
        CREATE TABLE IF NOT EXISTS prodex_gateway_schema_migrations (
            version BIGINT PRIMARY KEY,
            applied_at_epoch BIGINT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS prodex_gateway_virtual_keys (
            name TEXT PRIMARY KEY,
            tenant_id TEXT,
            token_hash_base64 TEXT NOT NULL,
            allowed_models_json TEXT NOT NULL DEFAULT '[]',
            budget_microusd BIGINT,
            request_budget BIGINT,
            rpm_limit BIGINT,
            tpm_limit BIGINT,
            disabled BOOLEAN NOT NULL DEFAULT FALSE,
            created_at_epoch BIGINT NOT NULL,
            updated_at_epoch BIGINT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS prodex_gateway_scim_users (
            id TEXT PRIMARY KEY,
            user_name TEXT NOT NULL UNIQUE,
            tenant_id TEXT,
            external_id TEXT,
            display_name TEXT,
            active BOOLEAN NOT NULL DEFAULT TRUE,
            role TEXT,
            allowed_key_prefixes_json TEXT NOT NULL DEFAULT '[]',
            created_at_epoch BIGINT NOT NULL,
            updated_at_epoch BIGINT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS prodex_gateway_virtual_key_usage (
            key_name TEXT PRIMARY KEY,
            minute_epoch BIGINT NOT NULL DEFAULT 0,
            requests_this_minute BIGINT NOT NULL DEFAULT 0,
            tokens_this_minute BIGINT NOT NULL DEFAULT 0,
            requests_total BIGINT NOT NULL DEFAULT 0,
            spend_microusd BIGINT NOT NULL DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS prodex_gateway_billing_ledger (
            id BIGSERIAL PRIMARY KEY,
            phase TEXT NOT NULL,
            request_id BIGINT NOT NULL,
            call_id TEXT NOT NULL,
            key_name TEXT NOT NULL,
            model TEXT NOT NULL,
            minute_epoch BIGINT NOT NULL,
            input_tokens BIGINT NOT NULL,
            estimated_cost_microusd BIGINT,
            created_at_epoch BIGINT NOT NULL,
            response_status BIGINT,
            response_bytes BIGINT,
            output_tokens BIGINT,
            final_cost_microusd BIGINT,
            final_cost_usd DOUBLE PRECISION,
            reconciled_at_epoch BIGINT,
            UNIQUE(request_id, key_name, phase)
        );
        INSERT INTO prodex_gateway_schema_migrations (version, applied_at_epoch)
        VALUES (1, EXTRACT(EPOCH FROM now())::BIGINT)
        ON CONFLICT (version) DO NOTHING;
        ALTER TABLE prodex_gateway_virtual_keys ADD COLUMN IF NOT EXISTS tenant_id TEXT;
        ALTER TABLE prodex_gateway_scim_users ADD COLUMN IF NOT EXISTS tenant_id TEXT;
        "#,
    )?;
    Ok(client)
}

fn runtime_gateway_redis_connection(url: &str) -> Result<redis::Connection> {
    let client = redis::Client::open(url).context("failed to open gateway redis client")?;
    client
        .get_connection()
        .context("failed to connect to gateway redis state")
}

fn runtime_gateway_redis_with_lock<F, T>(url: &str, lock_key: &str, operation: F) -> Result<T>
where
    F: FnOnce(&mut redis::Connection) -> Result<T>,
{
    let token = runtime_gateway_generate_virtual_key_token()?;
    let mut conn = runtime_gateway_redis_connection(url)?;
    let mut acquired = false;
    for _ in 0..50 {
        let result: Option<String> = redis::cmd("SET")
            .arg(lock_key)
            .arg(&token)
            .arg("NX")
            .arg("PX")
            .arg(5_000)
            .query(&mut conn)?;
        if result.is_some() {
            acquired = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    if !acquired {
        bail!("timed out waiting for redis lock {lock_key}");
    }

    let result = operation(&mut conn);
    let release_result: redis::RedisResult<()> = conn.del(lock_key);
    if result.is_ok() {
        release_result.context("failed to release gateway redis lock")?;
    }
    result
}

fn runtime_gateway_sqlite_load_key_store(path: &Path) -> Result<RuntimeGatewayVirtualKeyStoreFile> {
    let conn = runtime_gateway_sqlite_open(path)?;
    runtime_gateway_sqlite_load_key_store_from_conn(&conn)
}

fn runtime_gateway_sqlite_load_key_store_from_conn(
    conn: &Connection,
) -> Result<RuntimeGatewayVirtualKeyStoreFile> {
    let mut stmt = conn.prepare(
        r#"
        SELECT name, tenant_id, token_hash_base64, allowed_models_json, budget_microusd,
               request_budget, rpm_limit, tpm_limit, disabled,
               created_at_epoch, updated_at_epoch
        FROM prodex_gateway_virtual_keys
        ORDER BY name COLLATE NOCASE
        "#,
    )?;
    let rows = stmt.query_map([], |row| {
        let allowed_models_json: String = row.get(3)?;
        let allowed_models =
            serde_json::from_str::<Vec<String>>(&allowed_models_json).unwrap_or_default();
        Ok(RuntimeGatewayStoredVirtualKey {
            name: row.get(0)?,
            tenant_id: row.get(1)?,
            token_hash_base64: row.get(2)?,
            allowed_models,
            budget_microusd: runtime_gateway_sqlite_optional_i64_to_u64(row.get(4)?),
            request_budget: runtime_gateway_sqlite_optional_i64_to_u64(row.get(5)?),
            rpm_limit: runtime_gateway_sqlite_optional_i64_to_u64(row.get(6)?),
            tpm_limit: runtime_gateway_sqlite_optional_i64_to_u64(row.get(7)?),
            disabled: Some(row.get::<_, i64>(8)? != 0),
            created_at_epoch: runtime_gateway_sqlite_i64_to_u64(row.get(9)?),
            updated_at_epoch: runtime_gateway_sqlite_i64_to_u64(row.get(10)?),
        })
    })?;
    let mut keys = Vec::new();
    for row in rows {
        keys.push(row?);
    }
    Ok(RuntimeGatewayVirtualKeyStoreFile {
        version: runtime_gateway_virtual_key_store_version(),
        keys,
        scim_users: runtime_gateway_sqlite_load_scim_users_from_conn(conn)?,
    })
}

fn runtime_gateway_postgres_load_key_store(url: &str) -> Result<RuntimeGatewayVirtualKeyStoreFile> {
    let mut client = runtime_gateway_postgres_open(url)?;
    runtime_gateway_postgres_load_key_store_from_client(&mut client)
}

fn runtime_gateway_postgres_load_key_store_from_client<C: GenericClient>(
    client: &mut C,
) -> Result<RuntimeGatewayVirtualKeyStoreFile> {
    let rows = client.query(
        r#"
        SELECT name, tenant_id, token_hash_base64, allowed_models_json, budget_microusd,
               request_budget, rpm_limit, tpm_limit, disabled,
               created_at_epoch, updated_at_epoch
        FROM prodex_gateway_virtual_keys
        ORDER BY lower(name), name
        "#,
        &[],
    )?;
    let mut keys = Vec::new();
    for row in rows {
        keys.push(runtime_gateway_postgres_stored_key_from_row(&row));
    }
    Ok(RuntimeGatewayVirtualKeyStoreFile {
        version: runtime_gateway_virtual_key_store_version(),
        keys,
        scim_users: runtime_gateway_postgres_load_scim_users_from_client(client)?,
    })
}

fn runtime_gateway_sqlite_load_scim_users_from_conn(
    conn: &Connection,
) -> Result<Vec<RuntimeGatewayScimUser>> {
    let mut stmt = conn.prepare(
        r#"
        SELECT id, user_name, tenant_id, external_id, display_name, active, role,
               allowed_key_prefixes_json, created_at_epoch, updated_at_epoch
        FROM prodex_gateway_scim_users
        ORDER BY user_name COLLATE NOCASE
        "#,
    )?;
    let rows = stmt.query_map([], |row| {
        let prefixes_json: String = row.get(7)?;
        let allowed_key_prefixes =
            serde_json::from_str::<Vec<String>>(&prefixes_json).unwrap_or_default();
        Ok(RuntimeGatewayScimUser {
            id: row.get(0)?,
            user_name: row.get(1)?,
            tenant_id: row.get(2)?,
            external_id: row.get(3)?,
            display_name: row.get(4)?,
            active: row.get::<_, i64>(5)? != 0,
            role: row.get(6)?,
            allowed_key_prefixes,
            created_at_epoch: runtime_gateway_sqlite_i64_to_u64(row.get(8)?),
            updated_at_epoch: runtime_gateway_sqlite_i64_to_u64(row.get(9)?),
        })
    })?;
    let mut users = Vec::new();
    for row in rows {
        users.push(row?);
    }
    Ok(users)
}

fn runtime_gateway_postgres_load_scim_users_from_client<C: GenericClient>(
    client: &mut C,
) -> Result<Vec<RuntimeGatewayScimUser>> {
    let rows = client.query(
        r#"
        SELECT id, user_name, tenant_id, external_id, display_name, active, role,
               allowed_key_prefixes_json, created_at_epoch, updated_at_epoch
        FROM prodex_gateway_scim_users
        ORDER BY lower(user_name), user_name
        "#,
        &[],
    )?;
    let mut users = Vec::new();
    for row in rows {
        let prefixes_json: String = row.get(7);
        users.push(RuntimeGatewayScimUser {
            id: row.get(0),
            user_name: row.get(1),
            tenant_id: row.get(2),
            external_id: row.get(3),
            display_name: row.get(4),
            active: row.get::<_, bool>(5),
            role: row.get(6),
            allowed_key_prefixes: serde_json::from_str::<Vec<String>>(&prefixes_json)
                .unwrap_or_default(),
            created_at_epoch: runtime_gateway_sqlite_i64_to_u64(row.get(8)),
            updated_at_epoch: runtime_gateway_sqlite_i64_to_u64(row.get(9)),
        });
    }
    Ok(users)
}

fn runtime_gateway_postgres_stored_key_from_row(
    row: &postgres::Row,
) -> RuntimeGatewayStoredVirtualKey {
    let allowed_models_json: String = row.get(3);
    let allowed_models =
        serde_json::from_str::<Vec<String>>(&allowed_models_json).unwrap_or_default();
    RuntimeGatewayStoredVirtualKey {
        name: row.get(0),
        tenant_id: row.get(1),
        token_hash_base64: row.get(2),
        allowed_models,
        budget_microusd: runtime_gateway_sqlite_optional_i64_to_u64(row.get(4)),
        request_budget: runtime_gateway_sqlite_optional_i64_to_u64(row.get(5)),
        rpm_limit: runtime_gateway_sqlite_optional_i64_to_u64(row.get(6)),
        tpm_limit: runtime_gateway_sqlite_optional_i64_to_u64(row.get(7)),
        disabled: Some(row.get::<_, bool>(8)),
        created_at_epoch: runtime_gateway_sqlite_i64_to_u64(row.get(9)),
        updated_at_epoch: runtime_gateway_sqlite_i64_to_u64(row.get(10)),
    }
}

fn runtime_gateway_redis_load_key_store(url: &str) -> Result<RuntimeGatewayVirtualKeyStoreFile> {
    let mut conn = runtime_gateway_redis_connection(url)?;
    let payload: Option<String> = conn.get(RUNTIME_GATEWAY_REDIS_KEY_STORE_KEY)?;
    let Some(payload) = payload else {
        return Ok(RuntimeGatewayVirtualKeyStoreFile::default());
    };
    serde_json::from_str::<RuntimeGatewayVirtualKeyStoreFile>(&payload)
        .context("failed to parse gateway redis virtual key store")
}

fn runtime_gateway_redis_save_key_store(
    conn: &mut redis::Connection,
    store: &RuntimeGatewayVirtualKeyStoreFile,
) -> Result<()> {
    let payload = serde_json::to_string(store)?;
    let _: () = conn.set(RUNTIME_GATEWAY_REDIS_KEY_STORE_KEY, payload)?;
    Ok(())
}

fn runtime_gateway_sqlite_save_key_store_in_tx(
    tx: &rusqlite::Transaction<'_>,
    store: &RuntimeGatewayVirtualKeyStoreFile,
) -> Result<()> {
    tx.execute("DELETE FROM prodex_gateway_virtual_keys", [])?;
    for record in &store.keys {
        let allowed_models_json = serde_json::to_string(&record.allowed_models)?;
        tx.execute(
            r#"
            INSERT INTO prodex_gateway_virtual_keys (
                name, tenant_id, token_hash_base64, allowed_models_json, budget_microusd,
                request_budget, rpm_limit, tpm_limit, disabled,
                created_at_epoch, updated_at_epoch
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            "#,
            params![
                record.name,
                record.tenant_id,
                record.token_hash_base64,
                allowed_models_json,
                runtime_gateway_sqlite_optional_u64_to_i64(record.budget_microusd),
                runtime_gateway_sqlite_optional_u64_to_i64(record.request_budget),
                runtime_gateway_sqlite_optional_u64_to_i64(record.rpm_limit),
                runtime_gateway_sqlite_optional_u64_to_i64(record.tpm_limit),
                if record.disabled.unwrap_or(false) {
                    1_i64
                } else {
                    0_i64
                },
                runtime_gateway_sqlite_u64_to_i64(record.created_at_epoch),
                runtime_gateway_sqlite_u64_to_i64(record.updated_at_epoch),
            ],
        )?;
    }
    tx.execute("DELETE FROM prodex_gateway_scim_users", [])?;
    for user in &store.scim_users {
        let allowed_key_prefixes_json = serde_json::to_string(&user.allowed_key_prefixes)?;
        tx.execute(
            r#"
            INSERT INTO prodex_gateway_scim_users (
                id, user_name, tenant_id, external_id, display_name, active, role,
                allowed_key_prefixes_json, created_at_epoch, updated_at_epoch
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            "#,
            params![
                user.id,
                user.user_name,
                user.tenant_id,
                user.external_id,
                user.display_name,
                if user.active { 1_i64 } else { 0_i64 },
                user.role,
                allowed_key_prefixes_json,
                runtime_gateway_sqlite_u64_to_i64(user.created_at_epoch),
                runtime_gateway_sqlite_u64_to_i64(user.updated_at_epoch),
            ],
        )?;
    }
    Ok(())
}

fn runtime_gateway_postgres_save_key_store_in_tx(
    tx: &mut postgres::Transaction<'_>,
    store: &RuntimeGatewayVirtualKeyStoreFile,
) -> Result<()> {
    tx.execute("DELETE FROM prodex_gateway_virtual_keys", &[])?;
    for record in &store.keys {
        let allowed_models_json = serde_json::to_string(&record.allowed_models)?;
        let disabled = record.disabled.unwrap_or(false);
        tx.execute(
            r#"
            INSERT INTO prodex_gateway_virtual_keys (
                name, tenant_id, token_hash_base64, allowed_models_json, budget_microusd,
                request_budget, rpm_limit, tpm_limit, disabled,
                created_at_epoch, updated_at_epoch
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            "#,
            &[
                &record.name,
                &record.tenant_id,
                &record.token_hash_base64,
                &allowed_models_json,
                &runtime_gateway_sqlite_optional_u64_to_i64(record.budget_microusd),
                &runtime_gateway_sqlite_optional_u64_to_i64(record.request_budget),
                &runtime_gateway_sqlite_optional_u64_to_i64(record.rpm_limit),
                &runtime_gateway_sqlite_optional_u64_to_i64(record.tpm_limit),
                &disabled,
                &runtime_gateway_sqlite_u64_to_i64(record.created_at_epoch),
                &runtime_gateway_sqlite_u64_to_i64(record.updated_at_epoch),
            ],
        )?;
    }
    tx.execute("DELETE FROM prodex_gateway_scim_users", &[])?;
    for user in &store.scim_users {
        let allowed_key_prefixes_json = serde_json::to_string(&user.allowed_key_prefixes)?;
        tx.execute(
            r#"
            INSERT INTO prodex_gateway_scim_users (
                id, user_name, tenant_id, external_id, display_name, active, role,
                allowed_key_prefixes_json, created_at_epoch, updated_at_epoch
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
            &[
                &user.id,
                &user.user_name,
                &user.tenant_id,
                &user.external_id,
                &user.display_name,
                &user.active,
                &user.role,
                &allowed_key_prefixes_json,
                &runtime_gateway_sqlite_u64_to_i64(user.created_at_epoch),
                &runtime_gateway_sqlite_u64_to_i64(user.updated_at_epoch),
            ],
        )?;
    }
    Ok(())
}

fn runtime_gateway_sqlite_usage_load(
    path: &Path,
) -> Result<BTreeMap<String, runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage>> {
    let conn = runtime_gateway_sqlite_open(path)?;
    let mut stmt = conn.prepare(
        r#"
        SELECT key_name, minute_epoch, requests_this_minute, tokens_this_minute,
               requests_total, spend_microusd
        FROM prodex_gateway_virtual_key_usage
        ORDER BY key_name COLLATE NOCASE
        "#,
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            runtime_gateway_sqlite_usage_from_row(row)?,
        ))
    })?;
    let mut usage = BTreeMap::new();
    for row in rows {
        let (key_name, key_usage) = row?;
        usage.insert(key_name, key_usage);
    }
    Ok(usage)
}

fn runtime_gateway_sqlite_usage_apply_deltas(
    path: &Path,
    deltas: &[RuntimeGatewayVirtualKeyUsageDelta],
) -> Result<()> {
    let mut conn = runtime_gateway_sqlite_open(path)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    for delta in deltas {
        let mut usage = tx
            .query_row(
                r#"
                SELECT key_name, minute_epoch, requests_this_minute, tokens_this_minute,
                       requests_total, spend_microusd
                FROM prodex_gateway_virtual_key_usage
                WHERE key_name = ?1
                "#,
                params![delta.key_name],
                runtime_gateway_sqlite_usage_from_row,
            )
            .optional()?
            .unwrap_or_default();
        let admission = runtime_proxy_crate::RuntimeGatewayVirtualKeyAdmission {
            key_name: delta.key_name.clone(),
            model: None,
            input_tokens: delta.input_tokens,
            estimated_cost_microusd: delta.estimated_cost_microusd,
        };
        runtime_proxy_crate::runtime_gateway_record_virtual_key_usage(
            &mut usage,
            &admission,
            delta.minute_epoch,
        );
        tx.execute(
            r#"
            INSERT INTO prodex_gateway_virtual_key_usage (
                key_name, minute_epoch, requests_this_minute, tokens_this_minute,
                requests_total, spend_microusd
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(key_name) DO UPDATE SET
                minute_epoch = excluded.minute_epoch,
                requests_this_minute = excluded.requests_this_minute,
                tokens_this_minute = excluded.tokens_this_minute,
                requests_total = excluded.requests_total,
                spend_microusd = excluded.spend_microusd
            "#,
            params![
                delta.key_name,
                runtime_gateway_sqlite_u64_to_i64(usage.minute_epoch),
                runtime_gateway_sqlite_u64_to_i64(usage.requests_this_minute),
                runtime_gateway_sqlite_u64_to_i64(usage.tokens_this_minute),
                runtime_gateway_sqlite_u64_to_i64(usage.requests_total),
                runtime_gateway_sqlite_u64_to_i64(usage.spend_microusd),
            ],
        )?;
        let ledger = runtime_gateway_billing_ledger_entry_from_delta(delta);
        tx.execute(
            r#"
            INSERT OR IGNORE INTO prodex_gateway_billing_ledger (
                phase, request_id, call_id, key_name, model, minute_epoch,
                input_tokens, estimated_cost_microusd, created_at_epoch
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
            params![
                ledger.phase,
                runtime_gateway_sqlite_u64_to_i64(ledger.request),
                ledger.call_id,
                ledger.key_name,
                ledger.model,
                runtime_gateway_sqlite_u64_to_i64(ledger.minute_epoch),
                runtime_gateway_sqlite_u64_to_i64(ledger.input_tokens),
                runtime_gateway_sqlite_optional_u64_to_i64(ledger.estimated_cost_microusd),
                runtime_gateway_sqlite_u64_to_i64(ledger.created_at_epoch),
            ],
        )?;
    }
    tx.commit()?;
    Ok(())
}

fn runtime_gateway_postgres_usage_load(
    url: &str,
) -> Result<BTreeMap<String, runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage>> {
    let mut client = runtime_gateway_postgres_open(url)?;
    let rows = client.query(
        r#"
        SELECT key_name, minute_epoch, requests_this_minute, tokens_this_minute,
               requests_total, spend_microusd
        FROM prodex_gateway_virtual_key_usage
        ORDER BY lower(key_name), key_name
        "#,
        &[],
    )?;
    let mut usage = BTreeMap::new();
    for row in rows {
        usage.insert(row.get(0), runtime_gateway_postgres_usage_from_row(&row));
    }
    Ok(usage)
}

fn runtime_gateway_postgres_usage_apply_deltas(
    url: &str,
    deltas: &[RuntimeGatewayVirtualKeyUsageDelta],
) -> Result<()> {
    let mut client = runtime_gateway_postgres_open(url)?;
    let mut tx = client.transaction()?;
    for delta in deltas {
        let mut usage = tx
            .query_opt(
                r#"
                SELECT key_name, minute_epoch, requests_this_minute, tokens_this_minute,
                       requests_total, spend_microusd
                FROM prodex_gateway_virtual_key_usage
                WHERE lower(key_name) = lower($1)
                FOR UPDATE
                "#,
                &[&delta.key_name],
            )?
            .map(|row| runtime_gateway_postgres_usage_from_row(&row))
            .unwrap_or_default();
        let admission = runtime_proxy_crate::RuntimeGatewayVirtualKeyAdmission {
            key_name: delta.key_name.clone(),
            model: None,
            input_tokens: delta.input_tokens,
            estimated_cost_microusd: delta.estimated_cost_microusd,
        };
        runtime_proxy_crate::runtime_gateway_record_virtual_key_usage(
            &mut usage,
            &admission,
            delta.minute_epoch,
        );
        tx.execute(
            r#"
            INSERT INTO prodex_gateway_virtual_key_usage (
                key_name, minute_epoch, requests_this_minute, tokens_this_minute,
                requests_total, spend_microusd
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT(key_name) DO UPDATE SET
                minute_epoch = EXCLUDED.minute_epoch,
                requests_this_minute = EXCLUDED.requests_this_minute,
                tokens_this_minute = EXCLUDED.tokens_this_minute,
                requests_total = EXCLUDED.requests_total,
                spend_microusd = EXCLUDED.spend_microusd
            "#,
            &[
                &delta.key_name,
                &runtime_gateway_sqlite_u64_to_i64(usage.minute_epoch),
                &runtime_gateway_sqlite_u64_to_i64(usage.requests_this_minute),
                &runtime_gateway_sqlite_u64_to_i64(usage.tokens_this_minute),
                &runtime_gateway_sqlite_u64_to_i64(usage.requests_total),
                &runtime_gateway_sqlite_u64_to_i64(usage.spend_microusd),
            ],
        )?;
        let ledger = runtime_gateway_billing_ledger_entry_from_delta(delta);
        tx.execute(
            r#"
            INSERT INTO prodex_gateway_billing_ledger (
                phase, request_id, call_id, key_name, model, minute_epoch,
                input_tokens, estimated_cost_microusd, created_at_epoch
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT(request_id, key_name, phase) DO NOTHING
            "#,
            &[
                &ledger.phase,
                &runtime_gateway_sqlite_u64_to_i64(ledger.request),
                &ledger.call_id,
                &ledger.key_name,
                &ledger.model,
                &runtime_gateway_sqlite_u64_to_i64(ledger.minute_epoch),
                &runtime_gateway_sqlite_u64_to_i64(ledger.input_tokens),
                &runtime_gateway_sqlite_optional_u64_to_i64(ledger.estimated_cost_microusd),
                &runtime_gateway_sqlite_u64_to_i64(ledger.created_at_epoch),
            ],
        )?;
    }
    tx.commit()?;
    Ok(())
}

fn runtime_gateway_redis_usage_load(
    url: &str,
) -> Result<BTreeMap<String, runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage>> {
    let mut conn = runtime_gateway_redis_connection(url)?;
    let payload: Option<String> = conn.get(RUNTIME_GATEWAY_REDIS_USAGE_KEY)?;
    let Some(payload) = payload else {
        return Ok(BTreeMap::new());
    };
    serde_json::from_str::<BTreeMap<String, runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage>>(
        &payload,
    )
    .context("failed to parse gateway redis virtual key usage")
}

fn runtime_gateway_redis_usage_apply_deltas(
    url: &str,
    deltas: &[RuntimeGatewayVirtualKeyUsageDelta],
) -> Result<()> {
    runtime_gateway_redis_with_lock(url, RUNTIME_GATEWAY_REDIS_USAGE_LOCK, |conn| {
        let payload: Option<String> = conn.get(RUNTIME_GATEWAY_REDIS_USAGE_KEY)?;
        let mut usage = match payload {
            Some(payload) => serde_json::from_str::<
                BTreeMap<String, runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage>,
            >(&payload)
            .context("failed to parse gateway redis virtual key usage")?,
            None => BTreeMap::new(),
        };
        for delta in deltas {
            let entry = usage.entry(delta.key_name.clone()).or_default();
            let admission = runtime_proxy_crate::RuntimeGatewayVirtualKeyAdmission {
                key_name: delta.key_name.clone(),
                model: None,
                input_tokens: delta.input_tokens,
                estimated_cost_microusd: delta.estimated_cost_microusd,
            };
            runtime_proxy_crate::runtime_gateway_record_virtual_key_usage(
                entry,
                &admission,
                delta.minute_epoch,
            );
        }
        let payload = serde_json::to_string(&usage)?;
        let _: () = conn.set(RUNTIME_GATEWAY_REDIS_USAGE_KEY, payload)?;
        Ok(())
    })?;
    runtime_gateway_redis_append_ledger_deltas(url, deltas)?;
    Ok(())
}

fn runtime_gateway_sqlite_usage_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage> {
    Ok(runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage {
        minute_epoch: runtime_gateway_sqlite_i64_to_u64(row.get(1)?),
        requests_this_minute: runtime_gateway_sqlite_i64_to_u64(row.get(2)?),
        tokens_this_minute: runtime_gateway_sqlite_i64_to_u64(row.get(3)?),
        requests_total: runtime_gateway_sqlite_i64_to_u64(row.get(4)?),
        spend_microusd: runtime_gateway_sqlite_i64_to_u64(row.get(5)?),
    })
}

fn runtime_gateway_postgres_usage_from_row(
    row: &postgres::Row,
) -> runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage {
    runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage {
        minute_epoch: runtime_gateway_sqlite_i64_to_u64(row.get(1)),
        requests_this_minute: runtime_gateway_sqlite_i64_to_u64(row.get(2)),
        tokens_this_minute: runtime_gateway_sqlite_i64_to_u64(row.get(3)),
        requests_total: runtime_gateway_sqlite_i64_to_u64(row.get(4)),
        spend_microusd: runtime_gateway_sqlite_i64_to_u64(row.get(5)),
    }
}

fn runtime_gateway_billing_ledger_entry_from_delta(
    delta: &RuntimeGatewayVirtualKeyUsageDelta,
) -> RuntimeGatewayBillingLedgerEntry {
    RuntimeGatewayBillingLedgerEntry {
        object: "gateway.billing_ledger_entry".to_string(),
        phase: "request".to_string(),
        request: delta.request_id,
        call_id: format!("prodex-{}", delta.request_id),
        key_name: delta.key_name.clone(),
        model: delta.model.clone(),
        minute_epoch: delta.minute_epoch,
        input_tokens: delta.input_tokens,
        estimated_cost_microusd: delta.estimated_cost_microusd,
        estimated_cost_usd: delta.estimated_cost_microusd.map(microusd_to_usd),
        created_at_epoch: delta.created_at_epoch,
        response_status: None,
        response_bytes: None,
        output_tokens: None,
        final_cost_microusd: None,
        final_cost_usd: None,
        reconciled_at_epoch: None,
    }
}

fn runtime_gateway_file_ledger_append_deltas(
    path: &Path,
    deltas: &[RuntimeGatewayVirtualKeyUsageDelta],
) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let lock_path = path.with_extension("jsonl.lock");
    let lock_file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(lock_path)?;
    lock_file.lock_exclusive()?;
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    for delta in deltas {
        serde_json::to_writer(
            &mut file,
            &runtime_gateway_billing_ledger_entry_from_delta(delta),
        )
        .map_err(std::io::Error::other)?;
        use std::io::Write;
        file.write_all(b"\n")?;
    }
    let _ = lock_file.unlock();
    Ok(())
}

fn runtime_gateway_billing_ledger_load(
    state_store: &RuntimeGatewayStateStore,
    limit: usize,
) -> std::io::Result<Vec<RuntimeGatewayBillingLedgerEntry>> {
    match state_store {
        RuntimeGatewayStateStore::File { ledger_path, .. } => {
            runtime_gateway_file_ledger_load(ledger_path, limit)
        }
        RuntimeGatewayStateStore::Sqlite { path } => {
            runtime_gateway_sqlite_ledger_load(path, limit).map_err(std::io::Error::other)
        }
        RuntimeGatewayStateStore::Postgres { url, .. } => {
            runtime_gateway_postgres_ledger_load(url, limit).map_err(std::io::Error::other)
        }
        RuntimeGatewayStateStore::Redis { url, .. } => {
            runtime_gateway_redis_ledger_load(url, limit).map_err(std::io::Error::other)
        }
    }
}

pub(super) fn schedule_runtime_gateway_billing_ledger_reconcile(
    shared: &RuntimeLocalRewriteProxyShared,
    event: RuntimeProviderGatewaySpendEvent,
) {
    if event.phase != "response" {
        return;
    }
    let should_reconcile = shared
        .gateway_virtual_key_request_ids
        .lock()
        .map(|request_ids| request_ids.contains(&event.request))
        .unwrap_or(false);
    if !should_reconcile {
        return;
    }
    let state_store = shared.gateway_state_store.clone();
    let runtime_shared = shared.runtime_shared.clone();
    let request_ids = Arc::clone(&shared.gateway_virtual_key_request_ids);
    shared.runtime_shared.async_runtime.spawn_blocking(move || {
        let mut last_error = None;
        for attempt in 0..25 {
            match runtime_gateway_billing_ledger_reconcile_response(&state_store, &event) {
                Ok(true) => {
                    if let Ok(mut request_ids) = request_ids.lock() {
                        request_ids.remove(&event.request);
                    }
                    return;
                }
                Ok(false) => {
                    if attempt < 24 {
                        std::thread::sleep(std::time::Duration::from_millis(20));
                    }
                }
                Err(err) => {
                    last_error = Some(err);
                    break;
                }
            }
        }
        if let Ok(mut request_ids) = request_ids.lock() {
            request_ids.remove(&event.request);
        }
        if let Some(err) = last_error {
            crate::runtime_proxy_log(
                &runtime_shared,
                runtime_proxy_structured_log_message(
                    "gateway_billing_ledger_reconcile_failed",
                    [
                        runtime_proxy_log_field("request", event.request.to_string()),
                        runtime_proxy_log_field("backend", state_store.label()),
                        runtime_proxy_log_field("error", err.to_string()),
                    ],
                ),
            );
        }
    });
}

fn runtime_gateway_billing_ledger_reconcile_response(
    state_store: &RuntimeGatewayStateStore,
    event: &RuntimeProviderGatewaySpendEvent,
) -> std::io::Result<bool> {
    match state_store {
        RuntimeGatewayStateStore::File { ledger_path, .. } => {
            runtime_gateway_file_ledger_reconcile_response(ledger_path, event)
        }
        RuntimeGatewayStateStore::Sqlite { path } => {
            runtime_gateway_sqlite_ledger_reconcile_response(path, event)
                .map_err(std::io::Error::other)
        }
        RuntimeGatewayStateStore::Postgres { url, .. } => {
            runtime_gateway_postgres_ledger_reconcile_response(url, event)
                .map_err(std::io::Error::other)
        }
        RuntimeGatewayStateStore::Redis { url, .. } => {
            runtime_gateway_redis_ledger_reconcile_response(url, event)
                .map_err(std::io::Error::other)
        }
    }
}

fn runtime_gateway_file_ledger_reconcile_response(
    path: &Path,
    event: &RuntimeProviderGatewaySpendEvent,
) -> std::io::Result<bool> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let lock_path = path.with_extension("jsonl.lock");
    let lock_file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(lock_path)?;
    lock_file.lock_exclusive()?;
    let mut entries = runtime_gateway_file_ledger_load(path, usize::MAX)?;
    let reconciled_at_epoch = runtime_gateway_unix_epoch_seconds();
    let mut changed = false;
    for entry in &mut entries {
        if entry.request != event.request || entry.phase != "request" {
            continue;
        }
        runtime_gateway_apply_response_to_ledger_entry(entry, event, reconciled_at_epoch);
        changed = true;
    }
    if changed {
        let tmp_path = path.with_extension("jsonl.tmp");
        {
            let mut file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&tmp_path)?;
            for entry in &entries {
                serde_json::to_writer(&mut file, entry).map_err(std::io::Error::other)?;
                use std::io::Write;
                file.write_all(b"\n")?;
            }
            file.sync_all()?;
        }
        std::fs::rename(tmp_path, path)?;
    }
    let _ = lock_file.unlock();
    Ok(changed)
}

fn runtime_gateway_file_ledger_load(
    path: &Path,
    limit: usize,
) -> std::io::Result<Vec<RuntimeGatewayBillingLedgerEntry>> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err),
    };
    let mut entries = Vec::new();
    for line in String::from_utf8_lossy(&bytes).lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(entry) = serde_json::from_str::<RuntimeGatewayBillingLedgerEntry>(trimmed) {
            entries.push(entry);
        }
    }
    if entries.len() > limit {
        Ok(entries.split_off(entries.len() - limit))
    } else {
        Ok(entries)
    }
}

fn runtime_gateway_redis_append_ledger_deltas(
    url: &str,
    deltas: &[RuntimeGatewayVirtualKeyUsageDelta],
) -> Result<()> {
    runtime_gateway_redis_with_lock(url, RUNTIME_GATEWAY_REDIS_LEDGER_LOCK, |conn| {
        let mut entries = runtime_gateway_redis_ledger_load_from_connection(conn, usize::MAX)?;
        for delta in deltas {
            let next = runtime_gateway_billing_ledger_entry_from_delta(delta);
            let exists = entries.iter().any(|entry| {
                entry.request == next.request
                    && entry.key_name.eq_ignore_ascii_case(&next.key_name)
                    && entry.phase == next.phase
            });
            if !exists {
                entries.push(next);
            }
        }
        runtime_gateway_redis_replace_ledger_entries(conn, &entries)?;
        Ok(())
    })
}

fn runtime_gateway_redis_ledger_load(
    url: &str,
    limit: usize,
) -> Result<Vec<RuntimeGatewayBillingLedgerEntry>> {
    let mut conn = runtime_gateway_redis_connection(url)?;
    runtime_gateway_redis_ledger_load_from_connection(&mut conn, limit)
}

fn runtime_gateway_redis_ledger_load_from_connection(
    conn: &mut redis::Connection,
    limit: usize,
) -> Result<Vec<RuntimeGatewayBillingLedgerEntry>> {
    let start = if limit == usize::MAX {
        0
    } else {
        -(i64::try_from(limit).unwrap_or(i64::MAX))
    };
    let payloads: Vec<String> = redis::cmd("LRANGE")
        .arg(RUNTIME_GATEWAY_REDIS_LEDGER_KEY)
        .arg(start)
        .arg(-1)
        .query(conn)?;
    let mut entries = Vec::new();
    for payload in payloads {
        if let Ok(entry) = serde_json::from_str::<RuntimeGatewayBillingLedgerEntry>(&payload) {
            entries.push(entry);
        }
    }
    Ok(entries)
}

fn runtime_gateway_redis_replace_ledger_entries(
    conn: &mut redis::Connection,
    entries: &[RuntimeGatewayBillingLedgerEntry],
) -> Result<()> {
    let _: () = conn.del(RUNTIME_GATEWAY_REDIS_LEDGER_KEY)?;
    for entry in entries {
        let payload = serde_json::to_string(entry)?;
        let _: () = conn.rpush(RUNTIME_GATEWAY_REDIS_LEDGER_KEY, payload)?;
    }
    Ok(())
}

fn runtime_gateway_sqlite_ledger_load(
    path: &Path,
    limit: usize,
) -> Result<Vec<RuntimeGatewayBillingLedgerEntry>> {
    let conn = runtime_gateway_sqlite_open(path)?;
    let mut stmt = conn.prepare(
        r#"
        SELECT phase, request_id, call_id, key_name, model, minute_epoch,
               input_tokens, estimated_cost_microusd, created_at_epoch,
               response_status, response_bytes, output_tokens, final_cost_microusd,
               final_cost_usd, reconciled_at_epoch
        FROM prodex_gateway_billing_ledger
        ORDER BY id DESC
        LIMIT ?1
        "#,
    )?;
    let rows = stmt.query_map(
        params![runtime_gateway_sqlite_u64_to_i64(limit as u64)],
        |row| {
            let estimated_cost_microusd = runtime_gateway_sqlite_optional_i64_to_u64(row.get(7)?);
            Ok(RuntimeGatewayBillingLedgerEntry {
                object: "gateway.billing_ledger_entry".to_string(),
                phase: row.get(0)?,
                request: runtime_gateway_sqlite_i64_to_u64(row.get(1)?),
                call_id: row.get(2)?,
                key_name: row.get(3)?,
                model: row.get(4)?,
                minute_epoch: runtime_gateway_sqlite_i64_to_u64(row.get(5)?),
                input_tokens: runtime_gateway_sqlite_i64_to_u64(row.get(6)?),
                estimated_cost_microusd,
                estimated_cost_usd: estimated_cost_microusd.map(microusd_to_usd),
                created_at_epoch: runtime_gateway_sqlite_i64_to_u64(row.get(8)?),
                response_status: runtime_gateway_sqlite_optional_i64_to_u64(row.get(9)?)
                    .and_then(|value| u16::try_from(value).ok()),
                response_bytes: runtime_gateway_sqlite_optional_i64_to_u64(row.get(10)?),
                output_tokens: runtime_gateway_sqlite_optional_i64_to_u64(row.get(11)?),
                final_cost_microusd: runtime_gateway_sqlite_optional_i64_to_u64(row.get(12)?),
                final_cost_usd: row.get(13)?,
                reconciled_at_epoch: runtime_gateway_sqlite_optional_i64_to_u64(row.get(14)?),
            })
        },
    )?;
    let mut entries = Vec::new();
    for row in rows {
        entries.push(row?);
    }
    entries.reverse();
    Ok(entries)
}

fn runtime_gateway_postgres_ledger_load(
    url: &str,
    limit: usize,
) -> Result<Vec<RuntimeGatewayBillingLedgerEntry>> {
    let mut client = runtime_gateway_postgres_open(url)?;
    let rows = client.query(
        r#"
        SELECT phase, request_id, call_id, key_name, model, minute_epoch,
               input_tokens, estimated_cost_microusd, created_at_epoch,
               response_status, response_bytes, output_tokens, final_cost_microusd,
               final_cost_usd, reconciled_at_epoch
        FROM prodex_gateway_billing_ledger
        ORDER BY id DESC
        LIMIT $1
        "#,
        &[&runtime_gateway_sqlite_u64_to_i64(limit as u64)],
    )?;
    let mut entries = Vec::new();
    for row in rows {
        let estimated_cost_microusd = runtime_gateway_sqlite_optional_i64_to_u64(row.get(7));
        entries.push(RuntimeGatewayBillingLedgerEntry {
            object: "gateway.billing_ledger_entry".to_string(),
            phase: row.get(0),
            request: runtime_gateway_sqlite_i64_to_u64(row.get(1)),
            call_id: row.get(2),
            key_name: row.get(3),
            model: row.get(4),
            minute_epoch: runtime_gateway_sqlite_i64_to_u64(row.get(5)),
            input_tokens: runtime_gateway_sqlite_i64_to_u64(row.get(6)),
            estimated_cost_microusd,
            estimated_cost_usd: estimated_cost_microusd.map(microusd_to_usd),
            created_at_epoch: runtime_gateway_sqlite_i64_to_u64(row.get(8)),
            response_status: runtime_gateway_sqlite_optional_i64_to_u64(row.get(9))
                .and_then(|value| u16::try_from(value).ok()),
            response_bytes: runtime_gateway_sqlite_optional_i64_to_u64(row.get(10)),
            output_tokens: runtime_gateway_sqlite_optional_i64_to_u64(row.get(11)),
            final_cost_microusd: runtime_gateway_sqlite_optional_i64_to_u64(row.get(12)),
            final_cost_usd: row.get(13),
            reconciled_at_epoch: runtime_gateway_sqlite_optional_i64_to_u64(row.get(14)),
        });
    }
    entries.reverse();
    Ok(entries)
}

fn runtime_gateway_sqlite_ledger_reconcile_response(
    path: &Path,
    event: &RuntimeProviderGatewaySpendEvent,
) -> Result<bool> {
    let conn = runtime_gateway_sqlite_open(path)?;
    let changed = conn.execute(
        r#"
        UPDATE prodex_gateway_billing_ledger
        SET response_status = ?1,
            response_bytes = ?2,
            output_tokens = ?3,
            final_cost_microusd = ?4,
            final_cost_usd = ?5,
            reconciled_at_epoch = ?6
        WHERE request_id = ?7
          AND phase = 'request'
        "#,
        params![
            i64::from(event.status),
            runtime_gateway_sqlite_optional_u64_to_i64(
                event.response_bytes.map(|value| value as u64)
            ),
            runtime_gateway_sqlite_optional_u64_to_i64(event.output_tokens),
            runtime_gateway_sqlite_optional_u64_to_i64(runtime_gateway_usd_to_microusd(
                event.cost_usd
            )),
            event.cost_usd,
            runtime_gateway_sqlite_u64_to_i64(runtime_gateway_unix_epoch_seconds()),
            runtime_gateway_sqlite_u64_to_i64(event.request),
        ],
    )?;
    Ok(changed > 0)
}

fn runtime_gateway_postgres_ledger_reconcile_response(
    url: &str,
    event: &RuntimeProviderGatewaySpendEvent,
) -> Result<bool> {
    let mut client = runtime_gateway_postgres_open(url)?;
    let changed = client.execute(
        r#"
        UPDATE prodex_gateway_billing_ledger
        SET response_status = $1,
            response_bytes = $2,
            output_tokens = $3,
            final_cost_microusd = $4,
            final_cost_usd = $5,
            reconciled_at_epoch = $6
        WHERE request_id = $7
          AND phase = 'request'
        "#,
        &[
            &i64::from(event.status),
            &runtime_gateway_sqlite_optional_u64_to_i64(
                event.response_bytes.map(|value| value as u64),
            ),
            &runtime_gateway_sqlite_optional_u64_to_i64(event.output_tokens),
            &runtime_gateway_sqlite_optional_u64_to_i64(runtime_gateway_usd_to_microusd(
                event.cost_usd,
            )),
            &event.cost_usd,
            &runtime_gateway_sqlite_u64_to_i64(runtime_gateway_unix_epoch_seconds()),
            &runtime_gateway_sqlite_u64_to_i64(event.request),
        ],
    )?;
    Ok(changed > 0)
}

fn runtime_gateway_redis_ledger_reconcile_response(
    url: &str,
    event: &RuntimeProviderGatewaySpendEvent,
) -> Result<bool> {
    runtime_gateway_redis_with_lock(url, RUNTIME_GATEWAY_REDIS_LEDGER_LOCK, |conn| {
        let mut entries = runtime_gateway_redis_ledger_load_from_connection(conn, usize::MAX)?;
        let reconciled_at_epoch = runtime_gateway_unix_epoch_seconds();
        let mut changed = false;
        for entry in &mut entries {
            if entry.request != event.request || entry.phase != "request" {
                continue;
            }
            runtime_gateway_apply_response_to_ledger_entry(entry, event, reconciled_at_epoch);
            changed = true;
        }
        if changed {
            runtime_gateway_redis_replace_ledger_entries(conn, &entries)?;
        }
        Ok(changed)
    })
}

fn runtime_gateway_apply_response_to_ledger_entry(
    entry: &mut RuntimeGatewayBillingLedgerEntry,
    event: &RuntimeProviderGatewaySpendEvent,
    reconciled_at_epoch: u64,
) {
    entry.response_status = Some(event.status);
    entry.response_bytes = event.response_bytes.map(|value| value as u64);
    entry.output_tokens = event.output_tokens;
    entry.final_cost_usd = event.cost_usd;
    entry.final_cost_microusd = runtime_gateway_usd_to_microusd(event.cost_usd);
    entry.reconciled_at_epoch = Some(reconciled_at_epoch);
}

fn runtime_gateway_usd_to_microusd(value: Option<f64>) -> Option<u64> {
    let value = value?;
    if !value.is_finite() || value < 0.0 {
        return None;
    }
    u64::try_from((value * 1_000_000.0).round() as i128).ok()
}

fn runtime_gateway_sqlite_duplicate_column_error(err: &rusqlite::Error) -> bool {
    err.to_string()
        .to_ascii_lowercase()
        .contains("duplicate column")
}

fn runtime_gateway_sqlite_optional_i64_to_u64(value: Option<i64>) -> Option<u64> {
    value.map(runtime_gateway_sqlite_i64_to_u64)
}

fn runtime_gateway_sqlite_i64_to_u64(value: i64) -> u64 {
    u64::try_from(value).unwrap_or_default()
}

fn runtime_gateway_sqlite_optional_u64_to_i64(value: Option<u64>) -> Option<i64> {
    value.map(runtime_gateway_sqlite_u64_to_i64)
}

fn runtime_gateway_sqlite_u64_to_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn runtime_gateway_virtual_key_rejection(
    request_id: u64,
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Option<runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection> {
    if path_without_query(&captured.path_and_query) == runtime_proxy_crate::LOCAL_BRIDGE_HEALTH_PATH
    {
        return None;
    }
    if let Some(auth_token_hash) = shared.gateway_auth_token_hash.as_ref()
        && captured.headers.iter().any(|(name, value)| {
            name.eq_ignore_ascii_case("authorization")
                && auth_token_hash.verify_authorization_header(value)
        })
    {
        return None;
    }
    let active_keys = runtime_gateway_active_virtual_keys(shared);
    let key = match runtime_proxy_crate::runtime_gateway_virtual_key_from_headers(
        &captured.headers,
        &active_keys,
    ) {
        Ok(Some(key)) => key,
        Ok(None) => return None,
        Err(rejection) => return Some(rejection),
    };
    let model = runtime_proxy_crate::runtime_gateway_request_model(&captured.body)
        .unwrap_or_else(|| "unknown".to_string());
    let input_tokens = estimate_request_input_tokens(&captured.body);
    let route_load = shared
        .gateway_route_load
        .lock()
        .map(|load| load.clone())
        .unwrap_or_default();
    let cost = runtime_provider_gateway_cost_for_request(
        shared.provider.bridge_kind(),
        &shared.gateway_route_aliases,
        &route_load,
        request_id,
        &captured.body,
        &model,
    );
    let estimated_cost_microusd = calculate_cost_microusd(Some(input_tokens), None, cost);
    let minute_epoch = runtime_proxy_crate::runtime_gateway_minute_epoch();
    let usage = shared
        .gateway_virtual_key_usage
        .lock()
        .ok()
        .and_then(|usage| usage.get(&key.name).cloned());
    let admission = match runtime_proxy_crate::runtime_gateway_virtual_key_admission(
        key,
        usage.as_ref(),
        &captured.body,
        estimated_cost_microusd,
        minute_epoch,
    ) {
        Ok(admission) => admission,
        Err(rejection) => return Some(rejection),
    };
    if let Ok(mut usage) = shared.gateway_virtual_key_usage.lock() {
        let entry = usage.entry(admission.key_name.clone()).or_default();
        runtime_proxy_crate::runtime_gateway_record_virtual_key_usage(
            entry,
            &admission,
            minute_epoch,
        );
    }
    if let Ok(mut request_ids) = shared.gateway_virtual_key_request_ids.lock() {
        request_ids.insert(request_id);
    }
    schedule_runtime_gateway_virtual_key_usage_save(
        shared,
        RuntimeGatewayVirtualKeyUsageDelta {
            request_id,
            key_name: admission.key_name.clone(),
            model: model.clone(),
            minute_epoch,
            input_tokens: admission.input_tokens,
            estimated_cost_microusd: admission.estimated_cost_microusd,
            created_at_epoch: runtime_gateway_unix_epoch_seconds(),
        },
    );
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "gateway_virtual_key_admitted",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("key", admission.key_name.as_str()),
                runtime_proxy_log_field("model", model.as_str()),
                runtime_proxy_log_field("input_tokens", admission.input_tokens.to_string()),
                runtime_proxy_log_field(
                    "estimated_cost_microusd",
                    admission
                        .estimated_cost_microusd
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "unknown".to_string()),
                ),
            ],
        ),
    );
    None
}

fn runtime_gateway_virtual_key_usage_load(
    state_store: &RuntimeGatewayStateStore,
    log_path: &Path,
) -> BTreeMap<String, runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage> {
    let path = state_store.usage_path();
    match state_store {
        RuntimeGatewayStateStore::Sqlite { path } => {
            return match runtime_gateway_sqlite_usage_load(path) {
                Ok(usage) => usage,
                Err(err) => {
                    runtime_proxy_log_to_path(
                        log_path,
                        &runtime_proxy_structured_log_message(
                            "gateway_virtual_key_usage_load_failed",
                            [
                                runtime_proxy_log_field("backend", state_store.label()),
                                runtime_proxy_log_field("path", path.display().to_string()),
                                runtime_proxy_log_field("error", err.to_string()),
                            ],
                        ),
                    );
                    BTreeMap::new()
                }
            };
        }
        RuntimeGatewayStateStore::Postgres {
            url, state_path, ..
        } => {
            return match runtime_gateway_postgres_usage_load(url) {
                Ok(usage) => usage,
                Err(err) => {
                    runtime_proxy_log_to_path(
                        log_path,
                        &runtime_proxy_structured_log_message(
                            "gateway_virtual_key_usage_load_failed",
                            [
                                runtime_proxy_log_field("backend", state_store.label()),
                                runtime_proxy_log_field("path", state_path.display().to_string()),
                                runtime_proxy_log_field("error", err.to_string()),
                            ],
                        ),
                    );
                    BTreeMap::new()
                }
            };
        }
        RuntimeGatewayStateStore::Redis { url, state_path } => {
            return match runtime_gateway_redis_usage_load(url) {
                Ok(usage) => usage,
                Err(err) => {
                    runtime_proxy_log_to_path(
                        log_path,
                        &runtime_proxy_structured_log_message(
                            "gateway_virtual_key_usage_load_failed",
                            [
                                runtime_proxy_log_field("backend", state_store.label()),
                                runtime_proxy_log_field("path", state_path.display().to_string()),
                                runtime_proxy_log_field("error", err.to_string()),
                            ],
                        ),
                    );
                    BTreeMap::new()
                }
            };
        }
        RuntimeGatewayStateStore::File { .. } => {}
    }
    match std::fs::read(path) {
        Ok(bytes) => {
            match serde_json::from_slice::<
                BTreeMap<String, runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage>,
            >(&bytes)
            {
                Ok(usage) => usage,
                Err(err) => {
                    runtime_proxy_log_to_path(
                        log_path,
                        &runtime_proxy_structured_log_message(
                            "gateway_virtual_key_usage_load_failed",
                            [
                                runtime_proxy_log_field("path", path.display().to_string()),
                                runtime_proxy_log_field("error", err.to_string()),
                            ],
                        ),
                    );
                    BTreeMap::new()
                }
            }
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
        Err(err) => {
            runtime_proxy_log_to_path(
                log_path,
                &runtime_proxy_structured_log_message(
                    "gateway_virtual_key_usage_load_failed",
                    [
                        runtime_proxy_log_field("path", path.display().to_string()),
                        runtime_proxy_log_field("error", err.to_string()),
                    ],
                ),
            );
            BTreeMap::new()
        }
    }
}

fn schedule_runtime_gateway_virtual_key_usage_save(
    shared: &RuntimeLocalRewriteProxyShared,
    delta: RuntimeGatewayVirtualKeyUsageDelta,
) {
    let state_store = shared.gateway_state_store.clone();
    if let Ok(mut pending) = shared.gateway_virtual_key_usage_pending_deltas.lock() {
        pending.push(delta);
    } else {
        return;
    }
    shared
        .gateway_virtual_key_usage_save_dirty
        .store(true, Ordering::Release);
    if shared
        .gateway_virtual_key_usage_save_in_flight
        .swap(true, Ordering::AcqRel)
    {
        return;
    }

    let pending_deltas = Arc::clone(&shared.gateway_virtual_key_usage_pending_deltas);
    let dirty = Arc::clone(&shared.gateway_virtual_key_usage_save_dirty);
    let in_flight = Arc::clone(&shared.gateway_virtual_key_usage_save_in_flight);
    let log_path = shared.runtime_shared.log_path.clone();
    drop(shared.runtime_shared.async_runtime.spawn_blocking(move || {
        loop {
            dirty.store(false, Ordering::Release);
            let deltas = pending_deltas
                .lock()
                .map(|mut pending| pending.drain(..).collect::<Vec<_>>())
                .unwrap_or_default();
            if !deltas.is_empty()
                && let Err(err) =
                    runtime_gateway_virtual_key_usage_apply_deltas(&state_store, &deltas)
            {
                runtime_proxy_log_to_path(
                    &log_path,
                    &runtime_proxy_structured_log_message(
                        "gateway_virtual_key_usage_save_failed",
                        [
                            runtime_proxy_log_field("backend", state_store.label()),
                            runtime_proxy_log_field(
                                "path",
                                state_store.usage_path().display().to_string(),
                            ),
                            runtime_proxy_log_field("error", err.to_string()),
                        ],
                    ),
                );
            }
            if !dirty.load(Ordering::Acquire) {
                in_flight.store(false, Ordering::Release);
                if dirty.load(Ordering::Acquire) && !in_flight.swap(true, Ordering::AcqRel) {
                    continue;
                }
                break;
            }
        }
    }));
}

pub(super) fn runtime_gateway_virtual_key_usage_apply_deltas(
    state_store: &RuntimeGatewayStateStore,
    deltas: &[RuntimeGatewayVirtualKeyUsageDelta],
) -> std::io::Result<()> {
    let path = state_store.usage_path();
    match state_store {
        RuntimeGatewayStateStore::Sqlite { path } => {
            return runtime_gateway_sqlite_usage_apply_deltas(path, deltas)
                .map_err(std::io::Error::other);
        }
        RuntimeGatewayStateStore::Postgres { url, .. } => {
            return runtime_gateway_postgres_usage_apply_deltas(url, deltas)
                .map_err(std::io::Error::other);
        }
        RuntimeGatewayStateStore::Redis { url, .. } => {
            return runtime_gateway_redis_usage_apply_deltas(url, deltas)
                .map_err(std::io::Error::other);
        }
        RuntimeGatewayStateStore::File { .. } => {}
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let lock_path = path.with_extension("json.lock");
    let lock_file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(lock_path)?;
    lock_file.lock_exclusive()?;
    let mut usage = runtime_gateway_virtual_key_usage_load_strict(path)?;
    for delta in deltas {
        let entry = usage.entry(delta.key_name.clone()).or_default();
        let admission = runtime_proxy_crate::RuntimeGatewayVirtualKeyAdmission {
            key_name: delta.key_name.clone(),
            model: None,
            input_tokens: delta.input_tokens,
            estimated_cost_microusd: delta.estimated_cost_microusd,
        };
        runtime_proxy_crate::runtime_gateway_record_virtual_key_usage(
            entry,
            &admission,
            delta.minute_epoch,
        );
    }
    let payload = serde_json::to_vec_pretty(&usage).map_err(std::io::Error::other)?;
    let tmp_path = path.with_extension("json.tmp");
    std::fs::write(&tmp_path, payload)?;
    std::fs::rename(tmp_path, path)?;
    runtime_gateway_file_ledger_append_deltas(state_store.ledger_path(), deltas)?;
    let _ = lock_file.unlock();
    Ok(())
}

fn runtime_gateway_virtual_key_usage_load_strict(
    path: &Path,
) -> std::io::Result<BTreeMap<String, runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage>> {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice::<
            BTreeMap<String, runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage>,
        >(&bytes)
        .map_err(std::io::Error::other),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(err) => Err(err),
    }
}

fn runtime_gateway_admin_response(
    request_id: u64,
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Option<tiny_http::ResponseBox> {
    let path = path_without_query(&captured.path_and_query);
    let admin_prefix = format!("{}/prodex/gateway", shared.mount_path.trim_end_matches('/'));
    let keys_path = format!("{admin_prefix}/keys");
    let usage_path = format!("{admin_prefix}/usage");
    let ledger_path = format!("{admin_prefix}/ledger");
    let ledger_csv_path = format!("{ledger_path}.csv");
    let ledger_summary_path = format!("{ledger_path}/summary");
    let ledger_summary_csv_path = format!("{ledger_summary_path}.csv");
    let metrics_path = format!("{admin_prefix}/metrics");
    let openapi_path = format!("{admin_prefix}/openapi.json");
    let admin_path = format!("{admin_prefix}/admin");
    let scim_users_path = format!("{admin_prefix}/scim/v2/Users");
    let key_name = path
        .strip_prefix(&(keys_path.clone() + "/"))
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let scim_user_id = path
        .strip_prefix(&(scim_users_path.clone() + "/"))
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if path == admin_path {
        if !captured.method.eq_ignore_ascii_case("GET") {
            return Some(build_runtime_proxy_json_error_response(
                405,
                "method_not_allowed",
                "gateway admin dashboard endpoint requires GET",
            ));
        }
        return Some(runtime_gateway_admin_dashboard_response(shared));
    }
    if path != usage_path
        && path != ledger_path
        && path != ledger_csv_path
        && path != ledger_summary_path
        && path != ledger_summary_csv_path
        && path != metrics_path
        && path != keys_path
        && path != openapi_path
        && path != scim_users_path
        && key_name.is_none()
        && scim_user_id.is_none()
    {
        return None;
    }
    let Some(admin_auth) = runtime_gateway_admin_auth(captured, shared) else {
        if shared.gateway_auth_token_hash.is_none()
            && shared.gateway_admin_tokens.is_empty()
            && shared.gateway_sso.proxy_token_hash.is_none()
            && shared.gateway_sso.oidc.is_none()
        {
            return Some(build_runtime_proxy_json_error_response(
                403,
                "admin_auth_not_configured",
                "configure a gateway admin bearer token to use gateway admin endpoints",
            ));
        }
        runtime_proxy_log(
            &shared.runtime_shared,
            runtime_proxy_structured_log_message(
                "gateway_admin_auth_rejected",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("path", path),
                ],
            ),
        );
        return Some(build_runtime_proxy_json_error_response(
            401,
            "invalid_admin_token",
            "missing or invalid gateway admin bearer token",
        ));
    };
    let admin_method = captured.method.to_ascii_uppercase();
    let admin_write = (path == keys_path && admin_method == "POST")
        || (key_name.is_some() && matches!(admin_method.as_str(), "PATCH" | "DELETE"))
        || (path == scim_users_path && admin_method == "POST")
        || (scim_user_id.is_some() && matches!(admin_method.as_str(), "PATCH" | "PUT" | "DELETE"));
    if admin_write && !admin_auth.role.can_write() {
        runtime_proxy_log(
            &shared.runtime_shared,
            runtime_proxy_structured_log_message(
                "gateway_admin_role_rejected",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("path", path),
                    runtime_proxy_log_field("admin", admin_auth.name.as_str()),
                    runtime_proxy_log_field("role", admin_auth.role.as_str()),
                ],
            ),
        );
        return Some(build_runtime_proxy_json_error_response(
            403,
            "gateway_admin_role_forbidden",
            "gateway admin role does not allow this mutation",
        ));
    }

    if path == openapi_path {
        if !captured.method.eq_ignore_ascii_case("GET") {
            return Some(build_runtime_proxy_json_error_response(
                405,
                "method_not_allowed",
                "gateway OpenAPI endpoint requires GET",
            ));
        }
        return Some(runtime_gateway_admin_json_response(
            200,
            runtime_gateway_openapi_spec(shared),
        ));
    }

    if path == usage_path {
        if !captured.method.eq_ignore_ascii_case("GET") {
            return Some(build_runtime_proxy_json_error_response(
                405,
                "method_not_allowed",
                "gateway usage endpoint requires GET",
            ));
        }
        return Some(runtime_gateway_admin_json_response(
            200,
            runtime_gateway_admin_keys_payload(shared, "gateway.usage", Some(&admin_auth)),
        ));
    }

    if path == ledger_path {
        if !captured.method.eq_ignore_ascii_case("GET") {
            return Some(build_runtime_proxy_json_error_response(
                405,
                "method_not_allowed",
                "gateway ledger endpoint requires GET",
            ));
        }
        return Some(runtime_gateway_admin_ledger_response(shared, &admin_auth));
    }

    if path == ledger_csv_path {
        if !captured.method.eq_ignore_ascii_case("GET") {
            return Some(build_runtime_proxy_json_error_response(
                405,
                "method_not_allowed",
                "gateway ledger CSV endpoint requires GET",
            ));
        }
        return Some(runtime_gateway_admin_ledger_csv_response(
            shared,
            &admin_auth,
        ));
    }

    if path == ledger_summary_path {
        if !captured.method.eq_ignore_ascii_case("GET") {
            return Some(build_runtime_proxy_json_error_response(
                405,
                "method_not_allowed",
                "gateway ledger summary endpoint requires GET",
            ));
        }
        return Some(runtime_gateway_admin_ledger_summary_response(
            shared,
            &admin_auth,
        ));
    }

    if path == ledger_summary_csv_path {
        if !captured.method.eq_ignore_ascii_case("GET") {
            return Some(build_runtime_proxy_json_error_response(
                405,
                "method_not_allowed",
                "gateway ledger summary CSV endpoint requires GET",
            ));
        }
        return Some(runtime_gateway_admin_ledger_summary_csv_response(
            shared,
            &admin_auth,
        ));
    }

    if path == metrics_path {
        if !captured.method.eq_ignore_ascii_case("GET") {
            return Some(build_runtime_proxy_json_error_response(
                405,
                "method_not_allowed",
                "gateway metrics endpoint requires GET",
            ));
        }
        return Some(runtime_gateway_prometheus_response(shared, &admin_auth));
    }

    if path == keys_path {
        return match admin_method.as_str() {
            "GET" => Some(runtime_gateway_admin_json_response(
                200,
                runtime_gateway_admin_keys_payload(shared, "gateway.keys", Some(&admin_auth)),
            )),
            "POST" => Some(runtime_gateway_admin_create_key_response(
                captured,
                shared,
                &admin_auth,
            )),
            _ => Some(build_runtime_proxy_json_error_response(
                405,
                "method_not_allowed",
                "gateway keys endpoint supports GET and POST",
            )),
        };
    }

    if path == scim_users_path {
        return match admin_method.as_str() {
            "GET" => Some(runtime_gateway_admin_scim_list_users_response(
                shared,
                &admin_auth,
            )),
            "POST" => Some(runtime_gateway_admin_scim_create_user_response(
                captured,
                shared,
                &admin_auth,
            )),
            _ => Some(build_runtime_proxy_json_error_response(
                405,
                "method_not_allowed",
                "gateway SCIM Users endpoint supports GET and POST",
            )),
        };
    }

    if let Some(scim_user_id) = scim_user_id {
        return Some(match admin_method.as_str() {
            "GET" => {
                runtime_gateway_admin_scim_get_user_response(scim_user_id, shared, &admin_auth)
            }
            "PATCH" | "PUT" => runtime_gateway_admin_scim_update_user_response(
                scim_user_id,
                captured,
                shared,
                &admin_auth,
            ),
            "DELETE" => {
                runtime_gateway_admin_scim_delete_user_response(scim_user_id, shared, &admin_auth)
            }
            _ => build_runtime_proxy_json_error_response(
                405,
                "method_not_allowed",
                "gateway SCIM User endpoint supports GET, PATCH, PUT, and DELETE",
            ),
        });
    }

    let key_name = key_name.unwrap_or_default();
    Some(match admin_method.as_str() {
        "GET" => runtime_gateway_admin_get_key_response(key_name, shared, &admin_auth),
        "PATCH" => {
            runtime_gateway_admin_update_key_response(key_name, captured, shared, &admin_auth)
        }
        "DELETE" => runtime_gateway_admin_delete_key_response(key_name, shared, &admin_auth),
        _ => build_runtime_proxy_json_error_response(
            405,
            "method_not_allowed",
            "gateway key endpoint supports GET, PATCH, and DELETE",
        ),
    })
}

struct RuntimeGatewayAdminAuth {
    name: String,
    role: RuntimeGatewayAdminRole,
    tenant_id: Option<String>,
    allowed_key_prefixes: Vec<String>,
}

fn runtime_gateway_admin_auth(
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Option<RuntimeGatewayAdminAuth> {
    if let Some(auth) = runtime_gateway_oidc_admin_auth(captured, shared) {
        return Some(auth);
    }
    if let Some(auth) = runtime_gateway_sso_admin_auth(captured, shared) {
        return Some(auth);
    }
    let authorization = captured
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("authorization"))
        .map(|(_, value)| value.as_str())?;
    for token in &shared.gateway_admin_tokens {
        if token.token_hash.verify_authorization_header(authorization) {
            return Some(RuntimeGatewayAdminAuth {
                name: token.name.clone(),
                role: token.role,
                tenant_id: token.tenant_id.clone(),
                allowed_key_prefixes: token.allowed_key_prefixes.clone(),
            });
        }
    }
    if let Some(auth_token_hash) = shared.gateway_auth_token_hash.as_ref()
        && auth_token_hash.verify_authorization_header(authorization)
    {
        return Some(RuntimeGatewayAdminAuth {
            name: "default-admin".to_string(),
            role: RuntimeGatewayAdminRole::Admin,
            tenant_id: None,
            allowed_key_prefixes: Vec::new(),
        });
    }
    None
}

fn runtime_gateway_oidc_admin_auth(
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Option<RuntimeGatewayAdminAuth> {
    let config = shared.gateway_sso.oidc.as_ref()?;
    let authorization = runtime_gateway_header(captured, "authorization")?;
    let token = runtime_gateway_authorization_bearer_token(authorization)?;
    let claims = runtime_gateway_verify_oidc_token(token, config, shared).ok()?;
    let name = runtime_gateway_oidc_claim_string(&claims, &config.user_claim)
        .or_else(|| runtime_gateway_oidc_claim_string(&claims, "email"))
        .or_else(|| runtime_gateway_oidc_claim_string(&claims, "preferred_username"))
        .or_else(|| runtime_gateway_oidc_claim_string(&claims, "sub"))?;
    let scim_user = runtime_gateway_scim_user_by_name(shared, &name);
    if scim_user.as_ref().is_some_and(|user| !user.active) {
        return None;
    }
    let role = runtime_gateway_oidc_claim_string(&claims, &config.role_claim)
        .and_then(|role| RuntimeGatewayAdminRole::parse(&role))
        .or_else(|| {
            scim_user
                .as_ref()
                .and_then(|user| user.role.as_deref())
                .and_then(RuntimeGatewayAdminRole::parse)
        })
        .unwrap_or(shared.gateway_sso.default_role);
    let tenant_id = runtime_gateway_oidc_claim_string(&claims, &config.tenant_claim)
        .or_else(|| scim_user.as_ref().and_then(|user| user.tenant_id.clone()));
    let allowed_key_prefixes =
        runtime_gateway_oidc_claim_string_vec(&claims, &config.key_prefixes_claim)
            .or_else(|| {
                scim_user
                    .as_ref()
                    .map(|user| user.allowed_key_prefixes.clone())
            })
            .unwrap_or_default();
    Some(RuntimeGatewayAdminAuth {
        name: format!("oidc:{name}"),
        role,
        tenant_id,
        allowed_key_prefixes,
    })
}

fn runtime_gateway_verify_oidc_token(
    token: &str,
    config: &RuntimeGatewayOidcConfig,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Result<BTreeMap<String, serde_json::Value>> {
    let header = decode_header(token).context("failed to decode gateway OIDC JWT header")?;
    let alg = runtime_gateway_oidc_algorithm(header.alg)?;
    let jwks_url = runtime_gateway_oidc_jwks_url(config, shared)?;
    let jwks = shared
        .client
        .get(&jwks_url)
        .send()
        .context("failed to fetch gateway OIDC JWKS")?
        .error_for_status()
        .context("gateway OIDC JWKS endpoint returned an error")?
        .json::<JwkSet>()
        .context("failed to parse gateway OIDC JWKS")?;
    let key = jwks
        .keys
        .iter()
        .find(|key| {
            header
                .kid
                .as_deref()
                .map(|kid| key.common.key_id.as_deref() == Some(kid))
                .unwrap_or(true)
        })
        .ok_or_else(|| anyhow::anyhow!("gateway OIDC JWKS did not contain the JWT key"))?;
    let decoding_key =
        DecodingKey::from_jwk(key).context("failed to build gateway OIDC decoding key")?;
    let mut validation = Validation::new(alg);
    validation.set_audience(&[config.audience.as_str()]);
    validation.set_issuer(&[config.issuer.as_str()]);
    validation.algorithms = vec![alg];
    let token = decode::<BTreeMap<String, serde_json::Value>>(token, &decoding_key, &validation)
        .context("failed to verify gateway OIDC JWT")?;
    Ok(token.claims)
}

fn runtime_gateway_oidc_jwks_url(
    config: &RuntimeGatewayOidcConfig,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Result<String> {
    if let Some(jwks_url) = config.jwks_url.as_deref() {
        return Ok(jwks_url.to_string());
    }
    let discovery_url = format!(
        "{}/.well-known/openid-configuration",
        config.issuer.trim_end_matches('/')
    );
    let discovery = shared
        .client
        .get(&discovery_url)
        .send()
        .context("failed to fetch gateway OIDC discovery document")?
        .error_for_status()
        .context("gateway OIDC discovery endpoint returned an error")?
        .json::<serde_json::Value>()
        .context("failed to parse gateway OIDC discovery document")?;
    discovery
        .get("jwks_uri")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("gateway OIDC discovery document is missing jwks_uri"))
}

fn runtime_gateway_oidc_algorithm(alg: Algorithm) -> Result<Algorithm> {
    match alg {
        Algorithm::RS256
        | Algorithm::RS384
        | Algorithm::RS512
        | Algorithm::ES256
        | Algorithm::ES384 => Ok(alg),
        _ => bail!("gateway OIDC JWT algorithm is not allowed"),
    }
}

fn runtime_gateway_authorization_bearer_token(value: &str) -> Option<&str> {
    let mut parts = value.split_whitespace();
    let scheme = parts.next()?;
    if !scheme.eq_ignore_ascii_case("bearer") {
        return None;
    }
    let token = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    Some(token)
}

fn runtime_gateway_oidc_claim_string(
    claims: &BTreeMap<String, serde_json::Value>,
    field: &str,
) -> Option<String> {
    claims
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn runtime_gateway_oidc_claim_string_vec(
    claims: &BTreeMap<String, serde_json::Value>,
    field: &str,
) -> Option<Vec<String>> {
    let value = claims.get(field)?;
    if let Some(value) = value.as_str() {
        return Some(runtime_gateway_parse_sso_prefixes(value));
    }
    value.as_array().map(|values| {
        values
            .iter()
            .filter_map(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .collect()
    })
}

fn runtime_gateway_sso_admin_auth(
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Option<RuntimeGatewayAdminAuth> {
    let config = &shared.gateway_sso;
    let proxy_token_hash = config.proxy_token_hash.as_ref()?;
    let proxy_token = runtime_gateway_header(captured, &config.token_header)?;
    if !proxy_token_hash.verify_bearer_token(proxy_token.trim()) {
        return None;
    }
    let name = runtime_gateway_header(captured, &config.user_header)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("sso-admin");
    let scim_user = runtime_gateway_scim_user_by_name(shared, name);
    if scim_user.as_ref().is_some_and(|user| !user.active) {
        return None;
    }
    let role = runtime_gateway_header(captured, &config.role_header)
        .and_then(RuntimeGatewayAdminRole::parse)
        .or_else(|| {
            scim_user
                .as_ref()
                .and_then(|user| user.role.as_deref())
                .and_then(RuntimeGatewayAdminRole::parse)
        })
        .unwrap_or(config.default_role);
    let tenant_id = runtime_gateway_header(captured, &config.tenant_header)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| scim_user.as_ref().and_then(|user| user.tenant_id.clone()));
    let allowed_key_prefixes = runtime_gateway_header(captured, &config.key_prefixes_header)
        .map(runtime_gateway_parse_sso_prefixes)
        .or_else(|| {
            scim_user
                .as_ref()
                .map(|user| user.allowed_key_prefixes.clone())
        })
        .unwrap_or_default();
    Some(RuntimeGatewayAdminAuth {
        name: format!("sso:{name}"),
        role,
        tenant_id,
        allowed_key_prefixes,
    })
}

fn runtime_gateway_header<'a>(
    captured: &'a RuntimeProxyRequest,
    header_name: &str,
) -> Option<&'a str> {
    captured
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(header_name))
        .map(|(_, value)| value.as_str())
}

fn runtime_gateway_parse_sso_prefixes(value: &str) -> Vec<String> {
    value
        .split([',', ';', '\n'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

fn runtime_gateway_scim_user_by_name(
    shared: &RuntimeLocalRewriteProxyShared,
    name: &str,
) -> Option<RuntimeGatewayScimUser> {
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    runtime_gateway_virtual_key_store_load(
        &shared.gateway_state_store,
        &shared.runtime_shared.log_path,
    )
    .scim_users
    .into_iter()
    .find(|user| user.user_name.eq_ignore_ascii_case(name))
}

impl RuntimeGatewayAdminAuth {
    fn can_access_tenant(&self, tenant_id: Option<&str>) -> bool {
        self.tenant_id
            .as_deref()
            .map(|tenant| tenant_id == Some(tenant))
            .unwrap_or(true)
    }

    fn can_access_key(&self, key_name: &str) -> bool {
        self.allowed_key_prefixes.is_empty()
            || self
                .allowed_key_prefixes
                .iter()
                .any(|prefix| key_name.starts_with(prefix))
    }

    fn can_access_entry(&self, entry: &RuntimeGatewayVirtualKeyEntry) -> bool {
        self.can_access_tenant(entry.tenant_id.as_deref()) && self.can_access_key(&entry.key.name)
    }

    fn can_access_scim_user(&self, user: &RuntimeGatewayScimUser) -> bool {
        self.can_access_tenant(user.tenant_id.as_deref())
    }
}

fn runtime_gateway_openapi_spec(shared: &RuntimeLocalRewriteProxyShared) -> serde_json::Value {
    let mount_path = shared.mount_path.trim_end_matches('/');
    let responses_path = format!("{mount_path}/responses");
    let keys_path = format!("{mount_path}/prodex/gateway/keys");
    let key_path = format!("{keys_path}/{{name}}");
    let scim_users_path = format!("{mount_path}/prodex/gateway/scim/v2/Users");
    let scim_user_path = format!("{scim_users_path}/{{id}}");
    let usage_path = format!("{mount_path}/prodex/gateway/usage");
    let ledger_path = format!("{mount_path}/prodex/gateway/ledger");
    let ledger_csv_path = format!("{ledger_path}.csv");
    let ledger_summary_path = format!("{ledger_path}/summary");
    let ledger_summary_csv_path = format!("{ledger_summary_path}.csv");
    let metrics_path = format!("{mount_path}/prodex/gateway/metrics");
    let openapi_path = format!("{mount_path}/prodex/gateway/openapi.json");
    let admin_path = format!("{mount_path}/prodex/gateway/admin");

    let mut paths = serde_json::Map::new();
    paths.insert(
        responses_path,
        serde_json::json!({
            "post": {
                "operationId": "createGatewayResponse",
                "summary": "Create an OpenAI-compatible response through the Prodex gateway",
                "security": [{"GatewayBearerAuth": []}],
                "requestBody": {
                    "required": true,
                    "content": {
                        "application/json": {
                            "schema": {"type": "object", "additionalProperties": true}
                        }
                    }
                },
                "responses": {
                    "200": {
                        "description": "OpenAI-compatible response payload or stream"
                    },
                    "401": {"$ref": "#/components/responses/GatewayError"},
                    "429": {"$ref": "#/components/responses/GatewayError"}
                }
            }
        }),
    );
    paths.insert(
        keys_path,
        serde_json::json!({
            "get": {
                "operationId": "listGatewayVirtualKeys",
                "summary": "List gateway virtual keys and usage counters",
                "security": [{"GatewayBearerAuth": []}],
                "responses": {
                    "200": {
                        "description": "Gateway virtual key list",
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/GatewayKeyList"}
                            }
                        }
                    },
                    "401": {"$ref": "#/components/responses/GatewayError"}
                }
            },
            "post": {
                "operationId": "createGatewayVirtualKey",
                "summary": "Create an admin-managed gateway virtual key",
                "security": [{"GatewayBearerAuth": []}],
                "requestBody": {
                    "required": true,
                    "content": {
                        "application/json": {
                            "schema": {"$ref": "#/components/schemas/GatewayKeyCreateRequest"}
                        }
                    }
                },
                "responses": {
                    "201": {
                        "description": "Created virtual key. token is returned once when generated by Prodex.",
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/GatewayKeyMutationResponse"}
                            }
                        }
                    },
                    "400": {"$ref": "#/components/responses/GatewayError"},
                    "401": {"$ref": "#/components/responses/GatewayError"},
                    "403": {"$ref": "#/components/responses/GatewayError"},
                    "409": {"$ref": "#/components/responses/GatewayError"}
                }
            }
        }),
    );
    paths.insert(
        key_path,
        serde_json::json!({
            "parameters": [
                {
                    "name": "name",
                    "in": "path",
                    "required": true,
                    "schema": {"type": "string"}
                }
            ],
            "get": {
                "operationId": "getGatewayVirtualKey",
                "summary": "Get one gateway virtual key",
                "security": [{"GatewayBearerAuth": []}],
                "responses": {
                    "200": {
                        "description": "Gateway virtual key",
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/GatewayKeyResponse"}
                            }
                        }
                    },
                    "401": {"$ref": "#/components/responses/GatewayError"},
                    "404": {"$ref": "#/components/responses/GatewayError"}
                }
            },
            "patch": {
                "operationId": "updateGatewayVirtualKey",
                "summary": "Update, disable, or rotate an admin-managed gateway virtual key",
                "security": [{"GatewayBearerAuth": []}],
                "requestBody": {
                    "required": true,
                    "content": {
                        "application/json": {
                            "schema": {"$ref": "#/components/schemas/GatewayKeyPatchRequest"}
                        }
                    }
                },
                "responses": {
                    "200": {
                        "description": "Updated virtual key. token is returned once when rotated.",
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/GatewayKeyMutationResponse"}
                            }
                        }
                    },
                    "400": {"$ref": "#/components/responses/GatewayError"},
                    "401": {"$ref": "#/components/responses/GatewayError"},
                    "403": {"$ref": "#/components/responses/GatewayError"},
                    "404": {"$ref": "#/components/responses/GatewayError"}
                }
            },
            "delete": {
                "operationId": "deleteGatewayVirtualKey",
                "summary": "Delete an admin-managed gateway virtual key",
                "security": [{"GatewayBearerAuth": []}],
                "responses": {
                    "200": {
                        "description": "Deleted virtual key marker",
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/GatewayKeyDeleted"}
                            }
                        }
                    },
                    "401": {"$ref": "#/components/responses/GatewayError"},
                    "403": {"$ref": "#/components/responses/GatewayError"},
                    "404": {"$ref": "#/components/responses/GatewayError"}
                }
            }
        }),
    );
    paths.insert(
        scim_users_path,
        serde_json::json!({
            "get": {
                "operationId": "listGatewayScimUsers",
                "summary": "List gateway SCIM-provisioned admin users",
                "security": [{"GatewayBearerAuth": []}],
                "responses": {
                    "200": {
                        "description": "SCIM ListResponse of users",
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/GatewayScimUserList"}
                            }
                        }
                    },
                    "401": {"$ref": "#/components/responses/GatewayError"}
                }
            },
            "post": {
                "operationId": "createGatewayScimUser",
                "summary": "Provision a gateway admin user for trusted-proxy SSO",
                "security": [{"GatewayBearerAuth": []}],
                "requestBody": {
                    "required": true,
                    "content": {
                        "application/json": {
                            "schema": {"$ref": "#/components/schemas/GatewayScimUserWrite"}
                        }
                    }
                },
                "responses": {
                    "201": {
                        "description": "Created SCIM user",
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/GatewayScimUser"}
                            }
                        }
                    },
                    "400": {"$ref": "#/components/responses/GatewayError"},
                    "401": {"$ref": "#/components/responses/GatewayError"},
                    "403": {"$ref": "#/components/responses/GatewayError"},
                    "409": {"$ref": "#/components/responses/GatewayError"}
                }
            }
        }),
    );
    paths.insert(
        scim_user_path,
        serde_json::json!({
            "parameters": [
                {
                    "name": "id",
                    "in": "path",
                    "required": true,
                    "schema": {"type": "string"}
                }
            ],
            "get": {
                "operationId": "getGatewayScimUser",
                "summary": "Get one gateway SCIM user",
                "security": [{"GatewayBearerAuth": []}],
                "responses": {
                    "200": {
                        "description": "SCIM user",
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/GatewayScimUser"}
                            }
                        }
                    },
                    "401": {"$ref": "#/components/responses/GatewayError"},
                    "404": {"$ref": "#/components/responses/GatewayError"}
                }
            },
            "patch": {
                "operationId": "patchGatewayScimUser",
                "summary": "Patch one gateway SCIM user",
                "security": [{"GatewayBearerAuth": []}],
                "requestBody": {
                    "required": true,
                    "content": {
                        "application/json": {
                            "schema": {"type": "object", "additionalProperties": true}
                        }
                    }
                },
                "responses": {
                    "200": {
                        "description": "Updated SCIM user",
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/GatewayScimUser"}
                            }
                        }
                    },
                    "400": {"$ref": "#/components/responses/GatewayError"},
                    "401": {"$ref": "#/components/responses/GatewayError"},
                    "403": {"$ref": "#/components/responses/GatewayError"},
                    "404": {"$ref": "#/components/responses/GatewayError"},
                    "409": {"$ref": "#/components/responses/GatewayError"}
                }
            },
            "delete": {
                "operationId": "deleteGatewayScimUser",
                "summary": "Delete one gateway SCIM user",
                "security": [{"GatewayBearerAuth": []}],
                "responses": {
                    "200": {"description": "Deleted SCIM user marker"},
                    "401": {"$ref": "#/components/responses/GatewayError"},
                    "403": {"$ref": "#/components/responses/GatewayError"},
                    "404": {"$ref": "#/components/responses/GatewayError"}
                }
            }
        }),
    );
    paths.insert(
        usage_path,
        serde_json::json!({
            "get": {
                "operationId": "getGatewayVirtualKeyUsage",
                "summary": "List gateway virtual keys with persisted usage counters",
                "security": [{"GatewayBearerAuth": []}],
                "responses": {
                    "200": {
                        "description": "Gateway virtual key usage",
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/GatewayKeyList"}
                            }
                        }
                    },
                    "401": {"$ref": "#/components/responses/GatewayError"}
                }
            }
        }),
    );
    paths.insert(
        ledger_path,
        serde_json::json!({
            "get": {
                "operationId": "getGatewayBillingLedger",
                "summary": "Get recent gateway virtual-key billing ledger records",
                "security": [{"GatewayBearerAuth": []}],
                "responses": {
                    "200": {
                        "description": "Gateway billing ledger records",
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/GatewayBillingLedger"}
                            }
                        }
                    },
                    "401": {"$ref": "#/components/responses/GatewayError"}
                }
            }
        }),
    );
    paths.insert(
        ledger_csv_path,
        serde_json::json!({
            "get": {
                "operationId": "exportGatewayBillingLedgerCsv",
                "summary": "Export recent gateway virtual-key billing ledger records as CSV",
                "security": [{"GatewayBearerAuth": []}],
                "responses": {
                    "200": {
                        "description": "Gateway billing ledger CSV export",
                        "content": {
                            "text/csv": {
                                "schema": {"type": "string"}
                            }
                        }
                    },
                    "401": {"$ref": "#/components/responses/GatewayError"}
                }
            }
        }),
    );
    paths.insert(
        ledger_summary_path,
        serde_json::json!({
            "get": {
                "operationId": "getGatewayBillingSummary",
                "summary": "Get aggregated gateway virtual-key billing totals",
                "security": [{"GatewayBearerAuth": []}],
                "responses": {
                    "200": {
                        "description": "Gateway billing summary grouped by key and model",
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/GatewayBillingSummary"}
                            }
                        }
                    },
                    "401": {"$ref": "#/components/responses/GatewayError"}
                }
            }
        }),
    );
    paths.insert(
        ledger_summary_csv_path,
        serde_json::json!({
            "get": {
                "operationId": "exportGatewayBillingSummaryCsv",
                "summary": "Export aggregated gateway virtual-key billing totals as CSV",
                "security": [{"GatewayBearerAuth": []}],
                "responses": {
                    "200": {
                        "description": "Gateway billing summary CSV export",
                        "content": {
                            "text/csv": {
                                "schema": {"type": "string"}
                            }
                        }
                    },
                    "401": {"$ref": "#/components/responses/GatewayError"}
                }
            }
        }),
    );
    paths.insert(
        metrics_path,
        serde_json::json!({
            "get": {
                "operationId": "getGatewayPrometheusMetrics",
                "summary": "Get gateway virtual-key usage metrics in Prometheus text format",
                "security": [{"GatewayBearerAuth": []}],
                "responses": {
                    "200": {
                        "description": "Prometheus text metrics",
                        "content": {
                            "text/plain": {
                                "schema": {"type": "string"}
                            }
                        }
                    },
                    "401": {"$ref": "#/components/responses/GatewayError"}
                }
            }
        }),
    );
    paths.insert(
        openapi_path,
        serde_json::json!({
            "get": {
                "operationId": "getProdexGatewayOpenApi",
                "summary": "Get the Prodex gateway OpenAPI document",
                "security": [{"GatewayBearerAuth": []}],
                "responses": {
                    "200": {
                        "description": "OpenAPI document",
                        "content": {
                            "application/json": {
                                "schema": {"type": "object", "additionalProperties": true}
                            }
                        }
                    },
                    "401": {"$ref": "#/components/responses/GatewayError"}
                }
            }
        }),
    );
    paths.insert(
        admin_path,
        serde_json::json!({
            "get": {
                "operationId": "getProdexGatewayAdminDashboard",
                "summary": "Get the built-in Prodex gateway admin dashboard shell",
                "responses": {
                    "200": {
                        "description": "HTML dashboard shell. Data requests still require a gateway admin bearer token.",
                        "content": {
                            "text/html": {
                                "schema": {"type": "string"}
                            }
                        }
                    }
                }
            }
        }),
    );

    serde_json::json!({
        "openapi": "3.1.0",
        "info": {
            "title": "Prodex Gateway API",
            "version": env!("CARGO_PKG_VERSION"),
            "description": "Local Prodex gateway admin and OpenAI-compatible response surface."
        },
        "servers": [{"url": mount_path}],
        "security": [{"GatewayBearerAuth": []}],
        "paths": paths,
        "components": {
            "securitySchemes": {
                "GatewayBearerAuth": {
                    "type": "http",
                    "scheme": "bearer"
                }
            },
            "responses": {
                "GatewayError": {
                    "description": "Gateway JSON error",
                    "content": {
                        "application/json": {
                            "schema": {"$ref": "#/components/schemas/GatewayError"}
                        }
                    }
                }
            },
            "schemas": {
                "GatewayError": {
                    "type": "object",
                    "required": ["error"],
                    "properties": {
                        "error": {
                            "type": "object",
                            "required": ["code", "message"],
                            "properties": {
                                "code": {"type": "string"},
                                "message": {"type": "string"}
                            }
                        }
                    }
                },
                "GatewayKeyCreateRequest": {
                    "type": "object",
                    "required": ["name"],
                    "properties": {
                        "name": {"type": "string"},
                        "token": {
                            "type": "string",
                            "description": "Optional caller-supplied bearer token. Omit to have Prodex generate one."
                        },
                        "tenant_id": {"type": ["string", "null"]},
                        "allowed_models": {
                            "type": "array",
                            "items": {"type": "string"}
                        },
                        "budget_microusd": {"type": ["integer", "null"], "minimum": 0},
                        "budget_usd": {"type": ["number", "null"], "minimum": 0},
                        "request_budget": {"type": ["integer", "null"], "minimum": 0},
                        "rpm_limit": {"type": ["integer", "null"], "minimum": 0},
                        "tpm_limit": {"type": ["integer", "null"], "minimum": 0},
                        "disabled": {"type": "boolean"}
                    },
                    "additionalProperties": false
                },
                "GatewayKeyPatchRequest": {
                    "type": "object",
                    "properties": {
                        "token": {
                            "type": "string",
                            "description": "Optional replacement bearer token. The stored file only keeps its hash."
                        },
                        "rotate": {
                            "type": "boolean",
                            "description": "Generate and return a new bearer token once."
                        },
                        "tenant_id": {"type": ["string", "null"]},
                        "allowed_models": {
                            "type": "array",
                            "items": {"type": "string"}
                        },
                        "budget_microusd": {"type": ["integer", "null"], "minimum": 0},
                        "budget_usd": {"type": ["number", "null"], "minimum": 0},
                        "request_budget": {"type": ["integer", "null"], "minimum": 0},
                        "rpm_limit": {"type": ["integer", "null"], "minimum": 0},
                        "tpm_limit": {"type": ["integer", "null"], "minimum": 0},
                        "disabled": {"type": "boolean"}
                    },
                    "additionalProperties": false
                },
                "GatewayKeyMutationResponse": {
                    "type": "object",
                    "required": ["object", "key"],
                    "properties": {
                        "object": {"type": "string"},
                        "key": {"$ref": "#/components/schemas/GatewayKey"},
                        "token": {
                            "type": ["string", "null"],
                            "description": "Returned only when Prodex generated or rotated the token for this request."
                        }
                    }
                },
                "GatewayKeyResponse": {
                    "type": "object",
                    "required": ["object", "key"],
                    "properties": {
                        "object": {"type": "string"},
                        "key": {"$ref": "#/components/schemas/GatewayKey"}
                    }
                },
                "GatewayKeyList": {
                    "type": "object",
                    "required": ["object", "keys"],
                    "properties": {
                        "object": {"type": "string"},
                        "keys": {
                            "type": "array",
                            "items": {"$ref": "#/components/schemas/GatewayKey"}
                        },
                        "state_backend": {"type": "string", "enum": ["file", "sqlite", "postgres", "redis"]},
                        "state_path": {"type": "string"},
                        "key_store_path": {"type": "string"},
                        "usage_path": {"type": ["string", "null"]},
                        "unknown_persisted_keys": {
                            "type": "array",
                            "items": {"type": "string"}
                        }
                    }
                },
                "GatewayScimUserList": {
                    "type": "object",
                    "required": ["schemas", "totalResults", "Resources"],
                    "properties": {
                        "schemas": {
                            "type": "array",
                            "items": {"type": "string"}
                        },
                        "totalResults": {"type": "integer"},
                        "startIndex": {"type": "integer"},
                        "itemsPerPage": {"type": "integer"},
                        "Resources": {
                            "type": "array",
                            "items": {"$ref": "#/components/schemas/GatewayScimUser"}
                        }
                    }
                },
                "GatewayScimUserWrite": {
                    "type": "object",
                    "required": ["userName"],
                    "properties": {
                        "userName": {"type": "string"},
                        "externalId": {"type": ["string", "null"]},
                        "displayName": {"type": ["string", "null"]},
                        "active": {"type": "boolean"},
                        "tenant_id": {"type": ["string", "null"]},
                        "role": {"type": ["string", "null"], "enum": ["admin", "viewer", null]},
                        "allowed_key_prefixes": {
                            "type": "array",
                            "items": {"type": "string"}
                        }
                    },
                    "additionalProperties": true
                },
                "GatewayScimUser": {
                    "type": "object",
                    "required": ["schemas", "id", "userName", "active"],
                    "properties": {
                        "schemas": {
                            "type": "array",
                            "items": {"type": "string"}
                        },
                        "id": {"type": "string"},
                        "userName": {"type": "string"},
                        "externalId": {"type": ["string", "null"]},
                        "displayName": {"type": ["string", "null"]},
                        "active": {"type": "boolean"},
                        "tenant_id": {"type": ["string", "null"]},
                        "meta": {"type": "object", "additionalProperties": true}
                    },
                    "additionalProperties": true
                },
                "GatewayBillingLedger": {
                    "type": "object",
                    "required": ["object", "records"],
                    "properties": {
                        "object": {"type": "string"},
                        "state_backend": {"type": "string", "enum": ["file", "sqlite", "postgres", "redis"]},
                        "ledger_path": {"type": "string"},
                        "limit": {"type": "integer"},
                        "records": {
                            "type": "array",
                            "items": {"$ref": "#/components/schemas/GatewayBillingLedgerEntry"}
                        }
                    }
                },
                "GatewayBillingSummary": {
                    "type": "object",
                    "required": ["object", "totals", "by_key", "by_model", "by_key_model"],
                    "properties": {
                        "object": {"type": "string"},
                        "state_backend": {"type": "string", "enum": ["file", "sqlite", "postgres", "redis"]},
                        "ledger_path": {"type": "string"},
                        "record_count": {"type": "integer"},
                        "totals": {"$ref": "#/components/schemas/GatewayBillingSummaryBucket"},
                        "by_key": {
                            "type": "array",
                            "items": {"$ref": "#/components/schemas/GatewayBillingSummaryBucket"}
                        },
                        "by_model": {
                            "type": "array",
                            "items": {"$ref": "#/components/schemas/GatewayBillingSummaryBucket"}
                        },
                        "by_key_model": {
                            "type": "array",
                            "items": {"$ref": "#/components/schemas/GatewayBillingSummaryBucket"}
                        }
                    }
                },
                "GatewayBillingSummaryBucket": {
                    "type": "object",
                    "required": [
                        "requests",
                        "successful_requests",
                        "failed_requests",
                        "unreconciled_requests",
                        "input_tokens",
                        "output_tokens",
                        "response_bytes",
                        "estimated_cost_microusd",
                        "estimated_cost_usd",
                        "final_cost_microusd",
                        "final_cost_usd"
                    ],
                    "properties": {
                        "key_name": {"type": ["string", "null"]},
                        "model": {"type": ["string", "null"]},
                        "requests": {"type": "integer"},
                        "successful_requests": {"type": "integer"},
                        "failed_requests": {"type": "integer"},
                        "unreconciled_requests": {"type": "integer"},
                        "input_tokens": {"type": "integer"},
                        "output_tokens": {"type": "integer"},
                        "response_bytes": {"type": "integer"},
                        "estimated_cost_microusd": {"type": "integer"},
                        "estimated_cost_usd": {"type": "number"},
                        "final_cost_microusd": {"type": "integer"},
                        "final_cost_usd": {"type": "number"},
                        "first_created_at_epoch": {"type": ["integer", "null"]},
                        "last_created_at_epoch": {"type": ["integer", "null"]},
                        "last_reconciled_at_epoch": {"type": ["integer", "null"]}
                    }
                },
                "GatewayBillingLedgerEntry": {
                    "type": "object",
                    "required": [
                        "object",
                        "phase",
                        "request",
                        "call_id",
                        "key_name",
                        "model",
                        "minute_epoch",
                        "input_tokens",
                        "created_at_epoch"
                    ],
                    "properties": {
                        "object": {"type": "string"},
                        "phase": {"type": "string", "enum": ["request"]},
                        "request": {"type": "integer"},
                        "call_id": {"type": "string"},
                        "key_name": {"type": "string"},
                        "model": {"type": "string"},
                        "minute_epoch": {"type": "integer"},
                        "input_tokens": {"type": "integer"},
                        "estimated_cost_microusd": {"type": ["integer", "null"]},
                        "estimated_cost_usd": {"type": ["number", "null"]},
                        "created_at_epoch": {"type": "integer"},
                        "response_status": {"type": ["integer", "null"]},
                        "response_bytes": {"type": ["integer", "null"]},
                        "output_tokens": {"type": ["integer", "null"]},
                        "final_cost_microusd": {"type": ["integer", "null"]},
                        "final_cost_usd": {"type": ["number", "null"]},
                        "reconciled_at_epoch": {"type": ["integer", "null"]}
                    }
                },
                "GatewayKey": {
                    "type": "object",
                    "required": ["name", "source", "disabled", "editable", "usage"],
                    "properties": {
                        "name": {"type": "string"},
                        "tenant_id": {"type": ["string", "null"]},
                        "source": {"type": "string", "enum": ["policy", "admin"]},
                        "disabled": {"type": "boolean"},
                        "editable": {"type": "boolean"},
                        "created_at_epoch": {"type": ["integer", "null"]},
                        "updated_at_epoch": {"type": ["integer", "null"]},
                        "allowed_models": {
                            "type": "array",
                            "items": {"type": "string"}
                        },
                        "budget_microusd": {"type": ["integer", "null"]},
                        "budget_usd": {"type": ["number", "null"]},
                        "request_budget": {"type": ["integer", "null"]},
                        "rpm_limit": {"type": ["integer", "null"]},
                        "tpm_limit": {"type": ["integer", "null"]},
                        "usage": {"$ref": "#/components/schemas/GatewayUsage"}
                    }
                },
                "GatewayUsage": {
                    "type": "object",
                    "required": [
                        "minute_epoch",
                        "requests_this_minute",
                        "tokens_this_minute",
                        "requests_total",
                        "spend_microusd",
                        "spend_usd"
                    ],
                    "properties": {
                        "minute_epoch": {"type": "integer"},
                        "requests_this_minute": {"type": "integer"},
                        "tokens_this_minute": {"type": "integer"},
                        "requests_total": {"type": "integer"},
                        "spend_microusd": {"type": "integer"},
                        "spend_usd": {"type": "number"}
                    }
                },
                "GatewayKeyDeleted": {
                    "type": "object",
                    "required": ["object", "name", "deleted"],
                    "properties": {
                        "object": {"type": "string"},
                        "name": {"type": "string"},
                        "deleted": {"type": "boolean"}
                    }
                }
            }
        }
    })
}

fn runtime_gateway_admin_keys_payload(
    shared: &RuntimeLocalRewriteProxyShared,
    object: &str,
    admin_auth: Option<&RuntimeGatewayAdminAuth>,
) -> serde_json::Value {
    let usage = shared
        .gateway_virtual_key_usage
        .lock()
        .map(|usage| usage.clone())
        .unwrap_or_default();
    let entries = shared
        .gateway_virtual_keys
        .lock()
        .map(|entries| entries.clone())
        .unwrap_or_default();
    let configured_names = shared
        .gateway_virtual_keys
        .lock()
        .map(|entries| {
            entries
                .iter()
                .map(|entry| entry.key.name.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let keys = entries
        .iter()
        .filter(|entry| {
            admin_auth
                .map(|admin_auth| admin_auth.can_access_entry(entry))
                .unwrap_or(true)
        })
        .map(|entry| runtime_gateway_admin_key_json(entry, usage.get(&entry.key.name).cloned()))
        .collect::<Vec<_>>();
    let unknown_persisted_keys = usage
        .keys()
        .filter(|name| {
            !configured_names
                .iter()
                .any(|configured| *configured == name.as_str())
                && admin_auth
                    .map(|admin_auth| {
                        admin_auth.tenant_id.is_none() && admin_auth.can_access_key(name)
                    })
                    .unwrap_or(true)
        })
        .cloned()
        .collect::<Vec<_>>();
    serde_json::json!({
        "object": object,
        "keys": keys,
        "state_backend": shared.gateway_state_store.label(),
        "state_path": shared.gateway_state_store.key_store_path().display().to_string(),
        "key_store_path": shared.gateway_virtual_key_store_path.display().to_string(),
        "usage_path": shared.gateway_virtual_key_usage_path.as_ref().map(|path| path.display().to_string()),
        "unknown_persisted_keys": unknown_persisted_keys,
    })
}

fn runtime_gateway_admin_ledger_response(
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
) -> tiny_http::ResponseBox {
    match runtime_gateway_billing_ledger_load(&shared.gateway_state_store, 1000) {
        Ok(records) => {
            let records = runtime_gateway_admin_filter_ledger_records(records, shared, admin_auth);
            runtime_gateway_admin_json_response(
                200,
                serde_json::json!({
                    "object": "gateway.billing_ledger",
                    "state_backend": shared.gateway_state_store.label(),
                    "ledger_path": shared.gateway_state_store.ledger_path().display().to_string(),
                    "limit": 1000,
                    "records": records,
                }),
            )
        }
        Err(err) => build_runtime_proxy_json_error_response(
            500,
            "gateway_billing_ledger_load_failed",
            &err.to_string(),
        ),
    }
}

fn runtime_gateway_admin_ledger_csv_response(
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
) -> tiny_http::ResponseBox {
    match runtime_gateway_billing_ledger_load(&shared.gateway_state_store, usize::MAX) {
        Ok(records) => {
            let records = runtime_gateway_admin_filter_ledger_records(records, shared, admin_auth);
            runtime_gateway_admin_csv_response(runtime_gateway_billing_ledger_csv(&records))
        }
        Err(err) => build_runtime_proxy_json_error_response(
            500,
            "gateway_billing_ledger_load_failed",
            &err.to_string(),
        ),
    }
}

fn runtime_gateway_admin_ledger_summary_response(
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
) -> tiny_http::ResponseBox {
    match runtime_gateway_billing_ledger_load(&shared.gateway_state_store, usize::MAX) {
        Ok(records) => {
            let records = runtime_gateway_admin_filter_ledger_records(records, shared, admin_auth);
            runtime_gateway_admin_json_response(
                200,
                runtime_gateway_billing_summary_payload(shared, &records),
            )
        }
        Err(err) => build_runtime_proxy_json_error_response(
            500,
            "gateway_billing_summary_load_failed",
            &err.to_string(),
        ),
    }
}

fn runtime_gateway_admin_ledger_summary_csv_response(
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
) -> tiny_http::ResponseBox {
    match runtime_gateway_billing_ledger_load(&shared.gateway_state_store, usize::MAX) {
        Ok(records) => {
            let records = runtime_gateway_admin_filter_ledger_records(records, shared, admin_auth);
            runtime_gateway_admin_csv_response(runtime_gateway_billing_summary_csv(
                &runtime_gateway_billing_summary_payload(shared, &records),
            ))
        }
        Err(err) => build_runtime_proxy_json_error_response(
            500,
            "gateway_billing_summary_load_failed",
            &err.to_string(),
        ),
    }
}

fn runtime_gateway_admin_filter_ledger_records(
    records: Vec<RuntimeGatewayBillingLedgerEntry>,
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
) -> Vec<RuntimeGatewayBillingLedgerEntry> {
    let entries = shared
        .gateway_virtual_keys
        .lock()
        .map(|entries| entries.clone())
        .unwrap_or_default();
    records
        .into_iter()
        .filter(|record| runtime_gateway_admin_can_access_ledger_key(record, &entries, admin_auth))
        .collect()
}

fn runtime_gateway_admin_can_access_ledger_key(
    record: &RuntimeGatewayBillingLedgerEntry,
    entries: &[RuntimeGatewayVirtualKeyEntry],
    admin_auth: &RuntimeGatewayAdminAuth,
) -> bool {
    if !admin_auth.can_access_key(&record.key_name) {
        return false;
    }
    if admin_auth.tenant_id.is_none() {
        return true;
    }
    entries
        .iter()
        .find(|entry| entry.key.name.eq_ignore_ascii_case(&record.key_name))
        .map(|entry| admin_auth.can_access_tenant(entry.tenant_id.as_deref()))
        .unwrap_or(false)
}

fn runtime_gateway_billing_summary_payload(
    shared: &RuntimeLocalRewriteProxyShared,
    records: &[RuntimeGatewayBillingLedgerEntry],
) -> serde_json::Value {
    let mut totals = RuntimeGatewayBillingSummaryBucket::default();
    let mut by_key: BTreeMap<String, RuntimeGatewayBillingSummaryBucket> = BTreeMap::new();
    let mut by_model: BTreeMap<String, RuntimeGatewayBillingSummaryBucket> = BTreeMap::new();
    let mut by_key_model: BTreeMap<(String, String), RuntimeGatewayBillingSummaryBucket> =
        BTreeMap::new();
    for record in records.iter().filter(|record| record.phase == "request") {
        totals.record(record);
        by_key
            .entry(record.key_name.clone())
            .or_insert_with(|| {
                RuntimeGatewayBillingSummaryBucket::with_key_model(
                    Some(record.key_name.clone()),
                    None,
                )
            })
            .record(record);
        by_model
            .entry(record.model.clone())
            .or_insert_with(|| {
                RuntimeGatewayBillingSummaryBucket::with_key_model(None, Some(record.model.clone()))
            })
            .record(record);
        by_key_model
            .entry((record.key_name.clone(), record.model.clone()))
            .or_insert_with(|| {
                RuntimeGatewayBillingSummaryBucket::with_key_model(
                    Some(record.key_name.clone()),
                    Some(record.model.clone()),
                )
            })
            .record(record);
    }
    serde_json::json!({
        "object": "gateway.billing_summary",
        "state_backend": shared.gateway_state_store.label(),
        "ledger_path": shared.gateway_state_store.ledger_path().display().to_string(),
        "record_count": records.len(),
        "totals": totals,
        "by_key": by_key.into_values().collect::<Vec<_>>(),
        "by_model": by_model.into_values().collect::<Vec<_>>(),
        "by_key_model": by_key_model.into_values().collect::<Vec<_>>(),
    })
}

fn runtime_gateway_billing_ledger_csv(records: &[RuntimeGatewayBillingLedgerEntry]) -> String {
    let mut csv = String::new();
    runtime_gateway_csv_row(
        &mut csv,
        [
            "call_id",
            "key_name",
            "model",
            "phase",
            "request",
            "created_at_epoch",
            "minute_epoch",
            "input_tokens",
            "output_tokens",
            "response_status",
            "response_bytes",
            "estimated_cost_microusd",
            "estimated_cost_usd",
            "final_cost_microusd",
            "final_cost_usd",
            "reconciled_at_epoch",
        ],
    );
    for record in records {
        runtime_gateway_csv_row(
            &mut csv,
            &[
                record.call_id.clone(),
                record.key_name.clone(),
                record.model.clone(),
                record.phase.clone(),
                record.request.to_string(),
                record.created_at_epoch.to_string(),
                record.minute_epoch.to_string(),
                record.input_tokens.to_string(),
                runtime_gateway_csv_optional_u64(record.output_tokens),
                runtime_gateway_csv_optional_u16(record.response_status),
                runtime_gateway_csv_optional_u64(record.response_bytes),
                runtime_gateway_csv_optional_u64(record.estimated_cost_microusd),
                runtime_gateway_csv_optional_f64(record.estimated_cost_usd),
                runtime_gateway_csv_optional_u64(record.final_cost_microusd),
                runtime_gateway_csv_optional_f64(record.final_cost_usd),
                runtime_gateway_csv_optional_u64(record.reconciled_at_epoch),
            ],
        );
    }
    csv
}

fn runtime_gateway_billing_summary_csv(summary: &serde_json::Value) -> String {
    let mut csv = String::new();
    runtime_gateway_csv_row(
        &mut csv,
        [
            "group",
            "key_name",
            "model",
            "requests",
            "successful_requests",
            "failed_requests",
            "unreconciled_requests",
            "input_tokens",
            "output_tokens",
            "response_bytes",
            "estimated_cost_microusd",
            "estimated_cost_usd",
            "final_cost_microusd",
            "final_cost_usd",
            "first_created_at_epoch",
            "last_created_at_epoch",
            "last_reconciled_at_epoch",
        ],
    );
    if let Some(totals) = summary.get("totals") {
        runtime_gateway_billing_summary_csv_row(&mut csv, "totals", totals);
    }
    for group in ["by_key", "by_model", "by_key_model"] {
        if let Some(rows) = summary.get(group).and_then(serde_json::Value::as_array) {
            for row in rows {
                runtime_gateway_billing_summary_csv_row(&mut csv, group, row);
            }
        }
    }
    csv
}

fn runtime_gateway_billing_summary_csv_row(csv: &mut String, group: &str, row: &serde_json::Value) {
    runtime_gateway_csv_row(
        csv,
        &[
            group.to_string(),
            runtime_gateway_json_csv_string(row, "key_name"),
            runtime_gateway_json_csv_string(row, "model"),
            runtime_gateway_json_csv_u64(row, "requests"),
            runtime_gateway_json_csv_u64(row, "successful_requests"),
            runtime_gateway_json_csv_u64(row, "failed_requests"),
            runtime_gateway_json_csv_u64(row, "unreconciled_requests"),
            runtime_gateway_json_csv_u64(row, "input_tokens"),
            runtime_gateway_json_csv_u64(row, "output_tokens"),
            runtime_gateway_json_csv_u64(row, "response_bytes"),
            runtime_gateway_json_csv_u64(row, "estimated_cost_microusd"),
            runtime_gateway_json_csv_f64(row, "estimated_cost_usd"),
            runtime_gateway_json_csv_u64(row, "final_cost_microusd"),
            runtime_gateway_json_csv_f64(row, "final_cost_usd"),
            runtime_gateway_json_csv_u64(row, "first_created_at_epoch"),
            runtime_gateway_json_csv_u64(row, "last_created_at_epoch"),
            runtime_gateway_json_csv_u64(row, "last_reconciled_at_epoch"),
        ],
    );
}

fn runtime_gateway_json_csv_string(row: &serde_json::Value, field: &str) -> String {
    row.get(field)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn runtime_gateway_json_csv_u64(row: &serde_json::Value, field: &str) -> String {
    row.get(field)
        .and_then(serde_json::Value::as_u64)
        .map(|value| value.to_string())
        .unwrap_or_default()
}

fn runtime_gateway_json_csv_f64(row: &serde_json::Value, field: &str) -> String {
    row.get(field)
        .and_then(serde_json::Value::as_f64)
        .map(|value| value.to_string())
        .unwrap_or_default()
}

fn runtime_gateway_csv_optional_u16(value: Option<u16>) -> String {
    value.map(|value| value.to_string()).unwrap_or_default()
}

fn runtime_gateway_csv_optional_u64(value: Option<u64>) -> String {
    value.map(|value| value.to_string()).unwrap_or_default()
}

fn runtime_gateway_csv_optional_f64(value: Option<f64>) -> String {
    value.map(|value| value.to_string()).unwrap_or_default()
}

fn runtime_gateway_csv_row<I, S>(csv: &mut String, values: I)
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut first = true;
    for value in values {
        if first {
            first = false;
        } else {
            csv.push(',');
        }
        runtime_gateway_csv_cell(csv, value.as_ref());
    }
    csv.push('\n');
}

fn runtime_gateway_csv_cell(csv: &mut String, value: &str) {
    let needs_quote = value
        .bytes()
        .any(|byte| matches!(byte, b',' | b'"' | b'\n' | b'\r'));
    if !needs_quote {
        csv.push_str(value);
        return;
    }
    csv.push('"');
    for ch in value.chars() {
        if ch == '"' {
            csv.push('"');
        }
        csv.push(ch);
    }
    csv.push('"');
}

fn runtime_gateway_admin_key_json(
    entry: &RuntimeGatewayVirtualKeyEntry,
    usage: Option<runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage>,
) -> serde_json::Value {
    let usage = usage.unwrap_or_default();
    serde_json::json!({
        "name": entry.key.name,
        "tenant_id": entry.tenant_id,
        "source": entry.source.as_str(),
        "disabled": entry.disabled,
        "editable": entry.source == RuntimeGatewayVirtualKeySource::Admin,
        "created_at_epoch": entry.created_at_epoch,
        "updated_at_epoch": entry.updated_at_epoch,
        "allowed_models": entry.key.allowed_models,
        "budget_microusd": entry.key.budget_microusd,
        "budget_usd": entry.key.budget_microusd.map(microusd_to_usd),
        "request_budget": entry.key.request_budget,
        "rpm_limit": entry.key.rpm_limit,
        "tpm_limit": entry.key.tpm_limit,
        "usage": {
            "minute_epoch": usage.minute_epoch,
            "requests_this_minute": usage.requests_this_minute,
            "tokens_this_minute": usage.tokens_this_minute,
            "requests_total": usage.requests_total,
            "spend_microusd": usage.spend_microusd,
            "spend_usd": microusd_to_usd(usage.spend_microusd),
        }
    })
}

fn runtime_gateway_admin_get_key_response(
    name: &str,
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
) -> tiny_http::ResponseBox {
    let entries = shared
        .gateway_virtual_keys
        .lock()
        .map(|entries| entries.clone())
        .unwrap_or_default();
    let Some(entry) = entries
        .iter()
        .find(|entry| entry.key.name.eq_ignore_ascii_case(name))
    else {
        return build_runtime_proxy_json_error_response(
            404,
            "gateway_key_not_found",
            "gateway virtual key was not found",
        );
    };
    if !admin_auth.can_access_entry(entry) {
        return runtime_gateway_admin_key_scope_forbidden_response();
    }
    let usage = shared
        .gateway_virtual_key_usage
        .lock()
        .ok()
        .and_then(|usage| usage.get(&entry.key.name).cloned());
    runtime_gateway_admin_json_response(
        200,
        serde_json::json!({
            "object": "gateway.key",
            "key": runtime_gateway_admin_key_json(entry, usage),
        }),
    )
}

fn runtime_gateway_admin_create_key_response(
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
) -> tiny_http::ResponseBox {
    let body = match runtime_gateway_admin_json_body(captured) {
        Ok(body) => body,
        Err(response) => return response,
    };
    let name = match body
        .get("name")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(name) => name.to_string(),
        None => {
            return build_runtime_proxy_json_error_response(
                400,
                "invalid_gateway_key_name",
                "gateway virtual key name is required",
            );
        }
    };
    if let Err(message) = runtime_gateway_validate_virtual_key_name(&name) {
        return build_runtime_proxy_json_error_response(400, "invalid_gateway_key_name", message);
    }
    let requested_tenant_id = runtime_gateway_admin_request_tenant_id(&body, admin_auth);
    if !admin_auth.can_access_tenant(requested_tenant_id.as_deref())
        || !admin_auth.can_access_key(&name)
    {
        return runtime_gateway_admin_key_scope_forbidden_response();
    }
    if runtime_gateway_virtual_key_name_exists(shared, &name) {
        return build_runtime_proxy_json_error_response(
            409,
            "gateway_key_exists",
            "gateway virtual key name already exists",
        );
    }
    let supplied_token = body
        .get("token")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let generated_token = match supplied_token {
        Some(token) => token,
        None => match runtime_gateway_generate_virtual_key_token() {
            Ok(token) => token,
            Err(err) => {
                return build_runtime_proxy_json_error_response(
                    500,
                    "gateway_key_generation_failed",
                    &err.to_string(),
                );
            }
        },
    };
    let now = runtime_gateway_unix_epoch_seconds();
    let mut record = RuntimeGatewayStoredVirtualKey {
        name: name.clone(),
        tenant_id: requested_tenant_id,
        token_hash_base64: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
            &generated_token,
        )
        .hash_base64(),
        allowed_models: Vec::new(),
        budget_microusd: None,
        request_budget: None,
        rpm_limit: None,
        tpm_limit: None,
        disabled: Some(false),
        created_at_epoch: now,
        updated_at_epoch: now,
    };
    if let Err(err) = runtime_gateway_apply_virtual_key_patch(&mut record, &body, false) {
        return err.into_response();
    }
    if runtime_gateway_admin_body_tenant_id(&body).is_none() && record.tenant_id.is_none() {
        record.tenant_id = admin_auth.tenant_id.clone();
    }
    match runtime_gateway_mutate_admin_key_store(shared, |store| {
        if store
            .keys
            .iter()
            .any(|key| key.name.eq_ignore_ascii_case(&name))
        {
            return Err(RuntimeGatewayAdminError::new(
                409,
                "gateway_key_exists",
                "gateway virtual key name already exists",
            ));
        }
        store.keys.push(record.clone());
        Ok(())
    }) {
        Ok(()) => {
            runtime_gateway_audit_admin_key_event(
                shared,
                "create_key",
                "success",
                &name,
                serde_json::json!({
                    "generated_token": body.get("token").is_none(),
                    "tenant_id": record.tenant_id,
                    "allowed_models": record.allowed_models,
                    "disabled": record.disabled.unwrap_or(false),
                    "request_budget": record.request_budget,
                    "rpm_limit": record.rpm_limit,
                    "tpm_limit": record.tpm_limit,
                    "budget_microusd": record.budget_microusd,
                }),
            );
            runtime_gateway_admin_json_response(
                201,
                serde_json::json!({
                    "object": "gateway.key",
                    "key": runtime_gateway_admin_stored_key_json(&record),
                    "token": generated_token,
                }),
            )
        }
        Err(response) => response,
    }
}

fn runtime_gateway_admin_update_key_response(
    name: &str,
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
) -> tiny_http::ResponseBox {
    let Some(entry) = runtime_gateway_virtual_key_entry_by_name(shared, name) else {
        return build_runtime_proxy_json_error_response(
            404,
            "gateway_key_not_found",
            "gateway virtual key was not found",
        );
    };
    if !admin_auth.can_access_entry(&entry) {
        return runtime_gateway_admin_key_scope_forbidden_response();
    }
    if entry.source != RuntimeGatewayVirtualKeySource::Admin {
        return build_runtime_proxy_json_error_response(
            403,
            "gateway_key_read_only",
            "policy-backed gateway virtual keys cannot be edited through the admin API",
        );
    }
    let body = match runtime_gateway_admin_json_body(captured) {
        Ok(body) => body,
        Err(response) => return response,
    };
    if let Some(tenant_id) = runtime_gateway_admin_body_tenant_id(&body)
        && !admin_auth.can_access_tenant(tenant_id.as_deref())
    {
        return runtime_gateway_admin_key_scope_forbidden_response();
    }
    let rotate = body
        .get("rotate")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let mut rotated_token = None;
    let update_result = runtime_gateway_mutate_admin_key_store(shared, |store| {
        let Some(record) = store
            .keys
            .iter_mut()
            .find(|key| key.name.eq_ignore_ascii_case(name))
        else {
            return Err(RuntimeGatewayAdminError::new(
                404,
                "gateway_key_not_found",
                "gateway virtual key was not found",
            ));
        };
        if rotate {
            let token = runtime_gateway_generate_virtual_key_token().map_err(|err| {
                RuntimeGatewayAdminError::new(500, "gateway_key_generation_failed", err.to_string())
            })?;
            record.token_hash_base64 =
                runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(&token).hash_base64();
            rotated_token = Some(token);
        } else if let Some(token) = body
            .get("token")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            record.token_hash_base64 =
                runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(token).hash_base64();
        }
        runtime_gateway_apply_virtual_key_patch(record, &body, true)?;
        record.updated_at_epoch = runtime_gateway_unix_epoch_seconds();
        Ok(())
    });
    match update_result {
        Ok(()) => {
            let entry = runtime_gateway_virtual_key_entry_by_name(shared, name);
            runtime_gateway_audit_admin_key_event(
                shared,
                if rotate { "rotate_key" } else { "update_key" },
                "success",
                name,
                serde_json::json!({
                    "rotated": rotate,
                    "token_replaced": !rotate && body.get("token").is_some(),
                    "updated_fields": runtime_gateway_admin_key_patch_fields(&body),
                }),
            );
            runtime_gateway_admin_json_response(
                200,
                serde_json::json!({
                    "object": "gateway.key",
                    "key": entry.map(|entry| {
                        runtime_gateway_admin_key_json(&entry, shared.gateway_virtual_key_usage.lock().ok().and_then(|usage| usage.get(&entry.key.name).cloned()))
                    }),
                    "token": rotated_token,
                }),
            )
        }
        Err(response) => response,
    }
}

fn runtime_gateway_admin_delete_key_response(
    name: &str,
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
) -> tiny_http::ResponseBox {
    let Some(entry) = runtime_gateway_virtual_key_entry_by_name(shared, name) else {
        return build_runtime_proxy_json_error_response(
            404,
            "gateway_key_not_found",
            "gateway virtual key was not found",
        );
    };
    if !admin_auth.can_access_entry(&entry) {
        return runtime_gateway_admin_key_scope_forbidden_response();
    }
    if entry.source != RuntimeGatewayVirtualKeySource::Admin {
        return build_runtime_proxy_json_error_response(
            403,
            "gateway_key_read_only",
            "policy-backed gateway virtual keys cannot be deleted through the admin API",
        );
    }
    match runtime_gateway_mutate_admin_key_store(shared, |store| {
        let before = store.keys.len();
        store
            .keys
            .retain(|key| !key.name.eq_ignore_ascii_case(name));
        if store.keys.len() == before {
            return Err(RuntimeGatewayAdminError::new(
                404,
                "gateway_key_not_found",
                "gateway virtual key was not found",
            ));
        }
        Ok(())
    }) {
        Ok(()) => runtime_gateway_admin_json_response(
            {
                runtime_gateway_audit_admin_key_event(
                    shared,
                    "delete_key",
                    "success",
                    &entry.key.name,
                    serde_json::json!({
                        "source": entry.source.as_str(),
                        "disabled": entry.disabled,
                    }),
                );
                200
            },
            serde_json::json!({
                "object": "gateway.key.deleted",
                "name": entry.key.name,
                "deleted": true,
            }),
        ),
        Err(response) => response,
    }
}

fn runtime_gateway_admin_scim_list_users_response(
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
) -> tiny_http::ResponseBox {
    let store = runtime_gateway_virtual_key_store_load(
        &shared.gateway_state_store,
        &shared.runtime_shared.log_path,
    );
    let resources = store
        .scim_users
        .iter()
        .filter(|user| admin_auth.can_access_scim_user(user))
        .map(|user| runtime_gateway_scim_user_json(user, shared))
        .collect::<Vec<_>>();
    runtime_gateway_admin_json_response(
        200,
        serde_json::json!({
            "schemas": [RUNTIME_GATEWAY_SCIM_LIST_SCHEMA],
            "totalResults": resources.len(),
            "startIndex": 1,
            "itemsPerPage": resources.len(),
            "Resources": resources,
        }),
    )
}

fn runtime_gateway_admin_scim_create_user_response(
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
) -> tiny_http::ResponseBox {
    let body = match runtime_gateway_admin_json_body(captured) {
        Ok(body) => body,
        Err(response) => return response,
    };
    let now = runtime_gateway_unix_epoch_seconds();
    let id = match runtime_gateway_generate_scim_user_id() {
        Ok(id) => id,
        Err(err) => {
            return build_runtime_proxy_json_error_response(
                500,
                "gateway_scim_user_generation_failed",
                &err.to_string(),
            );
        }
    };
    let mut user = RuntimeGatewayScimUser {
        id,
        user_name: String::new(),
        external_id: None,
        display_name: None,
        active: true,
        role: None,
        tenant_id: admin_auth.tenant_id.clone(),
        allowed_key_prefixes: Vec::new(),
        created_at_epoch: now,
        updated_at_epoch: now,
    };
    if let Err(err) = runtime_gateway_apply_scim_user_patch(&mut user, &body, false) {
        return err.into_response();
    }
    if user.tenant_id.is_none() {
        user.tenant_id = admin_auth.tenant_id.clone();
    }
    if !admin_auth.can_access_scim_user(&user) {
        return runtime_gateway_admin_key_scope_forbidden_response();
    }
    let user_name = user.user_name.clone();
    match runtime_gateway_mutate_admin_key_store(shared, |store| {
        if store
            .scim_users
            .iter()
            .any(|stored| stored.user_name.eq_ignore_ascii_case(&user_name))
        {
            return Err(RuntimeGatewayAdminError::new(
                409,
                "gateway_scim_user_exists",
                "gateway SCIM userName already exists",
            ));
        }
        store.scim_users.push(user.clone());
        Ok(())
    }) {
        Ok(()) => {
            runtime_gateway_audit_admin_scim_user_event(
                shared,
                "create_scim_user",
                "success",
                &user,
            );
            runtime_gateway_admin_json_response(201, runtime_gateway_scim_user_json(&user, shared))
        }
        Err(response) => response,
    }
}

fn runtime_gateway_admin_scim_get_user_response(
    id: &str,
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
) -> tiny_http::ResponseBox {
    let store = runtime_gateway_virtual_key_store_load(
        &shared.gateway_state_store,
        &shared.runtime_shared.log_path,
    );
    let Some(user) = store.scim_users.iter().find(|user| user.id == id) else {
        return build_runtime_proxy_json_error_response(
            404,
            "gateway_scim_user_not_found",
            "gateway SCIM user was not found",
        );
    };
    if !admin_auth.can_access_scim_user(user) {
        return runtime_gateway_admin_key_scope_forbidden_response();
    }
    runtime_gateway_admin_json_response(200, runtime_gateway_scim_user_json(user, shared))
}

fn runtime_gateway_admin_scim_update_user_response(
    id: &str,
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
) -> tiny_http::ResponseBox {
    let body = match runtime_gateway_admin_json_body(captured) {
        Ok(body) => body,
        Err(response) => return response,
    };
    let partial = !captured.method.eq_ignore_ascii_case("PUT");
    let mut updated = None;
    let update_result = runtime_gateway_mutate_admin_key_store(shared, |store| {
        let Some(index) = store.scim_users.iter().position(|user| user.id == id) else {
            return Err(RuntimeGatewayAdminError::new(
                404,
                "gateway_scim_user_not_found",
                "gateway SCIM user was not found",
            ));
        };
        if !admin_auth.can_access_scim_user(&store.scim_users[index]) {
            return Err(RuntimeGatewayAdminError::new(
                403,
                "gateway_admin_key_scope_forbidden",
                "gateway admin token is not allowed to access this tenant",
            ));
        }
        let previous_name = store.scim_users[index].user_name.clone();
        runtime_gateway_apply_scim_user_patch(&mut store.scim_users[index], &body, partial)?;
        if !admin_auth.can_access_scim_user(&store.scim_users[index]) {
            return Err(RuntimeGatewayAdminError::new(
                403,
                "gateway_admin_key_scope_forbidden",
                "gateway admin token is not allowed to access this tenant",
            ));
        }
        let next_name = store.scim_users[index].user_name.clone();
        if !previous_name.eq_ignore_ascii_case(&next_name)
            && store.scim_users.iter().enumerate().any(|(other, user)| {
                other != index && user.user_name.eq_ignore_ascii_case(&next_name)
            })
        {
            return Err(RuntimeGatewayAdminError::new(
                409,
                "gateway_scim_user_exists",
                "gateway SCIM userName already exists",
            ));
        }
        store.scim_users[index].updated_at_epoch = runtime_gateway_unix_epoch_seconds();
        updated = Some(store.scim_users[index].clone());
        Ok(())
    });
    match update_result {
        Ok(()) => {
            if let Some(user) = updated {
                runtime_gateway_audit_admin_scim_user_event(
                    shared,
                    "update_scim_user",
                    "success",
                    &user,
                );
                runtime_gateway_admin_json_response(
                    200,
                    runtime_gateway_scim_user_json(&user, shared),
                )
            } else {
                build_runtime_proxy_json_error_response(
                    404,
                    "gateway_scim_user_not_found",
                    "gateway SCIM user was not found",
                )
            }
        }
        Err(response) => response,
    }
}

fn runtime_gateway_admin_scim_delete_user_response(
    id: &str,
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
) -> tiny_http::ResponseBox {
    let mut deleted = None;
    match runtime_gateway_mutate_admin_key_store(shared, |store| {
        let before = store.scim_users.len();
        store.scim_users.retain(|user| {
            if user.id == id {
                if !admin_auth.can_access_scim_user(user) {
                    return true;
                }
                deleted = Some(user.clone());
                false
            } else {
                true
            }
        });
        if store.scim_users.len() == before {
            return Err(RuntimeGatewayAdminError::new(
                404,
                "gateway_scim_user_not_found",
                "gateway SCIM user was not found",
            ));
        }
        Ok(())
    }) {
        Ok(()) => {
            if let Some(user) = deleted {
                runtime_gateway_audit_admin_scim_user_event(
                    shared,
                    "delete_scim_user",
                    "success",
                    &user,
                );
            }
            runtime_gateway_admin_json_response(
                200,
                serde_json::json!({
                    "object": "gateway.scim_user.deleted",
                    "id": id,
                    "deleted": true,
                }),
            )
        }
        Err(response) => response,
    }
}

fn runtime_gateway_admin_key_scope_forbidden_response() -> tiny_http::ResponseBox {
    build_runtime_proxy_json_error_response(
        403,
        "gateway_admin_key_scope_forbidden",
        "gateway admin token is not allowed to access this virtual key",
    )
}

fn runtime_gateway_audit_admin_key_event(
    shared: &RuntimeLocalRewriteProxyShared,
    action: &'static str,
    outcome: &'static str,
    key_name: &str,
    details: serde_json::Value,
) {
    let payload = serde_json::json!({
        "key_name": key_name,
        "state_backend": shared.gateway_state_store.label(),
        "details": details,
    });
    let default_log_dir = shared
        .runtime_shared
        .log_path
        .parent()
        .unwrap_or_else(|| Path::new("."));
    let path = prodex_audit_log::audit_log_path(default_log_dir);
    let _ = prodex_audit_log::append_audit_event(&path, "gateway_admin", action, outcome, payload);
}

fn runtime_gateway_audit_admin_scim_user_event(
    shared: &RuntimeLocalRewriteProxyShared,
    action: &'static str,
    outcome: &'static str,
    user: &RuntimeGatewayScimUser,
) {
    let payload = serde_json::json!({
        "user_id": user.id,
        "user_name": user.user_name,
        "state_backend": shared.gateway_state_store.label(),
        "details": {
            "active": user.active,
            "tenant_id": user.tenant_id,
            "role": user.role,
            "allowed_key_prefixes": user.allowed_key_prefixes,
        },
    });
    let default_log_dir = shared
        .runtime_shared
        .log_path
        .parent()
        .unwrap_or_else(|| Path::new("."));
    let path = prodex_audit_log::audit_log_path(default_log_dir);
    let _ = prodex_audit_log::append_audit_event(&path, "gateway_admin", action, outcome, payload);
}

fn runtime_gateway_scim_user_json(
    user: &RuntimeGatewayScimUser,
    shared: &RuntimeLocalRewriteProxyShared,
) -> serde_json::Value {
    let mount_path = shared.mount_path.trim_end_matches('/');
    let location = format!("{mount_path}/prodex/gateway/scim/v2/Users/{}", user.id);
    let mut payload = serde_json::json!({
        "schemas": [RUNTIME_GATEWAY_SCIM_USER_SCHEMA, RUNTIME_GATEWAY_SCIM_PRODEX_SCHEMA],
        "id": user.id,
        "userName": user.user_name,
        "tenant_id": user.tenant_id,
        "externalId": user.external_id,
        "displayName": user.display_name,
        "active": user.active,
        "meta": {
            "resourceType": "User",
            "location": location,
            "created": user.created_at_epoch,
            "lastModified": user.updated_at_epoch,
        }
    });
    payload[RUNTIME_GATEWAY_SCIM_PRODEX_SCHEMA] = serde_json::json!({
        "tenant_id": user.tenant_id,
        "role": user.role,
        "allowed_key_prefixes": user.allowed_key_prefixes,
    });
    payload
}

fn runtime_gateway_generate_scim_user_id() -> Result<String> {
    let token = runtime_gateway_generate_virtual_key_token()?;
    Ok(format!("user_{}", token.trim_start_matches("pk-")))
}

fn runtime_gateway_apply_scim_user_patch(
    user: &mut RuntimeGatewayScimUser,
    body: &serde_json::Value,
    partial: bool,
) -> Result<(), RuntimeGatewayAdminError> {
    if let Some(operations) = body
        .get("Operations")
        .and_then(serde_json::Value::as_array)
        .or_else(|| body.get("operations").and_then(serde_json::Value::as_array))
    {
        for operation in operations {
            runtime_gateway_apply_scim_operation(user, operation)?;
        }
    } else {
        runtime_gateway_apply_scim_user_fields(user, body, partial)?;
    }
    runtime_gateway_validate_scim_user(user)
}

fn runtime_gateway_apply_scim_operation(
    user: &mut RuntimeGatewayScimUser,
    operation: &serde_json::Value,
) -> Result<(), RuntimeGatewayAdminError> {
    let path = operation
        .get("path")
        .and_then(serde_json::Value::as_str)
        .map(|value| value.to_ascii_lowercase());
    let value = operation.get("value").unwrap_or(&serde_json::Value::Null);
    let op = operation
        .get("op")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("replace")
        .to_ascii_lowercase();
    if !matches!(op.as_str(), "add" | "replace" | "remove") {
        return Err(RuntimeGatewayAdminError::new(
            400,
            "invalid_scim_operation",
            "SCIM operation op must be add, replace, or remove",
        ));
    }
    let Some(path) = path else {
        if value.is_object() {
            return runtime_gateway_apply_scim_user_fields(user, value, true);
        }
        return Err(RuntimeGatewayAdminError::new(
            400,
            "invalid_scim_operation",
            "SCIM operation without path must provide an object value",
        ));
    };
    match path.as_str() {
        "username" | "userName" => {
            user.user_name = runtime_gateway_scim_string_value(value, "userName")?;
        }
        "externalid" => {
            user.external_id = runtime_gateway_scim_optional_string_value(value, "externalId")?;
        }
        "displayname" => {
            user.display_name = runtime_gateway_scim_optional_string_value(value, "displayName")?;
        }
        "active" => {
            user.active = runtime_gateway_scim_bool_value(value, "active")?;
        }
        "role" | "urn:prodex:params:scim:schemas:gateway:2.0:user.role" => {
            user.role = runtime_gateway_scim_optional_role_value(value)?;
        }
        "tenant_id" | "tenantid" | "urn:prodex:params:scim:schemas:gateway:2.0:user.tenant_id" => {
            user.tenant_id = runtime_gateway_scim_optional_string_value(value, "tenant_id")?;
        }
        "allowed_key_prefixes"
        | "allowedkeyprefixes"
        | "urn:prodex:params:scim:schemas:gateway:2.0:user.allowed_key_prefixes" => {
            user.allowed_key_prefixes =
                runtime_gateway_scim_key_prefixes_value(value, "allowed_key_prefixes")?;
        }
        _ => {
            return Err(RuntimeGatewayAdminError::new(
                400,
                "unsupported_scim_path",
                format!("unsupported SCIM path {path}"),
            ));
        }
    }
    Ok(())
}

fn runtime_gateway_apply_scim_user_fields(
    user: &mut RuntimeGatewayScimUser,
    body: &serde_json::Value,
    partial: bool,
) -> Result<(), RuntimeGatewayAdminError> {
    if let Some(value) = body.get("userName").or_else(|| body.get("user_name")) {
        user.user_name = runtime_gateway_scim_string_value(value, "userName")?;
    } else if !partial && user.user_name.trim().is_empty() {
        return Err(RuntimeGatewayAdminError::new(
            400,
            "invalid_scim_user_name",
            "SCIM userName is required",
        ));
    }
    if let Some(value) = body.get("externalId").or_else(|| body.get("external_id")) {
        user.external_id = runtime_gateway_scim_optional_string_value(value, "externalId")?;
    } else if !partial {
        user.external_id = None;
    }
    if let Some(value) = body.get("displayName").or_else(|| body.get("display_name")) {
        user.display_name = runtime_gateway_scim_optional_string_value(value, "displayName")?;
    } else if !partial {
        user.display_name = None;
    }
    if let Some(value) = body.get("active") {
        user.active = runtime_gateway_scim_bool_value(value, "active")?;
    } else if !partial {
        user.active = true;
    }
    if let Some(value) = runtime_gateway_scim_prodex_field(body, "role") {
        user.role = runtime_gateway_scim_optional_role_value(value)?;
    } else if !partial {
        user.role = None;
    }
    if let Some(value) = runtime_gateway_scim_prodex_field(body, "tenant_id")
        .or_else(|| runtime_gateway_scim_prodex_field(body, "tenantId"))
    {
        user.tenant_id = runtime_gateway_scim_optional_string_value(value, "tenant_id")?;
    } else if !partial {
        user.tenant_id = None;
    }
    if let Some(value) = runtime_gateway_scim_prodex_field(body, "allowed_key_prefixes")
        .or_else(|| runtime_gateway_scim_prodex_field(body, "allowedKeyPrefixes"))
    {
        user.allowed_key_prefixes =
            runtime_gateway_scim_key_prefixes_value(value, "allowed_key_prefixes")?;
    } else if !partial {
        user.allowed_key_prefixes = Vec::new();
    }
    Ok(())
}

fn runtime_gateway_scim_prodex_field<'a>(
    body: &'a serde_json::Value,
    field: &str,
) -> Option<&'a serde_json::Value> {
    body.get(field)
        .or_else(|| body.get(RUNTIME_GATEWAY_SCIM_PRODEX_SCHEMA)?.get(field))
}

fn runtime_gateway_scim_string_value(
    value: &serde_json::Value,
    field: &'static str,
) -> Result<String, RuntimeGatewayAdminError> {
    value
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            RuntimeGatewayAdminError::new(
                400,
                "invalid_scim_field",
                format!("{field} must be a non-empty string"),
            )
        })
}

fn runtime_gateway_scim_optional_string_value(
    value: &serde_json::Value,
    field: &'static str,
) -> Result<Option<String>, RuntimeGatewayAdminError> {
    if value.is_null() {
        return Ok(None);
    }
    value
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| Some(value.to_string()))
        .ok_or_else(|| {
            RuntimeGatewayAdminError::new(
                400,
                "invalid_scim_field",
                format!("{field} must be a string or null"),
            )
        })
}

fn runtime_gateway_scim_bool_value(
    value: &serde_json::Value,
    field: &'static str,
) -> Result<bool, RuntimeGatewayAdminError> {
    value.as_bool().ok_or_else(|| {
        RuntimeGatewayAdminError::new(
            400,
            "invalid_scim_field",
            format!("{field} must be a boolean"),
        )
    })
}

fn runtime_gateway_scim_optional_role_value(
    value: &serde_json::Value,
) -> Result<Option<String>, RuntimeGatewayAdminError> {
    if value.is_null() {
        return Ok(None);
    }
    let role = runtime_gateway_scim_string_value(value, "role")?;
    if RuntimeGatewayAdminRole::parse(&role).is_none() {
        return Err(RuntimeGatewayAdminError::new(
            400,
            "invalid_scim_role",
            "role must be admin or viewer",
        ));
    }
    Ok(Some(role.to_ascii_lowercase()))
}

fn runtime_gateway_scim_key_prefixes_value(
    value: &serde_json::Value,
    field: &'static str,
) -> Result<Vec<String>, RuntimeGatewayAdminError> {
    let Some(values) = value.as_array() else {
        return Err(RuntimeGatewayAdminError::new(
            400,
            "invalid_scim_field",
            format!("{field} must be an array of non-empty strings"),
        ));
    };
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| {
            RuntimeGatewayAdminError::new(
                400,
                "invalid_scim_field",
                format!("{field} must be an array of non-empty strings"),
            )
        })
}

fn runtime_gateway_validate_scim_user(
    user: &RuntimeGatewayScimUser,
) -> Result<(), RuntimeGatewayAdminError> {
    let user_name = user.user_name.trim();
    if user_name.is_empty() || user_name.len() > 320 {
        return Err(RuntimeGatewayAdminError::new(
            400,
            "invalid_scim_user_name",
            "SCIM userName must be 1-320 characters",
        ));
    }
    if let Some(role) = user.role.as_deref()
        && RuntimeGatewayAdminRole::parse(role).is_none()
    {
        return Err(RuntimeGatewayAdminError::new(
            400,
            "invalid_scim_role",
            "role must be admin or viewer",
        ));
    }
    Ok(())
}

fn runtime_gateway_admin_key_patch_fields(body: &serde_json::Value) -> Vec<String> {
    [
        "tenant_id",
        "token",
        "rotate",
        "allowed_models",
        "budget_microusd",
        "budget_usd",
        "request_budget",
        "rpm_limit",
        "tpm_limit",
        "disabled",
    ]
    .into_iter()
    .filter(|field| body.get(field).is_some())
    .map(str::to_string)
    .collect()
}

fn runtime_gateway_admin_request_tenant_id(
    body: &serde_json::Value,
    admin_auth: &RuntimeGatewayAdminAuth,
) -> Option<String> {
    runtime_gateway_admin_body_tenant_id(body).unwrap_or_else(|| admin_auth.tenant_id.clone())
}

fn runtime_gateway_admin_body_tenant_id(body: &serde_json::Value) -> Option<Option<String>> {
    body.get("tenant_id").map(|value| {
        value
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

struct RuntimeGatewayAdminError {
    status: u16,
    code: &'static str,
    message: String,
}

impl RuntimeGatewayAdminError {
    fn new(status: u16, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }

    fn into_response(self) -> tiny_http::ResponseBox {
        build_runtime_proxy_json_error_response(self.status, self.code, &self.message)
    }
}

fn runtime_gateway_mutate_admin_key_store<F>(
    shared: &RuntimeLocalRewriteProxyShared,
    mutation: F,
) -> Result<(), tiny_http::ResponseBox>
where
    F: FnOnce(&mut RuntimeGatewayVirtualKeyStoreFile) -> Result<(), RuntimeGatewayAdminError>,
{
    match &shared.gateway_state_store {
        RuntimeGatewayStateStore::File { key_store_path, .. } => {
            runtime_gateway_mutate_admin_key_store_file(shared, key_store_path, mutation)
        }
        RuntimeGatewayStateStore::Sqlite { path } => {
            runtime_gateway_mutate_admin_key_store_sqlite(shared, path, mutation)
        }
        RuntimeGatewayStateStore::Postgres { url, .. } => {
            runtime_gateway_mutate_admin_key_store_postgres(shared, url, mutation)
        }
        RuntimeGatewayStateStore::Redis { url, .. } => {
            runtime_gateway_mutate_admin_key_store_redis(shared, url, mutation)
        }
    }
}

fn runtime_gateway_mutate_admin_key_store_file<F>(
    shared: &RuntimeLocalRewriteProxyShared,
    path: &Path,
    mutation: F,
) -> Result<(), tiny_http::ResponseBox>
where
    F: FnOnce(&mut RuntimeGatewayVirtualKeyStoreFile) -> Result<(), RuntimeGatewayAdminError>,
{
    let lock_path = path.with_extension("json.lock");
    if let Some(parent) = lock_path.parent()
        && let Err(err) = std::fs::create_dir_all(parent)
    {
        return Err(build_runtime_proxy_json_error_response(
            500,
            "gateway_key_store_lock_failed",
            &err.to_string(),
        ));
    }
    let lock_file = match OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
    {
        Ok(file) => file,
        Err(err) => {
            return Err(build_runtime_proxy_json_error_response(
                500,
                "gateway_key_store_lock_failed",
                &err.to_string(),
            ));
        }
    };
    if let Err(err) = lock_file.lock_exclusive() {
        return Err(build_runtime_proxy_json_error_response(
            500,
            "gateway_key_store_lock_failed",
            &err.to_string(),
        ));
    }
    let mut store = match runtime_gateway_virtual_key_store_load_strict_file(path) {
        Ok(store) => store,
        Err(err) => {
            let _ = lock_file.unlock();
            return Err(err.into_response());
        }
    };
    store.version = runtime_gateway_virtual_key_store_version();
    if let Err(err) = mutation(&mut store) {
        let _ = lock_file.unlock();
        return Err(err.into_response());
    }
    store.keys.sort_by(|left, right| left.name.cmp(&right.name));
    store
        .scim_users
        .sort_by(|left, right| left.user_name.cmp(&right.user_name));
    if let Err(err) = runtime_gateway_virtual_key_store_save(path, &store) {
        let _ = lock_file.unlock();
        return Err(build_runtime_proxy_json_error_response(
            500,
            "gateway_key_store_save_failed",
            &err.to_string(),
        ));
    }
    runtime_gateway_apply_admin_virtual_key_store(shared, &store);
    let _ = lock_file.unlock();
    Ok(())
}

fn runtime_gateway_mutate_admin_key_store_sqlite<F>(
    shared: &RuntimeLocalRewriteProxyShared,
    path: &Path,
    mutation: F,
) -> Result<(), tiny_http::ResponseBox>
where
    F: FnOnce(&mut RuntimeGatewayVirtualKeyStoreFile) -> Result<(), RuntimeGatewayAdminError>,
{
    let mut conn = match runtime_gateway_sqlite_open(path) {
        Ok(conn) => conn,
        Err(err) => {
            return Err(build_runtime_proxy_json_error_response(
                500,
                "gateway_key_store_load_failed",
                &err.to_string(),
            ));
        }
    };
    let tx = match conn.transaction_with_behavior(TransactionBehavior::Immediate) {
        Ok(tx) => tx,
        Err(err) => {
            return Err(build_runtime_proxy_json_error_response(
                500,
                "gateway_key_store_lock_failed",
                &err.to_string(),
            ));
        }
    };
    let mut store = match runtime_gateway_sqlite_load_key_store_from_conn(&tx) {
        Ok(store) => store,
        Err(err) => {
            return Err(build_runtime_proxy_json_error_response(
                500,
                "gateway_key_store_load_failed",
                &err.to_string(),
            ));
        }
    };
    store.version = runtime_gateway_virtual_key_store_version();
    if let Err(err) = mutation(&mut store) {
        return Err(err.into_response());
    }
    store.keys.sort_by(|left, right| left.name.cmp(&right.name));
    if let Err(err) = runtime_gateway_sqlite_save_key_store_in_tx(&tx, &store) {
        return Err(build_runtime_proxy_json_error_response(
            500,
            "gateway_key_store_save_failed",
            &err.to_string(),
        ));
    }
    if let Err(err) = tx.commit() {
        return Err(build_runtime_proxy_json_error_response(
            500,
            "gateway_key_store_save_failed",
            &err.to_string(),
        ));
    }
    runtime_gateway_apply_admin_virtual_key_store(shared, &store);
    Ok(())
}

fn runtime_gateway_mutate_admin_key_store_postgres<F>(
    shared: &RuntimeLocalRewriteProxyShared,
    url: &str,
    mutation: F,
) -> Result<(), tiny_http::ResponseBox>
where
    F: FnOnce(&mut RuntimeGatewayVirtualKeyStoreFile) -> Result<(), RuntimeGatewayAdminError>,
{
    let mut client = match runtime_gateway_postgres_open(url) {
        Ok(client) => client,
        Err(err) => {
            return Err(build_runtime_proxy_json_error_response(
                500,
                "gateway_key_store_load_failed",
                &err.to_string(),
            ));
        }
    };
    let mut tx = match client.transaction() {
        Ok(tx) => tx,
        Err(err) => {
            return Err(build_runtime_proxy_json_error_response(
                500,
                "gateway_key_store_lock_failed",
                &err.to_string(),
            ));
        }
    };
    if let Err(err) = tx.batch_execute(
        "LOCK TABLE prodex_gateway_virtual_keys, prodex_gateway_scim_users IN EXCLUSIVE MODE",
    ) {
        return Err(build_runtime_proxy_json_error_response(
            500,
            "gateway_key_store_lock_failed",
            &err.to_string(),
        ));
    }
    let mut store = match runtime_gateway_postgres_load_key_store_from_client(&mut tx) {
        Ok(store) => store,
        Err(err) => {
            return Err(build_runtime_proxy_json_error_response(
                500,
                "gateway_key_store_load_failed",
                &err.to_string(),
            ));
        }
    };
    store.version = runtime_gateway_virtual_key_store_version();
    if let Err(err) = mutation(&mut store) {
        return Err(err.into_response());
    }
    store.keys.sort_by(|left, right| left.name.cmp(&right.name));
    store
        .scim_users
        .sort_by(|left, right| left.user_name.cmp(&right.user_name));
    if let Err(err) = runtime_gateway_postgres_save_key_store_in_tx(&mut tx, &store) {
        return Err(build_runtime_proxy_json_error_response(
            500,
            "gateway_key_store_save_failed",
            &err.to_string(),
        ));
    }
    if let Err(err) = tx.commit() {
        return Err(build_runtime_proxy_json_error_response(
            500,
            "gateway_key_store_save_failed",
            &err.to_string(),
        ));
    }
    runtime_gateway_apply_admin_virtual_key_store(shared, &store);
    Ok(())
}

fn runtime_gateway_mutate_admin_key_store_redis<F>(
    shared: &RuntimeLocalRewriteProxyShared,
    url: &str,
    mutation: F,
) -> Result<(), tiny_http::ResponseBox>
where
    F: FnOnce(&mut RuntimeGatewayVirtualKeyStoreFile) -> Result<(), RuntimeGatewayAdminError>,
{
    let result = runtime_gateway_redis_with_lock(
        url,
        RUNTIME_GATEWAY_REDIS_KEY_STORE_LOCK,
        |conn| -> Result<Result<RuntimeGatewayVirtualKeyStoreFile, RuntimeGatewayAdminError>> {
            let payload: Option<String> = conn.get(RUNTIME_GATEWAY_REDIS_KEY_STORE_KEY)?;
            let mut store = match payload {
                Some(payload) => {
                    serde_json::from_str::<RuntimeGatewayVirtualKeyStoreFile>(&payload)
                        .context("failed to parse gateway redis virtual key store")?
                }
                None => RuntimeGatewayVirtualKeyStoreFile::default(),
            };
            store.version = runtime_gateway_virtual_key_store_version();
            if let Err(err) = mutation(&mut store) {
                return Ok(Err(err));
            }
            store.keys.sort_by(|left, right| left.name.cmp(&right.name));
            store
                .scim_users
                .sort_by(|left, right| left.user_name.cmp(&right.user_name));
            runtime_gateway_redis_save_key_store(conn, &store)?;
            Ok(Ok(store))
        },
    );
    match result {
        Ok(Ok(store)) => {
            runtime_gateway_apply_admin_virtual_key_store(shared, &store);
            Ok(())
        }
        Ok(Err(err)) => Err(err.into_response()),
        Err(err) => Err(build_runtime_proxy_json_error_response(
            500,
            "gateway_key_store_save_failed",
            &err.to_string(),
        )),
    }
}

fn runtime_gateway_apply_admin_virtual_key_store(
    shared: &RuntimeLocalRewriteProxyShared,
    store: &RuntimeGatewayVirtualKeyStoreFile,
) {
    let Ok(mut entries) = shared.gateway_virtual_keys.lock() else {
        return;
    };
    let mut next = entries
        .iter()
        .filter(|entry| entry.source == RuntimeGatewayVirtualKeySource::Policy)
        .cloned()
        .collect::<Vec<_>>();
    let mut seen = next
        .iter()
        .map(|entry| entry.key.name.to_ascii_lowercase())
        .collect::<Vec<_>>();
    for record in &store.keys {
        if seen
            .iter()
            .any(|seen| seen == &record.name.to_ascii_lowercase())
        {
            continue;
        }
        if let Some(entry) = runtime_gateway_virtual_key_entry_from_stored(record) {
            seen.push(entry.key.name.to_ascii_lowercase());
            next.push(entry);
        }
    }
    *entries = next;
}

fn runtime_gateway_virtual_key_store_load_strict_file(
    path: &Path,
) -> Result<RuntimeGatewayVirtualKeyStoreFile, RuntimeGatewayAdminError> {
    match std::fs::read(path) {
        Ok(bytes) => {
            serde_json::from_slice::<RuntimeGatewayVirtualKeyStoreFile>(&bytes).map_err(|err| {
                RuntimeGatewayAdminError::new(500, "gateway_key_store_invalid", err.to_string())
            })
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            Ok(RuntimeGatewayVirtualKeyStoreFile::default())
        }
        Err(err) => Err(RuntimeGatewayAdminError::new(
            500,
            "gateway_key_store_load_failed",
            err.to_string(),
        )),
    }
}

fn runtime_gateway_admin_json_body(
    captured: &RuntimeProxyRequest,
) -> Result<serde_json::Value, tiny_http::ResponseBox> {
    serde_json::from_slice::<serde_json::Value>(&captured.body).map_err(|err| {
        build_runtime_proxy_json_error_response(400, "invalid_json", &err.to_string())
    })
}

fn runtime_gateway_validate_virtual_key_name(name: &str) -> Result<(), &'static str> {
    let valid = !name.is_empty()
        && name.len() <= 128
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'));
    if valid {
        Ok(())
    } else {
        Err("gateway virtual key name must use 1-128 ASCII letters, numbers, '.', '-', or '_'")
    }
}

fn runtime_gateway_virtual_key_name_exists(
    shared: &RuntimeLocalRewriteProxyShared,
    name: &str,
) -> bool {
    runtime_gateway_virtual_key_entry_by_name(shared, name).is_some()
}

fn runtime_gateway_virtual_key_entry_by_name(
    shared: &RuntimeLocalRewriteProxyShared,
    name: &str,
) -> Option<RuntimeGatewayVirtualKeyEntry> {
    shared
        .gateway_virtual_keys
        .lock()
        .ok()?
        .iter()
        .find(|entry| entry.key.name.eq_ignore_ascii_case(name))
        .cloned()
}

fn runtime_gateway_generate_virtual_key_token() -> Result<String> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).context("failed to generate gateway virtual key")?;
    Ok(format!(
        "pk-{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
    ))
}

fn runtime_gateway_apply_virtual_key_patch(
    record: &mut RuntimeGatewayStoredVirtualKey,
    body: &serde_json::Value,
    partial: bool,
) -> Result<(), RuntimeGatewayAdminError> {
    if let Some(value) = body.get("tenant_id") {
        record.tenant_id = if value.is_null() {
            None
        } else {
            Some(runtime_gateway_scim_string_value(value, "tenant_id")?)
        };
    } else if !partial {
        record.tenant_id = None;
    }

    if let Some(models) = body.get("allowed_models") {
        let Some(values) = models.as_array() else {
            return Err(RuntimeGatewayAdminError::new(
                400,
                "invalid_allowed_models",
                "allowed_models must be an array of strings",
            ));
        };
        record.allowed_models = values
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
            })
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| {
                RuntimeGatewayAdminError::new(
                    400,
                    "invalid_allowed_models",
                    "allowed_models must be an array of strings",
                )
            })?
            .into_iter()
            .map(str::to_string)
            .collect();
    } else if !partial {
        record.allowed_models = Vec::new();
    }

    if let Some(value) = body.get("budget_microusd") {
        record.budget_microusd = runtime_gateway_optional_u64(value, "budget_microusd")?;
    } else if let Some(value) = body.get("budget_usd") {
        record.budget_microusd = runtime_gateway_optional_f64_microusd(value, "budget_usd")?;
    } else if !partial {
        record.budget_microusd = None;
    }
    for (field, target) in [
        ("request_budget", &mut record.request_budget),
        ("rpm_limit", &mut record.rpm_limit),
        ("tpm_limit", &mut record.tpm_limit),
    ] {
        if let Some(value) = body.get(field) {
            *target = runtime_gateway_optional_u64(value, field)?;
        } else if !partial {
            *target = None;
        }
    }
    if let Some(value) = body.get("disabled") {
        let Some(disabled) = value.as_bool() else {
            return Err(RuntimeGatewayAdminError::new(
                400,
                "invalid_disabled",
                "disabled must be a boolean",
            ));
        };
        record.disabled = Some(disabled);
    } else if !partial {
        record.disabled = Some(false);
    }
    Ok(())
}

fn runtime_gateway_optional_u64(
    value: &serde_json::Value,
    field: &'static str,
) -> Result<Option<u64>, RuntimeGatewayAdminError> {
    if value.is_null() {
        return Ok(None);
    }
    value.as_u64().map(Some).ok_or_else(|| {
        RuntimeGatewayAdminError::new(
            400,
            "invalid_numeric_limit",
            format!("{field} must be a u64 or null"),
        )
    })
}

fn runtime_gateway_optional_f64_microusd(
    value: &serde_json::Value,
    field: &'static str,
) -> Result<Option<u64>, RuntimeGatewayAdminError> {
    if value.is_null() {
        return Ok(None);
    }
    value
        .as_f64()
        .filter(|value| value.is_finite() && *value >= 0.0)
        .map(|value| Some((value * 1_000_000.0).round().clamp(1.0, u64::MAX as f64) as u64))
        .ok_or_else(|| {
            RuntimeGatewayAdminError::new(
                400,
                "invalid_numeric_limit",
                format!("{field} must be a non-negative number or null"),
            )
        })
}

fn runtime_gateway_admin_stored_key_json(
    record: &RuntimeGatewayStoredVirtualKey,
) -> serde_json::Value {
    serde_json::json!({
        "name": record.name,
        "tenant_id": record.tenant_id,
        "source": "admin",
        "disabled": record.disabled.unwrap_or(false),
        "editable": true,
        "created_at_epoch": record.created_at_epoch,
        "updated_at_epoch": record.updated_at_epoch,
        "allowed_models": record.allowed_models,
        "budget_microusd": record.budget_microusd,
        "budget_usd": record.budget_microusd.map(microusd_to_usd),
        "request_budget": record.request_budget,
        "rpm_limit": record.rpm_limit,
        "tpm_limit": record.tpm_limit,
    })
}

fn runtime_gateway_unix_epoch_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn runtime_gateway_prometheus_response(
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
) -> tiny_http::ResponseBox {
    let usage = shared
        .gateway_virtual_key_usage
        .lock()
        .map(|usage| usage.clone())
        .unwrap_or_default();
    let entries = shared
        .gateway_virtual_keys
        .lock()
        .map(|entries| entries.clone())
        .unwrap_or_default();
    let mut rows = BTreeMap::new();
    for entry in entries {
        if !admin_auth.can_access_entry(&entry) {
            continue;
        }
        let usage = usage.get(&entry.key.name).cloned().unwrap_or_default();
        rows.insert(
            entry.key.name.clone(),
            (entry.source.as_str().to_string(), entry.disabled, usage),
        );
    }
    for (name, usage) in usage {
        if admin_auth.tenant_id.is_some() || !admin_auth.can_access_key(&name) {
            continue;
        }
        rows.entry(name)
            .or_insert_with(|| ("unknown".to_string(), false, usage));
    }

    let mut body = String::new();
    body.push_str(
        "# HELP prodex_gateway_virtual_key_requests_total Total accepted gateway requests by virtual key.\n",
    );
    body.push_str("# TYPE prodex_gateway_virtual_key_requests_total counter\n");
    for (name, (source, disabled, usage)) in &rows {
        let labels = runtime_gateway_prometheus_key_labels(name, source, *disabled);
        body.push_str(&format!(
            "prodex_gateway_virtual_key_requests_total{{{labels}}} {}\n",
            usage.requests_total
        ));
    }
    body.push_str(
        "# HELP prodex_gateway_virtual_key_spend_microusd_total Estimated gateway spend by virtual key in micro-USD.\n",
    );
    body.push_str("# TYPE prodex_gateway_virtual_key_spend_microusd_total counter\n");
    for (name, (source, disabled, usage)) in &rows {
        let labels = runtime_gateway_prometheus_key_labels(name, source, *disabled);
        body.push_str(&format!(
            "prodex_gateway_virtual_key_spend_microusd_total{{{labels}}} {}\n",
            usage.spend_microusd
        ));
    }
    body.push_str(
        "# HELP prodex_gateway_virtual_key_minute_requests Current minute accepted gateway requests by virtual key.\n",
    );
    body.push_str("# TYPE prodex_gateway_virtual_key_minute_requests gauge\n");
    for (name, (source, disabled, usage)) in &rows {
        let labels = runtime_gateway_prometheus_key_labels(name, source, *disabled);
        body.push_str(&format!(
            "prodex_gateway_virtual_key_minute_requests{{{labels},minute_epoch=\"{}\"}} {}\n",
            usage.minute_epoch, usage.requests_this_minute
        ));
    }
    body.push_str(
        "# HELP prodex_gateway_virtual_key_minute_tokens Current minute estimated input tokens by virtual key.\n",
    );
    body.push_str("# TYPE prodex_gateway_virtual_key_minute_tokens gauge\n");
    for (name, (source, disabled, usage)) in &rows {
        let labels = runtime_gateway_prometheus_key_labels(name, source, *disabled);
        body.push_str(&format!(
            "prodex_gateway_virtual_key_minute_tokens{{{labels},minute_epoch=\"{}\"}} {}\n",
            usage.minute_epoch, usage.tokens_this_minute
        ));
    }

    build_runtime_proxy_response_from_parts(RuntimeHeapTrimmedBufferedResponseParts {
        status: 200,
        headers: vec![(
            "content-type".to_string(),
            b"text/plain; version=0.0.4; charset=utf-8".to_vec(),
        )],
        body: body.into_bytes().into(),
    })
}

fn runtime_gateway_prometheus_key_labels(name: &str, source: &str, disabled: bool) -> String {
    format!(
        "key=\"{}\",source=\"{}\",disabled=\"{}\"",
        runtime_gateway_prometheus_label_escape(name),
        runtime_gateway_prometheus_label_escape(source),
        disabled
    )
}

fn runtime_gateway_prometheus_label_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn runtime_gateway_admin_json_response(
    status: u16,
    value: serde_json::Value,
) -> tiny_http::ResponseBox {
    let body = serde_json::to_vec_pretty(&value).unwrap_or_else(|_| b"{}".to_vec());
    build_runtime_proxy_response_from_parts(RuntimeHeapTrimmedBufferedResponseParts {
        status,
        headers: vec![(
            "content-type".to_string(),
            b"application/json; charset=utf-8".to_vec(),
        )],
        body: body.into(),
    })
}

fn runtime_gateway_admin_csv_response(body: String) -> tiny_http::ResponseBox {
    build_runtime_proxy_response_from_parts(RuntimeHeapTrimmedBufferedResponseParts {
        status: 200,
        headers: vec![
            (
                "content-type".to_string(),
                b"text/csv; charset=utf-8".to_vec(),
            ),
            ("cache-control".to_string(), b"no-store".to_vec()),
        ],
        body: body.into_bytes().into(),
    })
}

fn runtime_gateway_admin_dashboard_response(
    shared: &RuntimeLocalRewriteProxyShared,
) -> tiny_http::ResponseBox {
    let html = runtime_gateway_admin_dashboard_html(shared);
    build_runtime_proxy_response_from_parts(RuntimeHeapTrimmedBufferedResponseParts {
        status: 200,
        headers: vec![
            (
                "content-type".to_string(),
                b"text/html; charset=utf-8".to_vec(),
            ),
            ("cache-control".to_string(), b"no-store".to_vec()),
        ],
        body: html.into_bytes().into(),
    })
}

fn runtime_gateway_admin_dashboard_html(shared: &RuntimeLocalRewriteProxyShared) -> String {
    let admin_prefix = format!("{}/prodex/gateway", shared.mount_path.trim_end_matches('/'));
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Prodex Gateway Admin</title>
<style>
:root{{color-scheme:light dark;--bg:#f5f7fa;--fg:#172033;--muted:#667085;--line:#d7dde7;--panel:#fff;--accent:#0f766e;--danger:#b42318;--warn:#b45309}}
@media (prefers-color-scheme:dark){{:root{{--bg:#101214;--fg:#eef2f7;--muted:#9aa4b2;--line:#2b333d;--panel:#171a1f;--accent:#2dd4bf;--danger:#fb7185;--warn:#f59e0b}}}}
*{{box-sizing:border-box}}body{{margin:0;background:var(--bg);color:var(--fg);font:14px/1.45 system-ui,-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif}}
header{{padding:18px 22px;border-bottom:1px solid var(--line);display:flex;align-items:center;justify-content:space-between;gap:14px;flex-wrap:wrap}}
h1{{font-size:22px;margin:0}}main{{max-width:1320px;margin:0 auto;padding:18px 22px}}.muted{{color:var(--muted)}}.toolbar{{display:flex;gap:8px;align-items:center;flex-wrap:wrap}}
button,input,select{{border:1px solid var(--line);border-radius:6px;background:var(--panel);color:var(--fg);padding:8px 10px}}button{{cursor:pointer}}button.primary{{background:var(--accent);border-color:var(--accent);color:#fff}}button.danger{{color:var(--danger)}}button:disabled{{opacity:.55;cursor:not-allowed}}
.grid{{display:grid;grid-template-columns:repeat(5,minmax(0,1fr));gap:12px;margin-bottom:16px}}.stat,.panel{{background:var(--panel);border:1px solid var(--line);border-radius:8px}}.stat{{padding:13px 14px}}.stat b{{display:block;font-size:24px}}.panel{{margin:14px 0;overflow:hidden}}.panel h2{{font-size:15px;margin:0;padding:13px 15px;border-bottom:1px solid var(--line)}}.pad{{padding:13px 15px}}
table{{width:100%;border-collapse:collapse}}th,td{{padding:9px 11px;text-align:left;border-bottom:1px solid var(--line);vertical-align:top}}th{{font-size:12px;color:var(--muted);font-weight:600}}tr:last-child td{{border-bottom:0}}.actions{{display:flex;gap:6px;flex-wrap:wrap}}.pill{{display:inline-block;border:1px solid var(--line);border-radius:999px;padding:2px 7px;font-size:12px}}.ok{{color:var(--accent)}}.bad{{color:var(--danger)}}.warn{{color:var(--warn)}}pre{{margin:0;white-space:pre-wrap;word-break:break-word;font:12px/1.45 ui-monospace,SFMono-Regular,Menlo,monospace}}
dialog{{border:1px solid var(--line);border-radius:8px;background:var(--panel);color:var(--fg);width:min(560px,calc(100vw - 28px))}}dialog::backdrop{{background:rgba(0,0,0,.38)}}form{{display:grid;gap:10px}}label{{display:grid;gap:4px;color:var(--muted)}}label span{{font-size:12px}}.wide{{width:100%}}
@media (max-width:900px){{.grid{{grid-template-columns:repeat(2,minmax(0,1fr))}}th:nth-child(4),td:nth-child(4),th:nth-child(5),td:nth-child(5){{display:none}}}}
@media (max-width:560px){{main{{padding:12px}}.grid{{grid-template-columns:1fr}}table{{font-size:12px}}th,td{{padding:8px}}}}
</style>
</head>
<body>
<header><div><h1>Prodex Gateway Admin</h1><div class="muted">Keys, SSO users, usage, ledger, metrics</div></div><div class="toolbar"><input id="token" type="password" placeholder="Admin bearer token"><button id="saveToken">Use token</button><button id="refresh" class="primary">Refresh</button><span id="status" class="muted"></span></div></header>
<main>
<section class="grid"><div class="stat"><span class="muted">Keys</span><b id="keyCount">-</b></div><div class="stat"><span class="muted">Users</span><b id="userCount">-</b></div><div class="stat"><span class="muted">Requests</span><b id="requestCount">-</b></div><div class="stat"><span class="muted">Spend</span><b id="spend">-</b></div><div class="stat"><span class="muted">Ledger</span><b id="ledgerCount">-</b></div></section>
<section class="panel"><h2>Virtual Keys</h2><div class="pad toolbar"><button id="newKey" class="primary">New key</button><span id="generatedToken" class="muted"></span></div><table><thead><tr><th>Name</th><th>Tenant</th><th>Source</th><th>Models</th><th>Budget</th><th>Usage</th><th>Actions</th></tr></thead><tbody id="keys"></tbody></table></section>
<section class="panel"><h2>SSO Users</h2><div class="pad toolbar"><button id="newUser" class="primary">New user</button></div><table><thead><tr><th>User</th><th>Tenant</th><th>Name</th><th>Role</th><th>Prefixes</th><th>Status</th><th>Actions</th></tr></thead><tbody id="users"></tbody></table></section>
<section class="panel"><h2>Billing Summary</h2><div class="pad toolbar"><button id="exportSummary">Export summary CSV</button></div><table><thead><tr><th>Key</th><th>Model</th><th>Requests</th><th>Success</th><th>Input</th><th>Output</th><th>Final cost</th></tr></thead><tbody id="summary"></tbody></table></section>
<section class="panel"><h2>Billing Ledger</h2><div class="pad toolbar"><button id="exportLedger">Export ledger CSV</button></div><table><thead><tr><th>Call</th><th>Key</th><th>Model</th><th>Status</th><th>Input</th><th>Output</th><th>Final cost</th></tr></thead><tbody id="ledger"></tbody></table></section>
<section class="panel"><h2>Metrics</h2><div class="pad"><pre id="metrics">-</pre></div></section>
</main>
<dialog id="keyDialog"><form method="dialog"><h2 id="dialogTitle">New key</h2><label><span>Name</span><input id="keyName" class="wide"></label><label><span>Allowed models</span><input id="models" class="wide" placeholder="gpt-5.4, prodex-fast"></label><label><span>Request budget</span><input id="requestBudget" class="wide" type="number" min="0"></label><label><span>RPM limit</span><input id="rpm" class="wide" type="number" min="0"></label><label><span>TPM limit</span><input id="tpm" class="wide" type="number" min="0"></label><div class="toolbar"><button id="saveKey" class="primary" value="default">Save</button><button value="cancel">Cancel</button></div></form></dialog>
<dialog id="userDialog"><form method="dialog"><h2 id="userDialogTitle">New user</h2><label><span>User name</span><input id="userName" class="wide" placeholder="alice@example.com"></label><label><span>Display name</span><input id="displayName" class="wide"></label><label><span>Role</span><select id="userRole" class="wide"><option value="viewer">viewer</option><option value="admin">admin</option></select></label><label><span>Key prefixes</span><input id="keyPrefixes" class="wide" placeholder="team-a-, team-b-"></label><label><span>Status</span><select id="userActive" class="wide"><option value="true">active</option><option value="false">inactive</option></select></label><div class="toolbar"><button id="saveUser" class="primary" value="default">Save</button><button value="cancel">Cancel</button></div></form></dialog>
<script>
const base = {admin_prefix:?};
const $ = (id) => document.getElementById(id);
let editing = null;
let editingUser = null;
function token(){{return $("token").value.trim() || sessionStorage.getItem("prodex_gateway_admin_token") || ""}}
function headers(extra={{}}){{const h={{...extra}};const t=token();if(t)h.authorization=`Bearer ${{t}}`;return h}}
async function api(path, options={{}}){{const r=await fetch(base+path,{{...options,headers:headers(options.headers||{{}})}});const text=await r.text();const body=text && (r.headers.get("content-type")||"").includes("json") ? JSON.parse(text) : text;if(!r.ok)throw new Error((body.error&&body.error.message)||text||r.statusText);return body}}
function money(v){{return v==null ? "-" : "$"+Number(v).toFixed(6)}}
function int(v){{return v==null ? "-" : String(v)}}
function setStatus(text, cls="muted"){{$("status").textContent=text;$("status").className=cls}}
function td(text, cls){{const e=document.createElement("td");e.textContent=text ?? "-";if(cls)e.className=cls;return e}}
function button(text, cls, fn){{const b=document.createElement("button");b.textContent=text;if(cls)b.className=cls;b.onclick=fn;return b}}
async function refresh(){{setStatus("Loading...");try{{const [keys, users, ledger, summary, metrics]=await Promise.all([api("/keys"),api("/scim/v2/Users"),api("/ledger"),api("/ledger/summary"),api("/metrics",{{headers:{{accept:"text/plain"}}}})]);renderKeys(keys.keys||[]);renderUsers(users.Resources||[]);renderSummary(summary);renderLedger(ledger.records||[]);$("metrics").textContent=metrics.split("\\n").slice(0,24).join("\\n");setStatus("Updated "+new Date().toLocaleTimeString(),"ok")}}catch(err){{setStatus(err.message,"bad")}}}}
function renderKeys(rows){{$("keyCount").textContent=rows.length;$("requestCount").textContent=rows.reduce((n,k)=>n+(k.usage?.requests_total||0),0);$("spend").textContent=money(rows.reduce((n,k)=>n+(k.usage?.spend_usd||0),0));const body=$("keys");body.replaceChildren();for(const key of rows){{const tr=document.createElement("tr");tr.append(td(key.name),td(key.tenant_id||"*"),td(key.source+(key.disabled?" disabled":"")),td((key.allowed_models||[]).join(", ")||"*"),td([money(key.budget_usd),int(key.request_budget)+" req",int(key.rpm_limit)+" rpm"].join(" / ")),td(`${{int(key.usage?.requests_total)}} req / ${{money(key.usage?.spend_usd)}}`));const actions=td("");actions.className="actions";actions.append(button("Edit","",()=>openKey(key)),button("Rotate","",()=>rotateKey(key)),button(key.disabled?"Enable":"Disable","",()=>patchKey(key.name,{{disabled:!key.disabled}})),button("Delete","danger",()=>deleteKey(key)));if(!key.editable)for(const b of actions.querySelectorAll("button"))b.disabled=true;tr.append(actions);body.append(tr)}}}}
function userExt(user){{return user["urn:prodex:params:scim:schemas:gateway:2.0:User"]||{{}}}}
function renderUsers(rows){{$("userCount").textContent=rows.length;const body=$("users");body.replaceChildren();for(const user of rows){{const ext=userExt(user);const tr=document.createElement("tr");tr.append(td(user.userName),td(user.tenant_id||ext.tenant_id||"*"),td(user.displayName),td(ext.role||"-"),td((ext.allowed_key_prefixes||[]).join(", ")||"*"),td(user.active?"active":"inactive",user.active?"ok":"bad"));const actions=td("");actions.className="actions";actions.append(button("Edit","",()=>openUser(user)),button(user.active?"Deactivate":"Activate","",()=>patchUser(user.id,{{active:!user.active}})),button("Delete","danger",()=>deleteUser(user)));tr.append(actions);body.append(tr)}}}}
function renderSummary(summary){{const totals=summary.totals||{{}};$("requestCount").textContent=int(totals.requests);$("spend").textContent=money(totals.final_cost_usd||totals.estimated_cost_usd);const body=$("summary");body.replaceChildren();for(const row of (summary.by_key_model||[])){{const tr=document.createElement("tr");tr.append(td(row.key_name),td(row.model),td(int(row.requests)),td(`${{int(row.successful_requests)}} / ${{int(row.failed_requests)}} fail`),td(int(row.input_tokens)),td(int(row.output_tokens)),td(money(row.final_cost_usd||row.estimated_cost_usd)));body.append(tr)}}}}
function renderLedger(rows){{$("ledgerCount").textContent=rows.length;const body=$("ledger");body.replaceChildren();for(const row of rows.slice().reverse().slice(0,50)){{const tr=document.createElement("tr");tr.append(td(row.call_id),td(row.key_name),td(row.model),td(int(row.response_status)),td(int(row.input_tokens)),td(int(row.output_tokens)),td(money(row.final_cost_usd ?? row.estimated_cost_usd)));body.append(tr)}}}}
function openKey(key=null){{editing=key;$("dialogTitle").textContent=key?"Edit key":"New key";$("keyName").value=key?.name||"";$("keyName").disabled=!!key;$("models").value=(key?.allowed_models||[]).join(", ");$("requestBudget").value=key?.request_budget??"";$("rpm").value=key?.rpm_limit??"";$("tpm").value=key?.tpm_limit??"";$("keyDialog").showModal()}}
function openUser(user=null){{editingUser=user;const ext=user?userExt(user):{{}};$("userDialogTitle").textContent=user?"Edit user":"New user";$("userName").value=user?.userName||"";$("userName").disabled=!!user;$("displayName").value=user?.displayName||"";$("userRole").value=ext.role||"viewer";$("keyPrefixes").value=(ext.allowed_key_prefixes||[]).join(", ");$("userActive").value=String(user?.active ?? true);$("userDialog").showModal()}}
function payload(){{const models=$("models").value.split(",").map(v=>v.trim()).filter(Boolean);const body={{allowed_models:models,request_budget:num("requestBudget"),rpm_limit:num("rpm"),tpm_limit:num("tpm")}};if(!editing)body.name=$("keyName").value.trim();return body}}
function num(id){{const v=$(id).value.trim();return v===""?null:Number(v)}}
function userPayload(){{const prefixes=$("keyPrefixes").value.split(",").map(v=>v.trim()).filter(Boolean);const body={{userName:$("userName").value.trim(),displayName:$("displayName").value.trim()||null,active:$("userActive").value==="true"}};body["urn:prodex:params:scim:schemas:gateway:2.0:User"]={{role:$("userRole").value,allowed_key_prefixes:prefixes}};return body}}
async function saveKey(ev){{ev.preventDefault();const body=payload();const result=editing?await api("/keys/"+encodeURIComponent(editing.name),{{method:"PATCH",headers:{{"content-type":"application/json"}},body:JSON.stringify(body)}}):await api("/keys",{{method:"POST",headers:{{"content-type":"application/json"}},body:JSON.stringify(body)}});$("generatedToken").textContent=result.token?`Generated token: ${{result.token}}`:"";$("keyDialog").close();refresh()}}
async function saveUser(ev){{ev.preventDefault();const body=userPayload();const result=editingUser?await api("/scim/v2/Users/"+encodeURIComponent(editingUser.id),{{method:"PATCH",headers:{{"content-type":"application/json"}},body:JSON.stringify(body)}}):await api("/scim/v2/Users",{{method:"POST",headers:{{"content-type":"application/json"}},body:JSON.stringify(body)}});$("userDialog").close();refresh()}}
async function patchKey(name, body){{await api("/keys/"+encodeURIComponent(name),{{method:"PATCH",headers:{{"content-type":"application/json"}},body:JSON.stringify(body)}});refresh()}}
async function patchUser(id, body){{await api("/scim/v2/Users/"+encodeURIComponent(id),{{method:"PATCH",headers:{{"content-type":"application/json"}},body:JSON.stringify(body)}});refresh()}}
async function rotateKey(key){{const result=await api("/keys/"+encodeURIComponent(key.name),{{method:"PATCH",headers:{{"content-type":"application/json"}},body:JSON.stringify({{rotate:true}})}});$("generatedToken").textContent=result.token?`Generated token: ${{result.token}}`:"";refresh()}}
async function deleteKey(key){{if(confirm("Delete "+key.name+"?")){{await api("/keys/"+encodeURIComponent(key.name),{{method:"DELETE"}});refresh()}}}}
async function deleteUser(user){{if(confirm("Delete "+user.userName+"?")){{await api("/scim/v2/Users/"+encodeURIComponent(user.id),{{method:"DELETE"}});refresh()}}}}
async function downloadCsv(path, filename){{const r=await fetch(base+path,{{headers:headers({{accept:"text/csv"}})}});if(!r.ok)throw new Error(await r.text());const blob=await r.blob();const url=URL.createObjectURL(blob);const a=document.createElement("a");a.href=url;a.download=filename;document.body.append(a);a.click();a.remove();URL.revokeObjectURL(url)}}
$("saveToken").onclick=()=>{{sessionStorage.setItem("prodex_gateway_admin_token",$("token").value.trim());refresh()}};$("refresh").onclick=refresh;$("newKey").onclick=()=>openKey();$("newUser").onclick=()=>openUser();$("saveKey").onclick=saveKey;$("saveUser").onclick=saveUser;$("exportLedger").onclick=()=>downloadCsv("/ledger.csv","prodex-gateway-ledger.csv").catch(err=>setStatus(err.message,"bad"));$("exportSummary").onclick=()=>downloadCsv("/ledger/summary.csv","prodex-gateway-billing-summary.csv").catch(err=>setStatus(err.message,"bad"));$("token").value=sessionStorage.getItem("prodex_gateway_admin_token")||"";refresh();
</script>
</body>
</html>"#,
        admin_prefix = admin_prefix,
    )
}

fn runtime_local_rewrite_request_is_authorized(
    request: &tiny_http::Request,
    auth_token_hash: &runtime_proxy_crate::LocalBridgeBearerTokenHash,
) -> bool {
    let path = path_without_query(request.url());
    if path == runtime_proxy_crate::LOCAL_BRIDGE_HEALTH_PATH
        || path.ends_with("/prodex/gateway/admin")
    {
        return true;
    }
    request.headers().iter().any(|header| {
        header.field.equiv("Authorization")
            && auth_token_hash.verify_authorization_header(header.value.as_str())
    })
}

pub(super) struct RuntimeGatewayGuardrailWebhookBlock {
    pub(super) reason: String,
    pub(super) value: String,
}

pub(super) fn runtime_gateway_guardrail_webhook_block(
    phase: &str,
    request_id: u64,
    body: &[u8],
    shared: &RuntimeLocalRewriteProxyShared,
) -> Option<RuntimeGatewayGuardrailWebhookBlock> {
    if !shared.gateway_guardrail_webhook.enabled_for(phase) {
        return None;
    }
    let url = shared.gateway_guardrail_webhook.url.as_deref()?;
    let payload = serde_json::json!({
        "phase": phase,
        "request_id": request_id,
        "call_id": format!("prodex-{request_id}"),
        "body_base64": base64::engine::general_purpose::STANDARD.encode(body),
    });
    let mut request = shared.client.post(url).json(&payload);
    if let Some(token) = shared.gateway_guardrail_webhook.bearer_token.as_deref() {
        request = request.bearer_auth(token);
    }
    let response = match request.send() {
        Ok(response) => response,
        Err(err) => {
            runtime_proxy_log(
                &shared.runtime_shared,
                runtime_proxy_structured_log_message(
                    "gateway_guardrail_webhook_failed",
                    [
                        runtime_proxy_log_field("request", request_id.to_string()),
                        runtime_proxy_log_field("phase", phase),
                        runtime_proxy_log_field("url", url),
                        runtime_proxy_log_field("error", format!("{err:#}")),
                    ],
                ),
            );
            return shared.gateway_guardrail_webhook.fail_closed.then(|| {
                RuntimeGatewayGuardrailWebhookBlock {
                    reason: "webhook_error".to_string(),
                    value: err.to_string(),
                }
            });
        }
    };
    let status = response.status();
    let body = response.bytes().unwrap_or_default();
    if !status.is_success() {
        return shared.gateway_guardrail_webhook.fail_closed.then(|| {
            RuntimeGatewayGuardrailWebhookBlock {
                reason: "webhook_status".to_string(),
                value: status.as_u16().to_string(),
            }
        });
    }
    let value = serde_json::from_slice::<serde_json::Value>(&body).ok()?;
    let allow = value
        .get("allow")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);
    (!allow).then(|| RuntimeGatewayGuardrailWebhookBlock {
        reason: value
            .get("reason")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("webhook_denied")
            .to_string(),
        value: value
            .get("message")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("blocked")
            .to_string(),
    })
}

fn respond_runtime_gemini_compact_request(
    request_id: u64,
    request: tiny_http::Request,
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    auth: &RuntimeGeminiProviderAuth,
) {
    let semantic = runtime_gemini_semantic_compact_request_body(&captured.body).and_then(|body| {
        let semantic_request = RuntimeProxyRequest {
            method: captured.method.clone(),
            path_and_query: format!("{}/responses", shared.mount_path.trim_end_matches('/')),
            headers: captured.headers.clone(),
            body: body.clone(),
        };
        let result = send_runtime_gemini_upstream_request(
            request_id,
            &semantic_request,
            shared,
            body,
            auth,
        )?;
        let RuntimeLocalRewriteUpstreamResult {
            response,
            gemini_context,
            ..
        } = result;
        let profile_name = gemini_context
            .as_ref()
            .map(|context| context.profile_name.as_str())
            .unwrap_or(RUNTIME_LOCAL_REWRITE_PROFILE);
        let parts = match response {
            RuntimeLocalRewriteUpstreamResponse::Live(live) if live.prefix.is_empty() => {
                let status = live.response.status().as_u16();
                runtime_gemini_semantic_compact_response_parts(
                    status,
                    live.response,
                    request_id,
                    &captured.body,
                )?
            }
            RuntimeLocalRewriteUpstreamResponse::Live(_) => {
                anyhow::bail!("Gemini semantic compact unexpectedly returned a stream prefix")
            }
            RuntimeLocalRewriteUpstreamResponse::Buffered(parts) => {
                anyhow::bail!(
                    "Gemini semantic compact returned buffered HTTP {}",
                    parts.status
                )
            }
        };
        Ok((parts, profile_name.to_string()))
    });

    let parts = match semantic {
        Ok((parts, profile_name)) => {
            runtime_proxy_log(
                &shared.runtime_shared,
                runtime_proxy_structured_log_message(
                    "local_rewrite_gemini_compact_semantic",
                    [
                        runtime_proxy_log_field("request", request_id.to_string()),
                        runtime_proxy_log_field("transport", "http"),
                        runtime_proxy_log_field("profile", profile_name),
                        runtime_proxy_log_field(
                            "path",
                            path_without_query(&captured.path_and_query),
                        ),
                        runtime_proxy_log_field("body_bytes", captured.body.len().to_string()),
                    ],
                ),
            );
            parts
        }
        Err(err) => {
            runtime_proxy_log(
                &shared.runtime_shared,
                runtime_proxy_structured_log_message(
                    "local_rewrite_gemini_compact_fallback",
                    [
                        runtime_proxy_log_field("request", request_id.to_string()),
                        runtime_proxy_log_field("transport", "http"),
                        runtime_proxy_log_field(
                            "path",
                            path_without_query(&captured.path_and_query),
                        ),
                        runtime_proxy_log_field("body_bytes", captured.body.len().to_string()),
                        runtime_proxy_log_field("reason", format!("{err:#}")),
                    ],
                ),
            );
            runtime_gemini_local_compact_response_parts(&captured.body)
        }
    };
    let _ = request.respond(runtime_local_rewrite_response_with_call_id(
        parts, request_id, shared,
    ));
}

pub(super) struct RuntimeLocalRewriteUpstreamResult {
    pub(super) response: RuntimeLocalRewriteUpstreamResponse,
    pub(super) gemini_context: Option<RuntimeGeminiRequestContext>,
    pub(super) copilot_context: Option<RuntimeCopilotRequestContext>,
}

pub(super) enum RuntimeLocalRewriteUpstreamResponse {
    Live(RuntimeLocalRewriteLiveResponse),
    Buffered(RuntimeHeapTrimmedBufferedResponseParts),
}

pub(super) struct RuntimeLocalRewriteLiveResponse {
    pub(super) response: reqwest::blocking::Response,
    pub(super) prefix: Vec<u8>,
}

impl RuntimeLocalRewriteLiveResponse {
    pub(super) fn new(response: reqwest::blocking::Response) -> Self {
        Self {
            response,
            prefix: Vec::new(),
        }
    }

    pub(super) fn with_prefix(response: reqwest::blocking::Response, prefix: Vec<u8>) -> Self {
        Self { response, prefix }
    }
}

fn send_runtime_local_rewrite_upstream_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Result<RuntimeLocalRewriteUpstreamResult> {
    let route_kind = runtime_local_rewrite_route_kind(&request.path_and_query);
    let body = prepare_runtime_smart_context_http_body(
        request_id,
        request,
        &shared.runtime_shared,
        route_kind,
    )
    .into_owned();
    match &shared.provider {
        RuntimeLocalRewriteProviderOptions::Anthropic { auth } => {
            let auth_attempts = runtime_local_rewrite_anthropic_auth_attempts(shared, auth);
            if auth_attempts.is_empty() {
                anyhow::bail!("Anthropic provider has no auth configured");
            }
            let auth_attempt_count = auth_attempts.len();
            if path_without_query(&request.path_and_query).ends_with("/responses") {
                let model_selection = runtime_local_rewrite_model_selection(
                    shared,
                    RuntimeProviderBridgeKind::Anthropic,
                    request,
                    &body,
                    prodex_cli::SUPER_ANTHROPIC_DEFAULT_MODEL,
                );
                let model_chain = runtime_provider_model_fallback_chain(
                    RuntimeProviderBridgeKind::Anthropic,
                    &model_selection.model,
                );
                let upstream_url = runtime_openai_standard_provider_upstream_url(
                    RuntimeProviderBridgeKind::Anthropic,
                    &shared.upstream_base_url,
                    &shared.mount_path,
                    &request.path_and_query,
                );
                for (auth_index, selected_auth) in auth_attempts.into_iter().enumerate() {
                    for (model_index, model) in model_chain.iter().enumerate() {
                        let model_body =
                            runtime_provider_request_body_with_model(&model_selection.body, model);
                        let translated = runtime_chat_compatible_request_body(
                            &model_body,
                            &shared.deepseek_conversations,
                            RuntimeProviderBridgeKind::Anthropic,
                            prodex_cli::SUPER_ANTHROPIC_DEFAULT_MODEL,
                            false,
                        )?;
                        if let Ok(mut pending) = shared.deepseek_pending_messages.lock() {
                            pending.insert(request_id, translated.messages);
                        }
                        let send_result =
                            send_runtime_local_rewrite_prepared_request_with_chat_search_fallback(
                                RuntimeLocalRewriteSearchFallbackRequest {
                                    request_id,
                                    request,
                                    shared,
                                    upstream_url: &upstream_url,
                                    body: translated.body,
                                    provider_kind: RuntimeProviderBridgeKind::Anthropic,
                                    auth_label: selected_auth.label.as_str(),
                                    model,
                                    auth_factory: || RuntimeLocalRewritePreparedAuth::Anthropic {
                                        auth: &selected_auth.auth,
                                    },
                                },
                            )?;
                        let (status, parts, class) = match send_result {
                            RuntimeLocalRewritePreparedSendResult::Live(response) => {
                                return Ok(RuntimeLocalRewriteUpstreamResult {
                                    response: RuntimeLocalRewriteUpstreamResponse::Live(
                                        RuntimeLocalRewriteLiveResponse::new(response),
                                    ),
                                    gemini_context: None,
                                    copilot_context: None,
                                });
                            }
                            RuntimeLocalRewritePreparedSendResult::Error {
                                status,
                                parts,
                                class,
                            } => (status, parts, class),
                        };
                        if model_index + 1 < model_chain.len()
                            && runtime_provider_should_retry_with_next_model(class)
                        {
                            runtime_proxy_log(
                                &shared.runtime_shared,
                                runtime_proxy_structured_log_message(
                                    "local_rewrite_provider_model_fallback",
                                    [
                                        runtime_proxy_log_field("request", request_id.to_string()),
                                        runtime_proxy_log_field("provider", "anthropic"),
                                        runtime_proxy_log_field(
                                            "auth",
                                            selected_auth.label.as_str(),
                                        ),
                                        runtime_proxy_log_field("from_model", model.as_str()),
                                        runtime_proxy_log_field(
                                            "to_model",
                                            model_chain[model_index + 1].as_str(),
                                        ),
                                        runtime_proxy_log_field("status", status.to_string()),
                                        runtime_proxy_log_field("class", format!("{class:?}")),
                                    ],
                                ),
                            );
                            continue;
                        }
                        if auth_index + 1 < auth_attempt_count
                            && runtime_provider_should_rotate_auth_after_response(class)
                        {
                            runtime_proxy_log(
                                &shared.runtime_shared,
                                runtime_proxy_structured_log_message(
                                    "local_rewrite_provider_auth_rotate",
                                    [
                                        runtime_proxy_log_field("request", request_id.to_string()),
                                        runtime_proxy_log_field("provider", "anthropic"),
                                        runtime_proxy_log_field(
                                            "auth",
                                            selected_auth.label.as_str(),
                                        ),
                                        runtime_proxy_log_field("status", status.to_string()),
                                        runtime_proxy_log_field("class", format!("{class:?}")),
                                    ],
                                ),
                            );
                            break;
                        }
                        return Ok(RuntimeLocalRewriteUpstreamResult {
                            response: RuntimeLocalRewriteUpstreamResponse::Buffered(parts),
                            gemini_context: None,
                            copilot_context: None,
                        });
                    }
                    if auth_index + 1 < auth_attempt_count {
                        continue;
                    }
                }
                anyhow::bail!("no Anthropic model attempts were available");
            } else {
                let upstream_url = runtime_local_rewrite_upstream_url(
                    &shared.upstream_base_url,
                    &shared.mount_path,
                    &request.path_and_query,
                );
                for (auth_index, selected_auth) in auth_attempts.into_iter().enumerate() {
                    let response = send_runtime_local_rewrite_prepared_request(
                        request_id,
                        request,
                        shared,
                        &upstream_url,
                        body.clone(),
                        RuntimeLocalRewritePreparedAuth::Anthropic {
                            auth: &selected_auth.auth,
                        },
                    )?;
                    let status = response.status().as_u16();
                    if status >= 400 {
                        let parts =
                            runtime_local_rewrite_buffered_response_from_response(response)?;
                        let class = runtime_provider_error_class(
                            RuntimeProviderBridgeKind::Anthropic,
                            status,
                            &parts.body,
                        );
                        if auth_index + 1 < auth_attempt_count
                            && runtime_provider_should_rotate_auth_after_response(class)
                        {
                            runtime_proxy_log(
                                &shared.runtime_shared,
                                runtime_proxy_structured_log_message(
                                    "local_rewrite_provider_auth_rotate",
                                    [
                                        runtime_proxy_log_field("request", request_id.to_string()),
                                        runtime_proxy_log_field("provider", "anthropic"),
                                        runtime_proxy_log_field(
                                            "auth",
                                            selected_auth.label.as_str(),
                                        ),
                                        runtime_proxy_log_field("status", status.to_string()),
                                        runtime_proxy_log_field("class", format!("{class:?}")),
                                    ],
                                ),
                            );
                            continue;
                        }
                        return Ok(RuntimeLocalRewriteUpstreamResult {
                            response: RuntimeLocalRewriteUpstreamResponse::Buffered(parts),
                            gemini_context: None,
                            copilot_context: None,
                        });
                    }
                    return Ok(RuntimeLocalRewriteUpstreamResult {
                        response: RuntimeLocalRewriteUpstreamResponse::Live(
                            RuntimeLocalRewriteLiveResponse::new(response),
                        ),
                        gemini_context: None,
                        copilot_context: None,
                    });
                }
                anyhow::bail!("no Anthropic auth attempts were available")
            }
        }
        RuntimeLocalRewriteProviderOptions::Copilot { auth } => {
            send_runtime_copilot_upstream_request(request_id, request, shared, body, auth)
        }
        RuntimeLocalRewriteProviderOptions::OpenAiResponses { api_keys } => {
            let upstream_url = runtime_local_rewrite_upstream_url(
                &shared.upstream_base_url,
                &shared.mount_path,
                &request.path_and_query,
            );
            let body = if path_without_query(&request.path_and_query).ends_with("/responses") {
                runtime_local_rewrite_model_selection(
                    shared,
                    RuntimeProviderBridgeKind::OpenAiResponses,
                    request,
                    &body,
                    "",
                )
                .body
            } else {
                body
            };
            let auth_attempts = runtime_local_rewrite_api_key_attempts(shared, api_keys);
            let selected_api_key = auth_attempts.first().map(|(_, api_key)| *api_key);
            let response = send_runtime_local_rewrite_prepared_request(
                request_id,
                request,
                shared,
                &upstream_url,
                body,
                RuntimeLocalRewritePreparedAuth::OpenAiResponses {
                    api_key: selected_api_key,
                },
            )?;
            Ok(RuntimeLocalRewriteUpstreamResult {
                response: RuntimeLocalRewriteUpstreamResponse::Live(
                    RuntimeLocalRewriteLiveResponse::new(response),
                ),
                gemini_context: None,
                copilot_context: None,
            })
        }
        RuntimeLocalRewriteProviderOptions::DeepSeek { api_keys } => {
            send_runtime_deepseek_upstream_request(request_id, request, shared, body, api_keys)
        }
        RuntimeLocalRewriteProviderOptions::Gemini { auth, .. } => {
            send_runtime_gemini_upstream_request(request_id, request, shared, body, auth)
        }
    }
}

fn runtime_local_rewrite_route_kind(path_and_query: &str) -> RuntimeRouteKind {
    let path = path_without_query(path_and_query);
    if path.ends_with("/responses") || path.ends_with("/chat/completions") {
        RuntimeRouteKind::Responses
    } else if path.ends_with("/responses/compact") {
        RuntimeRouteKind::Compact
    } else {
        runtime_proxy_request_lane(path_and_query, false)
    }
}
