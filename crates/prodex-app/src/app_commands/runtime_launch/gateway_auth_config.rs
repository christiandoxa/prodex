use super::gateway_config_helpers::{
    gateway_budget_usd_to_microusd, gateway_optional_policy_string,
};
use super::*;
use std::env;

#[derive(Debug)]
pub(crate) struct ResolvedGatewayAuthConfig {
    pub(crate) auth_token_hash: Option<runtime_proxy_crate::LocalBridgeBearerTokenHash>,
    pub(crate) auth_required: bool,
    pub(crate) admin_tokens: Vec<RuntimeGatewayAdminToken>,
    pub(crate) virtual_keys: Vec<runtime_proxy_crate::RuntimeGatewayVirtualKey>,
}

pub(crate) fn resolve_gateway_auth_config(
    args: &GatewayArgs,
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
) -> Result<ResolvedGatewayAuthConfig> {
    let auth_token = args
        .auth_token
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            env::var("PRODEX_GATEWAY_TOKEN")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        });
    let admin_tokens = gateway_admin_tokens_config(auth_token.as_deref(), policy)?;
    if policy.require_auth == Some(true) && auth_token.is_none() && policy.virtual_keys.is_empty() {
        bail!(
            "gateway auth is required by policy.toml; set --auth-token, PRODEX_GATEWAY_TOKEN, or [[gateway.virtual_keys]]; [[gateway.admin_tokens]] only protects admin endpoints"
        );
    }
    let virtual_keys = gateway_virtual_keys_config(policy)?;
    let auth_required = auth_token.is_some() || !virtual_keys.is_empty();
    if policy.require_auth == Some(true) && !auth_required {
        bail!("gateway auth is required by policy.toml; configured virtual key env vars are empty");
    }
    Ok(ResolvedGatewayAuthConfig {
        auth_token_hash: auth_token
            .as_deref()
            .map(runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token),
        auth_required,
        admin_tokens,
        virtual_keys,
    })
}

fn gateway_admin_tokens_config(
    legacy_admin_token: Option<&str>,
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
) -> Result<Vec<RuntimeGatewayAdminToken>> {
    let mut tokens = Vec::new();
    if let Some(token) = legacy_admin_token
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        tokens.push(RuntimeGatewayAdminToken {
            name: "default-admin".to_string(),
            token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(token),
            role: RuntimeGatewayAdminRole::Admin,
            tenant_id: None,
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            allowed_key_prefixes: Vec::new(),
        });
    }
    for configured in &policy.admin_tokens {
        let Some(token) = env::var(&configured.token_env)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let role = configured
            .role
            .as_deref()
            .and_then(RuntimeGatewayAdminRole::parse)
            .unwrap_or(RuntimeGatewayAdminRole::Admin);
        tokens.push(RuntimeGatewayAdminToken {
            name: configured.name.trim().to_string(),
            token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(&token),
            role,
            tenant_id: configured
                .tenant_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            team_id: gateway_optional_policy_string(configured.team_id.as_deref()),
            project_id: gateway_optional_policy_string(configured.project_id.as_deref()),
            user_id: gateway_optional_policy_string(configured.user_id.as_deref()),
            budget_id: gateway_optional_policy_string(configured.budget_id.as_deref()),
            allowed_key_prefixes: configured
                .allowed_key_prefixes
                .iter()
                .map(|prefix| prefix.trim().to_string())
                .filter(|prefix| !prefix.is_empty())
                .collect(),
        });
    }
    Ok(tokens)
}

fn gateway_virtual_keys_config(
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
) -> Result<Vec<runtime_proxy_crate::RuntimeGatewayVirtualKey>> {
    policy
        .virtual_keys
        .iter()
        .map(|key| {
            let token_env = key.token_env.trim();
            let token = env::var(token_env)
                .with_context(|| {
                    format!("gateway virtual key '{}' requires {token_env}", key.name)
                })?
                .trim()
                .to_string();
            if token.is_empty() {
                bail!(
                    "gateway virtual key '{}' env {token_env} cannot be empty",
                    key.name
                );
            }
            Ok(runtime_proxy_crate::RuntimeGatewayVirtualKey {
                name: key.name.trim().to_string(),
                tenant_id: key
                    .tenant_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string),
                team_id: gateway_optional_policy_string(key.team_id.as_deref()),
                project_id: gateway_optional_policy_string(key.project_id.as_deref()),
                user_id: gateway_optional_policy_string(key.user_id.as_deref()),
                budget_id: gateway_optional_policy_string(key.budget_id.as_deref()),
                token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(&token),
                allowed_models: key
                    .allowed_models
                    .iter()
                    .map(|model| model.trim().to_string())
                    .filter(|model| !model.is_empty())
                    .collect(),
                budget_microusd: key.budget_usd.map(gateway_budget_usd_to_microusd),
                request_budget: key.request_budget,
                rpm_limit: key.rpm_limit,
                tpm_limit: key.tpm_limit,
            })
        })
        .collect()
}
