#![forbid(unsafe_code)]
//! Network-free authentication boundary primitives for Prodex.
//!
//! This crate validates already-decoded token metadata against domain OIDC,
//! JWKS-cache, role-mapping, and tenant requirements. It intentionally performs
//! no discovery, JWKS fetch, HTTP, filesystem, database, or async-runtime work.

use std::error::Error;
use std::fmt;

use prodex_domain::{
    Audience, CredentialScope, ExplicitRoleMapper, Issuer, JwksCacheSnapshot, JwksRefreshDecision,
    JwtAlgorithm, OidcValidationPolicy, Principal, PrincipalId, PrincipalKind, RoleClaimError,
    TenantId, TokenValidationError, evaluate_jwks_refresh,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenClaims {
    pub issuer: Issuer,
    pub audience: Audience,
    pub algorithm: JwtAlgorithm,
    pub key_id: String,
    pub key_known: bool,
    pub signature_verified: bool,
    pub principal_id: PrincipalId,
    pub tenant_id: Option<TenantId>,
    pub principal_kind: PrincipalKind,
    pub credential_scope: CredentialScope,
    pub role_claim: Option<String>,
    pub expires_at_unix_ms: u64,
    pub not_before_unix_ms: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthenticationError {
    SignatureNotVerified,
    JwksRefreshRequired,
    JwksUnavailable,
    JwksRefreshForbiddenOnRequestPath,
    InvalidJwksUrl,
    JwksUrlIssuerMismatch,
    UnknownKeyId,
    TokenExpired,
    TokenNotYetValid,
    MissingTenant,
    Claims(TokenValidationError),
    Role(RoleClaimError),
}

impl fmt::Display for AuthenticationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SignatureNotVerified => write!(f, "token signature is not verified"),
            Self::JwksRefreshRequired => write!(
                f,
                "JWKS cache refresh is required before token authentication"
            ),
            Self::JwksUnavailable => write!(f, "JWKS cache is unavailable"),
            Self::JwksRefreshForbiddenOnRequestPath => write!(
                f,
                "JWKS discovery and fetch must not run on the request path"
            ),
            Self::InvalidJwksUrl => write!(f, "JWKS URL is invalid"),
            Self::JwksUrlIssuerMismatch => write!(f, "JWKS URL issuer host does not match"),
            Self::UnknownKeyId => write!(f, "token key id is unknown"),
            Self::TokenExpired => write!(f, "token is expired"),
            Self::TokenNotYetValid => write!(f, "token is not yet valid"),
            Self::MissingTenant => write!(f, "tenant claim is required"),
            Self::Claims(err) => err.fmt(f),
            Self::Role(err) => err.fmt(f),
        }
    }
}

impl Error for AuthenticationError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthenticationErrorStatus {
    Unauthorized,
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthenticationErrorResponsePlan {
    pub status: AuthenticationErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_authentication_error_response(
    error: &AuthenticationError,
) -> AuthenticationErrorResponsePlan {
    match error {
        AuthenticationError::JwksRefreshRequired | AuthenticationError::JwksUnavailable => {
            AuthenticationErrorResponsePlan {
                status: AuthenticationErrorStatus::ServiceUnavailable,
                code: "authentication_temporarily_unavailable",
                message: "authentication is temporarily unavailable",
            }
        }
        AuthenticationError::JwksRefreshForbiddenOnRequestPath
        | AuthenticationError::InvalidJwksUrl
        | AuthenticationError::JwksUrlIssuerMismatch => AuthenticationErrorResponsePlan {
            status: AuthenticationErrorStatus::ServiceUnavailable,
            code: "authentication_temporarily_unavailable",
            message: "authentication is temporarily unavailable",
        },
        AuthenticationError::MissingTenant => AuthenticationErrorResponsePlan {
            status: AuthenticationErrorStatus::Unauthorized,
            code: "tenant_required",
            message: "tenant claim is required",
        },
        AuthenticationError::Role(_) => AuthenticationErrorResponsePlan {
            status: AuthenticationErrorStatus::Unauthorized,
            code: "role_not_authorized",
            message: "role claim is not authorized",
        },
        AuthenticationError::SignatureNotVerified
        | AuthenticationError::UnknownKeyId
        | AuthenticationError::TokenExpired
        | AuthenticationError::TokenNotYetValid
        | AuthenticationError::Claims(_) => AuthenticationErrorResponsePlan {
            status: AuthenticationErrorStatus::Unauthorized,
            code: "invalid_token",
            message: "token is invalid",
        },
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OidcRefreshRuntimeMode {
    ControlPlaneBackground,
    GatewayRequestPath,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OidcRefreshSource {
    Discovery { url: String },
    Jwks { url: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OidcJwksRefreshPlan {
    pub issuer: Issuer,
    pub decision: JwksRefreshDecision,
    pub source: Option<OidcRefreshSource>,
}

pub fn plan_oidc_jwks_refresh(
    policy: &OidcValidationPolicy,
    configured_jwks_url: Option<&str>,
    jwks_snapshot: Option<&JwksCacheSnapshot>,
    now_unix_ms: u64,
    mode: OidcRefreshRuntimeMode,
) -> Result<OidcJwksRefreshPlan, AuthenticationError> {
    let decision = evaluate_jwks_refresh(jwks_snapshot, now_unix_ms);
    match decision {
        JwksRefreshDecision::UseFresh => Ok(OidcJwksRefreshPlan {
            issuer: policy.issuer.clone(),
            decision,
            source: None,
        }),
        JwksRefreshDecision::UseStaleWhileRevalidate
            if mode == OidcRefreshRuntimeMode::GatewayRequestPath =>
        {
            Ok(OidcJwksRefreshPlan {
                issuer: policy.issuer.clone(),
                decision,
                source: None,
            })
        }
        JwksRefreshDecision::UseStaleWhileRevalidate
        | JwksRefreshDecision::RefreshNow
        | JwksRefreshDecision::Unavailable => {
            plan_oidc_refresh_source(policy, configured_jwks_url, decision, mode)
        }
        JwksRefreshDecision::UseLastKnownGoodDuringBackoff => Ok(OidcJwksRefreshPlan {
            issuer: policy.issuer.clone(),
            decision,
            source: None,
        }),
    }
}

fn plan_oidc_refresh_source(
    policy: &OidcValidationPolicy,
    configured_jwks_url: Option<&str>,
    decision: JwksRefreshDecision,
    mode: OidcRefreshRuntimeMode,
) -> Result<OidcJwksRefreshPlan, AuthenticationError> {
    if mode == OidcRefreshRuntimeMode::GatewayRequestPath {
        return Err(AuthenticationError::JwksRefreshForbiddenOnRequestPath);
    }
    let source = match configured_jwks_url {
        Some(url) => OidcRefreshSource::Jwks {
            url: validate_refresh_url(url, policy.issuer.as_str())?,
        },
        None => OidcRefreshSource::Discovery {
            url: validate_refresh_url(
                &format!(
                    "{}/.well-known/openid-configuration",
                    policy.issuer.as_str()
                ),
                policy.issuer.as_str(),
            )?,
        },
    };
    Ok(OidcJwksRefreshPlan {
        issuer: policy.issuer.clone(),
        decision,
        source: Some(source),
    })
}

fn validate_refresh_url(url: &str, issuer: &str) -> Result<String, AuthenticationError> {
    if !refresh_url_is_well_formed(url) {
        return Err(AuthenticationError::InvalidJwksUrl);
    }
    let Some(after_scheme) = url.strip_prefix("https://") else {
        return Err(AuthenticationError::InvalidJwksUrl);
    };
    let host = url_host_after_scheme(after_scheme);
    if host.is_empty() {
        return Err(AuthenticationError::InvalidJwksUrl);
    }
    let issuer_host = issuer
        .strip_prefix("https://")
        .map(url_host_after_scheme)
        .unwrap_or_default();
    if host != issuer_host {
        return Err(AuthenticationError::JwksUrlIssuerMismatch);
    }
    Ok(url.to_string())
}

fn refresh_url_is_well_formed(url: &str) -> bool {
    !url.is_empty() && url.len() <= 2048 && url.chars().all(|ch| ch.is_ascii_graphic())
}

fn url_host_after_scheme(after_scheme: &str) -> &str {
    after_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
}

pub fn authenticate_oidc_claims(
    policy: &OidcValidationPolicy,
    jwks_snapshot: Option<&JwksCacheSnapshot>,
    role_mapper: &ExplicitRoleMapper,
    claims: TokenClaims,
    now_unix_ms: u64,
) -> Result<Principal, AuthenticationError> {
    require_usable_jwks(jwks_snapshot, now_unix_ms)?;
    if !claims.signature_verified {
        return Err(AuthenticationError::SignatureNotVerified);
    }
    if !claims.key_known || !token_key_id_is_well_formed(&claims.key_id) {
        return Err(AuthenticationError::UnknownKeyId);
    }
    if now_unix_ms >= claims.expires_at_unix_ms {
        return Err(AuthenticationError::TokenExpired);
    }
    if let Some(not_before) = claims.not_before_unix_ms
        && now_unix_ms < not_before
    {
        return Err(AuthenticationError::TokenNotYetValid);
    }
    policy
        .validate_claims(&claims.issuer, &claims.audience, claims.algorithm)
        .map_err(AuthenticationError::Claims)?;
    let tenant_id = claims.tenant_id.ok_or(AuthenticationError::MissingTenant)?;
    let role = role_mapper
        .role_for_claim(claims.role_claim.as_deref())
        .map_err(AuthenticationError::Role)?;

    Ok(Principal::new(
        claims.principal_id,
        Some(tenant_id),
        claims.principal_kind,
        role,
        claims.credential_scope,
    ))
}

fn token_key_id_is_well_formed(key_id: &str) -> bool {
    !key_id.is_empty() && key_id.len() <= 128 && key_id.chars().all(|ch| ch.is_ascii_graphic())
}

fn require_usable_jwks(
    jwks_snapshot: Option<&JwksCacheSnapshot>,
    now_unix_ms: u64,
) -> Result<(), AuthenticationError> {
    match evaluate_jwks_refresh(jwks_snapshot, now_unix_ms) {
        JwksRefreshDecision::UseFresh
        | JwksRefreshDecision::UseStaleWhileRevalidate
        | JwksRefreshDecision::UseLastKnownGoodDuringBackoff => Ok(()),
        JwksRefreshDecision::RefreshNow => Err(AuthenticationError::JwksRefreshRequired),
        JwksRefreshDecision::Unavailable => Err(AuthenticationError::JwksUnavailable),
    }
}
