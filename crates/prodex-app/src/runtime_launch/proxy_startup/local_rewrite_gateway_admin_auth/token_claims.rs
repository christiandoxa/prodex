use super::super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::super::*;
use super::cache::runtime_gateway_log_jwks_snapshot_age_metric;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use std::collections::BTreeMap;
use std::time::Instant;

pub(super) fn runtime_gateway_verify_oidc_token(
    token: &str,
    config: &RuntimeGatewayOidcConfig,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Result<BTreeMap<String, serde_json::Value>> {
    let header = decode_header(token).context("failed to decode gateway OIDC JWT header")?;
    let alg = runtime_gateway_oidc_algorithm(header.alg)?;
    let snapshot = shared
        .gateway_oidc_jwks_snapshot
        .load_full()
        .ok_or_else(|| anyhow::anyhow!("gateway OIDC JWKS is not cached"))?;
    if !snapshot.usable_at(Instant::now()) {
        bail!("gateway OIDC JWKS cache is stale");
    }
    runtime_gateway_log_jwks_snapshot_age_metric(shared, &snapshot);
    let kid = header
        .kid
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("gateway OIDC JWT is missing kid"))?;
    let key = snapshot
        .jwks
        .find(kid)
        .ok_or_else(|| anyhow::anyhow!("gateway OIDC JWKS missing key for kid"))?;
    let decoding_key = DecodingKey::from_jwk(key).context("failed to build gateway OIDC key")?;
    let mut validation = Validation::new(alg);
    validation.set_audience(&[config.audience.as_str()]);
    validation.set_issuer(&[config.issuer.as_str()]);
    let data = decode::<BTreeMap<String, serde_json::Value>>(token, &decoding_key, &validation)
        .context("gateway OIDC token validation failed")?;
    Ok(data.claims)
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
