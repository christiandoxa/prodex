#![forbid(unsafe_code)]
//! Async execution for Prodex Redis rate-limit plans.

use std::{error::Error, fmt, time::Duration};

use prodex_storage_redis::{
    RedisRateLimitDecision, RedisRateLimitPlan, plan_redis_rate_limit_result,
};
use redis::aio::{ConnectionManager, ConnectionManagerConfig};

const DEFAULT_CONNECTION_TIMEOUT: Duration = Duration::from_secs(2);
const DEFAULT_RESPONSE_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone, PartialEq, Eq)]
pub struct RedisRuntimeConfig {
    redis_url: String,
    connection_timeout: Duration,
    response_timeout: Duration,
}

impl RedisRuntimeConfig {
    pub fn new(redis_url: impl Into<String>) -> Result<Self, RedisRuntimeError> {
        let redis_url = redis_url.into();
        redis::Client::open(redis_url.as_str()).map_err(|_| RedisRuntimeError::Configuration)?;
        Ok(Self {
            redis_url,
            connection_timeout: DEFAULT_CONNECTION_TIMEOUT,
            response_timeout: DEFAULT_RESPONSE_TIMEOUT,
        })
    }

    pub fn with_connection_timeout(mut self, timeout: Duration) -> Result<Self, RedisRuntimeError> {
        if timeout.is_zero() {
            return Err(RedisRuntimeError::Configuration);
        }
        self.connection_timeout = timeout;
        Ok(self)
    }

    pub fn with_response_timeout(mut self, timeout: Duration) -> Result<Self, RedisRuntimeError> {
        if timeout.is_zero() {
            return Err(RedisRuntimeError::Configuration);
        }
        self.response_timeout = timeout;
        Ok(self)
    }

    pub fn connection_timeout(&self) -> Duration {
        self.connection_timeout
    }

    pub fn response_timeout(&self) -> Duration {
        self.response_timeout
    }
}

impl fmt::Debug for RedisRuntimeConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RedisRuntimeConfig")
            .field("redis_url", &"<redacted>")
            .field("connection_timeout", &self.connection_timeout)
            .field("response_timeout", &self.response_timeout)
            .finish()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RedisRuntimeError {
    Configuration,
    Connection,
    NumericOverflow,
    Command,
    InvalidResponse,
}

impl fmt::Debug for RedisRuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Configuration => "Configuration",
            Self::Connection => "Connection",
            Self::NumericOverflow => "NumericOverflow",
            Self::Command => "Command",
            Self::InvalidResponse => "InvalidResponse",
        })
    }
}

impl fmt::Display for RedisRuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Configuration => "Redis runtime configuration is invalid",
            Self::Connection => "Redis connection is unavailable",
            Self::NumericOverflow => "Redis rate-limit value is out of range",
            Self::Command => "Redis rate-limit operation failed",
            Self::InvalidResponse => "Redis rate-limit response is invalid",
        })
    }
}

impl Error for RedisRuntimeError {}

#[derive(Clone)]
pub struct RedisRateLimitExecutor {
    connection: ConnectionManager,
}

impl RedisRateLimitExecutor {
    pub async fn connect(config: &RedisRuntimeConfig) -> Result<Self, RedisRuntimeError> {
        let client = redis::Client::open(config.redis_url.as_str())
            .map_err(|_| RedisRuntimeError::Configuration)?;
        let manager_config = ConnectionManagerConfig::new()
            .set_connection_timeout(Some(config.connection_timeout))
            .set_response_timeout(Some(config.response_timeout));
        let connection = ConnectionManager::new_with_config(client, manager_config)
            .await
            .map_err(|_| RedisRuntimeError::Connection)?;
        Ok(Self { connection })
    }

    pub async fn execute(
        &self,
        plan: &RedisRateLimitPlan,
    ) -> Result<RedisRateLimitDecision, RedisRuntimeError> {
        let arguments = &plan.arguments;
        let now_unix_ms = to_i64(arguments.now_unix_ms)?;
        let window_seconds = to_i64(arguments.window_seconds)?;
        let max_requests = to_i64(arguments.max_requests)?;
        let increment_requests = to_i64(arguments.increment_requests)?;
        let mut connection = self.connection.clone();
        let result: (i64, i64, i64) = redis::cmd("EVAL")
            .arg(plan.script.lua)
            .arg(1)
            .arg(plan.key.as_str())
            .arg(now_unix_ms)
            .arg(window_seconds)
            .arg(max_requests)
            .arg(increment_requests)
            .query_async(&mut connection)
            .await
            .map_err(|_| RedisRuntimeError::Command)?;

        plan_redis_rate_limit_result(result.0, result.1, result.2)
            .map_err(|_| RedisRuntimeError::InvalidResponse)
    }
}

impl fmt::Debug for RedisRateLimitExecutor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RedisRateLimitExecutor")
            .field("connection", &"<redacted>")
            .finish()
    }
}

fn to_i64(value: u64) -> Result<i64, RedisRuntimeError> {
    i64::try_from(value).map_err(|_| RedisRuntimeError::NumericOverflow)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_requires_a_valid_url_and_nonzero_timeouts() {
        assert_eq!(
            RedisRuntimeConfig::new(""),
            Err(RedisRuntimeError::Configuration)
        );
        assert_eq!(
            RedisRuntimeConfig::new("not a redis url"),
            Err(RedisRuntimeError::Configuration)
        );
        assert_eq!(
            RedisRuntimeConfig::new("redis://localhost/")
                .unwrap()
                .with_connection_timeout(Duration::ZERO),
            Err(RedisRuntimeError::Configuration)
        );
        assert_eq!(
            RedisRuntimeConfig::new("redis://localhost/")
                .unwrap()
                .with_response_timeout(Duration::ZERO),
            Err(RedisRuntimeError::Configuration)
        );
    }

    #[test]
    fn config_executor_and_errors_have_stable_redacted_debug() {
        let password = "do-not-print-this-password";
        let config =
            RedisRuntimeConfig::new(format!("redis://prodex:{password}@redis.internal/0")).unwrap();
        let debug = format!("{config:?}");
        assert!(!debug.contains(password));
        assert!(!debug.contains("redis.internal"));

        for error in [
            RedisRuntimeError::Configuration,
            RedisRuntimeError::Connection,
            RedisRuntimeError::NumericOverflow,
            RedisRuntimeError::Command,
            RedisRuntimeError::InvalidResponse,
        ] {
            assert!(!format!("{error:?}").contains(password));
            assert!(!error.to_string().contains(password));
        }
    }

    #[test]
    fn redis_integer_conversion_is_bounded() {
        assert_eq!(to_i64(i64::MAX as u64), Ok(i64::MAX));
        assert_eq!(
            to_i64(i64::MAX as u64 + 1),
            Err(RedisRuntimeError::NumericOverflow)
        );
    }
}
