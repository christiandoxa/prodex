use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{TenantId, VirtualKeyId};

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RateLimitRule {
    pub max_requests: u64,
    pub window_seconds: u64,
}

impl fmt::Debug for RateLimitRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RateLimitRule")
            .field("max_requests", &"<redacted>")
            .field("window_seconds", &"<redacted>")
            .finish()
    }
}

impl RateLimitRule {
    pub fn new(max_requests: u64, window_seconds: u64) -> Self {
        Self {
            max_requests,
            window_seconds,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RateLimitSnapshot {
    pub tenant_id: TenantId,
    pub used_requests: u64,
    pub window_reset_unix_ms: u64,
}

impl fmt::Debug for RateLimitSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RateLimitSnapshot")
            .field("tenant_id", &"<redacted>")
            .field("used_requests", &"<redacted>")
            .field("window_reset_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RateLimitBucketKey {
    pub tenant_id: TenantId,
    pub virtual_key_id: Option<VirtualKeyId>,
    pub window_start_unix_ms: u64,
}

impl RateLimitBucketKey {
    pub fn new(
        tenant_id: TenantId,
        virtual_key_id: Option<VirtualKeyId>,
        window_start_unix_ms: u64,
    ) -> Self {
        Self {
            tenant_id,
            virtual_key_id,
            window_start_unix_ms,
        }
    }

    pub fn cache_key(self) -> String {
        match self.virtual_key_id {
            Some(virtual_key_id) => {
                format!(
                    "tenant:{}:virtual_key:{}:window:{}",
                    self.tenant_id, virtual_key_id, self.window_start_unix_ms
                )
            }
            None => format!(
                "tenant:{}:window:{}",
                self.tenant_id, self.window_start_unix_ms
            ),
        }
    }
}

impl fmt::Debug for RateLimitBucketKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RateLimitBucketKey")
            .field("tenant_id", &"<redacted>")
            .field("virtual_key_id", &self.virtual_key_id.map(|_| "<redacted>"))
            .field("window_start_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RateLimitRequest {
    pub tenant_id: TenantId,
    pub requested_requests: u64,
    pub now_unix_ms: u64,
}

impl fmt::Debug for RateLimitRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RateLimitRequest")
            .field("tenant_id", &"<redacted>")
            .field("requested_requests", &"<redacted>")
            .field("now_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RateLimitAllowance {
    pub remaining_after_admission: u64,
    pub reset_unix_ms: u64,
}

impl fmt::Debug for RateLimitAllowance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RateLimitAllowance")
            .field("remaining_after_admission", &"<redacted>")
            .field("reset_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RateLimitRejection {
    pub retry_after_seconds: u64,
    pub reset_unix_ms: u64,
    pub remaining: u64,
}

impl fmt::Debug for RateLimitRejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RateLimitRejection")
            .field("retry_after_seconds", &"<redacted>")
            .field("reset_unix_ms", &"<redacted>")
            .field("remaining", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RateLimitDecision {
    Allow(RateLimitAllowance),
    Reject(RateLimitRejection),
}

impl fmt::Debug for RateLimitDecision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Allow(allowance) => f.debug_tuple("Allow").field(allowance).finish(),
            Self::Reject(rejection) => f.debug_tuple("Reject").field(rejection).finish(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RateLimitErrorStatus {
    TooManyRequests,
    InvalidRequest,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RateLimitErrorResponsePlan {
    pub status: RateLimitErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
    pub retry_after_seconds: Option<u64>,
}

impl fmt::Debug for RateLimitErrorResponsePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RateLimitErrorResponsePlan")
            .field("status", &self.status)
            .field("code", &self.code)
            .field("message", &self.message)
            .field(
                "retry_after_seconds",
                &self.retry_after_seconds.map(|_| "<redacted>"),
            )
            .finish()
    }
}

pub fn plan_rate_limit_decision_error_response(
    decision: &RateLimitDecision,
) -> Option<RateLimitErrorResponsePlan> {
    match decision {
        RateLimitDecision::Allow(_) => None,
        RateLimitDecision::Reject(rejection) => Some(RateLimitErrorResponsePlan {
            status: RateLimitErrorStatus::TooManyRequests,
            code: "rate_limit_exceeded",
            message: "rate limit exceeded",
            retry_after_seconds: Some(rejection.retry_after_seconds),
        }),
    }
}

pub fn plan_rate_limit_atomic_update_error_response(
    error: &RateLimitAtomicUpdateError,
) -> RateLimitErrorResponsePlan {
    let code = match error {
        RateLimitAtomicUpdateError::TenantMismatch => "rate_limit_tenant_mismatch",
        RateLimitAtomicUpdateError::ZeroIncrement => "rate_limit_increment_invalid",
        RateLimitAtomicUpdateError::ExpiredWindow => "rate_limit_window_invalid",
    };

    RateLimitErrorResponsePlan {
        status: RateLimitErrorStatus::InvalidRequest,
        code,
        message: "rate limit update is invalid",
        retry_after_seconds: None,
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RateLimitAtomicUpdate {
    pub key: RateLimitBucketKey,
    pub increment_requests: u64,
    pub expire_at_unix_ms: u64,
}

impl RateLimitAtomicUpdate {
    pub fn from_allowed_request(
        key: RateLimitBucketKey,
        request: RateLimitRequest,
        allowance: RateLimitAllowance,
    ) -> Result<Self, RateLimitAtomicUpdateError> {
        if key.tenant_id != request.tenant_id {
            return Err(RateLimitAtomicUpdateError::TenantMismatch);
        }
        if request.requested_requests == 0 {
            return Err(RateLimitAtomicUpdateError::ZeroIncrement);
        }
        if allowance.reset_unix_ms <= request.now_unix_ms {
            return Err(RateLimitAtomicUpdateError::ExpiredWindow);
        }
        Ok(Self {
            key,
            increment_requests: request.requested_requests,
            expire_at_unix_ms: allowance.reset_unix_ms,
        })
    }
}

impl fmt::Debug for RateLimitAtomicUpdate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RateLimitAtomicUpdate")
            .field("key", &self.key)
            .field("increment_requests", &"<redacted>")
            .field("expire_at_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RateLimitAtomicUpdateError {
    TenantMismatch,
    ZeroIncrement,
    ExpiredWindow,
}

pub fn evaluate_rate_limit(
    rule: RateLimitRule,
    snapshot: RateLimitSnapshot,
    request: RateLimitRequest,
) -> RateLimitDecision {
    if rule.max_requests == 0 || rule.window_seconds == 0 || rule.window_seconds > u64::MAX / 1_000
    {
        return RateLimitDecision::Reject(RateLimitRejection {
            retry_after_seconds: 0,
            reset_unix_ms: snapshot.window_reset_unix_ms,
            remaining: 0,
        });
    }
    let window_reset_unix_ms = if request.now_unix_ms >= snapshot.window_reset_unix_ms {
        request
            .now_unix_ms
            .saturating_add(rule.window_seconds.saturating_mul(1_000))
    } else {
        snapshot.window_reset_unix_ms
    };
    let used_requests = if request.now_unix_ms >= snapshot.window_reset_unix_ms {
        0
    } else {
        snapshot.used_requests
    };
    let remaining = rule.max_requests.saturating_sub(used_requests);
    if request.tenant_id != snapshot.tenant_id
        || request.requested_requests == 0
        || request.requested_requests > remaining
    {
        let retry_after_seconds = window_reset_unix_ms
            .saturating_sub(request.now_unix_ms)
            .saturating_add(999)
            / 1_000;
        return RateLimitDecision::Reject(RateLimitRejection {
            retry_after_seconds,
            reset_unix_ms: window_reset_unix_ms,
            remaining,
        });
    }

    RateLimitDecision::Allow(RateLimitAllowance {
        remaining_after_admission: remaining - request.requested_requests,
        reset_unix_ms: window_reset_unix_ms,
    })
}
