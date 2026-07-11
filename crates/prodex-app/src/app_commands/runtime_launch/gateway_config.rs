use super::*;

#[path = "gateway_auth_config.rs"]
mod gateway_auth_config;
#[path = "gateway_config_helpers.rs"]
mod gateway_config_helpers;
#[path = "gateway_guardrail_config.rs"]
mod gateway_guardrail_config;
#[path = "gateway_observability_config.rs"]
mod gateway_observability_config;
#[path = "gateway_provider_config.rs"]
mod gateway_provider_config;
#[path = "gateway_route_alias_config.rs"]
mod gateway_route_alias_config;
#[path = "gateway_secret_config.rs"]
mod gateway_secret_config;
#[path = "gateway_sso_config.rs"]
mod gateway_sso_config;
#[path = "gateway_state_store_config.rs"]
mod gateway_state_store_config;
#[cfg(test)]
pub(super) use gateway_auth_config::resolve_gateway_auth_config;
use gateway_auth_config::resolve_gateway_auth_config_with_resolver;
#[cfg(test)]
pub(super) use gateway_auth_config::{gateway_admin_tokens_config, gateway_virtual_keys_config};
pub(super) use gateway_config_helpers::gateway_api_keys_from_list;
use gateway_config_helpers::gateway_validate_listen_auth;
#[cfg(test)]
pub(super) use gateway_guardrail_config::resolve_gateway_guardrail_config;
use gateway_guardrail_config::resolve_gateway_guardrail_config_with_resolver;
#[cfg(test)]
pub(super) use gateway_guardrail_config::{
    gateway_guardrail_config, gateway_guardrail_webhook_config,
};
#[cfg(test)]
pub(super) use gateway_observability_config::gateway_observability_config;
use gateway_observability_config::gateway_observability_config_with_resolver;
use gateway_provider_config::resolve_gateway_provider_config_with_resolver;
#[cfg(test)]
pub(super) use gateway_provider_config::{gateway_openai_api_keys, gateway_upstream_base_url};
#[cfg(test)]
pub(super) use gateway_provider_config::{gateway_policy_provider, gateway_provider_options};
#[cfg(not(test))]
pub(super) use gateway_route_alias_config::gateway_route_aliases_config;
#[cfg(test)]
pub(super) use gateway_route_alias_config::{
    gateway_route_alias_model_metrics, gateway_route_aliases_config,
};
use gateway_secret_config::GatewaySecretResolver;
#[cfg(test)]
pub(super) use gateway_sso_config::gateway_sso_config;
use gateway_sso_config::gateway_sso_config_with_resolver;
#[cfg(test)]
pub(super) use gateway_state_store_config::gateway_state_store_config;
use gateway_state_store_config::{
    gateway_state_store_config_with_resolver, gateway_validate_runtime_topology,
};

pub(super) struct ResolvedGatewayLaunchConfig {
    pub(super) provider_name: Option<&'static str>,
    pub(super) upstream_base_url: String,
    pub(super) provider_options: RuntimeLocalRewriteProviderOptions,
    pub(super) auth_token_hash: Option<runtime_proxy_crate::LocalBridgeBearerTokenHash>,
    pub(super) auth_required: bool,
    pub(super) listen_addr: String,
    pub(super) admin_tokens: Vec<RuntimeGatewayAdminToken>,
    pub(super) sso: RuntimeGatewaySsoConfig,
    pub(super) state_store: RuntimeGatewayStateStore,
    pub(super) virtual_keys: Vec<runtime_proxy_crate::RuntimeGatewayVirtualKey>,
    pub(super) route_aliases: Vec<runtime_proxy_crate::RuntimeGatewayRouteAlias>,
    pub(super) request_constraints: prodex_provider_core::ProviderRequestConstraintPolicy,
    pub(super) guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig,
    pub(super) guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig,
    pub(super) call_id_header: String,
    pub(super) observability: RuntimeGatewayObservabilityConfig,
    pub(super) presidio_redaction_enabled: bool,
    pub(super) credential_fingerprint: [u8; 32],
}

#[cfg(test)]
pub(super) fn resolve_gateway_launch_config(
    paths: &AppPaths,
    state: &AppState,
    args: &GatewayArgs,
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
) -> Result<ResolvedGatewayLaunchConfig> {
    resolve_gateway_launch_config_with_secrets(paths, state, args, policy, &Default::default())
}

pub(super) fn resolve_gateway_launch_config_with_secrets(
    paths: &AppPaths,
    state: &AppState,
    args: &GatewayArgs,
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
    secrets: &prodex_runtime_policy::RuntimePolicySecretsSettings,
) -> Result<ResolvedGatewayLaunchConfig> {
    let secret_resolver = GatewaySecretResolver::from_policy(secrets)?;
    if secret_resolver.production() {
        if policy.require_auth != Some(true) {
            bail!("production gateway requires gateway.require_auth=true");
        }
        if policy.provider_api_key_ref.is_none() {
            bail!("production gateway requires gateway.provider_api_key_ref");
        }
        if policy.auth_token_ref.is_none() && policy.virtual_keys.is_empty() {
            bail!("production gateway requires gateway.auth_token_ref or a virtual key reference");
        }
    }
    gateway_validate_upstream_base_url_input(args, policy)?;
    let provider =
        resolve_gateway_provider_config_with_resolver(state, args, policy, &secret_resolver)?;
    let auth = resolve_gateway_auth_config_with_resolver(args, policy, &secret_resolver)?;
    if auth.auth_required
        && provider.provider_credential.is_none()
        && matches!(
            &provider.provider_options,
            RuntimeLocalRewriteProviderOptions::OpenAiResponses { api_keys }
                if api_keys.is_empty()
        )
    {
        bail!(
            "OpenAI-compatible gateway auth requires a separate upstream key; set --api-key, OPENAI_API_KEY, or OPENAI_API_KEYS"
        );
    }

    let listen_addr = match args.listen.as_deref().or(policy.listen_addr.as_deref()) {
        Some(value) if gateway_exact_policy_identifier(value) => value.to_string(),
        Some(_) => bail!("gateway.listen_addr must be non-empty without whitespace"),
        None => "127.0.0.1:4000".to_string(),
    };
    gateway_validate_listen_auth(&listen_addr, auth.auth_required)?;

    let route_aliases = gateway_route_aliases_config(policy, provider.provider)?;
    let guardrail = resolve_gateway_guardrail_config_with_resolver(args, policy, &secret_resolver)?;
    let call_id_header = gateway_call_id_header_config(policy)?;
    let state_store = gateway_state_store_config_with_resolver(paths, policy, &secret_resolver)?;
    gateway_validate_runtime_topology(&state_store)?;

    let sso = gateway_sso_config_with_resolver(policy, &secret_resolver)?;
    let observability =
        gateway_observability_config_with_resolver(paths, policy, &secret_resolver)?;
    let credential_fingerprint = secret_resolver.fingerprint()?;
    let provider_options = match provider.provider_credential {
        Some(credential) => provider
            .provider_options
            .with_projected_credential(credential),
        None => provider.provider_options,
    };

    Ok(ResolvedGatewayLaunchConfig {
        provider_name: provider.provider.map(SuperExternalProvider::as_str),
        upstream_base_url: provider.upstream_base_url,
        provider_options,
        auth_token_hash: auth.auth_token_hash,
        auth_required: auth.auth_required,
        listen_addr,
        admin_tokens: auth.admin_tokens,
        sso,
        state_store,
        virtual_keys: auth.virtual_keys,
        route_aliases,
        request_constraints: guardrail.request_constraints,
        guardrails: guardrail.guardrails,
        guardrail_webhook: guardrail.webhook,
        call_id_header,
        observability,
        presidio_redaction_enabled: guardrail.presidio_redaction_enabled,
        credential_fingerprint,
    })
}

fn gateway_validate_upstream_base_url_input(
    args: &GatewayArgs,
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
) -> Result<()> {
    let configured = args.base_url.as_deref().or(policy.base_url.as_deref());
    let provider = match args.provider {
        Some(provider) => Some(provider),
        None => policy
            .provider
            .as_deref()
            .map(gateway_provider_config::gateway_policy_provider)
            .transpose()?,
    };
    let selected = configured
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| provider.map(|provider| provider.default_base_url().to_string()))
        .or_else(|| {
            std::env::var("OPENAI_BASE_URL")
                .ok()
                .filter(|value| !value.is_empty())
        });
    if selected
        .as_deref()
        .is_some_and(|value| value.chars().any(char::is_whitespace))
    {
        bail!("gateway --base-url must not contain whitespace");
    }
    Ok(())
}

pub(super) fn gateway_call_id_header_config(
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
) -> Result<String> {
    Ok(policy
        .observability
        .call_id_header
        .as_deref()
        .map(|value| {
            gateway_exact_policy_identifier(value)
                .then_some(value)
                .context(
                    "gateway.observability.call_id_header must be non-empty without whitespace",
                )
        })
        .transpose()?
        .unwrap_or("x-prodex-call-id")
        .to_string())
}

fn gateway_exact_policy_identifier(value: &str) -> bool {
    !value.is_empty() && !value.chars().any(char::is_whitespace)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{KIRO_MODEL_CATALOG_FILE, ProfileEntry, ProfileProvider};
    use std::collections::BTreeMap;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be valid")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "prodex-gateway-config-{name}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).expect("temp dir should exist");
        dir
    }

    #[test]
    fn gateway_policy_provider_accepts_kiro() {
        assert_eq!(
            gateway_policy_provider("kiro").unwrap(),
            SuperExternalProvider::Kiro
        );
    }

    #[test]
    fn gateway_openai_api_keys_rejects_empty_explicit_inputs() {
        let err = gateway_openai_api_keys(Some("")).unwrap_err();
        assert!(
            err.to_string()
                .contains("gateway --api-key cannot be empty")
        );

        let err = gateway_openai_api_keys(Some(" sk-test ")).unwrap_err();
        assert!(
            err.to_string()
                .contains("gateway --api-key must not contain whitespace")
        );
    }

    #[test]
    fn gateway_provider_options_accepts_imported_kiro_profile() {
        let root = temp_dir("kiro");
        let codex_home = root.join("kiro-home");
        fs::create_dir_all(&codex_home).expect("codex home should exist");
        fs::write(
            codex_home.join(KIRO_MODEL_CATALOG_FILE),
            serde_json::json!({
                "models": [
                    { "id": "claude-sonnet-4", "name": "claude-sonnet-4", "owned_by": "kiro-cli" }
                ]
            })
            .to_string(),
        )
        .expect("kiro model catalog should be written");
        let state = AppState {
            active_profile: Some("kiro-main".to_string()),
            profiles: BTreeMap::from([(
                "kiro-main".to_string(),
                ProfileEntry {
                    codex_home: codex_home.clone(),
                    managed: true,
                    email: Some("kiro@example.com".to_string()),
                    provider: ProfileProvider::Kiro {
                        auth_key: "kiro-key".to_string(),
                        auth_kind: Some("builder-id".to_string()),
                        profile_arn: None,
                        profile_name: None,
                        start_url: None,
                        region: Some("us-east-1".to_string()),
                    },
                },
            )]),
            ..Default::default()
        };
        let options = gateway_provider_options(&state, Some(SuperExternalProvider::Kiro), None)
            .expect("kiro gateway should use imported profile");
        let RuntimeLocalRewriteProviderOptions::Kiro { auth } = options else {
            panic!("expected kiro provider options");
        };
        assert_eq!(auth.profile_name, "kiro-main");
        assert_eq!(auth.codex_home, codex_home);
        assert_eq!(auth.model_catalog.len(), 1);
    }

    #[test]
    fn gateway_kiro_route_alias_metrics_do_not_infer_openai_costs() {
        let metrics = gateway_route_alias_model_metrics(
            Some(prodex_provider_core::ProviderId::Kiro),
            &[String::from("gpt-5.4")],
            &[],
        )
        .unwrap();
        assert!(metrics.is_empty());
    }
}
