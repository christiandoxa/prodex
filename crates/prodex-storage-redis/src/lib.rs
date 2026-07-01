#![forbid(unsafe_code)]
//! Redis storage plans for distributed rate limiting and short-lived cache keys.
//!
//! This crate is intentionally client-free. It defines tenant-scoped Redis keys
//! and Lua script plans that adapter crates can execute atomically. Redis is not
//! a durable source of truth and must not store whole-map JSON usage state.

use std::error::Error;
use std::fmt;

use prodex_domain::{PolicyRevisionId, RateLimitBucketKey, RateLimitRule, TenantId, VirtualKeyId};
use prodex_storage::TenantStorageKey;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RedisKey(String);

impl RedisKey {
    pub fn new(value: impl Into<String>) -> Result<Self, RedisPlanError> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(RedisPlanError::EmptyKey);
        }
        if trimmed.contains('{') || trimmed.contains('}') {
            return Err(RedisPlanError::UnsafeKeyTag);
        }
        Ok(Self(trimmed.to_string()))
    }

    pub fn tenant_scoped(prefix: &str, tenant_id: TenantId, suffix: &str) -> Self {
        Self(format!("prodex:tenant:{tenant_id}:{prefix}:{suffix}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RedisPlanError {
    EmptyKey,
    UnsafeKeyTag,
    ZeroLimit,
    ZeroWindow,
    DurableStateForbidden,
    TenantMismatch {
        expected: TenantId,
        actual: TenantId,
    },
    MissingTtl,
    EmptyLeaseOwner,
    InvalidRateLimitResult,
}

impl fmt::Display for RedisPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyKey => write!(f, "Redis key must not be empty"),
            Self::UnsafeKeyTag => write!(
                f,
                "Redis key tags are constructed by Prodex and must not be user supplied"
            ),
            Self::ZeroLimit => write!(f, "Redis rate-limit max_requests must be greater than zero"),
            Self::ZeroWindow => write!(
                f,
                "Redis rate-limit window_seconds must be greater than zero"
            ),
            Self::DurableStateForbidden => {
                write!(f, "Redis must not be used as durable tenant-owned state")
            }
            Self::TenantMismatch { .. } => write!(f, "Redis tenant mismatch"),
            Self::MissingTtl => write!(f, "Redis cache/coordinator keys require a TTL"),
            Self::EmptyLeaseOwner => write!(f, "Redis recovery lease owner must not be empty"),
            Self::InvalidRateLimitResult => {
                write!(f, "Redis rate-limit script returned an invalid result")
            }
        }
    }
}

impl Error for RedisPlanError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RedisPlanErrorStatus {
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RedisPlanErrorResponsePlan {
    pub status: RedisPlanErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_redis_error_response(_error: &RedisPlanError) -> RedisPlanErrorResponsePlan {
    RedisPlanErrorResponsePlan {
        status: RedisPlanErrorStatus::ServiceUnavailable,
        code: "redis_coordination_unavailable",
        message: "redis coordination is temporarily unavailable",
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RedisStorageUse {
    DistributedRateLimit,
    ShortLivedCache,
    CoordinationLease,
    DurableUsageAccounting,
    DurableBillingLedger,
    DurableAuditLog,
    DurableConfiguration,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RedisStorageUsePlan {
    pub tenant_id: TenantId,
    pub storage_use: RedisStorageUse,
    pub durable_source_of_truth: &'static str,
}

pub fn plan_redis_storage_use(
    tenant_id: TenantId,
    storage_use: RedisStorageUse,
) -> Result<RedisStorageUsePlan, RedisPlanError> {
    match storage_use {
        RedisStorageUse::DistributedRateLimit
        | RedisStorageUse::ShortLivedCache
        | RedisStorageUse::CoordinationLease => Ok(RedisStorageUsePlan {
            tenant_id,
            storage_use,
            durable_source_of_truth: "postgres",
        }),
        RedisStorageUse::DurableUsageAccounting
        | RedisStorageUse::DurableBillingLedger
        | RedisStorageUse::DurableAuditLog
        | RedisStorageUse::DurableConfiguration => Err(RedisPlanError::DurableStateForbidden),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RedisRateLimitPlan {
    pub tenant_id: TenantId,
    pub bucket: RateLimitBucketKey,
    pub key: RedisKey,
    pub script: RedisScript,
    pub arguments: RedisRateLimitArguments,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RedisScript {
    pub name: &'static str,
    pub lua: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RedisRateLimitArguments {
    pub now_unix_ms: u64,
    pub window_seconds: u64,
    pub max_requests: u64,
    pub increment_requests: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RedisRateLimitDecision {
    Allowed {
        current_requests: u64,
        ttl_ms: Option<u64>,
    },
    Limited {
        current_requests: u64,
        retry_after_ms: Option<u64>,
    },
}

pub fn plan_redis_rate_limit_result(
    allowed_flag: i64,
    current_requests: i64,
    ttl_ms: i64,
) -> Result<RedisRateLimitDecision, RedisPlanError> {
    if current_requests < 0 {
        return Err(RedisPlanError::InvalidRateLimitResult);
    }
    let current_requests =
        u64::try_from(current_requests).map_err(|_| RedisPlanError::InvalidRateLimitResult)?;
    let ttl_ms = match ttl_ms {
        ttl_ms if ttl_ms > 0 => {
            Some(u64::try_from(ttl_ms).map_err(|_| RedisPlanError::InvalidRateLimitResult)?)
        }
        -1 | -2 => None,
        0 => Some(0),
        _ => return Err(RedisPlanError::InvalidRateLimitResult),
    };

    match allowed_flag {
        1 => Ok(RedisRateLimitDecision::Allowed {
            current_requests,
            ttl_ms,
        }),
        0 => Ok(RedisRateLimitDecision::Limited {
            current_requests,
            retry_after_ms: ttl_ms,
        }),
        _ => Err(RedisPlanError::InvalidRateLimitResult),
    }
}

pub const ATOMIC_RATE_LIMIT_LUA: RedisScript = RedisScript {
    name: "prodex_atomic_rate_limit_v1",
    lua: r#"
local now_ms = tonumber(ARGV[1])
local window_seconds = tonumber(ARGV[2])
local max_requests = tonumber(ARGV[3])
local increment = tonumber(ARGV[4])
local current = tonumber(redis.call('GET', KEYS[1]) or '0')
if current + increment > max_requests then
  local ttl_ms = redis.call('PTTL', KEYS[1])
  return {0, current, ttl_ms}
end
local next_value = redis.call('INCRBY', KEYS[1], increment)
if next_value == increment then
  redis.call('PEXPIREAT', KEYS[1], now_ms + (window_seconds * 1000))
end
local ttl_ms = redis.call('PTTL', KEYS[1])
return {1, next_value, ttl_ms}
"#,
};

pub fn plan_redis_rate_limit(
    bucket: RateLimitBucketKey,
    rule: RateLimitRule,
    increment_requests: u64,
    now_unix_ms: u64,
) -> Result<RedisRateLimitPlan, RedisPlanError> {
    if rule.max_requests == 0 {
        return Err(RedisPlanError::ZeroLimit);
    }
    if rule.window_seconds == 0 {
        return Err(RedisPlanError::ZeroWindow);
    }
    if increment_requests == 0 {
        return Err(RedisPlanError::ZeroLimit);
    }
    let key = RedisKey::tenant_scoped("rate_limit", bucket.tenant_id, &bucket.cache_key());
    Ok(RedisRateLimitPlan {
        tenant_id: bucket.tenant_id,
        bucket,
        key,
        script: ATOMIC_RATE_LIMIT_LUA,
        arguments: RedisRateLimitArguments {
            now_unix_ms,
            window_seconds: rule.window_seconds,
            max_requests: rule.max_requests,
            increment_requests,
        },
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RedisCachePlan {
    pub tenant_id: TenantId,
    pub revision_id: Option<PolicyRevisionId>,
    pub key: RedisKey,
    pub ttl_seconds: u64,
    pub purpose: RedisCachePurpose,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RedisCachePurpose {
    PolicyRevision,
    ProviderHealth,
    IdempotencyPending,
}

pub fn plan_policy_revision_cache(
    tenant_id: TenantId,
    revision_id: PolicyRevisionId,
    ttl_seconds: u64,
) -> Result<RedisCachePlan, RedisPlanError> {
    if ttl_seconds == 0 {
        return Err(RedisPlanError::MissingTtl);
    }
    Ok(RedisCachePlan {
        tenant_id,
        revision_id: Some(revision_id),
        key: RedisKey::tenant_scoped("policy", tenant_id, &format!("revision:{revision_id}")),
        ttl_seconds,
        purpose: RedisCachePurpose::PolicyRevision,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RedisCoordinationPlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub key: RedisKey,
    pub ttl_seconds: u64,
    pub script: RedisScript,
}

pub const SHORT_LIVED_LOCK_LUA: RedisScript = RedisScript {
    name: "prodex_short_lived_lock_v1",
    lua: r#"
if redis.call('SET', KEYS[1], ARGV[1], 'NX', 'PX', ARGV[2]) then
  return 1
end
return 0
"#,
};

pub fn plan_short_lived_coordination_lock(
    expected_tenant_id: TenantId,
    storage_key: TenantStorageKey,
    virtual_key_id: Option<VirtualKeyId>,
    ttl_seconds: u64,
) -> Result<RedisCoordinationPlan, RedisPlanError> {
    if storage_key.tenant_id != expected_tenant_id {
        return Err(RedisPlanError::TenantMismatch {
            expected: expected_tenant_id,
            actual: storage_key.tenant_id,
        });
    }
    if ttl_seconds == 0 {
        return Err(RedisPlanError::MissingTtl);
    }
    let suffix = match virtual_key_id.or(storage_key.virtual_key_id) {
        Some(virtual_key_id) => format!("virtual_key:{virtual_key_id}"),
        None => "tenant".to_string(),
    };
    Ok(RedisCoordinationPlan {
        tenant_id: expected_tenant_id,
        storage_key,
        key: RedisKey::tenant_scoped("coordination", expected_tenant_id, &suffix),
        ttl_seconds,
        script: SHORT_LIVED_LOCK_LUA,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RedisRecoveryLeasePlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub key: RedisKey,
    pub owner_token: String,
    pub ttl_seconds: u64,
    pub script: RedisScript,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RedisRecoveryLeaseReleasePlan {
    pub tenant_id: TenantId,
    pub storage_key: TenantStorageKey,
    pub key: RedisKey,
    pub owner_token: String,
    pub script: RedisScript,
}

pub const RECOVERY_LEASE_ACQUIRE_LUA: RedisScript = RedisScript {
    name: "prodex_recovery_lease_acquire_v1",
    lua: r#"
if redis.call('SET', KEYS[1], ARGV[1], 'NX', 'PX', ARGV[2]) then
  return 1
end
if redis.call('GET', KEYS[1]) == ARGV[1] then
  redis.call('PEXPIRE', KEYS[1], ARGV[2])
  return 1
end
return 0
"#,
};

pub const RECOVERY_LEASE_RELEASE_LUA: RedisScript = RedisScript {
    name: "prodex_recovery_lease_release_v1",
    lua: r#"
if redis.call('GET', KEYS[1]) == ARGV[1] then
  redis.call('PEXPIRE', KEYS[1], 1)
  return 1
end
return 0
"#,
};

pub fn plan_recovery_lease_acquire(
    expected_tenant_id: TenantId,
    storage_key: TenantStorageKey,
    shard: impl AsRef<str>,
    owner_token: impl Into<String>,
    ttl_seconds: u64,
) -> Result<RedisRecoveryLeasePlan, RedisPlanError> {
    if storage_key.tenant_id != expected_tenant_id {
        return Err(RedisPlanError::TenantMismatch {
            expected: expected_tenant_id,
            actual: storage_key.tenant_id,
        });
    }
    if ttl_seconds == 0 {
        return Err(RedisPlanError::MissingTtl);
    }
    let owner_token = owner_token.into();
    if owner_token.trim().is_empty() {
        return Err(RedisPlanError::EmptyLeaseOwner);
    }
    let shard = shard.as_ref().trim();
    let suffix = if shard.is_empty() { "tenant" } else { shard };
    Ok(RedisRecoveryLeasePlan {
        tenant_id: expected_tenant_id,
        storage_key,
        key: RedisKey::tenant_scoped("recovery_lease", expected_tenant_id, suffix),
        owner_token,
        ttl_seconds,
        script: RECOVERY_LEASE_ACQUIRE_LUA,
    })
}

pub fn plan_recovery_lease_release(
    expected_tenant_id: TenantId,
    storage_key: TenantStorageKey,
    shard: impl AsRef<str>,
    owner_token: impl Into<String>,
) -> Result<RedisRecoveryLeaseReleasePlan, RedisPlanError> {
    if storage_key.tenant_id != expected_tenant_id {
        return Err(RedisPlanError::TenantMismatch {
            expected: expected_tenant_id,
            actual: storage_key.tenant_id,
        });
    }
    let owner_token = owner_token.into();
    if owner_token.trim().is_empty() {
        return Err(RedisPlanError::EmptyLeaseOwner);
    }
    let shard = shard.as_ref().trim();
    let suffix = if shard.is_empty() { "tenant" } else { shard };
    Ok(RedisRecoveryLeaseReleasePlan {
        tenant_id: expected_tenant_id,
        storage_key,
        key: RedisKey::tenant_scoped("recovery_lease", expected_tenant_id, suffix),
        owner_token,
        script: RECOVERY_LEASE_RELEASE_LUA,
    })
}

pub fn script_uses_atomic_operations(script: RedisScript) -> bool {
    let lua = script.lua.to_ascii_uppercase();
    lua.contains("INCRBY") || lua.contains("SET") && lua.contains("NX") || lua.contains("PEXPIRE")
}

pub fn script_avoids_whole_map_json(script: RedisScript) -> bool {
    let lua = script.lua.to_ascii_lowercase();
    !(lua.contains("json")
        || lua.contains("lrange")
        || lua.contains("lset")
        || lua.contains("rpush")
        || lua.contains("del"))
}
