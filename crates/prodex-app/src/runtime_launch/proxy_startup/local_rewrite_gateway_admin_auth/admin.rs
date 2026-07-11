use super::super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::super::local_rewrite_gateway_scope::RuntimeGatewayGovernanceScope;
use super::super::local_rewrite_gateway_store_types::{
    RuntimeGatewayScimUser, RuntimeGatewayVirtualKeyEntry,
    runtime_gateway_scim_user_auth_entry_from_stored,
};
use super::super::*;
use super::token_claims::*;

pub(in super::super) struct RuntimeGatewayAdminAuth {
    pub(in super::super) name: String,
    pub(in super::super) role: RuntimeGatewayAdminRole,
    pub(in super::super) tenant_id: Option<String>,
    pub(in super::super) team_id: Option<String>,
    pub(in super::super) project_id: Option<String>,
    pub(in super::super) user_id: Option<String>,
    pub(in super::super) budget_id: Option<String>,
    pub(in super::super) allowed_key_prefixes: Vec<String>,
}

pub(in super::super) fn runtime_gateway_admin_auth(
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
    None
}

pub(super) fn runtime_gateway_oidc_admin_auth(
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Option<RuntimeGatewayAdminAuth> {
    let config = shared.gateway_sso.oidc.as_ref()?;
    let authorization = runtime_gateway_header(captured, "authorization")?;
    let token = runtime_gateway_authorization_bearer_token(authorization)?;
    let claims = runtime_gateway_verify_oidc_token(token, config, shared).ok()?;
    let name = runtime_gateway_oidc_admin_name(&claims, config)?;
    let claimed_tenant_id =
        runtime_gateway_oidc_claim_scope_string(&claims, &config.tenant_claim).ok()?;
    let scim_user = runtime_gateway_scim_user_for_claimed_tenant(
        runtime_gateway_scim_user_by_name(shared, &name).ok()?,
        claimed_tenant_id.as_deref(),
    );
    if scim_user.as_ref().is_some_and(|user| !user.active) {
        return None;
    }
    let role = runtime_gateway_sso_resolved_role(
        runtime_gateway_oidc_role_claim_string(&claims, &config.role_claim).as_deref(),
        scim_user.as_ref(),
    );
    let tenant_id =
        claimed_tenant_id.or_else(|| scim_user.as_ref().and_then(|user| user.tenant_id.clone()));
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

pub(super) fn runtime_gateway_sso_admin_auth(
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
    let claimed_tenant_id = match runtime_gateway_header(captured, &config.tenant_header) {
        Some(value) => Some(runtime_gateway_exact_scope_string(value)?),
        None => None,
    };
    let scim_user = runtime_gateway_scim_user_for_claimed_tenant(
        runtime_gateway_scim_user_by_name(shared, name).ok()?,
        claimed_tenant_id.as_deref(),
    );
    if scim_user.as_ref().is_some_and(|user| !user.active) {
        return None;
    }
    let role = runtime_gateway_sso_resolved_role(
        runtime_gateway_header(captured, &config.role_header),
        scim_user.as_ref(),
    );
    let tenant_id =
        claimed_tenant_id.or_else(|| scim_user.as_ref().and_then(|user| user.tenant_id.clone()));
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

pub(super) fn runtime_gateway_sso_proxy_token_matches(
    proxy_token_hash: &runtime_proxy_crate::LocalBridgeBearerTokenHash,
    proxy_token: &str,
) -> bool {
    proxy_token_hash.verify_bearer_token(proxy_token)
}

pub(super) fn runtime_gateway_sso_user_name(value: Option<&str>) -> Option<&str> {
    match value {
        Some(value) if !value.is_empty() && !value.chars().any(char::is_whitespace) => Some(value),
        Some(_) => None,
        None => None,
    }
}

pub(super) fn runtime_gateway_sso_resolved_role(
    role_claim: Option<&str>,
    scim_user: Option<&RuntimeGatewayScimUser>,
) -> RuntimeGatewayAdminRole {
    if let Some(role_claim) = role_claim {
        return RuntimeGatewayAdminRole::parse(role_claim)
            .unwrap_or(RuntimeGatewayAdminRole::Viewer);
    }
    scim_user
        .and_then(|user| user.role.as_deref())
        .and_then(RuntimeGatewayAdminRole::parse)
        .unwrap_or(RuntimeGatewayAdminRole::Viewer)
}

pub(super) fn runtime_gateway_header<'a>(
    captured: &'a RuntimeProxyRequest,
    header_name: &str,
) -> Option<&'a str> {
    captured
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(header_name))
        .map(|(_, value)| value.as_str())
}

pub(super) fn runtime_gateway_scim_user_by_name(
    shared: &RuntimeLocalRewriteProxyShared,
    name: &str,
) -> Result<Option<RuntimeGatewayScimUser>, ()> {
    if name.is_empty() || name.chars().any(char::is_whitespace) {
        return Ok(None);
    }
    let store = match super::super::local_rewrite::runtime_gateway_virtual_key_store_load_strict(
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
        .filter_map(|user| runtime_gateway_scim_user_auth_entry_from_stored(&user))
        .find(|user| user.user_name.eq_ignore_ascii_case(name)))
}

pub(super) fn runtime_gateway_scim_user_for_claimed_tenant(
    scim_user: Option<RuntimeGatewayScimUser>,
    claimed_tenant_id: Option<&str>,
) -> Option<RuntimeGatewayScimUser> {
    let user = scim_user?;
    if claimed_tenant_id.is_some_and(|tenant_id| user.tenant_id.as_deref() != Some(tenant_id)) {
        return None;
    }
    Some(user)
}

impl RuntimeGatewayAdminAuth {
    pub(in super::super) fn governance_scope(&self) -> RuntimeGatewayGovernanceScope {
        RuntimeGatewayGovernanceScope::new(
            self.tenant_id.clone(),
            self.team_id.clone(),
            self.project_id.clone(),
            self.user_id.clone(),
            self.budget_id.clone(),
        )
    }

    pub(in super::super) fn can_access_tenant(&self, tenant_id: Option<&str>) -> bool {
        self.governance_scope().matches_tenant(tenant_id)
    }

    pub(in super::super) fn can_access_dimensions(
        &self,
        team_id: Option<&str>,
        project_id: Option<&str>,
        user_id: Option<&str>,
        budget_id: Option<&str>,
    ) -> bool {
        self.governance_scope()
            .matches_dimensions(team_id, project_id, user_id, budget_id)
    }

    pub(in super::super) fn can_access_key(&self, key_name: &str) -> bool {
        self.allowed_key_prefixes.is_empty()
            || self
                .allowed_key_prefixes
                .iter()
                .any(|prefix| key_name.starts_with(prefix))
    }

    pub(in super::super) fn can_access_entry(&self, entry: &RuntimeGatewayVirtualKeyEntry) -> bool {
        self.governance_scope().matches(
            entry.tenant_id.as_deref(),
            entry.key.team_id.as_deref(),
            entry.key.project_id.as_deref(),
            entry.key.user_id.as_deref(),
            entry.key.budget_id.as_deref(),
        ) && self.can_access_key(&entry.key.name)
    }

    pub(in super::super) fn can_access_scim_user(&self, user: &RuntimeGatewayScimUser) -> bool {
        self.governance_scope().matches(
            user.tenant_id.as_deref(),
            user.team_id.as_deref(),
            user.project_id.as_deref(),
            user.user_id.as_deref(),
            user.budget_id.as_deref(),
        )
    }
}

pub(in super::super) fn runtime_gateway_admin_auth_is_unscoped(
    admin_auth: &RuntimeGatewayAdminAuth,
) -> bool {
    admin_auth.governance_scope().is_unscoped()
}

pub(in super::super) fn runtime_gateway_admin_auth_matches_entry(
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
