use super::*;
use std::env;

const PRODEX_GATEWAY_OIDC_PREFETCH_TIMEOUT_ENV: &str = "PRODEX_GATEWAY_OIDC_PREFETCH_TIMEOUT_MS";
const PRODEX_GATEWAY_OIDC_HTTP_CACHE_TTL_ENV: &str = "PRODEX_GATEWAY_OIDC_HTTP_CACHE_TTL_SECONDS";
const PRODEX_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_ENV: &str =
    "PRODEX_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_MS";
const PRODEX_GATEWAY_OIDC_LAST_KNOWN_GOOD_ENV: &str = "PRODEX_GATEWAY_OIDC_LAST_KNOWN_GOOD_SECONDS";

pub(crate) fn gateway_sso_config(
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
) -> Result<RuntimeGatewaySsoConfig> {
    let proxy_token_hash = match policy.sso.proxy_token_env.as_deref() {
        Some(env_name) if !gateway_exact_policy_identifier(env_name) => {
            bail!("gateway.sso.proxy_token_env must be non-empty without whitespace");
        }
        Some(env_name) => {
            let token = gateway_secret_value_from_env("gateway.sso.proxy_token_env", env_name)?;
            Some(runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
                &token,
            ))
        }
        None => None,
    };
    let oidc = gateway_sso_oidc_config(policy)?;
    if oidc.is_some() {
        gateway_validate_oidc_runtime_env()?;
    }
    Ok(RuntimeGatewaySsoConfig {
        proxy_token_hash,
        require_tenant: policy.sso.require_tenant.unwrap_or(false),
        token_header: gateway_sso_header(
            "gateway.sso.token_header",
            &policy.sso.token_header,
            "x-prodex-sso-token",
        )?,
        user_header: gateway_sso_header(
            "gateway.sso.user_header",
            &policy.sso.user_header,
            "x-prodex-sso-user",
        )?,
        role_header: gateway_sso_header(
            "gateway.sso.role_header",
            &policy.sso.role_header,
            "x-prodex-sso-role",
        )?,
        tenant_header: gateway_sso_header(
            "gateway.sso.tenant_header",
            &policy.sso.tenant_header,
            "x-prodex-sso-tenant",
        )?,
        key_prefixes_header: gateway_sso_header(
            "gateway.sso.key_prefixes_header",
            &policy.sso.key_prefixes_header,
            "x-prodex-sso-key-prefixes",
        )?,
        oidc,
    })
}

fn gateway_validate_oidc_runtime_env() -> Result<()> {
    gateway_validate_oidc_duration_env(PRODEX_GATEWAY_OIDC_PREFETCH_TIMEOUT_ENV, false)?;
    gateway_validate_oidc_duration_env(PRODEX_GATEWAY_OIDC_HTTP_CACHE_TTL_ENV, true)?;
    gateway_validate_oidc_duration_env(PRODEX_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_ENV, false)?;
    gateway_validate_oidc_duration_env(PRODEX_GATEWAY_OIDC_LAST_KNOWN_GOOD_ENV, true)
}

fn gateway_validate_oidc_duration_env(env_name: &str, allow_zero: bool) -> Result<()> {
    let Ok(value) = env::var(env_name) else {
        return Ok(());
    };
    if value.is_empty() {
        bail!("{env_name} cannot be empty");
    }
    if value.chars().any(char::is_whitespace) {
        bail!("{env_name} must not contain whitespace");
    }
    let parsed = value
        .parse::<u64>()
        .with_context(|| format!("{env_name} must be an unsigned integer"))?;
    if !allow_zero && parsed == 0 {
        bail!("{env_name} must be greater than zero");
    }
    Ok(())
}

fn gateway_sso_oidc_config(
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
) -> Result<Option<RuntimeGatewayOidcConfig>> {
    let Some(issuer) = policy.sso.oidc_issuer.as_deref() else {
        if policy.sso.oidc_audience.is_some() || policy.sso.oidc_jwks_url.is_some() {
            bail!("gateway.sso OIDC requires oidc_issuer and oidc_audience");
        }
        return Ok(None);
    };
    let issuer = gateway_sso_oidc_https_url("gateway.sso.oidc_issuer", issuer)?;
    let audience = policy
        .sso
        .oidc_audience
        .as_deref()
        .context("gateway.sso.oidc_audience is required when oidc_issuer is set")
        .and_then(|value| gateway_required_policy_identifier("gateway.sso.oidc_audience", value))?;
    Ok(Some(RuntimeGatewayOidcConfig {
        issuer,
        audience,
        jwks_url: policy
            .sso
            .oidc_jwks_url
            .as_deref()
            .map(|value| gateway_sso_oidc_https_url("gateway.sso.oidc_jwks_url", value))
            .transpose()?,
        user_claim: gateway_sso_header(
            "gateway.sso.oidc_user_claim",
            &policy.sso.oidc_user_claim,
            "email",
        )?,
        role_claim: gateway_sso_header(
            "gateway.sso.oidc_role_claim",
            &policy.sso.oidc_role_claim,
            "prodex_role",
        )?,
        tenant_claim: gateway_sso_header(
            "gateway.sso.oidc_tenant_claim",
            &policy.sso.oidc_tenant_claim,
            "prodex_tenant",
        )?,
        key_prefixes_claim: gateway_sso_header(
            "gateway.sso.oidc_key_prefixes_claim",
            &policy.sso.oidc_key_prefixes_claim,
            "prodex_key_prefixes",
        )?,
    }))
}

fn gateway_sso_oidc_https_url(field: &str, value: &str) -> Result<String> {
    if value.is_empty() {
        bail!("{field} cannot be empty");
    }
    if value.chars().any(char::is_whitespace) {
        bail!("{field} must not contain whitespace");
    }
    let parsed = reqwest::Url::parse(value)
        .with_context(|| format!("{field} must be an https URL with host"))?;
    if parsed.scheme() != "https"
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        bail!("{field} must be an https URL with host");
    }
    Ok(value.to_string())
}

fn gateway_required_policy_identifier(field: &str, value: &str) -> Result<String> {
    if !gateway_exact_policy_identifier(value) {
        bail!("{field} must be non-empty without whitespace");
    }
    Ok(value.to_string())
}

fn gateway_sso_header(field: &str, value: &Option<String>, default: &str) -> Result<String> {
    match value.as_deref() {
        Some(value) if !gateway_exact_policy_identifier(value) => {
            bail!("{field} must be non-empty without whitespace")
        }
        Some(value) => Ok(value.to_string()),
        None => Ok(default.to_string()),
    }
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

fn gateway_exact_policy_identifier(value: &str) -> bool {
    !value.is_empty() && !value.chars().any(char::is_whitespace)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestEnvVarGuard;

    fn gateway_oidc_policy() -> prodex_runtime_policy::RuntimePolicyGatewaySettings {
        let mut policy = prodex_runtime_policy::RuntimePolicyGatewaySettings::default();
        policy.sso.oidc_issuer = Some("https://idp.example.test".to_string());
        policy.sso.oidc_audience = Some("prodex".to_string());
        policy
    }

    #[test]
    fn gateway_sso_config_rejects_invalid_oidc_runtime_env_values() {
        for (env_name, value, message) in [
            (
                PRODEX_GATEWAY_OIDC_PREFETCH_TIMEOUT_ENV,
                "",
                "PRODEX_GATEWAY_OIDC_PREFETCH_TIMEOUT_MS cannot be empty",
            ),
            (
                PRODEX_GATEWAY_OIDC_HTTP_CACHE_TTL_ENV,
                " 25 ",
                "PRODEX_GATEWAY_OIDC_HTTP_CACHE_TTL_SECONDS must not contain whitespace",
            ),
            (
                PRODEX_GATEWAY_OIDC_LAST_KNOWN_GOOD_ENV,
                "not-a-number",
                "PRODEX_GATEWAY_OIDC_LAST_KNOWN_GOOD_SECONDS must be an unsigned integer",
            ),
            (
                PRODEX_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_ENV,
                "0",
                "PRODEX_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_MS must be greater than zero",
            ),
        ] {
            let _prefetch = TestEnvVarGuard::unset(PRODEX_GATEWAY_OIDC_PREFETCH_TIMEOUT_ENV);
            let _ttl = TestEnvVarGuard::unset(PRODEX_GATEWAY_OIDC_HTTP_CACHE_TTL_ENV);
            let _backoff = TestEnvVarGuard::unset(PRODEX_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_ENV);
            let _lkg = TestEnvVarGuard::unset(PRODEX_GATEWAY_OIDC_LAST_KNOWN_GOOD_ENV);
            let _target = TestEnvVarGuard::set(env_name, value);

            let err = gateway_sso_config(&gateway_oidc_policy()).unwrap_err();

            assert!(err.to_string().contains(message));
        }
    }

    #[test]
    fn gateway_sso_config_accepts_valid_oidc_runtime_env_values() {
        let _prefetch = TestEnvVarGuard::set(PRODEX_GATEWAY_OIDC_PREFETCH_TIMEOUT_ENV, "1");
        let _ttl = TestEnvVarGuard::set(PRODEX_GATEWAY_OIDC_HTTP_CACHE_TTL_ENV, "0");
        let _backoff = TestEnvVarGuard::set(PRODEX_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_ENV, "1");
        let _lkg = TestEnvVarGuard::set(PRODEX_GATEWAY_OIDC_LAST_KNOWN_GOOD_ENV, "0");

        assert!(
            gateway_sso_config(&gateway_oidc_policy())
                .unwrap()
                .oidc
                .is_some()
        );
    }
}
