use super::local_rewrite::{RuntimeLocalRewriteProviderOptions, RuntimeLocalRewriteProxyShared};
use super::local_rewrite_gateway_config::{
    RuntimeGatewayAdminToken, RuntimeGatewayGuardrailWebhookConfig,
    RuntimeGatewayObservabilityConfig, RuntimeGatewaySsoConfig,
};
use super::local_rewrite_gateway_store_types::{
    RuntimeGatewayVirtualKeyEntry, RuntimeGatewayVirtualKeySource,
};
use super::provider_bridge::RuntimeProviderBridgeKind;
use crate::{runtime_proxy_log, runtime_proxy_log_field, runtime_proxy_structured_log_message};
use anyhow::{Result, bail};
use arc_swap::ArcSwap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const GATEWAY_SECRET_REFRESH_INTERVAL: Duration = Duration::from_secs(5);
const GATEWAY_SECRET_REFRESH_SHUTDOWN_POLL: Duration = Duration::from_millis(100);

#[derive(Clone)]
pub(crate) struct RuntimeGatewayCredentialRefreshCandidate {
    pub(crate) fingerprint: [u8; 32],
    pub(crate) provider: RuntimeLocalRewriteProviderOptions,
    pub(crate) auth_token_hash: Option<runtime_proxy_crate::LocalBridgeBearerTokenHash>,
    pub(crate) admin_tokens: Vec<RuntimeGatewayAdminToken>,
    pub(crate) sso: RuntimeGatewaySsoConfig,
    pub(crate) virtual_keys: Vec<runtime_proxy_crate::RuntimeGatewayVirtualKey>,
    pub(crate) guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig,
    pub(crate) observability: RuntimeGatewayObservabilityConfig,
}

pub(crate) type RuntimeGatewayCredentialResolver =
    Arc<dyn Fn() -> Result<RuntimeGatewayCredentialRefreshCandidate> + Send + Sync>;

#[derive(Clone)]
pub(crate) struct RuntimeGatewayCredentialRefreshPlan {
    pub(crate) initial_fingerprint: [u8; 32],
    resolver: RuntimeGatewayCredentialResolver,
    interval: Duration,
}

impl RuntimeGatewayCredentialRefreshPlan {
    pub(crate) fn new(
        initial_fingerprint: [u8; 32],
        resolver: RuntimeGatewayCredentialResolver,
    ) -> Self {
        Self {
            initial_fingerprint,
            resolver,
            interval: GATEWAY_SECRET_REFRESH_INTERVAL,
        }
    }
}

pub(super) struct RuntimeGatewayCredentialSnapshot {
    pub(super) fingerprint: [u8; 32],
    pub(super) provider: RuntimeLocalRewriteProviderOptions,
    pub(super) auth_token_hash: Option<runtime_proxy_crate::LocalBridgeBearerTokenHash>,
    pub(super) admin_tokens: Vec<RuntimeGatewayAdminToken>,
    pub(super) sso: RuntimeGatewaySsoConfig,
    pub(super) virtual_keys: Arc<Mutex<Vec<RuntimeGatewayVirtualKeyEntry>>>,
    pub(super) guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig,
    pub(super) observability: RuntimeGatewayObservabilityConfig,
}

#[derive(Clone)]
pub(super) struct RuntimeGatewayCredentialState {
    pub(super) current: Arc<ArcSwap<RuntimeGatewayCredentialSnapshot>>,
    pub(super) update: Arc<Mutex<()>>,
}

impl RuntimeGatewayCredentialState {
    pub(super) fn new(snapshot: RuntimeGatewayCredentialSnapshot) -> Self {
        Self {
            current: Arc::new(ArcSwap::from_pointee(snapshot)),
            update: Arc::new(Mutex::new(())),
        }
    }
}

pub(super) fn runtime_gateway_initial_credential_snapshot(
    candidate: RuntimeGatewayCredentialRefreshCandidate,
    virtual_keys: Arc<Mutex<Vec<RuntimeGatewayVirtualKeyEntry>>>,
) -> RuntimeGatewayCredentialSnapshot {
    RuntimeGatewayCredentialSnapshot {
        fingerprint: candidate.fingerprint,
        provider: candidate.provider,
        auth_token_hash: candidate.auth_token_hash,
        admin_tokens: candidate.admin_tokens,
        sso: candidate.sso,
        virtual_keys,
        guardrail_webhook: candidate.guardrail_webhook,
        observability: candidate.observability,
    }
}

pub(super) fn runtime_gateway_pin_request_credentials(
    shared: &RuntimeLocalRewriteProxyShared,
) -> RuntimeLocalRewriteProxyShared {
    let snapshot = shared.gateway_credentials.current.load_full();
    let mut pinned = shared.clone();
    pinned.provider = snapshot.provider.clone();
    pinned.gateway_auth_token_hash = snapshot.auth_token_hash.clone();
    pinned.gateway_admin_tokens = snapshot.admin_tokens.clone();
    pinned.gateway_sso = snapshot.sso.clone();
    pinned.gateway_virtual_keys = Arc::clone(&snapshot.virtual_keys);
    pinned.gateway_guardrail_webhook = snapshot.guardrail_webhook.clone();
    pinned.gateway_observability = snapshot.observability.clone();
    pinned
}

pub(super) fn runtime_gateway_spawn_secret_refresh(
    shared: RuntimeLocalRewriteProxyShared,
    shutdown: Arc<AtomicBool>,
    plan: RuntimeGatewayCredentialRefreshPlan,
) -> thread::JoinHandle<()> {
    thread::spawn(move || runtime_gateway_run_secret_refresh_loop(shared, shutdown, plan))
}

fn runtime_gateway_run_secret_refresh_loop(
    shared: RuntimeLocalRewriteProxyShared,
    shutdown: Arc<AtomicBool>,
    plan: RuntimeGatewayCredentialRefreshPlan,
) {
    while runtime_gateway_wait_for_secret_refresh(&shutdown, plan.interval) {
        let candidate = match (plan.resolver)() {
            Ok(candidate) => candidate,
            Err(_) => {
                runtime_gateway_log_secret_refresh(&shared, "resolution_failed");
                continue;
            }
        };
        match runtime_gateway_apply_secret_refresh(
            &shared.gateway_credentials,
            shared.provider.bridge_kind(),
            candidate,
        ) {
            Ok(true) => runtime_gateway_log_secret_refresh(&shared, "applied"),
            Ok(false) => {}
            Err(_) => runtime_gateway_log_secret_refresh(&shared, "validation_failed"),
        }
    }
}

fn runtime_gateway_wait_for_secret_refresh(shutdown: &AtomicBool, interval: Duration) -> bool {
    let mut remaining = interval;
    while !remaining.is_zero() {
        if shutdown.load(Ordering::SeqCst) {
            return false;
        }
        let sleep = remaining.min(GATEWAY_SECRET_REFRESH_SHUTDOWN_POLL);
        thread::sleep(sleep);
        remaining = remaining.saturating_sub(sleep);
    }
    !shutdown.load(Ordering::SeqCst)
}

fn runtime_gateway_apply_secret_refresh(
    credentials: &RuntimeGatewayCredentialState,
    expected_provider: RuntimeProviderBridgeKind,
    candidate: RuntimeGatewayCredentialRefreshCandidate,
) -> Result<bool> {
    if candidate.provider.bridge_kind() != expected_provider {
        bail!("gateway provider kind cannot change during secret refresh");
    }
    if credentials.current.load().fingerprint == candidate.fingerprint {
        return Ok(false);
    }

    let _update = credentials
        .update
        .lock()
        .map_err(|_| anyhow::anyhow!("gateway credential update lock is unavailable"))?;
    let current = credentials.current.load_full();
    if current.fingerprint == candidate.fingerprint {
        return Ok(false);
    }
    let current_keys = current
        .virtual_keys
        .lock()
        .map_err(|_| anyhow::anyhow!("gateway virtual key state is unavailable"))?;
    let virtual_keys = runtime_gateway_merge_refreshed_virtual_keys(
        candidate.virtual_keys,
        current_keys
            .iter()
            .filter(|entry| entry.source == RuntimeGatewayVirtualKeySource::Admin),
    );
    drop(current_keys);

    credentials
        .current
        .store(Arc::new(RuntimeGatewayCredentialSnapshot {
            fingerprint: candidate.fingerprint,
            provider: candidate.provider,
            auth_token_hash: candidate.auth_token_hash,
            admin_tokens: candidate.admin_tokens,
            sso: candidate.sso,
            virtual_keys: Arc::new(Mutex::new(virtual_keys)),
            guardrail_webhook: candidate.guardrail_webhook,
            observability: candidate.observability,
        }));
    Ok(true)
}

fn runtime_gateway_merge_refreshed_virtual_keys<'a>(
    policy_keys: Vec<runtime_proxy_crate::RuntimeGatewayVirtualKey>,
    admin_keys: impl Iterator<Item = &'a RuntimeGatewayVirtualKeyEntry>,
) -> Vec<RuntimeGatewayVirtualKeyEntry> {
    let mut entries = policy_keys
        .into_iter()
        .map(|key| RuntimeGatewayVirtualKeyEntry {
            virtual_key_id: None,
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
    for entry in admin_keys {
        let normalized = entry.key.name.to_ascii_lowercase();
        if !seen.iter().any(|name| name == &normalized) {
            seen.push(normalized);
            entries.push(entry.clone());
        }
    }
    entries
}

fn runtime_gateway_log_secret_refresh(shared: &RuntimeLocalRewriteProxyShared, outcome: &str) {
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "gateway_secret_refresh",
            [runtime_proxy_log_field("outcome", outcome)],
        ),
    );
}

#[cfg(test)]
mod tests {
    use super::{
        RuntimeGatewayCredentialRefreshCandidate, RuntimeGatewayCredentialState,
        runtime_gateway_apply_secret_refresh, runtime_gateway_initial_credential_snapshot,
    };
    use crate::runtime_launch::proxy_startup::deepseek_rewrite::RuntimeDeepSeekWebSearchMode;
    use crate::runtime_launch::proxy_startup::local_rewrite::RuntimeLocalRewriteProviderOptions;
    use crate::runtime_launch::proxy_startup::local_rewrite_gateway_config::{
        RuntimeGatewayGuardrailWebhookConfig, RuntimeGatewayObservabilityConfig,
        RuntimeGatewaySsoConfig,
    };
    use crate::runtime_launch::proxy_startup::local_rewrite_gateway_store_types::{
        RuntimeGatewayVirtualKeyEntry, RuntimeGatewayVirtualKeySource,
    };
    use crate::runtime_launch::proxy_startup::provider_bridge::RuntimeProviderBridgeKind;
    use std::sync::{Arc, Mutex};

    fn openai_provider(api_key: &str) -> RuntimeLocalRewriteProviderOptions {
        RuntimeLocalRewriteProviderOptions::OpenAiResponses {
            api_keys: vec![api_key.to_string()],
        }
    }

    fn candidate(fingerprint: u8, api_key: &str) -> RuntimeGatewayCredentialRefreshCandidate {
        RuntimeGatewayCredentialRefreshCandidate {
            fingerprint: [fingerprint; 32],
            provider: openai_provider(api_key),
            auth_token_hash: None,
            admin_tokens: Vec::new(),
            sso: RuntimeGatewaySsoConfig::default(),
            virtual_keys: Vec::new(),
            guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig {
                bearer_token: Some(api_key.to_string()),
                ..Default::default()
            },
            observability: RuntimeGatewayObservabilityConfig {
                http_bearer_token: Some(api_key.to_string()),
                ..Default::default()
            },
        }
    }

    fn state(
        candidate: RuntimeGatewayCredentialRefreshCandidate,
        entries: Vec<RuntimeGatewayVirtualKeyEntry>,
    ) -> RuntimeGatewayCredentialState {
        RuntimeGatewayCredentialState::new(runtime_gateway_initial_credential_snapshot(
            candidate,
            Arc::new(Mutex::new(entries)),
        ))
    }

    fn provider_api_key(snapshot: &super::RuntimeGatewayCredentialSnapshot) -> &str {
        let RuntimeLocalRewriteProviderOptions::OpenAiResponses { api_keys } = &snapshot.provider
        else {
            panic!("expected OpenAI provider")
        };
        &api_keys[0]
    }

    fn virtual_key(name: &str, token: &str) -> runtime_proxy_crate::RuntimeGatewayVirtualKey {
        runtime_proxy_crate::RuntimeGatewayVirtualKey {
            name: name.to_string(),
            tenant_id: Some("tenant-a".to_string()),
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(token),
            allowed_models: Vec::new(),
            budget_microusd: None,
            request_budget: None,
            rpm_limit: None,
            tpm_limit: None,
        }
    }

    #[test]
    fn refresh_swaps_one_snapshot_and_pins_existing_requests() {
        let state = state(candidate(1, "old-secret"), Vec::new());
        let pinned = state.current.load_full();

        assert!(
            runtime_gateway_apply_secret_refresh(
                &state,
                RuntimeProviderBridgeKind::OpenAiResponses,
                candidate(2, "new-secret"),
            )
            .unwrap()
        );

        let current = state.current.load_full();
        assert_eq!(provider_api_key(&pinned), "old-secret");
        assert_eq!(
            pinned.observability.http_bearer_token.as_deref(),
            Some("old-secret")
        );
        assert_eq!(provider_api_key(&current), "new-secret");
        assert_eq!(
            current.observability.http_bearer_token.as_deref(),
            Some("new-secret")
        );
        assert!(
            !runtime_gateway_apply_secret_refresh(
                &state,
                RuntimeProviderBridgeKind::OpenAiResponses,
                candidate(2, "new-secret"),
            )
            .unwrap()
        );
    }

    #[test]
    fn refresh_preserves_last_known_good_on_validation_failure() {
        let state = state(candidate(1, "old-secret"), Vec::new());
        let mut invalid = candidate(2, "unused");
        invalid.provider = RuntimeLocalRewriteProviderOptions::DeepSeek {
            api_keys: vec!["new-secret".to_string()],
            strict_tools: false,
            beta_base_url: "https://example.invalid".to_string(),
            web_search_mode: RuntimeDeepSeekWebSearchMode::Off,
        };

        assert!(
            runtime_gateway_apply_secret_refresh(
                &state,
                RuntimeProviderBridgeKind::OpenAiResponses,
                invalid,
            )
            .is_err()
        );
        assert_eq!(provider_api_key(&state.current.load_full()), "old-secret");
    }

    #[test]
    fn refresh_replaces_policy_keys_without_dropping_admin_keys() {
        let admin_key = virtual_key("admin-key", "admin-secret");
        let admin_entry = RuntimeGatewayVirtualKeyEntry {
            virtual_key_id: None,
            tenant_id: admin_key.tenant_id.clone(),
            key: admin_key,
            source: RuntimeGatewayVirtualKeySource::Admin,
            created_at_epoch: Some(1),
            updated_at_epoch: Some(1),
            disabled: false,
        };
        let state = state(candidate(1, "old-secret"), vec![admin_entry]);
        let mut refreshed = candidate(2, "new-secret");
        refreshed.virtual_keys = vec![virtual_key("policy-key", "policy-secret")];

        assert!(
            runtime_gateway_apply_secret_refresh(
                &state,
                RuntimeProviderBridgeKind::OpenAiResponses,
                refreshed,
            )
            .unwrap()
        );
        let current = state.current.load_full();
        let entries = current.virtual_keys.lock().unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|entry| {
            entry.key.name == "admin-key" && entry.source == RuntimeGatewayVirtualKeySource::Admin
        }));
        assert!(entries.iter().any(|entry| {
            entry.key.name == "policy-key" && entry.source == RuntimeGatewayVirtualKeySource::Policy
        }));
    }
}
