use prodex_domain::RateLimitBucketKey;
use prodex_storage_redis::{
    RedisDualRateLimitPlan, RedisPlanError, RedisPlanErrorStatus, plan_redis_dual_rate_limit,
    plan_redis_error_response,
};
use std::{error::Error, fmt};

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationDistributedRateLimitRequest {
    pub bucket: RateLimitBucketKey,
    pub window_seconds: u64,
    pub max_requests: Option<u64>,
    pub max_tokens: Option<u64>,
    pub increment_tokens: u64,
    pub now_unix_ms: u64,
}

impl fmt::Debug for ApplicationDistributedRateLimitRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationDistributedRateLimitRequest")
            .field("bucket", &"<redacted>")
            .field("window_seconds", &"<redacted>")
            .field("max_requests", &"<redacted>")
            .field("max_tokens", &"<redacted>")
            .field("increment_tokens", &"<redacted>")
            .field("now_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationDistributedRateLimitPlan {
    pub redis: RedisDualRateLimitPlan,
}

impl fmt::Debug for ApplicationDistributedRateLimitPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationDistributedRateLimitPlan")
            .field("redis", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ApplicationDistributedRateLimitError {
    Redis(RedisPlanError),
}

impl fmt::Debug for ApplicationDistributedRateLimitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Redis(<redacted>)")
    }
}

impl fmt::Display for ApplicationDistributedRateLimitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Redis(error) => error.fmt(f),
        }
    }
}

impl Error for ApplicationDistributedRateLimitError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationDistributedRateLimitErrorStatus {
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationDistributedRateLimitErrorResponsePlan {
    pub status: ApplicationDistributedRateLimitErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_application_distributed_rate_limit_error_response(
    error: &ApplicationDistributedRateLimitError,
) -> ApplicationDistributedRateLimitErrorResponsePlan {
    let response = match error {
        ApplicationDistributedRateLimitError::Redis(error) => plan_redis_error_response(error),
    };
    ApplicationDistributedRateLimitErrorResponsePlan {
        status: match response.status {
            RedisPlanErrorStatus::ServiceUnavailable => {
                ApplicationDistributedRateLimitErrorStatus::ServiceUnavailable
            }
        },
        code: "distributed_rate_limit_unavailable",
        message: "distributed rate limiting is temporarily unavailable",
    }
}

pub fn plan_application_distributed_rate_limit(
    request: ApplicationDistributedRateLimitRequest,
) -> Result<ApplicationDistributedRateLimitPlan, ApplicationDistributedRateLimitError> {
    let redis = plan_redis_dual_rate_limit(
        request.bucket,
        request.window_seconds,
        request.max_requests,
        request.max_tokens,
        request.increment_tokens,
        request.now_unix_ms,
    )
    .map_err(ApplicationDistributedRateLimitError::Redis)?;
    Ok(ApplicationDistributedRateLimitPlan { redis })
}
