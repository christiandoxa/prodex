use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Issuer(String);

impl fmt::Debug for Issuer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Issuer").field(&"<redacted>").finish()
    }
}

impl Issuer {
    pub fn new(value: impl Into<String>) -> Result<Self, IdentityConfigError> {
        let value = value.into();
        if value.is_empty() {
            return Err(IdentityConfigError::EmptyIssuer);
        }
        if !value.starts_with("https://")
            || issuer_host(&value).is_none_or(|host| !issuer_host_is_well_formed(host))
        {
            return Err(IdentityConfigError::InvalidIssuer);
        }
        let issuer = value.trim_end_matches('/');
        Ok(Self(issuer.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn issuer_host(issuer: &str) -> Option<&str> {
    let after_scheme = issuer.split_once("://").map_or(issuer, |(_, rest)| rest);
    after_scheme.split(['/', '?', '#']).next()
}

fn issuer_host_is_well_formed(host: &str) -> bool {
    !host.is_empty()
        && !host.contains('@')
        && host
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | ':' | '[' | ']'))
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Audience(String);

impl fmt::Debug for Audience {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Audience").field(&"<redacted>").finish()
    }
}

impl Audience {
    pub fn new(value: impl Into<String>) -> Result<Self, IdentityConfigError> {
        let value = value.into();
        if value.is_empty() {
            return Err(IdentityConfigError::EmptyAudience);
        }
        if !value.chars().all(|character| character.is_ascii_graphic()) {
            return Err(IdentityConfigError::InvalidAudience);
        }
        Ok(Self(value))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JwtAlgorithm {
    Rs256,
    Rs384,
    Rs512,
    Es256,
    Es384,
    EdDsa,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OidcValidationPolicy {
    pub issuer: Issuer,
    pub audiences: Vec<Audience>,
    pub allowed_algorithms: Vec<JwtAlgorithm>,
}

impl fmt::Debug for OidcValidationPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OidcValidationPolicy")
            .field("issuer", &self.issuer)
            .field("audiences", &self.audiences)
            .field("allowed_algorithms", &self.allowed_algorithms)
            .finish()
    }
}

impl OidcValidationPolicy {
    pub fn new(
        issuer: Issuer,
        mut audiences: Vec<Audience>,
        mut allowed_algorithms: Vec<JwtAlgorithm>,
    ) -> Result<Self, IdentityConfigError> {
        audiences.sort();
        audiences.dedup();
        allowed_algorithms.sort();
        allowed_algorithms.dedup();
        if audiences.is_empty() {
            return Err(IdentityConfigError::NoAudience);
        }
        if allowed_algorithms.is_empty() {
            return Err(IdentityConfigError::NoAllowedAlgorithm);
        }
        Ok(Self {
            issuer,
            audiences,
            allowed_algorithms,
        })
    }

    pub fn validate_claims(
        &self,
        issuer: &Issuer,
        audience: &Audience,
        algorithm: JwtAlgorithm,
    ) -> Result<(), TokenValidationError> {
        if &self.issuer != issuer {
            return Err(TokenValidationError::IssuerMismatch);
        }
        if self.audiences.binary_search(audience).is_err() {
            return Err(TokenValidationError::AudienceMismatch);
        }
        if self.allowed_algorithms.binary_search(&algorithm).is_err() {
            return Err(TokenValidationError::AlgorithmNotAllowed);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IdentityConfigError {
    EmptyIssuer,
    InvalidIssuer,
    EmptyAudience,
    InvalidAudience,
    NoAudience,
    NoAllowedAlgorithm,
}

impl fmt::Display for IdentityConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "identity configuration is invalid")
    }
}

impl Error for IdentityConfigError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TokenValidationError {
    IssuerMismatch,
    AudienceMismatch,
    AlgorithmNotAllowed,
}

impl fmt::Display for TokenValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "token validation failed")
    }
}

impl Error for TokenValidationError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityErrorStatus {
    InvalidRequest,
    Unauthorized,
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityErrorResponsePlan {
    pub status: IdentityErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_identity_config_error_response(
    error: &IdentityConfigError,
) -> IdentityErrorResponsePlan {
    let code = match error {
        IdentityConfigError::EmptyIssuer | IdentityConfigError::InvalidIssuer => {
            "identity_issuer_invalid"
        }
        IdentityConfigError::EmptyAudience | IdentityConfigError::InvalidAudience => {
            "identity_audience_invalid"
        }
        IdentityConfigError::NoAudience => "identity_audience_required",
        IdentityConfigError::NoAllowedAlgorithm => "identity_algorithm_allowlist_required",
    };

    IdentityErrorResponsePlan {
        status: IdentityErrorStatus::InvalidRequest,
        code,
        message: "identity configuration is invalid",
    }
}

pub fn plan_token_validation_error_response(
    error: &TokenValidationError,
) -> IdentityErrorResponsePlan {
    let code = match error {
        TokenValidationError::IssuerMismatch => "token_issuer_invalid",
        TokenValidationError::AudienceMismatch => "token_audience_invalid",
        TokenValidationError::AlgorithmNotAllowed => "token_algorithm_not_allowed",
    };

    IdentityErrorResponsePlan {
        status: IdentityErrorStatus::Unauthorized,
        code,
        message: "token validation failed",
    }
}

pub fn plan_jwks_refresh_decision_error_response(
    decision: JwksRefreshDecision,
) -> Option<IdentityErrorResponsePlan> {
    match decision {
        JwksRefreshDecision::UseFresh
        | JwksRefreshDecision::RefreshNow
        | JwksRefreshDecision::UseStaleWhileRevalidate
        | JwksRefreshDecision::UseLastKnownGoodDuringBackoff => None,
        JwksRefreshDecision::Unavailable => Some(IdentityErrorResponsePlan {
            status: IdentityErrorStatus::ServiceUnavailable,
            code: "jwks_cache_unavailable",
            message: "identity key cache is unavailable",
        }),
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JwksCacheSnapshot {
    pub fetched_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
    pub stale_until_unix_ms: u64,
    pub key_count: u16,
    pub last_refresh_error_at_unix_ms: Option<u64>,
    pub retry_after_unix_ms: Option<u64>,
}

impl fmt::Debug for JwksCacheSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JwksCacheSnapshot")
            .field("fetched_at_unix_ms", &"<redacted>")
            .field("expires_at_unix_ms", &"<redacted>")
            .field("stale_until_unix_ms", &"<redacted>")
            .field("has_keys", &(self.key_count > 0))
            .field(
                "last_refresh_error_at_unix_ms",
                &self.last_refresh_error_at_unix_ms.map(|_| "<redacted>"),
            )
            .field(
                "retry_after_unix_ms",
                &self.retry_after_unix_ms.map(|_| "<redacted>"),
            )
            .finish()
    }
}

impl JwksCacheSnapshot {
    pub fn is_fresh_at(&self, now_unix_ms: u64) -> bool {
        self.key_count > 0 && now_unix_ms < self.expires_at_unix_ms
    }

    pub fn is_usable_last_known_good_at(&self, now_unix_ms: u64) -> bool {
        self.key_count > 0 && now_unix_ms < self.stale_until_unix_ms
    }

    pub fn should_refresh_at(&self, now_unix_ms: u64) -> bool {
        now_unix_ms >= self.expires_at_unix_ms
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JwksRefreshDecision {
    UseFresh,
    RefreshNow,
    UseStaleWhileRevalidate,
    UseLastKnownGoodDuringBackoff,
    Unavailable,
}

pub fn evaluate_jwks_refresh(
    snapshot: Option<&JwksCacheSnapshot>,
    now_unix_ms: u64,
) -> JwksRefreshDecision {
    let Some(snapshot) = snapshot else {
        return JwksRefreshDecision::RefreshNow;
    };
    if snapshot.is_fresh_at(now_unix_ms) {
        return JwksRefreshDecision::UseFresh;
    }
    if let Some(retry_after) = snapshot.retry_after_unix_ms
        && now_unix_ms < retry_after
    {
        return if snapshot.is_usable_last_known_good_at(now_unix_ms) {
            JwksRefreshDecision::UseLastKnownGoodDuringBackoff
        } else {
            JwksRefreshDecision::Unavailable
        };
    }
    if snapshot.is_usable_last_known_good_at(now_unix_ms) {
        return JwksRefreshDecision::UseStaleWhileRevalidate;
    }
    JwksRefreshDecision::Unavailable
}
