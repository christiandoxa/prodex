#![forbid(unsafe_code)]
//! Async execution for Prodex Redis rate-limit plans.

use std::{error::Error, fmt, time::Duration};

use prodex_storage_redis::{
    REDIS_LUA_SAFE_INTEGER, RedisDualRateLimitDecision, RedisDualRateLimitPlan,
    RedisRateLimitDecision, RedisRateLimitPlan, plan_redis_dual_rate_limit_result,
    plan_redis_rate_limit_result,
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
        let [
            now_unix_ms,
            window_seconds,
            max_requests,
            increment_requests,
        ] = rate_limit_arguments(plan)?;
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

    pub async fn execute_dual(
        &self,
        plan: &RedisDualRateLimitPlan,
    ) -> Result<RedisDualRateLimitDecision, RedisRuntimeError> {
        let [
            now_unix_ms,
            window_seconds,
            max_requests,
            max_tokens,
            increment_tokens,
        ] = dual_rate_limit_arguments(plan)?;
        let mut connection = self.connection.clone();
        let result: (i64, i64, i64, i64, i64) = redis::cmd("EVAL")
            .arg(plan.script.lua)
            .arg(2)
            .arg(plan.request_key.as_str())
            .arg(plan.token_key.as_str())
            .arg(now_unix_ms)
            .arg(window_seconds)
            .arg(max_requests)
            .arg(max_tokens)
            .arg(increment_tokens)
            .query_async(&mut connection)
            .await
            .map_err(|_| RedisRuntimeError::Command)?;

        plan_redis_dual_rate_limit_result(result.0, result.1, result.2, result.3, result.4)
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

fn rate_limit_arguments(plan: &RedisRateLimitPlan) -> Result<[i64; 4], RedisRuntimeError> {
    let arguments = plan.arguments;
    validate_expiration(arguments.now_unix_ms, arguments.window_seconds)?;
    Ok([
        to_lua_integer(arguments.now_unix_ms)?,
        to_lua_integer(arguments.window_seconds)?,
        to_lua_integer(arguments.max_requests)?,
        to_lua_integer(arguments.increment_requests)?,
    ])
}

fn dual_rate_limit_arguments(plan: &RedisDualRateLimitPlan) -> Result<[i64; 5], RedisRuntimeError> {
    let arguments = plan.arguments;
    validate_expiration(arguments.now_unix_ms, arguments.window_seconds)?;
    Ok([
        to_lua_integer(arguments.now_unix_ms)?,
        to_lua_integer(arguments.window_seconds)?,
        to_lua_integer(arguments.max_requests.unwrap_or_default())?,
        to_lua_integer(arguments.max_tokens.unwrap_or_default())?,
        to_lua_integer(arguments.increment_tokens)?,
    ])
}

fn validate_expiration(now_unix_ms: u64, window_seconds: u64) -> Result<(), RedisRuntimeError> {
    let expires_at = window_seconds
        .checked_mul(1_000)
        .and_then(|window_ms| now_unix_ms.checked_add(window_ms))
        .ok_or(RedisRuntimeError::NumericOverflow)?;
    to_lua_integer(expires_at).map(|_| ())
}

fn to_lua_integer(value: u64) -> Result<i64, RedisRuntimeError> {
    if value > REDIS_LUA_SAFE_INTEGER {
        return Err(RedisRuntimeError::NumericOverflow);
    }
    i64::try_from(value).map_err(|_| RedisRuntimeError::NumericOverflow)
}

#[cfg(test)]
mod tests {
    use super::*;
    use prodex_domain::{RateLimitBucketKey, RateLimitRule, TenantId};
    use prodex_storage_redis::{plan_redis_dual_rate_limit, plan_redis_rate_limit};

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
    fn redis_integer_conversion_is_bounded_by_lua_precision() {
        assert_eq!(
            to_lua_integer(REDIS_LUA_SAFE_INTEGER),
            Ok(REDIS_LUA_SAFE_INTEGER as i64)
        );
        assert_eq!(
            to_lua_integer(REDIS_LUA_SAFE_INTEGER + 1),
            Err(RedisRuntimeError::NumericOverflow)
        );
    }

    #[test]
    fn forged_single_plan_rejects_every_unsafe_numeric_argument() {
        let bucket = RateLimitBucketKey::new(TenantId::new(), None, 0);
        let plan = plan_redis_rate_limit(bucket, RateLimitRule::new(10, 60), 1, 0).unwrap();

        for mutate in [
            |plan: &mut RedisRateLimitPlan| {
                plan.arguments.now_unix_ms = REDIS_LUA_SAFE_INTEGER + 1;
            },
            |plan: &mut RedisRateLimitPlan| {
                plan.arguments.window_seconds = REDIS_LUA_SAFE_INTEGER + 1;
            },
            |plan: &mut RedisRateLimitPlan| {
                plan.arguments.max_requests = REDIS_LUA_SAFE_INTEGER + 1;
            },
            |plan: &mut RedisRateLimitPlan| {
                plan.arguments.increment_requests = REDIS_LUA_SAFE_INTEGER + 1;
            },
        ] {
            let mut forged = plan.clone();
            mutate(&mut forged);
            assert_eq!(
                rate_limit_arguments(&forged),
                Err(RedisRuntimeError::NumericOverflow)
            );
        }
    }

    #[test]
    fn forged_dual_plan_rejects_every_unsafe_numeric_argument() {
        let bucket = RateLimitBucketKey::new(TenantId::new(), None, 0);
        let plan = plan_redis_dual_rate_limit(bucket, 60, Some(10), Some(1_000), 10, 0).unwrap();

        for mutate in [
            |plan: &mut RedisDualRateLimitPlan| {
                plan.arguments.now_unix_ms = REDIS_LUA_SAFE_INTEGER + 1;
            },
            |plan: &mut RedisDualRateLimitPlan| {
                plan.arguments.window_seconds = REDIS_LUA_SAFE_INTEGER + 1;
            },
            |plan: &mut RedisDualRateLimitPlan| {
                plan.arguments.max_requests = Some(REDIS_LUA_SAFE_INTEGER + 1);
            },
            |plan: &mut RedisDualRateLimitPlan| {
                plan.arguments.max_tokens = Some(REDIS_LUA_SAFE_INTEGER + 1);
            },
            |plan: &mut RedisDualRateLimitPlan| {
                plan.arguments.increment_tokens = REDIS_LUA_SAFE_INTEGER + 1;
            },
        ] {
            let mut forged = plan.clone();
            mutate(&mut forged);
            assert_eq!(
                dual_rate_limit_arguments(&forged),
                Err(RedisRuntimeError::NumericOverflow)
            );
        }
    }

    #[test]
    fn forged_plan_rejects_unsafe_lua_expiration_arithmetic() {
        let bucket = RateLimitBucketKey::new(TenantId::new(), None, 0);
        let mut plan = plan_redis_rate_limit(bucket, RateLimitRule::new(10, 60), 1, 0).unwrap();
        plan.arguments.now_unix_ms = REDIS_LUA_SAFE_INTEGER;
        plan.arguments.window_seconds = 1;

        assert_eq!(
            rate_limit_arguments(&plan),
            Err(RedisRuntimeError::NumericOverflow)
        );
    }
}
