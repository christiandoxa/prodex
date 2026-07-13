use super::super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::super::local_rewrite_gateway_util::runtime_gateway_unix_epoch_millis;
use super::super::*;
use super::cache::runtime_gateway_log_jwks_snapshot_age_metric;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use prodex_authn::TokenClaims;
use prodex_domain::{
    Audience, CredentialScope, Issuer, JwksCacheSnapshot, JwtAlgorithm, OidcValidationPolicy,
    PrincipalId, PrincipalKind, TenantId,
};
use std::collections::BTreeMap;
use std::time::Instant;

pub(in super::super) struct RuntimeGatewayVerifiedOidcToken {
    pub(super) claims: BTreeMap<String, serde_json::Value>,
    policy: OidcValidationPolicy,
    jwks_snapshot: JwksCacheSnapshot,
    algorithm: JwtAlgorithm,
    key_id: String,
    issuer: Issuer,
    audience: Audience,
    expires_at_unix_ms: u64,
    not_before_unix_ms: Option<u64>,
    now_unix_ms: u64,
}

impl RuntimeGatewayVerifiedOidcToken {
    pub(in super::super) fn canonical_claims(
        &self,
        principal_id: PrincipalId,
        claimed_tenant_id: Option<TenantId>,
        role_claim: Option<String>,
    ) -> TokenClaims {
        TokenClaims {
            issuer: self.issuer.clone(),
            audience: self.audience.clone(),
            algorithm: self.algorithm,
            key_id: self.key_id.clone(),
            key_known: true,
            signature_verified: true,
            principal_id,
            tenant_id: claimed_tenant_id,
            principal_kind: PrincipalKind::User,
            credential_scope: CredentialScope::ControlPlane,
            role_claim,
            expires_at_unix_ms: self.expires_at_unix_ms,
            not_before_unix_ms: self.not_before_unix_ms,
        }
    }

    pub(in super::super) fn policy(&self) -> OidcValidationPolicy {
        self.policy.clone()
    }

    pub(in super::super) fn jwks_snapshot(&self) -> JwksCacheSnapshot {
        self.jwks_snapshot.clone()
    }

    pub(in super::super) fn now_unix_ms(&self) -> u64 {
        self.now_unix_ms
    }
}

#[cfg(test)]
pub(in super::super) fn runtime_gateway_test_verified_oidc_token() -> RuntimeGatewayVerifiedOidcToken
{
    let issuer = Issuer::new("https://issuer.example.com").unwrap();
    let audience = Audience::new("prodex-gateway").unwrap();
    RuntimeGatewayVerifiedOidcToken {
        claims: BTreeMap::new(),
        policy: OidcValidationPolicy::new(
            issuer.clone(),
            vec![audience.clone()],
            vec![JwtAlgorithm::Rs256],
        )
        .unwrap(),
        jwks_snapshot: JwksCacheSnapshot {
            fetched_at_unix_ms: 1_000,
            expires_at_unix_ms: 10_000,
            stale_until_unix_ms: 20_000,
            key_count: 1,
            last_refresh_error_at_unix_ms: None,
            retry_after_unix_ms: None,
        },
        algorithm: JwtAlgorithm::Rs256,
        key_id: "kid-1".to_string(),
        issuer,
        audience,
        expires_at_unix_ms: 9_000,
        not_before_unix_ms: Some(1_000),
        now_unix_ms: 2_000,
    }
}

pub(super) fn runtime_gateway_verify_oidc_token(
    token: &str,
    config: &RuntimeGatewayOidcConfig,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Result<RuntimeGatewayVerifiedOidcToken> {
    let header = decode_header(token).context("failed to decode gateway OIDC JWT header")?;
    let alg = runtime_gateway_oidc_algorithm(header.alg)?;
    let algorithm = runtime_gateway_domain_oidc_algorithm(alg)?;
    let now = Instant::now();
    let now_unix_ms = runtime_gateway_unix_epoch_millis();
    let snapshot = shared
        .gateway_oidc_jwks_snapshot
        .load_full()
        .ok_or_else(|| anyhow::anyhow!("gateway OIDC JWKS is not cached"))?;
    if !snapshot.usable_at(now) {
        bail!("gateway OIDC JWKS cache is stale");
    }
    runtime_gateway_log_jwks_snapshot_age_metric(shared, &snapshot);
    let kid = header
        .kid
        .ok_or_else(|| anyhow::anyhow!("gateway OIDC JWT is missing kid"))?;
    let key = snapshot
        .jwks
        .find(&kid)
        .ok_or_else(|| anyhow::anyhow!("gateway OIDC JWKS missing key for kid"))?;
    let decoding_key = DecodingKey::from_jwk(key).context("failed to build gateway OIDC key")?;
    let mut validation = Validation::new(alg);
    validation.set_audience(&[config.audience.as_str()]);
    validation.set_issuer(&[config.issuer.as_str()]);
    let data = decode::<BTreeMap<String, serde_json::Value>>(token, &decoding_key, &validation)
        .context("gateway OIDC token validation failed")?;
    runtime_gateway_verified_oidc_token(
        data.claims,
        config,
        snapshot.domain_snapshot_at(now, now_unix_ms),
        algorithm,
        kid,
        now_unix_ms,
    )
}

fn runtime_gateway_verified_oidc_token(
    claims: BTreeMap<String, serde_json::Value>,
    config: &RuntimeGatewayOidcConfig,
    jwks_snapshot: JwksCacheSnapshot,
    algorithm: JwtAlgorithm,
    key_id: String,
    now_unix_ms: u64,
) -> Result<RuntimeGatewayVerifiedOidcToken> {
    let issuer = claims
        .get("iss")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("gateway OIDC token is missing issuer"))?;
    let issuer = runtime_gateway_oidc_domain_issuer(issuer)?;
    runtime_gateway_require_oidc_audience(&claims, &config.audience)?;
    runtime_gateway_require_oidc_authentication_strength(
        &claims,
        config.authentication_strength.as_deref(),
    )?;
    let audience =
        Audience::new(config.audience.clone()).context("gateway OIDC token audience is invalid")?;
    let expires_at_unix_ms = runtime_gateway_oidc_numeric_date_ms(&claims, "exp")?
        .ok_or_else(|| anyhow::anyhow!("gateway OIDC token is missing expiration"))?;
    let not_before_unix_ms = runtime_gateway_oidc_numeric_date_ms(&claims, "nbf")?;
    let policy = runtime_gateway_oidc_validation_policy(config)?;
    Ok(RuntimeGatewayVerifiedOidcToken {
        claims,
        policy,
        jwks_snapshot,
        algorithm,
        key_id,
        issuer,
        audience,
        expires_at_unix_ms,
        not_before_unix_ms,
        now_unix_ms,
    })
}

fn runtime_gateway_oidc_validation_policy(
    config: &RuntimeGatewayOidcConfig,
) -> Result<OidcValidationPolicy> {
    OidcValidationPolicy::new(
        runtime_gateway_oidc_domain_issuer(&config.issuer)?,
        vec![Audience::new(config.audience.clone())?],
        vec![
            JwtAlgorithm::Rs256,
            JwtAlgorithm::Rs384,
            JwtAlgorithm::Rs512,
            JwtAlgorithm::Es256,
            JwtAlgorithm::Es384,
        ],
    )
    .map_err(Into::into)
}

fn runtime_gateway_oidc_domain_issuer(value: &str) -> Result<Issuer> {
    match Issuer::new(value) {
        Ok(issuer) => Ok(issuer),
        Err(error) => {
            #[cfg(test)]
            if reqwest::Url::parse(value).is_ok_and(|url| {
                super::endpoint_policy::runtime_gateway_oidc_test_url_is_insecure_loopback(&url)
            }) {
                return Issuer::new("https://loopback-oidc-test.invalid").map_err(Into::into);
            }
            Err(error).context("gateway OIDC token issuer is invalid")
        }
    }
}

fn runtime_gateway_require_oidc_audience(
    claims: &BTreeMap<String, serde_json::Value>,
    expected: &str,
) -> Result<()> {
    let audience_matches = claims.get("aud").is_some_and(|audience| {
        audience.as_str() == Some(expected)
            || audience.as_array().is_some_and(|audiences| {
                audiences
                    .iter()
                    .any(|audience| audience.as_str() == Some(expected))
            })
    });
    if !audience_matches {
        bail!("gateway OIDC token audience is missing or invalid");
    }
    Ok(())
}

fn runtime_gateway_require_oidc_authentication_strength(
    claims: &BTreeMap<String, serde_json::Value>,
    expected: Option<&str>,
) -> Result<()> {
    let Some(expected) = expected else {
        return Ok(());
    };
    if claims.get("acr").and_then(serde_json::Value::as_str) != Some(expected) {
        bail!("gateway OIDC token authentication strength is missing or invalid");
    }
    Ok(())
}

fn runtime_gateway_oidc_numeric_date_ms(
    claims: &BTreeMap<String, serde_json::Value>,
    field: &str,
) -> Result<Option<u64>> {
    let Some(value) = claims.get(field) else {
        return Ok(None);
    };
    value
        .as_u64()
        .and_then(|seconds| seconds.checked_mul(1_000))
        .map(Some)
        .ok_or_else(|| anyhow::anyhow!("gateway OIDC token time claim is invalid"))
}

pub(super) fn runtime_gateway_oidc_algorithm(alg: Algorithm) -> Result<Algorithm> {
    match alg {
        Algorithm::RS256
        | Algorithm::RS384
        | Algorithm::RS512
        | Algorithm::ES256
        | Algorithm::ES384 => Ok(alg),
        _ => bail!("gateway OIDC JWT algorithm is not allowed"),
    }
}

pub(super) fn runtime_gateway_domain_oidc_algorithm(alg: Algorithm) -> Result<JwtAlgorithm> {
    match alg {
        Algorithm::RS256 => Ok(JwtAlgorithm::Rs256),
        Algorithm::RS384 => Ok(JwtAlgorithm::Rs384),
        Algorithm::RS512 => Ok(JwtAlgorithm::Rs512),
        Algorithm::ES256 => Ok(JwtAlgorithm::Es256),
        Algorithm::ES384 => Ok(JwtAlgorithm::Es384),
        _ => bail!("gateway OIDC JWT algorithm is not allowed"),
    }
}

pub(super) fn runtime_gateway_authorization_bearer_token(value: &str) -> Option<&str> {
    let mut parts = value.split_whitespace();
    let scheme = parts.next()?;
    if !scheme.eq_ignore_ascii_case("bearer") {
        return None;
    }
    let token = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    Some(token)
}

#[cfg(test)]
pub(super) fn runtime_gateway_oidc_claim_string(
    claims: &BTreeMap<String, serde_json::Value>,
    field: &str,
) -> Option<String> {
    claims
        .get(field)
        .and_then(serde_json::Value::as_str)
        .and_then(runtime_gateway_exact_scope_string)
}

pub(super) fn runtime_gateway_oidc_admin_name(
    claims: &BTreeMap<String, serde_json::Value>,
    config: &RuntimeGatewayOidcConfig,
) -> Option<String> {
    let configured_claim =
        runtime_gateway_oidc_claim_scope_string(claims, &config.user_claim).ok()?;
    if let Some(name) = configured_claim {
        return Some(name);
    }
    for claim in ["email", "preferred_username", "sub"] {
        if claim == config.user_claim {
            continue;
        }
        if let Some(name) = runtime_gateway_oidc_claim_scope_string(claims, claim).ok()? {
            return Some(name);
        }
    }
    None
}

pub(super) fn runtime_gateway_oidc_claim_scope_string(
    claims: &BTreeMap<String, serde_json::Value>,
    field: &str,
) -> Result<Option<String>, ()> {
    let Some(value) = claims.get(field) else {
        return Ok(None);
    };
    value
        .as_str()
        .and_then(runtime_gateway_exact_scope_string)
        .map(Some)
        .ok_or(())
}

pub(super) fn runtime_gateway_oidc_role_claim_string(
    claims: &BTreeMap<String, serde_json::Value>,
    field: &str,
) -> Option<String> {
    runtime_gateway_oidc_claim_scope_string(claims, field).unwrap_or_else(|()| Some(String::new()))
}

pub(super) fn runtime_gateway_oidc_claim_string_vec(
    claims: &BTreeMap<String, serde_json::Value>,
    field: &str,
) -> Result<Option<Vec<String>>, ()> {
    let Some(value) = claims.get(field) else {
        return Ok(None);
    };
    if let Some(value) = value.as_str() {
        return runtime_gateway_parse_sso_prefixes(value)
            .map(Some)
            .ok_or(());
    }
    let values = value.as_array().ok_or(())?;
    values
        .iter()
        .map(|value| runtime_gateway_exact_scope_string(value.as_str()?))
        .collect::<Option<Vec<_>>>()
        .map(Some)
        .ok_or(())
}

pub(super) fn runtime_gateway_parse_sso_prefixes(value: &str) -> Option<Vec<String>> {
    value
        .split([',', ';', '\n'])
        .map(runtime_gateway_exact_scope_string)
        .collect()
}

pub(super) fn runtime_gateway_exact_scope_string(value: &str) -> Option<String> {
    (!value.is_empty() && !value.chars().any(char::is_whitespace)).then(|| value.to_string())
}

#[cfg(test)]
mod authentication_strength_tests {
    use super::*;

    #[test]
    fn configured_authentication_strength_requires_exact_acr_claim() {
        let mut claims = BTreeMap::new();
        assert!(
            runtime_gateway_require_oidc_authentication_strength(
                &claims,
                Some("phishing_resistant")
            )
            .is_err()
        );
        claims.insert("acr".to_string(), serde_json::json!("mfa"));
        assert!(
            runtime_gateway_require_oidc_authentication_strength(
                &claims,
                Some("phishing_resistant")
            )
            .is_err()
        );
        claims.insert("acr".to_string(), serde_json::json!("phishing_resistant"));
        runtime_gateway_require_oidc_authentication_strength(&claims, Some("phishing_resistant"))
            .unwrap();
    }
}
