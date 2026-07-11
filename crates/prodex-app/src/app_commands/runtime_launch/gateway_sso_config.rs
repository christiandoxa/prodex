use super::gateway_secret_config::GatewaySecretResolver;
use super::*;
use prodex_authn::{OidcEndpointPolicy, ValidatedOidcIssuer};
use prodex_domain::SecretPurpose;
use std::env;

const PRODEX_GATEWAY_OIDC_PREFETCH_TIMEOUT_ENV: &str = "PRODEX_GATEWAY_OIDC_PREFETCH_TIMEOUT_MS";
const PRODEX_GATEWAY_OIDC_HTTP_CACHE_TTL_ENV: &str = "PRODEX_GATEWAY_OIDC_HTTP_CACHE_TTL_SECONDS";
const PRODEX_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_ENV: &str =
    "PRODEX_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_MS";
const PRODEX_GATEWAY_OIDC_LAST_KNOWN_GOOD_ENV: &str = "PRODEX_GATEWAY_OIDC_LAST_KNOWN_GOOD_SECONDS";
const MAX_PRODEX_GATEWAY_OIDC_PREFETCH_TIMEOUT_MS: u64 = 10_000;
const MAX_PRODEX_GATEWAY_OIDC_HTTP_CACHE_TTL_SECONDS: u64 = 86_400;
const MAX_PRODEX_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_MS: u64 = 3_600_000;
const MAX_PRODEX_GATEWAY_OIDC_LAST_KNOWN_GOOD_SECONDS: u64 = 604_800;

#[cfg(test)]
pub(crate) fn gateway_sso_config(
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
) -> Result<RuntimeGatewaySsoConfig> {
    let resolver = GatewaySecretResolver::from_policy(&Default::default())?;
    gateway_sso_config_with_resolver(policy, &resolver)
}

pub(crate) fn gateway_sso_config_with_resolver(
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
    resolver: &GatewaySecretResolver,
) -> Result<RuntimeGatewaySsoConfig> {
    let proxy_token_context = if policy.sso.proxy_token_ref.is_some() {
        "gateway.sso.proxy_token_ref"
    } else {
        "gateway.sso.proxy_token_env"
    };
    let proxy_token_hash = resolver
        .resolve(
            proxy_token_context,
            policy.sso.proxy_token_ref.as_ref(),
            policy.sso.proxy_token_env.as_deref(),
            None,
            SecretPurpose::ControlPlaneCredential,
        )?
        .as_deref()
        .map(runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token);
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
    gateway_validate_oidc_duration_env(
        PRODEX_GATEWAY_OIDC_PREFETCH_TIMEOUT_ENV,
        false,
        MAX_PRODEX_GATEWAY_OIDC_PREFETCH_TIMEOUT_MS,
    )?;
    gateway_validate_oidc_duration_env(
        PRODEX_GATEWAY_OIDC_HTTP_CACHE_TTL_ENV,
        true,
        MAX_PRODEX_GATEWAY_OIDC_HTTP_CACHE_TTL_SECONDS,
    )?;
    gateway_validate_oidc_duration_env(
        PRODEX_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_ENV,
        false,
        MAX_PRODEX_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_MS,
    )?;
    gateway_validate_oidc_duration_env(
        PRODEX_GATEWAY_OIDC_LAST_KNOWN_GOOD_ENV,
        true,
        MAX_PRODEX_GATEWAY_OIDC_LAST_KNOWN_GOOD_SECONDS,
    )
}

fn gateway_validate_oidc_duration_env(env_name: &str, allow_zero: bool, max: u64) -> Result<()> {
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
    if parsed > max {
        bail!("{env_name} must not exceed {max}");
    }
    Ok(())
}

fn gateway_sso_oidc_config(
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
) -> Result<Option<RuntimeGatewayOidcConfig>> {
    let Some(issuer) = policy.sso.oidc_issuer.as_deref() else {
        if policy.sso.oidc_audience.is_some()
            || policy.sso.oidc_jwks_url.is_some()
            || !policy.sso.oidc_jwks_origin_allowlist.is_empty()
        {
            bail!("gateway.sso OIDC requires oidc_issuer and oidc_audience");
        }
        return Ok(None);
    };
    let issuer = ValidatedOidcIssuer::parse(issuer)
        .context("gateway.sso.oidc_issuer must be a permitted HTTPS OIDC issuer")?;
    let audience = policy
        .sso
        .oidc_audience
        .as_deref()
        .context("gateway.sso.oidc_audience is required when oidc_issuer is set")
        .and_then(|value| gateway_required_policy_identifier("gateway.sso.oidc_audience", value))?;
    let endpoints = OidcEndpointPolicy::with_jwks_origin_allowlist(
        issuer.as_str(),
        policy.sso.oidc_jwks_url.as_deref(),
        policy
            .sso
            .oidc_jwks_origin_allowlist
            .iter()
            .map(String::as_str),
    )
    .context(
        "gateway.sso.oidc_jwks_url/gateway.sso.oidc_jwks_origin_allowlist must be permitted exact HTTPS OIDC origins",
    )?;
    Ok(Some(RuntimeGatewayOidcConfig {
        issuer: endpoints.issuer().as_str().to_string(),
        audience,
        jwks_url: endpoints
            .configured_jwks()
            .map(|endpoint| endpoint.as_str().to_string()),
        jwks_origin_allowlist: endpoints
            .allowed_jwks_origins()
            .map(str::to_string)
            .collect(),
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

    #[test]
    fn gateway_sso_config_rejects_malicious_oidc_urls() {
        for issuer in [
            "http://idp.example.test",
            "https://user@idp.example.test",
            "https://idp.example.test?token=secret",
            "https://idp.example.test#fragment",
            "https://127.0.0.1",
            "https://169.254.169.254",
            "https://[::1]",
        ] {
            let mut policy = gateway_oidc_policy();
            policy.sso.oidc_issuer = Some(issuer.to_string());
            let error = gateway_sso_config(&policy).unwrap_err();
            assert!(error.to_string().contains("gateway.sso.oidc_issuer"));
        }

        for jwks in [
            "https://keys.example.test/jwks.json",
            "http://idp.example.test/jwks.json",
            "https://idp.example.test:8443/jwks.json",
            "https://idp.example.test/jwks.json?token=secret",
            "https://10.0.0.1/jwks.json",
        ] {
            let mut policy = gateway_oidc_policy();
            policy.sso.oidc_jwks_url = Some(jwks.to_string());
            let error = gateway_sso_config(&policy).unwrap_err();
            assert!(error.to_string().contains("gateway.sso.oidc_jwks_url"));
        }
    }

    #[test]
    fn gateway_sso_config_bounds_oidc_runtime_durations() {
        let _prefetch = TestEnvVarGuard::set(PRODEX_GATEWAY_OIDC_PREFETCH_TIMEOUT_ENV, "10001");
        let error = gateway_sso_config(&gateway_oidc_policy()).unwrap_err();
        assert!(error.to_string().contains("must not exceed"));
    }

    #[test]
    fn gateway_sso_config_canonicalizes_explicit_jwks_origin_allowlist() {
        let mut policy = gateway_oidc_policy();
        policy.sso.oidc_jwks_url = Some("https://KEYS.example.test:8443/jwks.json".to_string());
        policy.sso.oidc_jwks_origin_allowlist = vec!["https://KEYS.example.test:8443".to_string()];

        let config = gateway_sso_config(&policy).unwrap().oidc.unwrap();
        assert_eq!(
            config.jwks_url.as_deref(),
            Some("https://keys.example.test:8443/jwks.json")
        );
        assert_eq!(
            config.jwks_origin_allowlist,
            ["https://keys.example.test:8443"]
        );
    }

    #[test]
    fn gateway_sso_config_rejects_non_origin_jwks_allowlist_entry() {
        let mut policy = gateway_oidc_policy();
        policy.sso.oidc_jwks_origin_allowlist =
            vec!["https://keys.example.test/not-an-origin".to_string()];

        let error = gateway_sso_config(&policy).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("gateway.sso.oidc_jwks_origin_allowlist")
        );
    }
}
