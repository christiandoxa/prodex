#![forbid(unsafe_code)]
//! Network-free authentication boundary primitives for Prodex.
//!
//! This crate validates already-decoded token metadata against domain OIDC,
//! JWKS-cache, role-mapping, and tenant requirements. It intentionally performs
//! no discovery, JWKS fetch, HTTP, filesystem, database, or async-runtime work.

mod compatibility;
mod evidence;

pub use compatibility::{
    CompatibilityAuthenticationError, CompatibilityAuthenticationRequest,
    authenticate_compatibility_request,
};
pub use evidence::{
    VerifiedCredentialAuthenticationError, VerifiedCredentialAuthenticationRequest,
    VerifiedCredentialEvidence, VerifiedOidcCredentialEvidence, VerifiedOidcRoleEvidence,
    authenticate_verified_credential,
};

use std::error::Error;
use std::fmt;

use prodex_domain::{
    Audience, CredentialScope, ExplicitRoleMapper, Issuer, JwksCacheSnapshot, JwksRefreshDecision,
    JwtAlgorithm, OidcValidationPolicy, Principal, PrincipalId, PrincipalKind, RoleClaimError,
    TenantId, TokenValidationError, evaluate_jwks_refresh,
};
use url::{Host, Url};

pub const OIDC_ENDPOINT_MAX_BYTES: usize = 2_048;
pub const OIDC_JWKS_ORIGIN_ALLOWLIST_MAX_ENTRIES: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OidcEndpointValidationError {
    Empty,
    TooLong,
    InvalidUrl,
    HttpsRequired,
    HostRequired,
    UserinfoForbidden,
    QueryForbidden,
    FragmentForbidden,
    PortForbidden,
    AddressForbidden,
    OriginForbidden,
    TooManyOrigins,
    IssuerMismatch,
    RedirectForbidden,
}

impl fmt::Display for OidcEndpointValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::Empty => "OIDC URL is empty",
            Self::TooLong => "OIDC URL exceeds the length limit",
            Self::InvalidUrl => "OIDC URL is invalid",
            Self::HttpsRequired => "OIDC URL must use HTTPS",
            Self::HostRequired => "OIDC URL must contain a host",
            Self::UserinfoForbidden => "OIDC URL userinfo is forbidden",
            Self::QueryForbidden => "OIDC URL query parameters are forbidden",
            Self::FragmentForbidden => "OIDC URL fragments are forbidden",
            Self::PortForbidden => "OIDC URL port is forbidden",
            Self::AddressForbidden => "OIDC URL address is forbidden",
            Self::OriginForbidden => "OIDC URL origin is not allowed",
            Self::TooManyOrigins => "OIDC JWKS origin allowlist has too many entries",
            Self::IssuerMismatch => "OIDC discovery issuer does not match configuration",
            Self::RedirectForbidden => "OIDC endpoint redirects are forbidden",
        };
        f.write_str(message)
    }
}

impl Error for OidcEndpointValidationError {}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ValidatedOidcIssuer {
    url: Url,
    canonical: String,
    origin: OidcOrigin,
}

impl fmt::Debug for ValidatedOidcIssuer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ValidatedOidcIssuer")
            .field(&"<redacted>")
            .finish()
    }
}

impl ValidatedOidcIssuer {
    pub fn parse(value: &str) -> Result<Self, OidcEndpointValidationError> {
        let url = parse_oidc_https_url(value)?;
        let canonical = if url.path() == "/" {
            url.as_str().trim_end_matches('/').to_string()
        } else {
            url.as_str().to_string()
        };
        let origin = OidcOrigin::from_url(&url)?;
        Ok(Self {
            url,
            canonical,
            origin,
        })
    }

    pub fn as_str(&self) -> &str {
        &self.canonical
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ValidatedOidcEndpoint {
    url: Url,
    canonical: String,
    origin: OidcOrigin,
}

impl fmt::Debug for ValidatedOidcEndpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ValidatedOidcEndpoint")
            .field(&"<redacted>")
            .finish()
    }
}

impl ValidatedOidcEndpoint {
    fn parse(value: &str) -> Result<Self, OidcEndpointValidationError> {
        let url = parse_oidc_https_url(value)?;
        let canonical = url.as_str().to_string();
        let origin = OidcOrigin::from_url(&url)?;
        Ok(Self {
            url,
            canonical,
            origin,
        })
    }

    pub fn as_str(&self) -> &str {
        &self.canonical
    }

    pub fn host(&self) -> &str {
        self.url
            .host_str()
            .expect("validated OIDC endpoint always has a host")
    }

    pub fn port(&self) -> u16 {
        self.url
            .port_or_known_default()
            .expect("validated HTTPS endpoint always has a port")
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct OidcOrigin {
    host: String,
    port: u16,
    canonical: String,
}

impl OidcOrigin {
    fn from_url(url: &Url) -> Result<Self, OidcEndpointValidationError> {
        Ok(Self {
            host: url
                .host_str()
                .ok_or(OidcEndpointValidationError::HostRequired)?
                .to_string(),
            port: url
                .port_or_known_default()
                .ok_or(OidcEndpointValidationError::PortForbidden)?,
            canonical: url.origin().ascii_serialization(),
        })
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct OidcEndpointPolicy {
    issuer: ValidatedOidcIssuer,
    configured_jwks: Option<ValidatedOidcEndpoint>,
    allowed_jwks_origins: Vec<OidcOrigin>,
}

impl fmt::Debug for OidcEndpointPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OidcEndpointPolicy")
            .field("issuer", &self.issuer)
            .field("configured_jwks", &self.configured_jwks.is_some())
            .field(
                "allowed_jwks_origin_count",
                &self.allowed_jwks_origins.len(),
            )
            .finish()
    }
}

impl OidcEndpointPolicy {
    pub fn new(
        configured_issuer: &str,
        configured_jwks: Option<&str>,
    ) -> Result<Self, OidcEndpointValidationError> {
        Self::with_jwks_origin_allowlist(configured_issuer, configured_jwks, std::iter::empty())
    }

    pub fn with_jwks_origin_allowlist<'a>(
        configured_issuer: &str,
        configured_jwks: Option<&str>,
        allowed_origins: impl IntoIterator<Item = &'a str>,
    ) -> Result<Self, OidcEndpointValidationError> {
        let issuer = ValidatedOidcIssuer::parse(configured_issuer)?;
        let allowed_origins = allowed_origins
            .into_iter()
            .take(OIDC_JWKS_ORIGIN_ALLOWLIST_MAX_ENTRIES + 1)
            .collect::<Vec<_>>();
        if allowed_origins.len() > OIDC_JWKS_ORIGIN_ALLOWLIST_MAX_ENTRIES {
            return Err(OidcEndpointValidationError::TooManyOrigins);
        }
        let mut allowed_jwks_origins = Vec::new();
        for value in allowed_origins {
            let url = parse_oidc_https_url(value)?;
            if url.path() != "/" {
                return Err(OidcEndpointValidationError::OriginForbidden);
            }
            let origin = OidcOrigin::from_url(&url)?;
            if origin != issuer.origin && !allowed_jwks_origins.contains(&origin) {
                allowed_jwks_origins.push(origin);
            }
        }
        let configured_jwks = configured_jwks
            .map(ValidatedOidcEndpoint::parse)
            .transpose()?;
        if configured_jwks.as_ref().is_some_and(|endpoint| {
            endpoint.origin != issuer.origin && !allowed_jwks_origins.contains(&endpoint.origin)
        }) {
            return Err(OidcEndpointValidationError::OriginForbidden);
        }
        Ok(Self {
            issuer,
            configured_jwks,
            allowed_jwks_origins,
        })
    }

    pub fn issuer(&self) -> &ValidatedOidcIssuer {
        &self.issuer
    }

    pub fn configured_jwks(&self) -> Option<&ValidatedOidcEndpoint> {
        self.configured_jwks.as_ref()
    }

    pub fn allowed_jwks_origins(&self) -> impl Iterator<Item = &str> {
        self.allowed_jwks_origins
            .iter()
            .map(|origin| origin.canonical.as_str())
    }

    pub fn discovery_endpoint(&self) -> ValidatedOidcEndpoint {
        let mut url = self.issuer.url.clone();
        let path = if url.path() == "/" {
            "/.well-known/openid-configuration".to_string()
        } else {
            format!(
                "{}/.well-known/openid-configuration",
                url.path().trim_end_matches('/')
            )
        };
        url.set_path(&path);
        let canonical = url.as_str().to_string();
        ValidatedOidcEndpoint {
            url,
            canonical,
            origin: self.issuer.origin.clone(),
        }
    }

    pub fn validate_discovery_document(
        &self,
        document_issuer: &str,
        jwks_uri: &str,
    ) -> Result<ValidatedOidcEndpoint, OidcEndpointValidationError> {
        if ValidatedOidcIssuer::parse(document_issuer)? != self.issuer {
            return Err(OidcEndpointValidationError::IssuerMismatch);
        }
        self.validate_jwks_endpoint(jwks_uri)
    }

    pub fn validate_jwks_endpoint(
        &self,
        value: &str,
    ) -> Result<ValidatedOidcEndpoint, OidcEndpointValidationError> {
        let endpoint = ValidatedOidcEndpoint::parse(value)?;
        if endpoint.origin != self.issuer.origin
            && !self.allowed_jwks_origins.contains(&endpoint.origin)
        {
            return Err(OidcEndpointValidationError::OriginForbidden);
        }
        Ok(endpoint)
    }

    pub fn redirects_allowed(&self) -> bool {
        false
    }

    pub fn validate_redirect(
        &self,
        _target: &str,
    ) -> Result<ValidatedOidcEndpoint, OidcEndpointValidationError> {
        Err(OidcEndpointValidationError::RedirectForbidden)
    }
}

fn parse_oidc_https_url(value: &str) -> Result<Url, OidcEndpointValidationError> {
    if value.is_empty() {
        return Err(OidcEndpointValidationError::Empty);
    }
    if value.len() > OIDC_ENDPOINT_MAX_BYTES {
        return Err(OidcEndpointValidationError::TooLong);
    }
    if !value.is_ascii() || value.chars().any(char::is_whitespace) || value.contains('\\') {
        return Err(OidcEndpointValidationError::InvalidUrl);
    }
    let Some((raw_scheme, raw_authority)) = value.split_once("://") else {
        return Err(OidcEndpointValidationError::InvalidUrl);
    };
    if !raw_scheme.eq_ignore_ascii_case("https")
        || raw_authority.is_empty()
        || raw_authority.starts_with('/')
    {
        return Err(OidcEndpointValidationError::InvalidUrl);
    }
    let url = Url::parse(value).map_err(|_| OidcEndpointValidationError::InvalidUrl)?;
    if url.scheme() != "https" {
        return Err(OidcEndpointValidationError::HttpsRequired);
    }
    let host = url
        .host_str()
        .ok_or(OidcEndpointValidationError::HostRequired)?;
    if host.is_empty() || host.ends_with('.') {
        return Err(OidcEndpointValidationError::HostRequired);
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(OidcEndpointValidationError::UserinfoForbidden);
    }
    if url.query().is_some() {
        return Err(OidcEndpointValidationError::QueryForbidden);
    }
    if url.fragment().is_some() {
        return Err(OidcEndpointValidationError::FragmentForbidden);
    }
    if url.port() == Some(0) || url.port_or_known_default().is_none() {
        return Err(OidcEndpointValidationError::PortForbidden);
    }
    match url.host() {
        Some(Host::Ipv4(address)) if ipv4_address_is_forbidden(address.octets()) => {
            return Err(OidcEndpointValidationError::AddressForbidden);
        }
        Some(Host::Ipv6(address)) if ipv6_address_is_forbidden(address.octets()) => {
            return Err(OidcEndpointValidationError::AddressForbidden);
        }
        _ => {}
    }
    Ok(url)
}

fn ipv4_address_is_forbidden(octets: [u8; 4]) -> bool {
    matches!(
        octets,
        [0, ..]
            | [10, ..]
            | [100, 64..=127, ..]
            | [127, ..]
            | [169, 254, ..]
            | [172, 16..=31, ..]
            | [192, 0, 0, ..]
            | [192, 0, 2, ..]
            | [192, 168, ..]
            | [198, 18..=19, ..]
            | [198, 51, 100, ..]
            | [203, 0, 113, ..]
            | [224..=255, ..]
    )
}

fn ipv6_address_is_forbidden(octets: [u8; 16]) -> bool {
    if octets == [0; 16] || octets == [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1] {
        return true;
    }
    if octets[0] == 0xff
        || octets[0] & 0xfe == 0xfc
        || (octets[0] == 0xfe && octets[1] & 0xc0 == 0x80)
        || (octets[0] == 0x20 && octets[1] == 0x01 && octets[2] == 0x0d && octets[3] == 0xb8)
    {
        return true;
    }
    if octets[..10] == [0; 10] && octets[10] == 0xff && octets[11] == 0xff {
        return ipv4_address_is_forbidden([octets[12], octets[13], octets[14], octets[15]]);
    }
    if octets[..12] == [0; 12] || octets[..12] == [0, 0x64, 0xff, 0x9b, 0, 0, 0, 0, 0, 0, 0, 0] {
        return ipv4_address_is_forbidden([octets[12], octets[13], octets[14], octets[15]]);
    }
    if octets[..6] == [0, 0x64, 0xff, 0x9b, 0, 1] || octets[..4] == [0x20, 0x01, 0, 0] {
        return true;
    }
    if octets[..2] == [0x20, 0x02] {
        return ipv4_address_is_forbidden([octets[2], octets[3], octets[4], octets[5]]);
    }
    false
}

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
    let endpoints = OidcEndpointPolicy::new(policy.issuer.as_str(), configured_jwks_url)
        .map_err(map_endpoint_validation_error)?;
    let source = match configured_jwks_url {
        Some(_) => OidcRefreshSource::Jwks {
            url: endpoints
                .configured_jwks()
                .expect("configured JWKS endpoint was validated")
                .as_str()
                .to_string(),
        },
        None => OidcRefreshSource::Discovery {
            url: endpoints.discovery_endpoint().as_str().to_string(),
        },
    };
    Ok(OidcJwksRefreshPlan {
        issuer: policy.issuer.clone(),
        decision,
        source: Some(source),
    })
}

fn map_endpoint_validation_error(error: OidcEndpointValidationError) -> AuthenticationError {
    match error {
        OidcEndpointValidationError::OriginForbidden
        | OidcEndpointValidationError::IssuerMismatch => AuthenticationError::JwksUrlIssuerMismatch,
        _ => AuthenticationError::InvalidJwksUrl,
    }
}

pub fn authenticate_oidc_claims(
    policy: &OidcValidationPolicy,
    jwks_snapshot: Option<&JwksCacheSnapshot>,
    role_mapper: &ExplicitRoleMapper,
    claims: TokenClaims,
    now_unix_ms: u64,
) -> Result<Principal, AuthenticationError> {
    validate_oidc_token_claims(policy, jwks_snapshot, &claims, now_unix_ms)?;
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

pub fn validate_oidc_token_claims(
    policy: &OidcValidationPolicy,
    jwks_snapshot: Option<&JwksCacheSnapshot>,
    claims: &TokenClaims,
    now_unix_ms: u64,
) -> Result<(), AuthenticationError> {
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
    Ok(())
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
