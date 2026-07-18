use anyhow::{Context, Result, bail};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use prodex_authn::{
    VerifiedCredentialEvidence, VerifiedMtlsPeerEvidence, VerifiedOidcCredentialEvidence,
    VerifiedOidcRoleEvidence, VerifiedWorkloadCredentialEvidence,
};
use prodex_domain::{CredentialScope, Principal, PrincipalId, PrincipalKind, Role, TenantId};

use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::local_rewrite_application_boundary::runtime_gateway_stable_id;
use super::local_rewrite_gateway_admin_auth::runtime_gateway_verify_workload_token;
use super::local_rewrite_gateway_config::RuntimeGatewayWorkloadIdentityConfig;
use super::local_rewrite_request::RuntimeLocalRewriteRequest;

pub(super) fn runtime_gateway_workload_credential(
    request: &RuntimeLocalRewriteRequest,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Result<Option<VerifiedCredentialEvidence>> {
    let Some(config) = shared.gateway_sso.workload_identity.as_ref() else {
        return Ok(None);
    };
    let Some(token) = request_bearer_token(request) else {
        return Ok(None);
    };
    if token.split('.').count() != 3 {
        return Ok(None);
    }
    let token = runtime_gateway_verify_workload_token(token, &config.oidc, shared)
        .context("gateway workload token verification failed")?;
    runtime_gateway_workload_evidence_from_verified(
        token,
        config,
        request.mtls_peer_certificate_sha256(),
    )
    .map(Some)
}

fn runtime_gateway_workload_evidence_from_verified(
    token: super::local_rewrite_gateway_admin_auth::RuntimeGatewayVerifiedOidcToken,
    config: &RuntimeGatewayWorkloadIdentityConfig,
    peer_certificate_sha256: Option<[u8; 32]>,
) -> Result<VerifiedCredentialEvidence> {
    let subject = exact_claim(&token.claims, &config.subject_claim)
        .context("gateway workload token subject is missing or invalid")?;
    let tenant = exact_claim(&token.claims, &config.tenant_claim)
        .context("gateway workload token tenant is missing or invalid")?;
    let tenant_id = tenant
        .parse::<TenantId>()
        .context("gateway workload token tenant is invalid")?;
    if !scope_claim_contains(
        token.claims.get(&config.scope_claim),
        &config.required_scope,
    ) {
        bail!("gateway workload token scope is missing or invalid");
    }

    let tenant_text = tenant_id.to_string();
    let principal_id = PrincipalId::from_uuid(runtime_gateway_stable_id(
        "prodex:gateway-workload-principal:v1",
        &[
            token.policy().issuer.as_str().as_bytes(),
            subject.as_bytes(),
            tenant_text.as_bytes(),
        ],
    ));
    let principal = Principal::new(
        principal_id,
        Some(tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::DataPlane,
    );
    let policy = token.policy();
    let expected_issuer = policy.issuer.clone();
    let expected_audience = policy
        .audiences
        .first()
        .cloned()
        .context("gateway workload token audience is unavailable")?;
    let mtls_peer = if config.mtls_required {
        let fingerprint =
            peer_certificate_sha256.context("gateway workload client certificate is required")?;
        require_certificate_binding(&token.claims, fingerprint)?;
        Some(VerifiedMtlsPeerEvidence {
            certificate_chain_verified: true,
            bound_principal_id: principal_id,
        })
    } else {
        None
    };
    let claims = token.canonical_workload_claims(principal_id, tenant_id);
    Ok(VerifiedCredentialEvidence::Workload(
        VerifiedWorkloadCredentialEvidence {
            credential: Box::new(VerifiedOidcCredentialEvidence {
                policy,
                jwks_snapshot: token.jwks_snapshot(),
                claims,
                role_evidence: VerifiedOidcRoleEvidence::TrustedMissingClaimFallback(
                    Role::Operator,
                ),
                resolved_principal: principal,
                now_unix_ms: token.now_unix_ms(),
            }),
            expected_issuer,
            expected_audience,
            mtls_required: config.mtls_required,
            mtls_peer,
        },
    ))
}

fn request_bearer_token(request: &RuntimeLocalRewriteRequest) -> Option<&str> {
    let authorization = request
        .headers()
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("authorization"))?
        .1
        .as_str();
    let mut parts = authorization.split_whitespace();
    if !parts.next()?.eq_ignore_ascii_case("bearer") {
        return None;
    }
    let token = parts.next()?;
    (parts.next().is_none() && !token.is_empty()).then_some(token)
}

fn exact_claim<'a>(
    claims: &'a std::collections::BTreeMap<String, serde_json::Value>,
    name: &str,
) -> Option<&'a str> {
    let value = claims.get(name)?.as_str()?;
    (!value.is_empty()
        && value.len() <= 512
        && value
            .chars()
            .all(|character| character.is_ascii_graphic() && !matches!(character, '"' | '\\')))
    .then_some(value)
}

fn scope_claim_contains(value: Option<&serde_json::Value>, required: &str) -> bool {
    value.is_some_and(|value| {
        value.as_str().is_some_and(|scopes| {
            scopes
                .split_ascii_whitespace()
                .any(|scope| scope == required)
        }) || value.as_array().is_some_and(|scopes| {
            scopes.len() <= 64 && scopes.iter().any(|scope| scope.as_str() == Some(required))
        })
    })
}

fn require_certificate_binding(
    claims: &std::collections::BTreeMap<String, serde_json::Value>,
    peer_certificate_sha256: [u8; 32],
) -> Result<()> {
    let encoded = claims
        .get("cnf")
        .and_then(serde_json::Value::as_object)
        .and_then(|confirmation| confirmation.get("x5t#S256"))
        .and_then(serde_json::Value::as_str)
        .context("gateway workload token certificate binding is missing")?;
    let decoded = URL_SAFE_NO_PAD
        .decode(encoded)
        .context("gateway workload token certificate binding is invalid")?;
    if decoded.as_slice() != peer_certificate_sha256 {
        bail!("gateway workload token certificate binding does not match");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        RuntimeGatewayWorkloadIdentityConfig, URL_SAFE_NO_PAD,
        runtime_gateway_workload_evidence_from_verified,
    };
    use crate::runtime_launch::proxy_startup::local_rewrite_gateway_admin_auth::runtime_gateway_test_verified_oidc_token;
    use base64::Engine as _;

    fn config(mtls_required: bool) -> RuntimeGatewayWorkloadIdentityConfig {
        RuntimeGatewayWorkloadIdentityConfig {
            oidc: super::super::local_rewrite_gateway_config::RuntimeGatewayOidcConfig {
                issuer: "https://issuer.example.com".to_string(),
                audience: "prodex-gateway".to_string(),
                jwks_url: None,
                jwks_origin_allowlist: Vec::new(),
                user_claim: "sub".to_string(),
                role_claim: String::new(),
                tenant_claim: "prodex_tenant".to_string(),
                key_prefixes_claim: String::new(),
                authentication_strength: None,
            },
            subject_claim: "sub".to_string(),
            tenant_claim: "prodex_tenant".to_string(),
            scope_claim: "scope".to_string(),
            required_scope: "data_plane".to_string(),
            mtls_required,
        }
    }

    fn token(
        binding: Option<[u8; 32]>,
    ) -> super::super::local_rewrite_gateway_admin_auth::RuntimeGatewayVerifiedOidcToken {
        let mut token = runtime_gateway_test_verified_oidc_token();
        token
            .claims
            .insert("sub".to_string(), serde_json::json!("service-a"));
        token.claims.insert(
            "prodex_tenant".to_string(),
            serde_json::json!("00000000-0000-7000-8000-000000000002"),
        );
        token
            .claims
            .insert("scope".to_string(), serde_json::json!("read data_plane"));
        if let Some(binding) = binding {
            token.claims.insert(
                "cnf".to_string(),
                serde_json::json!({"x5t#S256": URL_SAFE_NO_PAD.encode(binding)}),
            );
        }
        token
    }

    #[test]
    fn workload_evidence_requires_scope_and_exact_mtls_binding() {
        let fingerprint = [7_u8; 32];
        assert!(
            runtime_gateway_workload_evidence_from_verified(
                token(Some(fingerprint)),
                &config(true),
                Some(fingerprint),
            )
            .is_ok()
        );
        assert!(
            runtime_gateway_workload_evidence_from_verified(
                token(Some(fingerprint)),
                &config(true),
                Some([8_u8; 32]),
            )
            .is_err()
        );
        assert!(
            runtime_gateway_workload_evidence_from_verified(
                token(None),
                &config(true),
                Some(fingerprint),
            )
            .is_err()
        );
    }
}
