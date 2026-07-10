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
    let auth_token = match args.auth_token.as_deref() {
        Some(value) => Some(gateway_exact_secret_value("gateway --auth-token", value)?),
        None => match env::var("PRODEX_GATEWAY_TOKEN") {
            Ok(value) => Some(gateway_exact_secret_value("PRODEX_GATEWAY_TOKEN", &value)?),
            Err(_) => None,
        },
    };
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
    _legacy_admin_token: Option<&str>,
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
) -> Result<Vec<RuntimeGatewayAdminToken>> {
    let mut tokens = Vec::new();
    for configured in &policy.admin_tokens {
        if !gateway_exact_policy_identifier(&configured.token_env) {
            bail!(
                "gateway.admin_tokens token_env for {:?} must be non-empty without whitespace",
                configured.name
            );
        }
        let context = format!("gateway.admin_tokens token_env for {:?}", configured.name);
        let token = gateway_secret_value_from_env(&context, &configured.token_env)?;
        let role = match configured.role.as_deref() {
            Some(role) => RuntimeGatewayAdminRole::parse(role).with_context(|| {
                format!(
                    "gateway.admin_tokens role for {:?} must be admin or viewer",
                    configured.name
                )
            })?,
            None => RuntimeGatewayAdminRole::Viewer,
        };
        for prefix in &configured.allowed_key_prefixes {
            if !gateway_exact_policy_identifier(prefix) {
                anyhow::bail!(
                    "gateway.admin_tokens allowed_key_prefixes for {:?} must be non-empty strings without whitespace",
                    configured.name
                );
            }
        }
        if !gateway_exact_policy_identifier(&configured.name) {
            anyhow::bail!("gateway.admin_tokens name must be non-empty without whitespace");
        }
        tokens.push(RuntimeGatewayAdminToken {
            name: configured.name.clone(),
            token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(&token),
            role,
            tenant_id: gateway_policy_optional_scope(
                "admin_tokens",
                &configured.name,
                "tenant_id",
                configured.tenant_id.as_deref(),
            )?,
            team_id: gateway_policy_optional_scope(
                "admin_tokens",
                &configured.name,
                "team_id",
                configured.team_id.as_deref(),
            )?,
            project_id: gateway_policy_optional_scope(
                "admin_tokens",
                &configured.name,
                "project_id",
                configured.project_id.as_deref(),
            )?,
            user_id: gateway_policy_optional_scope(
                "admin_tokens",
                &configured.name,
                "user_id",
                configured.user_id.as_deref(),
            )?,
            budget_id: gateway_policy_optional_scope(
                "admin_tokens",
                &configured.name,
                "budget_id",
                configured.budget_id.as_deref(),
            )?,
            allowed_key_prefixes: configured.allowed_key_prefixes.clone(),
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
            let context = format!("gateway virtual key '{}'", key.name);
            let token = gateway_secret_value_from_env(&context, token_env)?;
            for model in &key.allowed_models {
                if !gateway_exact_policy_identifier(model) {
                    anyhow::bail!(
                        "gateway.virtual_keys allowed_models for {:?} must be non-empty strings without whitespace",
                        key.name
                    );
                }
            }
            if !gateway_exact_policy_identifier(&key.name) {
                anyhow::bail!("gateway virtual key name must be non-empty without whitespace");
            }
            Ok(runtime_proxy_crate::RuntimeGatewayVirtualKey {
                name: key.name.clone(),
                tenant_id: gateway_policy_optional_scope(
                    "virtual_keys",
                    &key.name,
                    "tenant_id",
                    key.tenant_id.as_deref(),
                )?,
                team_id: gateway_policy_optional_scope(
                    "virtual_keys",
                    &key.name,
                    "team_id",
                    key.team_id.as_deref(),
                )?,
                project_id: gateway_policy_optional_scope(
                    "virtual_keys",
                    &key.name,
                    "project_id",
                    key.project_id.as_deref(),
                )?,
                user_id: gateway_policy_optional_scope(
                    "virtual_keys",
                    &key.name,
                    "user_id",
                    key.user_id.as_deref(),
                )?,
                budget_id: gateway_policy_optional_scope(
                    "virtual_keys",
                    &key.name,
                    "budget_id",
                    key.budget_id.as_deref(),
                )?,
                token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(&token),
                allowed_models: key.allowed_models.clone(),
                budget_microusd: key.budget_usd.map(gateway_budget_usd_to_microusd),
                request_budget: key.request_budget,
                rpm_limit: key.rpm_limit,
                tpm_limit: key.tpm_limit,
            })
        })
        .collect()
}

fn gateway_policy_optional_scope(
    collection: &str,
    item_name: &str,
    field: &str,
    value: Option<&str>,
) -> Result<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };
    if !gateway_exact_policy_identifier(value) {
        anyhow::bail!(
            "gateway.{collection} {field} for {item_name:?} must be a non-empty string without whitespace"
        );
    }
    Ok(Some(value.to_string()))
}

fn gateway_secret_value_from_env(context: &str, env_name: &str) -> Result<String> {
    let value = env::var(env_name).with_context(|| format!("{context} requires {env_name}"))?;
    if value.is_empty() {
        bail!("{context} env {env_name} cannot be empty");
    }
    if value.chars().any(char::is_whitespace) {
        bail!("{context} env {env_name} must not contain whitespace");
    }
    Ok(value)
}

fn gateway_exact_secret_value(name: &str, value: &str) -> Result<String> {
    if value.is_empty() {
        bail!("{name} cannot be empty");
    }
    if value.chars().any(char::is_whitespace) {
        bail!("{name} must not contain whitespace");
    }
    Ok(value.to_string())
}

fn gateway_exact_policy_identifier(value: &str) -> bool {
    !value.is_empty() && !value.chars().any(char::is_whitespace)
}
