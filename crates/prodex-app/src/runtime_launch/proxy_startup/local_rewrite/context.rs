use super::super::deepseek_rewrite::{
    RuntimeDeepSeekConversationStore, RuntimeDeepSeekPendingMessages,
};
use super::super::local_rewrite_classification_rules::RuntimeClassificationRulesSnapshotSet;
use super::super::local_rewrite_copilot::RuntimeCopilotOAuthPool;
use super::super::local_rewrite_gateway_admin_auth::RuntimeGatewayOidcJwksSnapshot;
use super::super::local_rewrite_gateway_config::{
    RuntimeGatewayAdminToken, RuntimeGatewayGuardrailWebhookConfig,
    RuntimeGatewayObservabilityConfig, RuntimeGatewaySsoConfig, RuntimeGatewayStateStore,
};
use super::super::local_rewrite_gateway_credentials::{
    RuntimeGatewayCredentialSnapshot, RuntimeGatewayCredentialState,
};
use super::super::local_rewrite_gateway_store_types::RuntimeGatewayVirtualKeyEntry;
use super::super::local_rewrite_gemini::RuntimeGeminiOAuthPool;
use super::super::local_rewrite_governance_audit::RuntimeGovernanceAuditWriter;
use super::super::local_rewrite_governance_session::RuntimeGatewayGovernanceSessionStore;
use super::super::local_rewrite_options::{
    RuntimeLocalRewriteProviderOptions, RuntimeProjectedProviderCredential,
};
use super::super::local_rewrite_provider_registry::{
    RuntimeGatewayProviderRegistrySnapshotSet, RuntimeGatewayRoutingScoresSnapshotSet,
};
use super::{
    RuntimeGatewayOidcHttpCacheEntry, RuntimeGatewayRouteLoadState,
    RuntimeGatewayVirtualKeyUsageState, RuntimeGovernanceAuthority, RuntimeLocalRewriteModelMemory,
};
use crate::runtime_state_shared::RuntimeRotationProxyShared;
use arc_swap::{ArcSwap, ArcSwapOption};
use std::collections::BTreeMap;
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::{Arc, Mutex};

pub(in super::super) struct RuntimeLocalRewriteProcessServices {
    pub(in super::super) runtime_shared: RuntimeRotationProxyShared,
    pub(in super::super) mount_path: String,
    pub(in super::super) resolved_harness: prodex_provider_core::ResolvedHarnessMode,
    pub(in super::super) deepseek_conversations: RuntimeDeepSeekConversationStore,
    pub(in super::super) deepseek_pending_messages: RuntimeDeepSeekPendingMessages,
    pub(in super::super) gemini_conversations: RuntimeDeepSeekConversationStore,
    pub(in super::super) gemini_oauth_pool: Option<RuntimeGeminiOAuthPool>,
    pub(in super::super) copilot_oauth_pool: Option<RuntimeCopilotOAuthPool>,
    pub(in super::super) model_memory: RuntimeLocalRewriteModelMemory,
    pub(in super::super) governance_sessions: RuntimeGatewayGovernanceSessionStore,
    pub(in super::super) governance_audit_writer: RuntimeGovernanceAuditWriter,
    pub(in super::super) governed_provider_registry:
        Arc<ArcSwap<RuntimeGatewayProviderRegistrySnapshotSet>>,
    pub(in super::super) governed_routing_scores:
        Arc<ArcSwap<RuntimeGatewayRoutingScoresSnapshotSet>>,
    pub(in super::super) classification_rules: Arc<ArcSwap<RuntimeClassificationRulesSnapshotSet>>,
    pub(in super::super) governance_snapshot:
        Arc<ArcSwap<crate::runtime_governance::RuntimeGovernanceAuthoritySnapshotSet>>,
    pub(in super::super) governance_authority: Option<RuntimeGovernanceAuthority>,
    pub(in super::super) api_key_cursor: Arc<AtomicUsize>,
    pub(in super::super) client: reqwest::blocking::Client,
    pub(in super::super) gateway_oidc_http_cache:
        Arc<Mutex<BTreeMap<String, RuntimeGatewayOidcHttpCacheEntry>>>,
    pub(in super::super) gateway_oidc_jwks_snapshot:
        Arc<ArcSwapOption<RuntimeGatewayOidcJwksSnapshot>>,
    pub(in super::super) gateway_credentials: RuntimeGatewayCredentialState,
    pub(in super::super) gateway_state_store: RuntimeGatewayStateStore,
    pub(in super::super) gateway_postgres_repository:
        Option<prodex_storage_postgres_runtime::PostgresRepository>,
    pub(in super::super) gateway_redis_rate_limit_executor:
        Option<prodex_storage_redis_runtime::RedisRateLimitExecutor>,
    pub(in super::super) gateway_policy_version: Option<u32>,
    pub(in super::super) gateway_virtual_key_store_path: PathBuf,
    pub(in super::super) gateway_usage: RuntimeGatewayVirtualKeyUsageState,
    pub(in super::super) gateway_route_aliases: Vec<runtime_proxy_crate::RuntimeGatewayRouteAlias>,
    pub(in super::super) gateway_request_constraints:
        prodex_provider_core::ProviderRequestConstraintPolicy,
    pub(in super::super) gateway_route_load: RuntimeGatewayRouteLoadState,
    pub(in super::super) gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig,
    pub(in super::super) gateway_call_id_header: Option<String>,
    pub(in super::super) gateway_observability_slots: Arc<tokio::sync::Semaphore>,
    pub(in super::super) gateway_background_task_count: Arc<AtomicUsize>,
    pub(in super::super) allow_local_file_access: bool,
    pub(in super::super) gateway_draining: Arc<AtomicBool>,
}

#[derive(Clone)]
pub(in super::super) struct RuntimeLocalRewriteRequestContext {
    pub(in super::super) process: Arc<RuntimeLocalRewriteProcessServices>,
    pub(in super::super) upstream_base_url: String,
    pub(in super::super) provider: RuntimeLocalRewriteProviderOptions,
    pub(in super::super) provider_credential: Option<RuntimeProjectedProviderCredential>,
    pub(in super::super) gateway_auth_token_hash:
        Option<runtime_proxy_crate::LocalBridgeBearerTokenHash>,
    pub(in super::super) gateway_admin_tokens: Vec<RuntimeGatewayAdminToken>,
    pub(in super::super) gateway_sso: RuntimeGatewaySsoConfig,
    pub(in super::super) gateway_virtual_keys: Arc<Mutex<Vec<RuntimeGatewayVirtualKeyEntry>>>,
    pub(in super::super) gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig,
    pub(in super::super) gateway_observability: RuntimeGatewayObservabilityConfig,
}

pub(in super::super) type RuntimeLocalRewriteProxyShared = RuntimeLocalRewriteRequestContext;

impl Deref for RuntimeLocalRewriteRequestContext {
    type Target = RuntimeLocalRewriteProcessServices;

    fn deref(&self) -> &Self::Target {
        self.process.as_ref()
    }
}

impl RuntimeLocalRewriteRequestContext {
    pub(in super::super) fn with_request_credentials(
        &self,
        snapshot: &RuntimeGatewayCredentialSnapshot,
    ) -> Self {
        Self {
            process: Arc::clone(&self.process),
            upstream_base_url: self.upstream_base_url.clone(),
            provider: snapshot.provider.clone(),
            provider_credential: snapshot.provider_credential.clone(),
            gateway_auth_token_hash: snapshot.auth_token_hash.clone(),
            gateway_admin_tokens: snapshot.admin_tokens.clone(),
            gateway_sso: snapshot.sso.clone(),
            gateway_virtual_keys: Arc::clone(&snapshot.virtual_keys),
            gateway_guardrail_webhook: snapshot.guardrail_webhook.clone(),
            gateway_observability: snapshot.observability.clone(),
        }
    }

    pub(in super::super) fn with_selected_upstream(
        &self,
        provider: RuntimeLocalRewriteProviderOptions,
        provider_credential: RuntimeProjectedProviderCredential,
        upstream_base_url: String,
    ) -> Self {
        Self {
            process: Arc::clone(&self.process),
            upstream_base_url,
            provider,
            provider_credential: Some(provider_credential),
            gateway_auth_token_hash: self.gateway_auth_token_hash.clone(),
            gateway_admin_tokens: self.gateway_admin_tokens.clone(),
            gateway_sso: self.gateway_sso.clone(),
            gateway_virtual_keys: Arc::clone(&self.gateway_virtual_keys),
            gateway_guardrail_webhook: self.gateway_guardrail_webhook.clone(),
            gateway_observability: self.gateway_observability.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn selected_upstream_reuses_process_services_without_mutation() {
        let source = include_str!("context.rs");
        let method = source
            .split("fn with_selected_upstream")
            .nth(1)
            .unwrap()
            .split("\n    }\n}")
            .next()
            .unwrap();

        assert!(method.contains("process: Arc::clone(&self.process)"));
        assert_eq!(method.matches("process:").count(), 1);
        assert!(!method.contains("self.process."));
    }
}
