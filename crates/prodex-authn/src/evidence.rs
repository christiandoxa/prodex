use std::error::Error;
use std::fmt;

use prodex_domain::{
    CredentialScope, ExplicitRoleMapper, JwksCacheSnapshot, OidcValidationPolicy, Principal, Role,
};

use crate::{AuthenticationError, TokenClaims, validate_oidc_token_claims};

#[derive(Clone, PartialEq, Eq)]
pub struct VerifiedOidcCredentialEvidence {
    pub policy: OidcValidationPolicy,
    pub jwks_snapshot: JwksCacheSnapshot,
    pub claims: TokenClaims,
    pub role_evidence: VerifiedOidcRoleEvidence,
    pub resolved_principal: Principal,
    pub now_unix_ms: u64,
}

#[derive(Clone, PartialEq, Eq)]
pub enum VerifiedOidcRoleEvidence {
    Claim(ExplicitRoleMapper),
    TrustedMissingClaimFallback(Role),
}

impl fmt::Debug for VerifiedOidcRoleEvidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Claim(_) => f.debug_tuple("Claim").field(&"<redacted>").finish(),
            Self::TrustedMissingClaimFallback(_) => f
                .debug_tuple("TrustedMissingClaimFallback")
                .field(&"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Debug for VerifiedOidcCredentialEvidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VerifiedOidcCredentialEvidence")
            .field("policy", &"<redacted>")
            .field("jwks_snapshot", &"<redacted>")
            .field("claims", &"<redacted>")
            .field("role_evidence", &"<redacted>")
            .field("resolved_principal", &"<redacted>")
            .field("now_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum VerifiedCredentialEvidence {
    Principal(Principal),
    Oidc(Box<VerifiedOidcCredentialEvidence>),
}

impl fmt::Debug for VerifiedCredentialEvidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Principal(_) => f.debug_tuple("Principal").field(&"<redacted>").finish(),
            Self::Oidc(_) => f.debug_tuple("Oidc").field(&"<redacted>").finish(),
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct VerifiedCredentialAuthenticationRequest {
    pub evidence: Option<VerifiedCredentialEvidence>,
    pub required_scope: Option<CredentialScope>,
    pub anonymous_allowed: bool,
}

impl fmt::Debug for VerifiedCredentialAuthenticationRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VerifiedCredentialAuthenticationRequest")
            .field("evidence", &self.evidence)
            .field("required_scope", &self.required_scope)
            .field("anonymous_allowed", &self.anonymous_allowed)
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VerifiedCredentialAuthenticationError {
    CredentialRequired,
    CredentialScopeMismatch {
        actual: CredentialScope,
        required: CredentialScope,
    },
    Oidc(AuthenticationError),
    OidcPrincipalMismatch,
}

impl fmt::Display for VerifiedCredentialAuthenticationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CredentialRequired => f.write_str("credential is required"),
            Self::CredentialScopeMismatch { .. } => {
                f.write_str("credential scope is not allowed for this endpoint")
            }
            Self::Oidc(error) => error.fmt(f),
            Self::OidcPrincipalMismatch => {
                f.write_str("OIDC principal evidence does not match verified claims")
            }
        }
    }
}

impl Error for VerifiedCredentialAuthenticationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Oidc(error) => Some(error),
            _ => None,
        }
    }
}

pub fn authenticate_verified_credential(
    request: VerifiedCredentialAuthenticationRequest,
) -> Result<Option<Principal>, VerifiedCredentialAuthenticationError> {
    let principal = match request.evidence {
        Some(VerifiedCredentialEvidence::Principal(principal)) => principal,
        Some(VerifiedCredentialEvidence::Oidc(evidence)) => {
            authenticate_verified_oidc_evidence(*evidence)?
        }
        None if request.anonymous_allowed => return Ok(None),
        None => return Err(VerifiedCredentialAuthenticationError::CredentialRequired),
    };
    if let Some(required) = request.required_scope
        && principal.credential_scope != required
    {
        return Err(
            VerifiedCredentialAuthenticationError::CredentialScopeMismatch {
                actual: principal.credential_scope,
                required,
            },
        );
    }
    Ok(Some(principal))
}

fn authenticate_verified_oidc_evidence(
    evidence: VerifiedOidcCredentialEvidence,
) -> Result<Principal, VerifiedCredentialAuthenticationError> {
    validate_oidc_token_claims(
        &evidence.policy,
        Some(&evidence.jwks_snapshot),
        &evidence.claims,
        evidence.now_unix_ms,
    )
    .map_err(VerifiedCredentialAuthenticationError::Oidc)?;
    if !oidc_claims_bind_principal(
        &evidence.claims,
        &evidence.role_evidence,
        &evidence.resolved_principal,
    )
    .map_err(VerifiedCredentialAuthenticationError::Oidc)?
    {
        return Err(VerifiedCredentialAuthenticationError::OidcPrincipalMismatch);
    }
    Ok(evidence.resolved_principal)
}

fn oidc_claims_bind_principal(
    claims: &TokenClaims,
    role_evidence: &VerifiedOidcRoleEvidence,
    principal: &Principal,
) -> Result<bool, AuthenticationError> {
    let role_matches = match (&claims.role_claim, role_evidence) {
        (Some(role_claim), VerifiedOidcRoleEvidence::Claim(mapper)) => mapper
            .role_for_claim(Some(role_claim))
            .map(|role| role == principal.role)
            .map_err(AuthenticationError::Role)?,
        (None, VerifiedOidcRoleEvidence::TrustedMissingClaimFallback(role)) => {
            *role == principal.role
        }
        (None, VerifiedOidcRoleEvidence::Claim(mapper)) => mapper
            .role_for_claim(None)
            .map(|role| role == principal.role)
            .map_err(AuthenticationError::Role)?,
        (Some(_), VerifiedOidcRoleEvidence::TrustedMissingClaimFallback(_)) => false,
    };
    Ok(claims.principal_id == principal.id
        && claims.principal_kind == principal.kind
        && claims.credential_scope == principal.credential_scope
        && claims
            .tenant_id
            .is_none_or(|tenant_id| principal.tenant_id == Some(tenant_id))
        && role_matches)
    // A missing typed tenant may use the trusted adapter's SCIM or unscoped
    // admin projection. Missing roles require explicit trusted fallback evidence;
    // present roles are mapped and bound here in canonical authentication.
}

#[cfg(test)]
mod tests {
    use super::*;
    use prodex_domain::{
        Audience, Issuer, JwtAlgorithm, PrincipalId, PrincipalKind, Role, RoleClaimError, TenantId,
    };

    fn id<T: std::str::FromStr>(value: &str) -> T
    where
        T::Err: fmt::Debug,
    {
        value.parse().unwrap()
    }

    fn policy() -> OidcValidationPolicy {
        OidcValidationPolicy::new(
            Issuer::new("https://issuer.example.com").unwrap(),
            vec![Audience::new("prodex").unwrap()],
            vec![JwtAlgorithm::Rs256],
        )
        .unwrap()
    }

    fn jwks() -> JwksCacheSnapshot {
        JwksCacheSnapshot {
            fetched_at_unix_ms: 1_000,
            expires_at_unix_ms: 10_000,
            stale_until_unix_ms: 20_000,
            key_count: 1,
            last_refresh_error_at_unix_ms: None,
            retry_after_unix_ms: None,
        }
    }

    fn principal(scope: CredentialScope, tenant_id: Option<TenantId>) -> Principal {
        Principal::new(
            id::<PrincipalId>("00000000-0000-7000-8000-000000000001"),
            tenant_id,
            PrincipalKind::User,
            Role::Admin,
            scope,
        )
    }

    fn claims(principal: &Principal, tenant_id: Option<TenantId>) -> TokenClaims {
        TokenClaims {
            issuer: Issuer::new("https://issuer.example.com").unwrap(),
            audience: Audience::new("prodex").unwrap(),
            algorithm: JwtAlgorithm::Rs256,
            key_id: "kid-1".to_string(),
            key_known: true,
            signature_verified: true,
            principal_id: principal.id,
            tenant_id,
            principal_kind: principal.kind,
            credential_scope: principal.credential_scope,
            role_claim: None,
            expires_at_unix_ms: 9_000,
            not_before_unix_ms: Some(1_000),
        }
    }

    fn role_mapper() -> ExplicitRoleMapper {
        ExplicitRoleMapper::new([("admin", Role::Admin), ("viewer", Role::Viewer)])
    }

    #[test]
    fn principal_evidence_enforces_required_scope_and_anonymous_policy() {
        let tenant_id = Some(id::<TenantId>("00000000-0000-7000-8000-000000000002"));
        let data = principal(CredentialScope::DataPlane, tenant_id);
        let authenticated =
            authenticate_verified_credential(VerifiedCredentialAuthenticationRequest {
                evidence: Some(VerifiedCredentialEvidence::Principal(data)),
                required_scope: Some(CredentialScope::DataPlane),
                anonymous_allowed: false,
            })
            .unwrap();
        assert!(authenticated.is_some());
        assert_eq!(
            authenticate_verified_credential(VerifiedCredentialAuthenticationRequest {
                evidence: None,
                required_scope: Some(CredentialScope::DataPlane),
                anonymous_allowed: false,
            }),
            Err(VerifiedCredentialAuthenticationError::CredentialRequired),
        );
    }

    #[test]
    fn oidc_evidence_allows_trusted_fallback_only_when_tenant_claim_is_absent() {
        let fallback_tenant = Some(id::<TenantId>("00000000-0000-7000-8000-000000000002"));
        let resolved = principal(CredentialScope::ControlPlane, fallback_tenant);
        let evidence = VerifiedOidcCredentialEvidence {
            policy: policy(),
            jwks_snapshot: jwks(),
            claims: claims(&resolved, None),
            role_evidence: VerifiedOidcRoleEvidence::TrustedMissingClaimFallback(Role::Admin),
            resolved_principal: resolved.clone(),
            now_unix_ms: 2_000,
        };
        assert_eq!(
            authenticate_verified_credential(VerifiedCredentialAuthenticationRequest {
                evidence: Some(VerifiedCredentialEvidence::Oidc(Box::new(evidence))),
                required_scope: Some(CredentialScope::ControlPlane),
                anonymous_allowed: false,
            })
            .unwrap(),
            Some(resolved),
        );
    }

    #[test]
    fn oidc_evidence_rejects_unrelated_principal_and_present_tenant() {
        let claimed_tenant = id::<TenantId>("00000000-0000-7000-8000-000000000002");
        let foreign_tenant = id::<TenantId>("00000000-0000-7000-8000-000000000003");
        let claimed = principal(CredentialScope::ControlPlane, Some(claimed_tenant));
        let mut resolved = principal(CredentialScope::ControlPlane, Some(foreign_tenant));
        resolved.id = id::<PrincipalId>("00000000-0000-7000-8000-000000000004");
        let evidence = VerifiedOidcCredentialEvidence {
            policy: policy(),
            jwks_snapshot: jwks(),
            claims: claims(&claimed, Some(claimed_tenant)),
            role_evidence: VerifiedOidcRoleEvidence::TrustedMissingClaimFallback(Role::Admin),
            resolved_principal: resolved,
            now_unix_ms: 2_000,
        };
        assert_eq!(
            authenticate_verified_credential(VerifiedCredentialAuthenticationRequest {
                evidence: Some(VerifiedCredentialEvidence::Oidc(Box::new(evidence))),
                required_scope: Some(CredentialScope::ControlPlane),
                anonymous_allowed: false,
            }),
            Err(VerifiedCredentialAuthenticationError::OidcPrincipalMismatch),
        );
    }

    #[test]
    fn oidc_role_claim_must_map_to_the_resolved_role_and_unknown_fails_closed() {
        let resolved = principal(CredentialScope::ControlPlane, None);
        let mut viewer_claims = claims(&resolved, None);
        viewer_claims.role_claim = Some("viewer".to_string());
        let authenticate = |claims| {
            authenticate_verified_credential(VerifiedCredentialAuthenticationRequest {
                evidence: Some(VerifiedCredentialEvidence::Oidc(Box::new(
                    VerifiedOidcCredentialEvidence {
                        policy: policy(),
                        jwks_snapshot: jwks(),
                        claims,
                        role_evidence: VerifiedOidcRoleEvidence::Claim(role_mapper()),
                        resolved_principal: resolved.clone(),
                        now_unix_ms: 2_000,
                    },
                ))),
                required_scope: Some(CredentialScope::ControlPlane),
                anonymous_allowed: false,
            })
        };
        assert_eq!(
            authenticate(viewer_claims),
            Err(VerifiedCredentialAuthenticationError::OidcPrincipalMismatch),
        );

        let mut unknown_claims = claims(&resolved, None);
        unknown_claims.role_claim = Some("owner".to_string());
        assert_eq!(
            authenticate(unknown_claims),
            Err(VerifiedCredentialAuthenticationError::Oidc(
                AuthenticationError::Role(RoleClaimError::Unknown),
            )),
        );
    }

    #[test]
    fn oidc_evidence_debug_is_a_redacted_snapshot() {
        let resolved = principal(CredentialScope::ControlPlane, None);
        let oidc = VerifiedOidcCredentialEvidence {
            policy: policy(),
            jwks_snapshot: jwks(),
            claims: claims(&resolved, None),
            role_evidence: VerifiedOidcRoleEvidence::TrustedMissingClaimFallback(Role::Admin),
            resolved_principal: resolved,
            now_unix_ms: 2_000,
        };
        assert_eq!(
            format!("{oidc:?}"),
            "VerifiedOidcCredentialEvidence { policy: \"<redacted>\", jwks_snapshot: \"<redacted>\", claims: \"<redacted>\", role_evidence: \"<redacted>\", resolved_principal: \"<redacted>\", now_unix_ms: \"<redacted>\" }",
        );
        let request = VerifiedCredentialAuthenticationRequest {
            evidence: Some(VerifiedCredentialEvidence::Oidc(Box::new(oidc))),
            required_scope: Some(CredentialScope::ControlPlane),
            anonymous_allowed: false,
        };
        assert_eq!(
            format!("{request:?}"),
            "VerifiedCredentialAuthenticationRequest { evidence: Some(Oidc(\"<redacted>\")), required_scope: Some(ControlPlane), anonymous_allowed: false }",
        );
    }
}
