use super::*;
use std::collections::{BTreeMap, BTreeSet};

#[allow(clippy::too_many_arguments)]
pub(super) fn build_runtime_local_rewrite_proxy_shared(
    runtime_shared: RuntimeRotationProxyShared,
    upstream_base_url: String,
    provider: RuntimeLocalRewriteProviderOptions,
    resolved_harness: prodex_provider_core::ResolvedHarnessMode,
    gemini_oauth_pool: Option<RuntimeGeminiOAuthPool>,
    copilot_oauth_pool: Option<RuntimeCopilotOAuthPool>,
    gateway_auth_token_hash: Option<runtime_proxy_crate::LocalBridgeBearerTokenHash>,
    gateway_admin_tokens: Vec<RuntimeGatewayAdminToken>,
    gateway_sso: RuntimeGatewaySsoConfig,
    gateway_state_store: RuntimeGatewayStateStore,
    gateway_virtual_key_entries: Vec<RuntimeGatewayVirtualKeyEntry>,
    gateway_virtual_key_store_path: PathBuf,
    gateway_virtual_key_usage: BTreeMap<String, runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage>,
    gateway_virtual_key_usage_path: PathBuf,
    gateway_route_aliases: Vec<runtime_proxy_crate::RuntimeGatewayRouteAlias>,
    gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig,
    gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig,
    gateway_call_id_header: Option<String>,
    gateway_observability: RuntimeGatewayObservabilityConfig,
) -> Result<RuntimeLocalRewriteProxyShared> {
    let (provider, provider_credential) = provider.into_runtime_parts();
    let allow_governance_fallback = !runtime_shared.runtime_config.governance.mode.is_enforcing();
    let governed_provider_registry = Arc::new(ArcSwap::from_pointee(
        super::local_rewrite_provider_registry::RuntimeGatewayProviderRegistrySnapshotSet::bootstrap(
            super::local_rewrite_provider_registry::runtime_gateway_bootstrap_provider_registry_snapshot(
        &runtime_shared.runtime_config.governance_policy,
        &provider,
        provider_credential.as_ref(),
            )?,
            allow_governance_fallback,
        ),
    ));
    let governed_routing_scores = Arc::new(ArcSwap::from_pointee(
        super::local_rewrite_provider_registry::RuntimeGatewayRoutingScoresSnapshotSet::bootstrap(
            super::local_rewrite_provider_registry::runtime_gateway_bootstrap_routing_scores_snapshot(
                &runtime_shared.runtime_config.governance_policy,
            ),
            allow_governance_fallback,
        ),
    ));
    let classification_rules = Arc::new(ArcSwap::from_pointee(
        super::local_rewrite_classification_rules::RuntimeClassificationRulesSnapshotSet::bootstrap(
            &runtime_shared.runtime_config.governance_policy,
            allow_governance_fallback,
        )?,
    ));
    let bootstrap = crate::runtime_governance::compile_runtime_governance_settings(
        &runtime_shared.runtime_config.governance_policy,
    )?;
    let governance_snapshot = Arc::new(ArcSwap::from_pointee(
        crate::runtime_governance::RuntimeGovernanceAuthoritySnapshotSet::bootstrap(
            bootstrap,
            !runtime_shared.runtime_config.governance.mode.is_enforcing(),
        ),
    ));
    Ok(RuntimeLocalRewriteProxyShared {
        runtime_shared,
        upstream_base_url,
        mount_path: RUNTIME_LOCAL_REWRITE_PROXY_MOUNT_PATH.to_string(),
        provider,
        provider_credential,
        resolved_harness,
        deepseek_conversations: Arc::new(Mutex::new(BTreeMap::new())),
        deepseek_pending_messages: Arc::new(Mutex::new(BTreeMap::new())),
        gemini_conversations: Arc::new(Mutex::new(BTreeMap::new())),
        gemini_oauth_pool,
        copilot_oauth_pool,
        model_memory: Arc::new(Mutex::new(RuntimeLocalRewriteModelMemoryState::default())),
        governance_sessions: Default::default(),
        governed_provider_registry,
        governed_routing_scores,
        classification_rules,
        governance_snapshot,
        governance_authority: None,
        api_key_cursor: Arc::new(AtomicUsize::new(0)),
        client: build_runtime_local_rewrite_http_client()?,
        gateway_auth_token_hash,
        gateway_admin_tokens,
        gateway_sso,
        gateway_state_store,
        gateway_virtual_keys: Arc::new(Mutex::new(gateway_virtual_key_entries)),
        gateway_virtual_key_store_path,
        gateway_usage: RuntimeGatewayVirtualKeyUsageState {
            usage: Arc::new(Mutex::new(gateway_virtual_key_usage)),
            path: Some(gateway_virtual_key_usage_path),
            save_in_flight: Arc::new(AtomicBool::new(false)),
            save_dirty: Arc::new(AtomicBool::new(false)),
            pending_deltas: Arc::new(Mutex::new(Vec::new())),
            request_ids: Arc::new(Mutex::new(BTreeSet::new())),
        },
        gateway_route_aliases,
        gateway_route_load: Arc::new(Mutex::new(BTreeMap::new())),
        gateway_guardrails,
        gateway_guardrail_webhook,
        gateway_call_id_header,
        gateway_observability,
    })
}
