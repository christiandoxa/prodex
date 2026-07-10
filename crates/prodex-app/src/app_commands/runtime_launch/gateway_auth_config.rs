use super::gateway_config_helpers::gateway_budget_usd_to_microusd;
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

pub(crate) fn gateway_admin_tokens_config(
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
        if !gateway_exact_policy_identifier(&configured.token_env) {
            continue;
        }
        let Some(token) = env::var(&configured.token_env)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let role = match configured.role.as_deref() {
            Some(role) => RuntimeGatewayAdminRole::parse(role).with_context(|| {
                format!(
                    "gateway.admin_tokens role for {:?} must be admin or viewer",
                    configured.name
                )
            })?,
            None => RuntimeGatewayAdminRole::Viewer,
        };
        tokens.push(RuntimeGatewayAdminToken {
            name: if gateway_exact_policy_identifier(&configured.name) {
                configured.name.clone()
            } else {
                String::new()
            },
            token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(&token),
            role,
            tenant_id: gateway_optional_policy_string(configured.tenant_id.as_deref()),
            team_id: gateway_optional_policy_string(configured.team_id.as_deref()),
            project_id: gateway_optional_policy_string(configured.project_id.as_deref()),
            user_id: gateway_optional_policy_string(configured.user_id.as_deref()),
            budget_id: gateway_optional_policy_string(configured.budget_id.as_deref()),
            allowed_key_prefixes: configured
                .allowed_key_prefixes
                .iter()
                .filter(|prefix| gateway_exact_policy_identifier(prefix))
                .cloned()
                .collect(),
        });
    }
    Ok(tokens)
}

pub(crate) fn gateway_virtual_keys_config(
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
) -> Result<Vec<runtime_proxy_crate::RuntimeGatewayVirtualKey>> {
    policy
        .virtual_keys
        .iter()
        .map(|key| {
            let token_env = key.token_env.as_str();
            if !gateway_exact_policy_identifier(token_env) {
                bail!("gateway virtual key '{}' token_env is invalid", key.name);
            }
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
                name: if gateway_exact_policy_identifier(&key.name) {
                    key.name.clone()
                } else {
                    String::new()
                },
                tenant_id: gateway_optional_policy_string(key.tenant_id.as_deref()),
                team_id: gateway_optional_policy_string(key.team_id.as_deref()),
                project_id: gateway_optional_policy_string(key.project_id.as_deref()),
                user_id: gateway_optional_policy_string(key.user_id.as_deref()),
                budget_id: gateway_optional_policy_string(key.budget_id.as_deref()),
                token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(&token),
                allowed_models: key
                    .allowed_models
                    .iter()
                    .filter(|model| gateway_exact_policy_identifier(model))
                    .cloned()
                    .collect(),
                budget_microusd: key.budget_usd.map(gateway_budget_usd_to_microusd),
                request_budget: key.request_budget,
                rpm_limit: key.rpm_limit,
                tpm_limit: key.tpm_limit,
            })
        })
        .collect()
}

fn gateway_optional_policy_string(value: Option<&str>) -> Option<String> {
    super::gateway_config_helpers::gateway_optional_policy_string(
        value.filter(|value| gateway_exact_policy_identifier(value)),
    )
}

fn gateway_exact_policy_identifier(value: &str) -> bool {
    !value.is_empty() && !value.chars().any(char::is_whitespace)
}
