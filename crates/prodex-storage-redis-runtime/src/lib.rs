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
const MAX_EPHEMERAL_KEY_BYTES: usize = 256;
const MAX_EPHEMERAL_VALUE_BYTES: usize = 64 * 1_024;
const MAX_EPHEMERAL_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const MAX_EPHEMERAL_SET_MEMBERS: usize = 4_096;
const MAX_EPHEMERAL_MEMBER_BYTES: usize = 128;
const MAX_EPHEMERAL_SET_STORAGE_BYTES: usize = 1_024 * 1_024;
const EPHEMERAL_LIMIT_ERROR: &str = "PRODEX_EPHEMERAL_LIMIT";
const GET_EPHEMERAL_SCRIPT: &str = "if redis.call('STRLEN', KEYS[1]) > tonumber(ARGV[1]) then return redis.error_reply('PRODEX_EPHEMERAL_LIMIT') end; return redis.call('GET', KEYS[1])";
const TAKE_EPHEMERAL_SCRIPT: &str = "if redis.call('STRLEN', KEYS[1]) > tonumber(ARGV[1]) then return redis.error_reply('PRODEX_EPHEMERAL_LIMIT') end; local value = redis.call('GET', KEYS[1]); if value then redis.call('DEL', KEYS[1]); end; return value";
const TAKE_EPHEMERAL_MEMBERS_SCRIPT: &str = "local max_members = tonumber(ARGV[1]); local max_member_bytes = tonumber(ARGV[2]); local max_total_bytes = tonumber(ARGV[3]); local max_storage_bytes = tonumber(ARGV[4]); if redis.call('SCARD', KEYS[1]) > max_members then return redis.error_reply('PRODEX_EPHEMERAL_LIMIT') end; local stored_bytes = redis.call('MEMORY', 'USAGE', KEYS[1], 'SAMPLES', 0); if stored_bytes and stored_bytes > max_storage_bytes then return redis.error_reply('PRODEX_EPHEMERAL_LIMIT') end; local values = redis.call('SMEMBERS', KEYS[1]); local total_bytes = 0; for _, member in ipairs(values) do local member_bytes = string.len(member); if member_bytes == 0 or member_bytes > max_member_bytes or not string.match(member, '^[A-Za-z0-9_-]+$') then return redis.error_reply('PRODEX_EPHEMERAL_LIMIT') end; total_bytes = total_bytes + member_bytes; if total_bytes > max_total_bytes then return redis.error_reply('PRODEX_EPHEMERAL_LIMIT') end end; redis.call('DEL', KEYS[1]); return values";

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
            Self::NumericOverflow => "Redis numeric value is out of range",
            Self::Command => "Redis operation failed",
            Self::InvalidResponse => "Redis response is invalid",
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

    pub async fn put_ephemeral(
        &self,
        key: &str,
        value: &str,
        ttl: Duration,
    ) -> Result<bool, RedisRuntimeError> {
        let ttl_ms = validate_ephemeral_entry(key, value, ttl)?;
        let mut connection = self.connection.clone();
        let result: Option<String> = redis::cmd("SET")
            .arg(key)
            .arg(value)
            .arg("NX")
            .arg("PX")
            .arg(ttl_ms)
            .query_async(&mut connection)
            .await
            .map_err(|_| RedisRuntimeError::Command)?;
        Ok(result.as_deref() == Some("OK"))
    }

    pub async fn get_ephemeral(&self, key: &str) -> Result<Option<String>, RedisRuntimeError> {
        validate_ephemeral_key(key)?;
        let mut connection = self.connection.clone();
        redis::cmd("EVAL")
            .arg(GET_EPHEMERAL_SCRIPT)
            .arg(1)
            .arg(key)
            .arg(MAX_EPHEMERAL_VALUE_BYTES)
            .query_async(&mut connection)
            .await
            .map_err(map_ephemeral_read_error)
    }

    pub async fn take_ephemeral(&self, key: &str) -> Result<Option<String>, RedisRuntimeError> {
        validate_ephemeral_key(key)?;
        let mut connection = self.connection.clone();
        redis::cmd("EVAL")
            .arg(TAKE_EPHEMERAL_SCRIPT)
            .arg(1)
            .arg(key)
            .arg(MAX_EPHEMERAL_VALUE_BYTES)
            .query_async(&mut connection)
            .await
            .map_err(map_ephemeral_read_error)
    }

    pub async fn delete_ephemeral(&self, key: &str) -> Result<(), RedisRuntimeError> {
        validate_ephemeral_key(key)?;
        let mut connection = self.connection.clone();
        let _: i64 = redis::cmd("DEL")
            .arg(key)
            .query_async(&mut connection)
            .await
            .map_err(|_| RedisRuntimeError::Command)?;
        Ok(())
    }

    pub async fn add_ephemeral_member(
        &self,
        key: &str,
        member: &str,
        ttl: Duration,
    ) -> Result<(), RedisRuntimeError> {
        validate_ephemeral_member(key, member, ttl)?;
        let ttl_ms =
            i64::try_from(ttl.as_millis()).map_err(|_| RedisRuntimeError::NumericOverflow)?;
        let mut connection = self.connection.clone();
        let _: i64 = redis::cmd("EVAL")
            .arg("if redis.call('SISMEMBER', KEYS[1], ARGV[1]) == 0 and redis.call('SCARD', KEYS[1]) >= tonumber(ARGV[3]) then return redis.error_reply('ephemeral set limit exceeded') end; redis.call('SADD', KEYS[1], ARGV[1]); redis.call('PEXPIRE', KEYS[1], ARGV[2]); return 1")
            .arg(1)
            .arg(key)
            .arg(member)
            .arg(ttl_ms)
            .arg(MAX_EPHEMERAL_SET_MEMBERS)
            .query_async(&mut connection)
            .await
            .map_err(|_| RedisRuntimeError::Command)?;
        Ok(())
    }

    pub async fn remove_ephemeral_member(
        &self,
        key: &str,
        member: &str,
    ) -> Result<(), RedisRuntimeError> {
        validate_ephemeral_key(key)?;
        validate_ephemeral_member_value(member)?;
        let mut connection = self.connection.clone();
        let _: i64 = redis::cmd("SREM")
            .arg(key)
            .arg(member)
            .query_async(&mut connection)
            .await
            .map_err(|_| RedisRuntimeError::Command)?;
        Ok(())
    }

    pub async fn take_ephemeral_members(
        &self,
        key: &str,
    ) -> Result<Vec<String>, RedisRuntimeError> {
        validate_ephemeral_key(key)?;
        let mut connection = self.connection.clone();
        let members: Vec<String> = redis::cmd("EVAL")
            .arg(TAKE_EPHEMERAL_MEMBERS_SCRIPT)
            .arg(1)
            .arg(key)
            .arg(MAX_EPHEMERAL_SET_MEMBERS)
            .arg(MAX_EPHEMERAL_MEMBER_BYTES)
            .arg(MAX_EPHEMERAL_VALUE_BYTES)
            .arg(MAX_EPHEMERAL_SET_STORAGE_BYTES)
            .query_async(&mut connection)
            .await
            .map_err(map_ephemeral_read_error)?;
        if members.len() > MAX_EPHEMERAL_SET_MEMBERS
            || members
                .iter()
                .any(|member| validate_ephemeral_member_value(member).is_err())
        {
            return Err(RedisRuntimeError::InvalidResponse);
        }
        Ok(members)
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

fn validate_ephemeral_entry(
    key: &str,
    value: &str,
    ttl: Duration,
) -> Result<i64, RedisRuntimeError> {
    validate_ephemeral_key(key)?;
    if value.is_empty()
        || value.len() > MAX_EPHEMERAL_VALUE_BYTES
        || ttl.is_zero()
        || ttl > MAX_EPHEMERAL_TTL
    {
        return Err(RedisRuntimeError::Configuration);
    }
    i64::try_from(ttl.as_millis()).map_err(|_| RedisRuntimeError::NumericOverflow)
}

fn map_ephemeral_read_error(error: redis::RedisError) -> RedisRuntimeError {
    if error.code() == Some(EPHEMERAL_LIMIT_ERROR)
        || error
            .detail()
            .is_some_and(|detail| detail.contains(EPHEMERAL_LIMIT_ERROR))
    {
        RedisRuntimeError::InvalidResponse
    } else {
        RedisRuntimeError::Command
    }
}

fn validate_ephemeral_key(key: &str) -> Result<(), RedisRuntimeError> {
    if key.is_empty()
        || key.len() > MAX_EPHEMERAL_KEY_BYTES
        || !key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'_' | b'-' | b'.'))
    {
        return Err(RedisRuntimeError::Configuration);
    }
    Ok(())
}

fn validate_ephemeral_member(
    key: &str,
    member: &str,
    ttl: Duration,
) -> Result<(), RedisRuntimeError> {
    validate_ephemeral_key(key)?;
    validate_ephemeral_member_value(member)?;
    if ttl.is_zero() || ttl > MAX_EPHEMERAL_TTL {
        return Err(RedisRuntimeError::Configuration);
    }
    Ok(())
}

fn validate_ephemeral_member_value(member: &str) -> Result<(), RedisRuntimeError> {
    if member.is_empty()
        || member.len() > MAX_EPHEMERAL_MEMBER_BYTES
        || !member
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(RedisRuntimeError::Configuration);
    }
    Ok(())
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
    fn ephemeral_entries_are_strictly_bounded() {
        assert_eq!(
            validate_ephemeral_entry("prodex:browser:session_1", "value", Duration::from_secs(1)),
            Ok(1_000)
        );
        for key in ["", "contains space", "contains/slash"] {
            assert_eq!(
                validate_ephemeral_entry(key, "value", Duration::from_secs(1)),
                Err(RedisRuntimeError::Configuration)
            );
        }
        assert_eq!(
            validate_ephemeral_entry("valid", "", Duration::from_secs(1)),
            Err(RedisRuntimeError::Configuration)
        );
        assert_eq!(
            validate_ephemeral_entry("valid", "value", Duration::ZERO),
            Err(RedisRuntimeError::Configuration)
        );
        assert_eq!(
            validate_ephemeral_entry(
                "valid",
                "value",
                MAX_EPHEMERAL_TTL + Duration::from_millis(1)
            ),
            Err(RedisRuntimeError::Configuration)
        );
        assert!(
            validate_ephemeral_member(
                "prodex:browser:index:abc",
                "session_1",
                Duration::from_secs(1)
            )
            .is_ok()
        );
        for member in ["", "contains space", "contains/slash"] {
            assert_eq!(
                validate_ephemeral_member(
                    "prodex:browser:index:abc",
                    member,
                    Duration::from_secs(1)
                ),
                Err(RedisRuntimeError::Configuration)
            );
        }
    }

    #[test]
    fn ephemeral_reads_guard_external_values_before_return_or_delete() {
        assert!(
            GET_EPHEMERAL_SCRIPT.find("STRLEN").unwrap()
                < GET_EPHEMERAL_SCRIPT.find("GET").unwrap()
        );
        assert!(
            TAKE_EPHEMERAL_SCRIPT.find("STRLEN").unwrap()
                < TAKE_EPHEMERAL_SCRIPT.find("GET").unwrap()
        );
        assert!(
            TAKE_EPHEMERAL_SCRIPT.find("GET").unwrap() < TAKE_EPHEMERAL_SCRIPT.find("DEL").unwrap()
        );
        assert!(GET_EPHEMERAL_SCRIPT.contains("ARGV[1]"));
        assert_eq!(
            map_ephemeral_read_error(redis::make_extension_error(
                "ERR".to_string(),
                Some(EPHEMERAL_LIMIT_ERROR.to_string()),
            )),
            RedisRuntimeError::InvalidResponse
        );
    }

    #[test]
    fn ephemeral_member_take_preflights_limits_before_smembers_or_delete() {
        let memory = TAKE_EPHEMERAL_MEMBERS_SCRIPT.find("MEMORY").unwrap();
        let smembers = TAKE_EPHEMERAL_MEMBERS_SCRIPT.find("SMEMBERS").unwrap();
        let delete = TAKE_EPHEMERAL_MEMBERS_SCRIPT.find("DEL").unwrap();

        assert!(memory < smembers);
        assert!(smembers < delete);
        assert!(TAKE_EPHEMERAL_MEMBERS_SCRIPT.contains("'SAMPLES', 0"));
        assert!(TAKE_EPHEMERAL_MEMBERS_SCRIPT.contains("string.len(member)"));
        assert!(TAKE_EPHEMERAL_MEMBERS_SCRIPT.contains("string.match(member"));
        assert!(TAKE_EPHEMERAL_MEMBERS_SCRIPT.contains("total_bytes"));
        assert!(TAKE_EPHEMERAL_MEMBERS_SCRIPT.contains("ARGV[2]"));
        assert!(TAKE_EPHEMERAL_MEMBERS_SCRIPT.contains("ARGV[3]"));
        assert!(TAKE_EPHEMERAL_MEMBERS_SCRIPT.contains("ARGV[4]"));
        assert_eq!(
            map_ephemeral_read_error(redis::make_extension_error(
                EPHEMERAL_LIMIT_ERROR.to_string(),
                None,
            )),
            RedisRuntimeError::InvalidResponse
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
