use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::PolicyRevisionId;

const MAX_POLICY_INTEGRITY_METADATA_BYTES: usize = 512;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicySignature(String);

impl PolicySignature {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn is_well_formed(&self) -> bool {
        policy_integrity_metadata_is_well_formed(&self.0)
    }
}

impl fmt::Debug for PolicySignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("PolicySignature")
            .field(&"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyDigest(String);

impl PolicyDigest {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn is_well_formed(&self) -> bool {
        policy_integrity_metadata_is_well_formed(&self.0)
    }
}

impl fmt::Debug for PolicyDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("PolicyDigest").field(&"<redacted>").finish()
    }
}

fn policy_integrity_metadata_is_well_formed(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_POLICY_INTEGRITY_METADATA_BYTES
        && value.chars().all(|ch| ch.is_ascii_graphic())
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicySnapshot<T> {
    pub revision_id: PolicyRevisionId,
    pub issued_at_unix_ms: u64,
    pub payload: T,
    pub digest: PolicyDigest,
    pub signature: PolicySignature,
}

impl<T> fmt::Debug for PolicySnapshot<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PolicySnapshot")
            .field("revision_id", &self.revision_id)
            .field("issued_at_unix_ms", &"<redacted>")
            .field("payload", &"<redacted>")
            .field("digest", &self.digest)
            .field("signature", &self.signature)
            .finish()
    }
}

impl<T> PolicySnapshot<T> {
    pub fn new(
        revision_id: PolicyRevisionId,
        issued_at_unix_ms: u64,
        payload: T,
        digest: PolicyDigest,
        signature: PolicySignature,
    ) -> Self {
        Self {
            revision_id,
            issued_at_unix_ms,
            payload,
            digest,
            signature,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyValidation {
    pub digest_matches: bool,
    pub signature_verified: bool,
}

impl fmt::Debug for PolicyValidation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PolicyValidation")
            .field("digest_matches", &"<redacted>")
            .field("signature_verified", &"<redacted>")
            .finish()
    }
}

impl PolicyValidation {
    pub fn verified() -> Self {
        Self {
            digest_matches: true,
            signature_verified: true,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatedPolicySnapshot<T> {
    snapshot: PolicySnapshot<T>,
}

impl<T> fmt::Debug for ValidatedPolicySnapshot<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ValidatedPolicySnapshot")
            .field("snapshot", &self.snapshot)
            .finish()
    }
}

impl<T> ValidatedPolicySnapshot<T> {
    pub fn revision_id(&self) -> PolicyRevisionId {
        self.snapshot.revision_id
    }

    pub fn issued_at_unix_ms(&self) -> u64 {
        self.snapshot.issued_at_unix_ms
    }

    pub fn payload(&self) -> &T {
        &self.snapshot.payload
    }

    pub fn into_payload(self) -> T {
        self.snapshot.payload
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PolicyActivationError {
    InvalidIssuedAt,
    EmptyDigest,
    InvalidDigest,
    EmptySignature,
    InvalidSignature,
    DigestMismatch,
    SignatureNotVerified,
}

impl fmt::Display for PolicyActivationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "policy snapshot is invalid")
    }
}

impl Error for PolicyActivationError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyErrorStatus {
    InvalidRequest,
    InvalidPolicy,
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyErrorResponsePlan {
    pub status: PolicyErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_policy_activation_error_response(
    error: &PolicyActivationError,
) -> PolicyErrorResponsePlan {
    let (code, message) = match error {
        PolicyActivationError::InvalidIssuedAt => (
            "policy_issued_at_invalid",
            "policy snapshot timestamp is invalid",
        ),
        PolicyActivationError::EmptyDigest => (
            "policy_digest_missing",
            "policy snapshot is missing required integrity metadata",
        ),
        PolicyActivationError::InvalidDigest => (
            "policy_digest_invalid",
            "policy snapshot has invalid integrity metadata",
        ),
        PolicyActivationError::EmptySignature => (
            "policy_signature_missing",
            "policy snapshot is missing required signature metadata",
        ),
        PolicyActivationError::InvalidSignature => (
            "policy_signature_invalid",
            "policy snapshot has invalid signature metadata",
        ),
        PolicyActivationError::DigestMismatch => (
            "policy_integrity_verification_failed",
            "policy snapshot failed integrity verification",
        ),
        PolicyActivationError::SignatureNotVerified => (
            "policy_signature_verification_failed",
            "policy snapshot failed signature verification",
        ),
    };

    PolicyErrorResponsePlan {
        status: PolicyErrorStatus::InvalidPolicy,
        code,
        message,
    }
}

pub fn plan_policy_refresh_window_error_response(
    error: &PolicyRefreshWindowError,
) -> PolicyErrorResponsePlan {
    let code = match error {
        PolicyRefreshWindowError::RefreshAfterStale => "policy_refresh_window_invalid",
        PolicyRefreshWindowError::StaleAfterExpiry => "policy_refresh_window_invalid",
    };

    PolicyErrorResponsePlan {
        status: PolicyErrorStatus::InvalidRequest,
        code,
        message: "policy refresh window is invalid",
    }
}

pub fn plan_policy_refresh_decision_error_response(
    decision: PolicyRefreshDecision,
) -> Option<PolicyErrorResponsePlan> {
    match decision {
        PolicyRefreshDecision::UseActive
        | PolicyRefreshDecision::RefreshAsync
        | PolicyRefreshDecision::UseLastKnownGoodAndRefresh => None,
        PolicyRefreshDecision::Expired => Some(PolicyErrorResponsePlan {
            status: PolicyErrorStatus::ServiceUnavailable,
            code: "policy_cache_expired",
            message: "policy cache is unavailable",
        }),
        PolicyRefreshDecision::Invalidated => Some(PolicyErrorResponsePlan {
            status: PolicyErrorStatus::ServiceUnavailable,
            code: "policy_cache_invalidated",
            message: "policy cache is unavailable",
        }),
    }
}

pub fn validate_policy_snapshot<T>(
    snapshot: PolicySnapshot<T>,
    validation: PolicyValidation,
) -> Result<ValidatedPolicySnapshot<T>, PolicyActivationError> {
    if snapshot.issued_at_unix_ms == 0 {
        return Err(PolicyActivationError::InvalidIssuedAt);
    }
    if snapshot.digest.is_empty() {
        return Err(PolicyActivationError::EmptyDigest);
    }
    if !snapshot.digest.is_well_formed() {
        return Err(PolicyActivationError::InvalidDigest);
    }
    if snapshot.signature.is_empty() {
        return Err(PolicyActivationError::EmptySignature);
    }
    if !snapshot.signature.is_well_formed() {
        return Err(PolicyActivationError::InvalidSignature);
    }
    if !validation.digest_matches {
        return Err(PolicyActivationError::DigestMismatch);
    }
    if !validation.signature_verified {
        return Err(PolicyActivationError::SignatureNotVerified);
    }
    Ok(ValidatedPolicySnapshot { snapshot })
}

#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyActivationState<T> {
    active: Option<ValidatedPolicySnapshot<T>>,
    last_known_good: Option<ValidatedPolicySnapshot<T>>,
}

impl<T> fmt::Debug for PolicyActivationState<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PolicyActivationState")
            .field("has_active", &self.active.is_some())
            .field("has_last_known_good", &self.last_known_good.is_some())
            .finish()
    }
}

impl<T: Clone> PolicyActivationState<T> {
    pub fn active(&self) -> Option<&ValidatedPolicySnapshot<T>> {
        self.active.as_ref()
    }

    pub fn last_known_good(&self) -> Option<&ValidatedPolicySnapshot<T>> {
        self.last_known_good.as_ref()
    }

    pub fn active_revision_id(&self) -> Option<PolicyRevisionId> {
        self.active
            .as_ref()
            .map(ValidatedPolicySnapshot::revision_id)
    }

    pub fn activate(&self, snapshot: ValidatedPolicySnapshot<T>) -> Self {
        Self {
            active: Some(snapshot.clone()),
            last_known_good: Some(snapshot),
        }
    }

    pub fn keep_last_known_good_after_refresh_failure(&self) -> Self {
        Self {
            active: self.last_known_good.clone().or_else(|| self.active.clone()),
            last_known_good: self.last_known_good.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyAuditAction {
    Validated,
    Activated,
    Rejected,
    RolledBackToLastKnownGood,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyAuditRecord {
    pub revision_id: Option<PolicyRevisionId>,
    pub action: PolicyAuditAction,
    pub reason: Option<String>,
}

impl fmt::Debug for PolicyAuditRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PolicyAuditRecord")
            .field("revision_id", &self.revision_id.map(|_| "<redacted>"))
            .field("action", &self.action)
            .field("reason", &self.reason.as_deref().map(|_| "<redacted>"))
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyRefreshWindow {
    pub refresh_after_unix_ms: u64,
    pub stale_after_unix_ms: u64,
    pub expires_after_unix_ms: u64,
}

impl fmt::Debug for PolicyRefreshWindow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PolicyRefreshWindow")
            .field("refresh_after_unix_ms", &"<redacted>")
            .field("stale_after_unix_ms", &"<redacted>")
            .field("expires_after_unix_ms", &"<redacted>")
            .finish()
    }
}

impl PolicyRefreshWindow {
    pub fn new(
        refresh_after_unix_ms: u64,
        stale_after_unix_ms: u64,
        expires_after_unix_ms: u64,
    ) -> Result<Self, PolicyRefreshWindowError> {
        if refresh_after_unix_ms > stale_after_unix_ms {
            return Err(PolicyRefreshWindowError::RefreshAfterStale);
        }
        if stale_after_unix_ms > expires_after_unix_ms {
            return Err(PolicyRefreshWindowError::StaleAfterExpiry);
        }
        Ok(Self {
            refresh_after_unix_ms,
            stale_after_unix_ms,
            expires_after_unix_ms,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyRefreshWindowError {
    RefreshAfterStale,
    StaleAfterExpiry,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyCacheStatus {
    pub active_revision_id: Option<PolicyRevisionId>,
    pub last_known_good_revision_id: Option<PolicyRevisionId>,
    pub refresh_window: PolicyRefreshWindow,
    pub invalidated_revision_id: Option<PolicyRevisionId>,
}

impl fmt::Debug for PolicyCacheStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PolicyCacheStatus")
            .field("has_active_revision", &self.active_revision_id.is_some())
            .field(
                "has_last_known_good_revision",
                &self.last_known_good_revision_id.is_some(),
            )
            .field("refresh_window", &self.refresh_window)
            .field(
                "has_invalidated_revision",
                &self.invalidated_revision_id.is_some(),
            )
            .finish()
    }
}

impl PolicyCacheStatus {
    pub fn from_state<T: Clone>(
        state: &PolicyActivationState<T>,
        refresh_window: PolicyRefreshWindow,
    ) -> Self {
        Self {
            active_revision_id: state.active_revision_id(),
            last_known_good_revision_id: state
                .last_known_good()
                .map(ValidatedPolicySnapshot::revision_id),
            refresh_window,
            invalidated_revision_id: None,
        }
    }

    pub fn invalidate_revision(mut self, revision_id: PolicyRevisionId) -> Self {
        self.invalidated_revision_id = Some(revision_id);
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyRefreshDecision {
    UseActive,
    RefreshAsync,
    UseLastKnownGoodAndRefresh,
    Expired,
    Invalidated,
}

pub fn evaluate_policy_refresh(
    status: &PolicyCacheStatus,
    now_unix_ms: u64,
) -> PolicyRefreshDecision {
    if status.invalidated_revision_id.is_some()
        && status.invalidated_revision_id == status.active_revision_id
    {
        return PolicyRefreshDecision::Invalidated;
    }
    if now_unix_ms >= status.refresh_window.expires_after_unix_ms {
        return PolicyRefreshDecision::Expired;
    }
    if now_unix_ms >= status.refresh_window.stale_after_unix_ms {
        if status.last_known_good_revision_id.is_some()
            && status.last_known_good_revision_id != status.invalidated_revision_id
        {
            return PolicyRefreshDecision::UseLastKnownGoodAndRefresh;
        }
        return PolicyRefreshDecision::RefreshAsync;
    }
    if now_unix_ms >= status.refresh_window.refresh_after_unix_ms {
        return PolicyRefreshDecision::RefreshAsync;
    }
    PolicyRefreshDecision::UseActive
}
