use super::*;
use std::env;

pub(crate) fn gateway_sso_config(
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
) -> Result<RuntimeGatewaySsoConfig> {
    let proxy_token_hash = match policy
        .sso
        .proxy_token_env
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(env_name) => {
            let token = env::var(env_name)
                .with_context(|| format!("gateway.sso.proxy_token_env requires {env_name}"))?
                .trim()
                .to_string();
            if token.is_empty() {
                bail!("gateway.sso.proxy_token_env env {env_name} cannot be empty");
            }
            Some(runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
                &token,
            ))
        }
        None => None,
    };
    let default_role = policy
        .sso
        .default_role
        .as_deref()
        .and_then(RuntimeGatewayAdminRole::parse)
        .unwrap_or(RuntimeGatewayAdminRole::Admin);
    Ok(RuntimeGatewaySsoConfig {
        proxy_token_hash,
        token_header: gateway_sso_header(&policy.sso.token_header, "x-prodex-sso-token"),
        user_header: gateway_sso_header(&policy.sso.user_header, "x-prodex-sso-user"),
        role_header: gateway_sso_header(&policy.sso.role_header, "x-prodex-sso-role"),
        tenant_header: gateway_sso_header(&policy.sso.tenant_header, "x-prodex-sso-tenant"),
        key_prefixes_header: gateway_sso_header(
            &policy.sso.key_prefixes_header,
            "x-prodex-sso-key-prefixes",
        ),
        oidc: gateway_sso_oidc_config(policy)?,
        default_role,
    })
}

fn gateway_sso_oidc_config(
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
) -> Result<Option<RuntimeGatewayOidcConfig>> {
    let Some(issuer) = policy
        .sso
        .oidc_issuer
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    let audience = policy
        .sso
        .oidc_audience
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .context("gateway.sso.oidc_audience is required when oidc_issuer is set")?;
    Ok(Some(RuntimeGatewayOidcConfig {
        issuer: issuer.to_string(),
        audience: audience.to_string(),
        jwks_url: policy
            .sso
            .oidc_jwks_url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        user_claim: gateway_sso_header(&policy.sso.oidc_user_claim, "email"),
        role_claim: gateway_sso_header(&policy.sso.oidc_role_claim, "prodex_role"),
        tenant_claim: gateway_sso_header(&policy.sso.oidc_tenant_claim, "prodex_tenant"),
        key_prefixes_claim: gateway_sso_header(
            &policy.sso.oidc_key_prefixes_claim,
            "prodex_key_prefixes",
        ),
    }))
}

fn gateway_sso_header(value: &Option<String>, default: &str) -> String {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(default)
        .to_string()
}
