use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::local_rewrite_gateway_scope::RuntimeGatewayGovernanceScope;
use super::local_rewrite_gateway_store_types::{
    RuntimeGatewayScimUser, RuntimeGatewayVirtualKeyEntry,
};
use super::*;
use crate::{RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES, read_blocking_response_body_with_limit};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header, jwk::JwkSet};
use std::collections::BTreeMap;

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
    let role = runtime_gateway_oidc_claim_string(&claims, &config.role_claim)
        .and_then(|role| RuntimeGatewayAdminRole::parse(&role))
        .or_else(|| {
            scim_user
                .as_ref()
                .and_then(|user| user.role.as_deref())
                .and_then(RuntimeGatewayAdminRole::parse)
        })
        .unwrap_or(shared.gateway_sso.default_role);
    let tenant_id = runtime_gateway_oidc_claim_string(&claims, &config.tenant_claim)
        .or_else(|| scim_user.as_ref().and_then(|user| user.tenant_id.clone()));
    let team_id = scim_user.as_ref().and_then(|user| user.team_id.clone());
    let project_id = scim_user.as_ref().and_then(|user| user.project_id.clone());
    let user_id = scim_user.as_ref().and_then(|user| user.user_id.clone());
    let budget_id = scim_user.as_ref().and_then(|user| user.budget_id.clone());
    let allowed_key_prefixes =
        runtime_gateway_oidc_claim_string_vec(&claims, &config.key_prefixes_claim)
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
    let jwks_url = runtime_gateway_oidc_jwks_url(config, shared)?;
    let jwks_response = shared
        .client
        .get(&jwks_url)
        .send()
        .context("failed to fetch gateway OIDC JWKS")?
        .error_for_status()
        .context("gateway OIDC JWKS endpoint returned an error")?;
    let jwks_body = read_blocking_response_body_with_limit(
        jwks_response,
        RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES,
        "failed to read gateway OIDC JWKS",
    )?;
    let jwks = serde_json::from_slice::<JwkSet>(&jwks_body)
        .context("failed to parse gateway OIDC JWKS")?;
    let key = jwks
        .keys
        .iter()
        .find(|key| {
            header
                .kid
                .as_deref()
                .map(|kid| key.common.key_id.as_deref() == Some(kid))
                .unwrap_or(true)
        })
        .ok_or_else(|| anyhow::anyhow!("gateway OIDC JWKS did not contain the JWT key"))?;
    let decoding_key =
        DecodingKey::from_jwk(key).context("failed to build gateway OIDC decoding key")?;
    let mut validation = Validation::new(alg);
    validation.set_audience(&[config.audience.as_str()]);
    validation.set_issuer(&[config.issuer.as_str()]);
    validation.algorithms = vec![alg];
    let token = decode::<BTreeMap<String, serde_json::Value>>(token, &decoding_key, &validation)
        .context("failed to verify gateway OIDC JWT")?;
    Ok(token.claims)
}

fn runtime_gateway_oidc_jwks_url(
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
    let discovery_response = shared
        .client
        .get(&discovery_url)
        .send()
        .context("failed to fetch gateway OIDC discovery document")?
        .error_for_status()
        .context("gateway OIDC discovery endpoint returned an error")?;
    let discovery_body = read_blocking_response_body_with_limit(
        discovery_response,
        RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES,
        "failed to read gateway OIDC discovery document",
    )?;
    let discovery = serde_json::from_slice::<serde_json::Value>(&discovery_body)
        .context("failed to parse gateway OIDC discovery document")?;
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
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn runtime_gateway_oidc_claim_string_vec(
    claims: &BTreeMap<String, serde_json::Value>,
    field: &str,
) -> Option<Vec<String>> {
    let value = claims.get(field)?;
    if let Some(value) = value.as_str() {
        return Some(runtime_gateway_parse_sso_prefixes(value));
    }
    value.as_array().map(|values| {
        values
            .iter()
            .filter_map(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .collect()
    })
}

fn runtime_gateway_sso_admin_auth(
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Option<RuntimeGatewayAdminAuth> {
    let config = &shared.gateway_sso;
    let proxy_token_hash = config.proxy_token_hash.as_ref()?;
    let proxy_token = runtime_gateway_header(captured, &config.token_header)?;
    if !proxy_token_hash.verify_bearer_token(proxy_token.trim()) {
        return None;
    }
    let name = runtime_gateway_header(captured, &config.user_header)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("sso-admin");
    let scim_user = match runtime_gateway_scim_user_by_name(shared, name) {
        Ok(user) => user,
        Err(()) => return None,
    };
    if scim_user.as_ref().is_some_and(|user| !user.active) {
        return None;
    }
    let role = runtime_gateway_header(captured, &config.role_header)
        .and_then(RuntimeGatewayAdminRole::parse)
        .or_else(|| {
            scim_user
                .as_ref()
                .and_then(|user| user.role.as_deref())
                .and_then(RuntimeGatewayAdminRole::parse)
        })
        .unwrap_or(config.default_role);
    let tenant_id = runtime_gateway_header(captured, &config.tenant_header)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| scim_user.as_ref().and_then(|user| user.tenant_id.clone()));
    let team_id = scim_user.as_ref().and_then(|user| user.team_id.clone());
    let project_id = scim_user.as_ref().and_then(|user| user.project_id.clone());
    let user_id = scim_user.as_ref().and_then(|user| user.user_id.clone());
    let budget_id = scim_user.as_ref().and_then(|user| user.budget_id.clone());
    let allowed_key_prefixes = runtime_gateway_header(captured, &config.key_prefixes_header)
        .map(runtime_gateway_parse_sso_prefixes)
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

fn runtime_gateway_parse_sso_prefixes(value: &str) -> Vec<String> {
    value
        .split([',', ';', '\n'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

fn runtime_gateway_scim_user_by_name(
    shared: &RuntimeLocalRewriteProxyShared,
    name: &str,
) -> Result<Option<RuntimeGatewayScimUser>, ()> {
    let name = name.trim();
    if name.is_empty() {
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
