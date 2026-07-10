use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::local_rewrite_gateway_scope::RuntimeGatewayGovernanceScope;
use super::local_rewrite_gateway_store_types::{
    RuntimeGatewayScimUser, RuntimeGatewayVirtualKeyEntry,
};
use super::*;
use crate::{RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES, read_blocking_response_body_with_limit};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header, jwk::JwkSet};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

const RUNTIME_GATEWAY_OIDC_PREFETCH_TIMEOUT_ENV: &str = "PRODEX_GATEWAY_OIDC_PREFETCH_TIMEOUT_MS";
const DEFAULT_RUNTIME_GATEWAY_OIDC_PREFETCH_TIMEOUT_MS: u64 = 2_000;

pub(super) struct RuntimeGatewayAdminAuth {
    pub(super) name: String,
    pub(super) role: RuntimeGatewayAdminRole,
    pub(super) tenant_id: Option<String>,
    pub(super) team_id: Option<String>,
    pub(super) project_id: Option<String>,
    pub(super) user_id: Option<String>,
    pub(super) budget_id: Option<String>,
    pub(super) allowed_key_prefixes: Vec<String>,
}

pub(super) fn runtime_gateway_admin_auth(
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Option<RuntimeGatewayAdminAuth> {
    if let Some(auth) = runtime_gateway_oidc_admin_auth(captured, shared) {
        return Some(auth);
    }
    if let Some(auth) = runtime_gateway_sso_admin_auth(captured, shared) {
        return Some(auth);
    }
    let authorization = captured
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("authorization"))
        .map(|(_, value)| value.as_str())?;
    for token in &shared.gateway_admin_tokens {
        if token.token_hash.verify_authorization_header(authorization) {
            return Some(RuntimeGatewayAdminAuth {
                name: token.name.clone(),
                role: token.role,
                tenant_id: token.tenant_id.clone(),
                team_id: token.team_id.clone(),
                project_id: token.project_id.clone(),
                user_id: token.user_id.clone(),
                budget_id: token.budget_id.clone(),
                allowed_key_prefixes: token.allowed_key_prefixes.clone(),
            });
        }
    }
    if let Some(auth_token_hash) = shared.gateway_auth_token_hash.as_ref()
        && auth_token_hash.verify_authorization_header(authorization)
    {
        return Some(RuntimeGatewayAdminAuth {
            name: "default-admin".to_string(),
            role: RuntimeGatewayAdminRole::Admin,
            tenant_id: None,
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            allowed_key_prefixes: Vec::new(),
        });
    }
    None
}

fn runtime_gateway_oidc_admin_auth(
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Option<RuntimeGatewayAdminAuth> {
    let config = shared.gateway_sso.oidc.as_ref()?;
    let authorization = runtime_gateway_header(captured, "authorization")?;
    let token = runtime_gateway_authorization_bearer_token(authorization)?;
    let claims = runtime_gateway_verify_oidc_token(token, config, shared).ok()?;
    let name = runtime_gateway_oidc_claim_string(&claims, &config.user_claim)
        .or_else(|| runtime_gateway_oidc_claim_string(&claims, "email"))
        .or_else(|| runtime_gateway_oidc_claim_string(&claims, "preferred_username"))
        .or_else(|| runtime_gateway_oidc_claim_string(&claims, "sub"))?;
    let scim_user = match runtime_gateway_scim_user_by_name(shared, &name) {
        Ok(user) => user,
        Err(()) => return None,
    };
    if scim_user.as_ref().is_some_and(|user| !user.active) {
        return None;
    }
    let role = runtime_gateway_sso_resolved_role(
        runtime_gateway_oidc_claim_string(&claims, &config.role_claim).as_deref(),
        scim_user.as_ref(),
    );
    let tenant_id = runtime_gateway_oidc_claim_scope_string(&claims, &config.tenant_claim)
        .ok()?
        .or_else(|| scim_user.as_ref().and_then(|user| user.tenant_id.clone()));
    if shared.gateway_sso.require_tenant && tenant_id.is_none() {
        return None;
    }
    let team_id = scim_user.as_ref().and_then(|user| user.team_id.clone());
    let project_id = scim_user.as_ref().and_then(|user| user.project_id.clone());
    let user_id = scim_user.as_ref().and_then(|user| user.user_id.clone());
    let budget_id = scim_user.as_ref().and_then(|user| user.budget_id.clone());
    let allowed_key_prefixes =
        runtime_gateway_oidc_claim_string_vec(&claims, &config.key_prefixes_claim)
            .ok()?
            .or_else(|| {
                scim_user
                    .as_ref()
                    .map(|user| user.allowed_key_prefixes.clone())
            })
            .unwrap_or_default();
    Some(RuntimeGatewayAdminAuth {
        name: format!("oidc:{name}"),
        role,
        tenant_id,
        team_id,
        project_id,
        user_id,
        budget_id,
        allowed_key_prefixes,
    })
}

fn runtime_gateway_verify_oidc_token(
    token: &str,
    config: &RuntimeGatewayOidcConfig,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Result<BTreeMap<String, serde_json::Value>> {
    let header = decode_header(token).context("failed to decode gateway OIDC JWT header")?;
    let alg = runtime_gateway_oidc_algorithm(header.alg)?;
    let jwks_url = runtime_gateway_oidc_cached_jwks_url(config, shared)?;
    let jwks_value = runtime_gateway_oidc_cached_json(shared, &jwks_url, "JWKS")?;
    let jwks: JwkSet =
        serde_json::from_value(jwks_value).context("failed to parse gateway OIDC JWKS")?;
    let kid = header
        .kid
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("gateway OIDC JWT is missing kid"))?;
    let key = jwks
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

pub(super) fn runtime_gateway_prefetch_oidc_cache(
    shared: &RuntimeLocalRewriteProxyShared,
) -> Result<()> {
    let Some(config) = shared.gateway_sso.oidc.as_ref() else {
        return Ok(());
    };
    let jwks_url = runtime_gateway_oidc_fetch_jwks_url(config, shared)?;
    runtime_gateway_oidc_fetch_json(shared, &jwks_url, "JWKS")?;
    Ok(())
}

fn runtime_gateway_oidc_cached_json(
    shared: &RuntimeLocalRewriteProxyShared,
    url: &str,
    description: &str,
) -> Result<serde_json::Value> {
    let cached_payload = shared
        .gateway_oidc_http_cache
        .lock()
        .ok()
        .and_then(|cache| cache.get(url).cloned());
    if let Some((_, payload)) = cached_payload.as_ref() {
        return serde_json::from_str(payload)
            .with_context(|| format!("failed to parse cached gateway OIDC {description}"));
    }
    bail!("gateway OIDC {description} is not cached")
}

fn runtime_gateway_oidc_fetch_json(
    shared: &RuntimeLocalRewriteProxyShared,
    url: &str,
    description: &str,
) -> Result<serde_json::Value> {
    let now = Instant::now();
    let fetched_payload = shared
        .client
        .get(url)
        .timeout(runtime_gateway_oidc_prefetch_timeout())
        .send()
        .with_context(|| format!("failed to fetch gateway OIDC {description}"))
        .and_then(|response| {
            response
                .error_for_status()
                .with_context(|| format!("gateway OIDC {description} endpoint returned an error"))
        })
        .and_then(|response| {
            read_blocking_response_body_with_limit(
                response,
                RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES,
                &format!("failed to read gateway OIDC {description}"),
            )
        });
    match fetched_payload {
        Ok(payload) => {
            let parsed: serde_json::Value = serde_json::from_slice(&payload)
                .with_context(|| format!("failed to parse gateway OIDC {description}"))?;
            let payload = String::from_utf8(payload)
                .with_context(|| format!("gateway OIDC {description} is not UTF-8"))?;
            if let Ok(mut cache) = shared.gateway_oidc_http_cache.lock() {
                cache.insert(url.to_string(), (now, payload));
            }
            Ok(parsed)
        }
        Err(err) => Err(err),
    }
}

fn runtime_gateway_oidc_prefetch_timeout() -> Duration {
    Duration::from_millis(
        std::env::var(RUNTIME_GATEWAY_OIDC_PREFETCH_TIMEOUT_ENV)
            .ok()
            .and_then(|value| value.trim().parse::<u64>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(DEFAULT_RUNTIME_GATEWAY_OIDC_PREFETCH_TIMEOUT_MS),
    )
}

fn runtime_gateway_oidc_cached_jwks_url(
    config: &RuntimeGatewayOidcConfig,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Result<String> {
    if let Some(jwks_url) = config.jwks_url.as_deref() {
        return Ok(jwks_url.to_string());
    }
    let discovery_url = format!(
        "{}/.well-known/openid-configuration",
        config.issuer.trim_end_matches('/')
    );
    let discovery = runtime_gateway_oidc_cached_json(shared, &discovery_url, "discovery document")?;
    discovery
        .get("jwks_uri")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("gateway OIDC discovery document is missing jwks_uri"))
}

fn runtime_gateway_oidc_fetch_jwks_url(
    config: &RuntimeGatewayOidcConfig,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Result<String> {
    if let Some(jwks_url) = config.jwks_url.as_deref() {
        return Ok(jwks_url.to_string());
    }
    let discovery_url = format!(
        "{}/.well-known/openid-configuration",
        config.issuer.trim_end_matches('/')
    );
    let discovery = runtime_gateway_oidc_fetch_json(shared, &discovery_url, "discovery document")?;
    discovery
        .get("jwks_uri")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("gateway OIDC discovery document is missing jwks_uri"))
}

fn runtime_gateway_oidc_algorithm(alg: Algorithm) -> Result<Algorithm> {
    match alg {
        Algorithm::RS256
        | Algorithm::RS384
        | Algorithm::RS512
        | Algorithm::ES256
        | Algorithm::ES384 => Ok(alg),
        _ => bail!("gateway OIDC JWT algorithm is not allowed"),
    }
}

fn runtime_gateway_authorization_bearer_token(value: &str) -> Option<&str> {
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

fn runtime_gateway_oidc_claim_string(
    claims: &BTreeMap<String, serde_json::Value>,
    field: &str,
) -> Option<String> {
    claims
        .get(field)
        .and_then(serde_json::Value::as_str)
        .and_then(runtime_gateway_exact_scope_string)
}

fn runtime_gateway_oidc_claim_scope_string(
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

fn runtime_gateway_oidc_claim_string_vec(
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

fn runtime_gateway_sso_admin_auth(
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Option<RuntimeGatewayAdminAuth> {
    let config = &shared.gateway_sso;
    let proxy_token_hash = config.proxy_token_hash.as_ref()?;
    let proxy_token = runtime_gateway_header(captured, &config.token_header)?;
    if !runtime_gateway_sso_proxy_token_matches(proxy_token_hash, proxy_token) {
        return None;
    }
    let name =
        runtime_gateway_sso_user_name(runtime_gateway_header(captured, &config.user_header))?;
    let scim_user = match runtime_gateway_scim_user_by_name(shared, name) {
        Ok(user) => user,
        Err(()) => return None,
    };
    if scim_user.as_ref().is_some_and(|user| !user.active) {
        return None;
    }
    let role = runtime_gateway_sso_resolved_role(
        runtime_gateway_header(captured, &config.role_header),
        scim_user.as_ref(),
    );
    let tenant_id = match runtime_gateway_header(captured, &config.tenant_header) {
        Some(value) => Some(runtime_gateway_exact_scope_string(value)?),
        None => None,
    }
    .or_else(|| scim_user.as_ref().and_then(|user| user.tenant_id.clone()));
    if config.require_tenant && tenant_id.is_none() {
        return None;
    }
    let team_id = scim_user.as_ref().and_then(|user| user.team_id.clone());
    let project_id = scim_user.as_ref().and_then(|user| user.project_id.clone());
    let user_id = scim_user.as_ref().and_then(|user| user.user_id.clone());
    let budget_id = scim_user.as_ref().and_then(|user| user.budget_id.clone());
    let allowed_key_prefixes =
        match runtime_gateway_header(captured, &config.key_prefixes_header) {
            Some(value) => Some(runtime_gateway_parse_sso_prefixes(value)?),
            None => None,
        }
        .or_else(|| {
            scim_user
                .as_ref()
                .map(|user| user.allowed_key_prefixes.clone())
        })
        .unwrap_or_default();
    Some(RuntimeGatewayAdminAuth {
        name: format!("sso:{name}"),
        role,
        tenant_id,
        team_id,
        project_id,
        user_id,
        budget_id,
        allowed_key_prefixes,
    })
}

fn runtime_gateway_sso_proxy_token_matches(
    proxy_token_hash: &runtime_proxy_crate::LocalBridgeBearerTokenHash,
    proxy_token: &str,
) -> bool {
    proxy_token_hash.verify_bearer_token(proxy_token)
}

fn runtime_gateway_sso_user_name(value: Option<&str>) -> Option<&str> {
    match value {
        Some(value) if !value.is_empty() && !value.chars().any(char::is_whitespace) => Some(value),
        Some(_) => None,
        None => Some("sso-admin"),
    }
}

fn runtime_gateway_sso_resolved_role(
    role_claim: Option<&str>,
    scim_user: Option<&RuntimeGatewayScimUser>,
) -> RuntimeGatewayAdminRole {
    role_claim
        .and_then(RuntimeGatewayAdminRole::parse)
        .or_else(|| {
            scim_user
                .and_then(|user| user.role.as_deref())
                .and_then(RuntimeGatewayAdminRole::parse)
        })
        .unwrap_or(RuntimeGatewayAdminRole::Viewer)
}

fn runtime_gateway_header<'a>(
    captured: &'a RuntimeProxyRequest,
    header_name: &str,
) -> Option<&'a str> {
    captured
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(header_name))
        .map(|(_, value)| value.as_str())
}

fn runtime_gateway_parse_sso_prefixes(value: &str) -> Option<Vec<String>> {
    value
        .split([',', ';', '\n'])
        .map(runtime_gateway_exact_scope_string)
        .collect()
}

fn runtime_gateway_exact_scope_string(value: &str) -> Option<String> {
    (!value.is_empty() && !value.chars().any(char::is_whitespace)).then(|| value.to_string())
}

fn runtime_gateway_scim_user_by_name(
    shared: &RuntimeLocalRewriteProxyShared,
    name: &str,
) -> Result<Option<RuntimeGatewayScimUser>, ()> {
    if name.is_empty() || name.chars().any(char::is_whitespace) {
        return Ok(None);
    }
    let store = match super::local_rewrite::runtime_gateway_virtual_key_store_load_strict(
        &shared.gateway_state_store,
        &shared.runtime_shared.log_path,
    ) {
        Ok(store) => store,
        Err(err) => {
            runtime_proxy_log(
                &shared.runtime_shared,
                runtime_proxy_structured_log_message(
                    "gateway_admin_scim_state_unavailable",
                    [runtime_proxy_log_field("error", err.to_string())],
                ),
            );
            return Err(());
        }
    };
    Ok(store
        .scim_users
        .into_iter()
        .find(|user| user.user_name.eq_ignore_ascii_case(name)))
}

impl RuntimeGatewayAdminAuth {
    pub(super) fn governance_scope(&self) -> RuntimeGatewayGovernanceScope {
        RuntimeGatewayGovernanceScope::new(
            self.tenant_id.clone(),
            self.team_id.clone(),
            self.project_id.clone(),
            self.user_id.clone(),
            self.budget_id.clone(),
        )
    }

    pub(super) fn can_access_tenant(&self, tenant_id: Option<&str>) -> bool {
        self.governance_scope().matches_tenant(tenant_id)
    }

    pub(super) fn can_access_dimensions(
        &self,
        team_id: Option<&str>,
        project_id: Option<&str>,
        user_id: Option<&str>,
        budget_id: Option<&str>,
    ) -> bool {
        self.governance_scope()
            .matches_dimensions(team_id, project_id, user_id, budget_id)
    }

    pub(super) fn can_access_key(&self, key_name: &str) -> bool {
        self.allowed_key_prefixes.is_empty()
            || self
                .allowed_key_prefixes
                .iter()
                .any(|prefix| key_name.starts_with(prefix))
    }

    pub(super) fn can_access_entry(&self, entry: &RuntimeGatewayVirtualKeyEntry) -> bool {
        self.governance_scope().matches(
            entry.tenant_id.as_deref(),
            entry.key.team_id.as_deref(),
            entry.key.project_id.as_deref(),
            entry.key.user_id.as_deref(),
            entry.key.budget_id.as_deref(),
        ) && self.can_access_key(&entry.key.name)
    }

    pub(super) fn can_access_scim_user(&self, user: &RuntimeGatewayScimUser) -> bool {
        self.governance_scope().matches(
            user.tenant_id.as_deref(),
            user.team_id.as_deref(),
            user.project_id.as_deref(),
            user.user_id.as_deref(),
            user.budget_id.as_deref(),
        )
    }
}

pub(super) fn runtime_gateway_admin_auth_is_unscoped(admin_auth: &RuntimeGatewayAdminAuth) -> bool {
    admin_auth.governance_scope().is_unscoped()
}

pub(super) fn runtime_gateway_admin_auth_matches_entry(
    admin_auth: &RuntimeGatewayAdminAuth,
    entry: &RuntimeGatewayVirtualKeyEntry,
) -> bool {
    admin_auth.governance_scope().matches(
        entry.tenant_id.as_deref(),
        entry.key.team_id.as_deref(),
        entry.key.project_id.as_deref(),
        entry.key.user_id.as_deref(),
        entry.key.budget_id.as_deref(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oidc_prefetch_timeout_uses_positive_env_override() {
        let _guard = crate::TestEnvVarGuard::set(RUNTIME_GATEWAY_OIDC_PREFETCH_TIMEOUT_ENV, "25");

        assert_eq!(
            runtime_gateway_oidc_prefetch_timeout(),
            Duration::from_millis(25)
        );
    }

    #[test]
    fn oidc_prefetch_timeout_rejects_zero_and_invalid_values() {
        for value in ["0", "not-a-number"] {
            let _guard =
                crate::TestEnvVarGuard::set(RUNTIME_GATEWAY_OIDC_PREFETCH_TIMEOUT_ENV, value);

            assert_eq!(
                runtime_gateway_oidc_prefetch_timeout(),
                Duration::from_millis(DEFAULT_RUNTIME_GATEWAY_OIDC_PREFETCH_TIMEOUT_MS)
            );
        }
    }

    #[test]
    fn sso_role_resolution_never_defaults_missing_or_unknown_to_admin() {
        assert_eq!(
            runtime_gateway_sso_resolved_role(None, None),
            RuntimeGatewayAdminRole::Viewer
        );
        assert_eq!(
            runtime_gateway_sso_resolved_role(Some("not-admin"), None),
            RuntimeGatewayAdminRole::Viewer
        );
        assert_eq!(
            runtime_gateway_sso_resolved_role(Some(" admin "), None),
            RuntimeGatewayAdminRole::Viewer
        );
    }

    #[test]
    fn sso_proxy_token_matches_exactly_without_trimming() {
        let token_hash =
            runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token("sso-proxy-token");

        assert!(runtime_gateway_sso_proxy_token_matches(
            &token_hash,
            "sso-proxy-token"
        ));
        assert!(!runtime_gateway_sso_proxy_token_matches(
            &token_hash,
            " sso-proxy-token "
        ));
    }

    #[test]
    fn sso_user_names_match_exactly_without_trimming() {
        assert_eq!(runtime_gateway_sso_user_name(None), Some("sso-admin"));
        assert_eq!(
            runtime_gateway_sso_user_name(Some("alice@example.com")),
            Some("alice@example.com")
        );
        assert_eq!(
            runtime_gateway_sso_user_name(Some(" alice@example.com ")),
            None
        );
        assert_eq!(runtime_gateway_sso_user_name(Some("")), None);
    }

    #[test]
    fn oidc_principal_claims_match_exactly_without_trimming() {
        let claims = BTreeMap::from([
            (
                "email".to_string(),
                serde_json::json!(" alice@example.com "),
            ),
            (
                "preferred_username".to_string(),
                serde_json::json!("bob@example.com"),
            ),
        ]);

        assert_eq!(runtime_gateway_oidc_claim_string(&claims, "email"), None);
        assert_eq!(
            runtime_gateway_oidc_claim_string(&claims, "preferred_username"),
            Some("bob@example.com".to_string())
        );
    }

    #[test]
    fn sso_key_prefixes_reject_empty_or_whitespace_scope_parts() {
        assert_eq!(
            runtime_gateway_parse_sso_prefixes("team-a-,team-b-"),
            Some(vec!["team-a-".to_string(), "team-b-".to_string()])
        );
        assert_eq!(runtime_gateway_parse_sso_prefixes(""), None);
        assert_eq!(runtime_gateway_parse_sso_prefixes("team-a-, team-b-"), None);
        assert_eq!(runtime_gateway_parse_sso_prefixes("team-a-,"), None);
    }

    #[test]
    fn oidc_key_prefix_claim_rejects_whitespace_scope_parts() {
        let claims = BTreeMap::from([(
            "prodex_key_prefixes".to_string(),
            serde_json::json!(["team-a-", " team-b-"]),
        )]);

        assert!(runtime_gateway_oidc_claim_string_vec(&claims, "prodex_key_prefixes").is_err());
    }
}
