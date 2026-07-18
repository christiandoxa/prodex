use super::{validate_gateway_exact_identifier, validate_gateway_optional_scope};
use crate::types::RuntimePolicyFile;
use crate::validate_helpers::{validate_gateway_admin_role, validate_optional_u64};
use crate::validate_secrets::validate_gateway_secret_source;
use anyhow::{Context, Result, bail};
use prodex_authn::{
    OIDC_JWKS_ORIGIN_ALLOWLIST_MAX_ENTRIES, OidcEndpointPolicy, ValidatedOidcEndpoint,
    ValidatedOidcIssuer,
};
use std::path::Path;

pub(super) fn validate_gateway_admin_tokens(policy: &RuntimePolicyFile, path: &Path) -> Result<()> {
    for (index, token) in policy.gateway.admin_tokens.iter().enumerate() {
        let field = format!("gateway.admin_tokens[{index}]");
        validate_gateway_exact_identifier(&token.name, path, &format!("{field}.name"))?;
        validate_gateway_secret_source(
            policy,
            path,
            &format!("{field}.token"),
            (!token.token_env.is_empty()).then_some(token.token_env.as_str()),
            token.token_ref.as_ref(),
            true,
        )?;
        if let Some(role) = token.role.as_deref() {
            validate_gateway_admin_role(role)
                .with_context(|| format!("{field}.role in {} is invalid", path.display()))?;
        }
        validate_gateway_optional_scope(token.tenant_id.as_deref(), path, &field, "tenant_id")?;
        for (name, value) in [
            ("team_id", token.team_id.as_deref()),
            ("project_id", token.project_id.as_deref()),
            ("user_id", token.user_id.as_deref()),
            ("budget_id", token.budget_id.as_deref()),
        ] {
            validate_gateway_optional_scope(value, path, &field, name)?;
        }
        for (prefix_index, prefix) in token.allowed_key_prefixes.iter().enumerate() {
            validate_gateway_exact_identifier(
                prefix,
                path,
                &format!("{field}.allowed_key_prefixes[{prefix_index}]"),
            )?;
        }
    }
    Ok(())
}

pub(super) fn validate_gateway_virtual_keys(policy: &RuntimePolicyFile, path: &Path) -> Result<()> {
    for (index, key) in policy.gateway.virtual_keys.iter().enumerate() {
        let field = format!("gateway.virtual_keys[{index}]");
        validate_gateway_exact_identifier(&key.name, path, &format!("{field}.name"))?;
        validate_gateway_secret_source(
            policy,
            path,
            &format!("{field}.token"),
            (!key.token_env.is_empty()).then_some(key.token_env.as_str()),
            key.token_ref.as_ref(),
            true,
        )?;
        validate_gateway_optional_scope(key.tenant_id.as_deref(), path, &field, "tenant_id")?;
        for (name, value) in [
            ("team_id", key.team_id.as_deref()),
            ("project_id", key.project_id.as_deref()),
            ("user_id", key.user_id.as_deref()),
            ("budget_id", key.budget_id.as_deref()),
        ] {
            validate_gateway_optional_scope(value, path, &field, name)?;
        }
        for (model_index, model) in key.allowed_models.iter().enumerate() {
            validate_gateway_exact_identifier(
                model,
                path,
                &format!("{field}.allowed_models[{model_index}]"),
            )?;
        }
        if let Some(budget_usd) = key.budget_usd
            && (!budget_usd.is_finite() || budget_usd <= 0.0)
        {
            bail!(
                "{field}.budget_usd in {} must be greater than 0",
                path.display()
            );
        }
        validate_optional_u64(key.request_budget, path, &format!("{field}.request_budget"))?;
        validate_optional_u64(key.rpm_limit, path, &format!("{field}.rpm_limit"))?;
        validate_optional_u64(key.tpm_limit, path, &format!("{field}.tpm_limit"))?;
    }
    Ok(())
}

pub(super) fn validate_gateway_sso(policy: &RuntimePolicyFile, path: &Path) -> Result<()> {
    let sso = &policy.gateway.sso;
    validate_gateway_secret_source(
        policy,
        path,
        "gateway.sso.proxy_token",
        sso.proxy_token_env.as_deref(),
        sso.proxy_token_ref.as_ref(),
        false,
    )?;
    for (field, value) in [
        ("gateway.sso.token_header", sso.token_header.as_deref()),
        ("gateway.sso.user_header", sso.user_header.as_deref()),
        ("gateway.sso.role_header", sso.role_header.as_deref()),
        ("gateway.sso.tenant_header", sso.tenant_header.as_deref()),
        (
            "gateway.sso.key_prefixes_header",
            sso.key_prefixes_header.as_deref(),
        ),
        ("gateway.sso.oidc_audience", sso.oidc_audience.as_deref()),
        (
            "gateway.sso.oidc_user_claim",
            sso.oidc_user_claim.as_deref(),
        ),
        (
            "gateway.sso.oidc_role_claim",
            sso.oidc_role_claim.as_deref(),
        ),
        (
            "gateway.sso.oidc_tenant_claim",
            sso.oidc_tenant_claim.as_deref(),
        ),
        (
            "gateway.sso.oidc_key_prefixes_claim",
            sso.oidc_key_prefixes_claim.as_deref(),
        ),
        ("gateway.sso.required_scope", sso.required_scope.as_deref()),
        (
            "gateway.sso.authentication_strength",
            sso.authentication_strength.as_deref(),
        ),
        ("gateway.sso.oidc_client_id", sso.oidc_client_id.as_deref()),
    ] {
        if let Some(value) = value {
            validate_gateway_exact_identifier(value, path, field)?;
        }
    }
    for (field, value) in [
        ("gateway.sso.oidc_issuer", sso.oidc_issuer.as_deref()),
        ("gateway.sso.oidc_jwks_url", sso.oidc_jwks_url.as_deref()),
        (
            "gateway.sso.oidc_authorization_url",
            sso.oidc_authorization_url.as_deref(),
        ),
        ("gateway.sso.oidc_token_url", sso.oidc_token_url.as_deref()),
        (
            "gateway.sso.oidc_redirect_uri",
            sso.oidc_redirect_uri.as_deref(),
        ),
    ] {
        if matches!(value.map(str::trim), Some("")) {
            bail!("{field} in {} cannot be empty", path.display());
        }
    }
    let oidc_enabled = sso.oidc_issuer.is_some()
        || sso.oidc_audience.is_some()
        || sso.oidc_jwks_url.is_some()
        || !sso.oidc_jwks_origin_allowlist.is_empty();
    if sso.remote_human == Some(true) && !oidc_enabled {
        bail!(
            "gateway.sso.remote_human in {} requires exact OIDC issuer and audience",
            path.display()
        );
    }
    if sso
        .required_scope
        .as_deref()
        .is_some_and(|scope| !matches!(scope, "control_plane" | "data_plane"))
    {
        bail!(
            "gateway.sso.required_scope in {} must be control_plane or data_plane",
            path.display()
        );
    }
    if sso
        .authentication_strength
        .as_deref()
        .is_some_and(|strength| !matches!(strength, "mfa" | "phishing_resistant"))
    {
        bail!(
            "gateway.sso.authentication_strength in {} must be mfa or phishing_resistant",
            path.display()
        );
    }
    let browser_configured = sso.browser_flow.is_some()
        || sso.pkce_method.is_some()
        || sso.oidc_authorization_url.is_some()
        || sso.oidc_token_url.is_some()
        || sso.oidc_client_id.is_some()
        || sso.oidc_client_secret_ref.is_some()
        || sso.oidc_redirect_uri.is_some();
    if browser_configured && sso.browser_flow != Some(true) {
        bail!(
            "gateway.sso browser settings in {} require browser_flow=true",
            path.display()
        );
    }
    if sso.browser_flow == Some(true) && !oidc_enabled {
        bail!(
            "gateway.sso.browser_flow in {} requires exact OIDC issuer and audience",
            path.display()
        );
    }
    if oidc_enabled {
        if sso
            .required_scope
            .as_deref()
            .is_some_and(|scope| scope != "control_plane")
        {
            bail!(
                "gateway.sso.required_scope in {} must be control_plane for human OIDC",
                path.display()
            );
        }
        if sso.oidc_issuer.is_none() || sso.oidc_audience.is_none() {
            bail!(
                "gateway.sso OIDC in {} requires oidc_issuer and oidc_audience",
                path.display()
            );
        }
        let issuer = sso
            .oidc_issuer
            .as_deref()
            .expect("OIDC issuer presence checked above");
        ValidatedOidcIssuer::parse(issuer).with_context(|| {
            format!(
                "gateway.sso.oidc_issuer in {} must be an https URL with host and permitted OIDC policy",
                path.display()
            )
        })?;
        if sso.oidc_jwks_origin_allowlist.len() > OIDC_JWKS_ORIGIN_ALLOWLIST_MAX_ENTRIES {
            bail!(
                "gateway.sso.oidc_jwks_origin_allowlist in {} must contain at most {} entries",
                path.display(),
                OIDC_JWKS_ORIGIN_ALLOWLIST_MAX_ENTRIES
            );
        }
        OidcEndpointPolicy::with_jwks_origin_allowlist(
            issuer,
            None,
            sso.oidc_jwks_origin_allowlist.iter().map(String::as_str),
        )
        .with_context(|| {
            format!(
                "gateway.sso.oidc_jwks_origin_allowlist in {} is not permitted",
                path.display()
            )
        })?;
        if let Some(jwks_url) = sso.oidc_jwks_url.as_deref() {
            OidcEndpointPolicy::with_jwks_origin_allowlist(
                issuer,
                Some(jwks_url),
                sso.oidc_jwks_origin_allowlist.iter().map(String::as_str),
            )
            .with_context(|| {
                format!(
                    "gateway.sso.oidc_jwks_url in {} must be an https URL with host and permitted OIDC policy",
                    path.display()
                )
            })?;
        }
        if sso.browser_flow == Some(true) {
            if sso.remote_human != Some(true) {
                bail!(
                    "gateway.sso.browser_flow in {} requires remote_human=true",
                    path.display()
                );
            }
            if sso.pkce_method.as_deref() != Some("S256") {
                bail!(
                    "gateway.sso.browser_flow in {} requires pkce_method=S256",
                    path.display()
                );
            }
            let endpoints = OidcEndpointPolicy::new(issuer, None).with_context(|| {
                format!(
                    "gateway.sso browser issuer in {} is invalid",
                    path.display()
                )
            })?;
            for (field, value) in [
                (
                    "gateway.sso.oidc_authorization_url",
                    sso.oidc_authorization_url.as_deref(),
                ),
                ("gateway.sso.oidc_token_url", sso.oidc_token_url.as_deref()),
            ] {
                let value = value.with_context(|| {
                    format!("{field} in {} is required for browser OIDC", path.display())
                })?;
                endpoints
                    .validate_issuer_endpoint(value)
                    .with_context(|| format!("{field} in {} is not permitted", path.display()))?;
            }
            let redirect = sso.oidc_redirect_uri.as_deref().with_context(|| {
                format!(
                    "gateway.sso.oidc_redirect_uri in {} is required for browser OIDC",
                    path.display()
                )
            })?;
            ValidatedOidcEndpoint::parse(redirect).with_context(|| {
                format!(
                    "gateway.sso.oidc_redirect_uri in {} is not permitted",
                    path.display()
                )
            })?;
            let redirect_path = url::Url::parse(redirect)
                .map(|redirect| redirect.path().to_string())
                .with_context(|| {
                    format!(
                        "gateway.sso.oidc_redirect_uri in {} is invalid",
                        path.display()
                    )
                })?;
            if !matches!(
                redirect_path.as_str(),
                "/prodex/gateway/auth/callback" | "/v1/prodex/gateway/auth/callback"
            ) {
                bail!(
                    "gateway.sso.oidc_redirect_uri in {} must target the gateway OIDC callback",
                    path.display()
                );
            }
            if sso.oidc_client_id.is_none() {
                bail!(
                    "gateway.sso.oidc_client_id in {} is required for browser OIDC",
                    path.display()
                );
            }
        }
    }
    validate_gateway_secret_source(
        policy,
        path,
        "gateway.sso.oidc_client_secret",
        None,
        sso.oidc_client_secret_ref.as_ref(),
        false,
    )?;
    if let Some(role) = sso.default_role.as_deref() {
        validate_gateway_admin_role(role).with_context(|| {
            format!("gateway.sso.default_role in {} is invalid", path.display())
        })?;
    }
    validate_gateway_workload_identity(policy, path)?;
    Ok(())
}

fn validate_gateway_workload_identity(policy: &RuntimePolicyFile, path: &Path) -> Result<()> {
    let workload = &policy.gateway.workload_identity;
    let configured = workload.enabled.is_some()
        || workload.issuer.is_some()
        || workload.audience.is_some()
        || workload.jwks_url.is_some()
        || !workload.jwks_origin_allowlist.is_empty()
        || workload.subject_claim.is_some()
        || workload.tenant_claim.is_some()
        || workload.scope_claim.is_some()
        || workload.required_scope.is_some()
        || workload.mtls_required.is_some()
        || workload.mtls_ca_ref.is_some()
        || workload.tls_identity_ref.is_some();
    if !configured {
        return Ok(());
    }
    if workload.enabled != Some(true) {
        bail!(
            "gateway.workload_identity.enabled in {} must be true when configured",
            path.display()
        );
    }
    let issuer = workload.issuer.as_deref().with_context(|| {
        format!(
            "gateway.workload_identity.issuer in {} is required",
            path.display()
        )
    })?;
    ValidatedOidcIssuer::parse(issuer).with_context(|| {
        format!(
            "gateway.workload_identity.issuer in {} must be a permitted HTTPS issuer",
            path.display()
        )
    })?;
    let audience = workload.audience.as_deref().with_context(|| {
        format!(
            "gateway.workload_identity.audience in {} is required",
            path.display()
        )
    })?;
    validate_gateway_exact_identifier(audience, path, "gateway.workload_identity.audience")?;
    for (field, value) in [
        ("subject_claim", workload.subject_claim.as_deref()),
        ("tenant_claim", workload.tenant_claim.as_deref()),
        ("scope_claim", workload.scope_claim.as_deref()),
    ] {
        if let Some(value) = value {
            validate_gateway_exact_identifier(
                value,
                path,
                &format!("gateway.workload_identity.{field}"),
            )?;
        }
    }
    if workload.required_scope.as_deref().unwrap_or("data_plane") != "data_plane" {
        bail!(
            "gateway.workload_identity.required_scope in {} must be data_plane",
            path.display()
        );
    }
    if workload.jwks_origin_allowlist.len() > OIDC_JWKS_ORIGIN_ALLOWLIST_MAX_ENTRIES {
        bail!(
            "gateway.workload_identity.jwks_origin_allowlist in {} is too large",
            path.display()
        );
    }
    OidcEndpointPolicy::with_jwks_origin_allowlist(
        issuer,
        workload.jwks_url.as_deref(),
        workload.jwks_origin_allowlist.iter().map(String::as_str),
    )
    .with_context(|| {
        format!(
            "gateway.workload_identity JWKS policy in {} is invalid",
            path.display()
        )
    })?;
    let mtls_required = workload.mtls_required.unwrap_or(false);
    validate_gateway_secret_source(
        policy,
        path,
        "gateway.workload_identity.mtls_ca_ref",
        None,
        workload.mtls_ca_ref.as_ref(),
        mtls_required,
    )?;
    validate_gateway_secret_source(
        policy,
        path,
        "gateway.workload_identity.tls_identity_ref",
        None,
        workload.tls_identity_ref.as_ref(),
        mtls_required,
    )?;
    if mtls_required && (workload.mtls_ca_ref.is_none() || workload.tls_identity_ref.is_none()) {
        bail!(
            "gateway.workload_identity mTLS in {} requires mtls_ca_ref and tls_identity_ref",
            path.display()
        );
    }
    if !mtls_required && workload.mtls_ca_ref.is_some() {
        bail!(
            "gateway.workload_identity.mtls_ca_ref in {} requires mtls_required=true",
            path.display()
        );
    }
    if workload.tls_identity_ref.is_some() != mtls_required {
        bail!(
            "gateway.workload_identity.tls_identity_ref in {} requires mtls_required=true",
            path.display()
        );
    }
    Ok(())
}
