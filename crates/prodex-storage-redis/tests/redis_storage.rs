use prodex_domain::{PolicyRevisionId, RateLimitBucketKey, RateLimitRule, TenantId, VirtualKeyId};
use prodex_storage::TenantStorageKey;
use prodex_storage_redis::RedisStorageUse::{
    CoordinationLease, DistributedRateLimit, DurableAuditLog, DurableBillingLedger,
    DurableConfiguration, DurableUsageAccounting, ShortLivedCache,
};
use prodex_storage_redis::{
    ATOMIC_RATE_LIMIT_LUA, RECOVERY_LEASE_ACQUIRE_LUA, RECOVERY_LEASE_RELEASE_LUA,
    RedisCachePurpose, RedisPlanError, RedisPlanErrorStatus, RedisRateLimitDecision, RedisScript,
    SHORT_LIVED_LOCK_LUA, plan_policy_revision_cache, plan_recovery_lease_acquire,
    plan_recovery_lease_release, plan_redis_error_response, plan_redis_rate_limit,
    plan_redis_rate_limit_result, plan_redis_storage_use, plan_short_lived_coordination_lock,
    script_avoids_whole_map_json, script_uses_atomic_operations,
};

#[test]
fn rate_limit_plan_uses_tenant_scoped_key_and_atomic_lua() {
    let tenant_id = TenantId::new();
    let bucket = RateLimitBucketKey::new(tenant_id, Some(VirtualKeyId::new()), 1_900_000_000_000);
    let plan =
        plan_redis_rate_limit(bucket, RateLimitRule::new(100, 60), 1, 1_900_000_000_001).unwrap();

    assert_eq!(plan.tenant_id, tenant_id);
    assert!(
        plan.key
            .as_str()
            .starts_with(&format!("prodex:tenant:{tenant_id}:rate_limit:"))
    );
    assert_eq!(plan.script.name, "prodex_atomic_rate_limit_v1");
    assert!(plan.script.lua.contains("INCRBY"));
    assert!(plan.script.lua.contains("PEXPIREAT"));
    assert!(script_uses_atomic_operations(ATOMIC_RATE_LIMIT_LUA));
    assert!(script_avoids_whole_map_json(ATOMIC_RATE_LIMIT_LUA));
}

#[test]
fn rate_limit_plan_rejects_zero_limits_or_zero_window() {
    let tenant_id = TenantId::new();
    let bucket = RateLimitBucketKey::new(tenant_id, None, 0);

    assert_eq!(
        plan_redis_rate_limit(bucket, RateLimitRule::new(0, 60), 1, 0),
        Err(RedisPlanError::ZeroLimit)
    );
    assert_eq!(
        plan_redis_rate_limit(bucket, RateLimitRule::new(10, 0), 1, 0),
        Err(RedisPlanError::ZeroWindow)
    );
}

#[test]
fn rate_limit_result_maps_lua_tuple_to_typed_decision() {
    assert_eq!(
        plan_redis_rate_limit_result(1, 7, 59_000).unwrap(),
        RedisRateLimitDecision::Allowed {
            current_requests: 7,
            ttl_ms: Some(59_000),
        }
    );
    assert_eq!(
        plan_redis_rate_limit_result(0, 100, 12_500).unwrap(),
        RedisRateLimitDecision::Limited {
            current_requests: 100,
            retry_after_ms: Some(12_500),
        }
    );
    assert_eq!(
        plan_redis_rate_limit_result(0, 100, -2).unwrap(),
        RedisRateLimitDecision::Limited {
            current_requests: 100,
            retry_after_ms: None,
        }
    );
}

#[test]
fn rate_limit_result_rejects_invalid_lua_tuple() {
    assert_eq!(
        plan_redis_rate_limit_result(2, 1, 1_000),
        Err(RedisPlanError::InvalidRateLimitResult)
    );
    assert_eq!(
        plan_redis_rate_limit_result(1, -1, 1_000),
        Err(RedisPlanError::InvalidRateLimitResult)
    );
    assert_eq!(
        plan_redis_rate_limit_result(1, 1, -3),
        Err(RedisPlanError::InvalidRateLimitResult)
    );
}

#[test]
fn redis_storage_use_allows_only_rebuildable_runtime_roles() {
    let tenant_id = TenantId::new();

    for storage_use in [DistributedRateLimit, ShortLivedCache, CoordinationLease] {
        let plan = plan_redis_storage_use(tenant_id, storage_use).unwrap();
        assert_eq!(plan.tenant_id, tenant_id);
        assert_eq!(plan.storage_use, storage_use);
        assert_eq!(plan.durable_source_of_truth, "postgres");
    }
}

#[test]
fn redis_storage_use_rejects_durable_tenant_owned_state() {
    let tenant_id = TenantId::new();

    for storage_use in [
        DurableUsageAccounting,
        DurableBillingLedger,
        DurableAuditLog,
        DurableConfiguration,
    ] {
        assert_eq!(
            plan_redis_storage_use(tenant_id, storage_use),
            Err(RedisPlanError::DurableStateForbidden)
        );
    }
}

#[test]
fn policy_cache_plan_requires_tenant_key_and_ttl() {
    let tenant_id = TenantId::new();
    let revision_id = PolicyRevisionId::new();

    let plan = plan_policy_revision_cache(tenant_id, revision_id, 300).unwrap();

    assert_eq!(plan.tenant_id, tenant_id);
    assert_eq!(plan.revision_id, Some(revision_id));
    assert_eq!(plan.purpose, RedisCachePurpose::PolicyRevision);
    assert!(
        plan.key
            .as_str()
            .contains(&format!("prodex:tenant:{tenant_id}:policy:"))
    );
    assert_eq!(
        plan_policy_revision_cache(tenant_id, revision_id, 0),
        Err(RedisPlanError::MissingTtl)
    );
}

#[test]
fn coordination_lock_is_short_lived_tenant_scoped_and_separate_from_json_ledger() {
    let tenant_id = TenantId::new();
    let virtual_key_id = VirtualKeyId::new();
    let storage_key = TenantStorageKey::virtual_key(tenant_id, virtual_key_id);

    let plan = plan_short_lived_coordination_lock(tenant_id, storage_key, None, 30).unwrap();

    assert_eq!(plan.tenant_id, tenant_id);
    assert!(
        plan.key
            .as_str()
            .contains(&format!("prodex:tenant:{tenant_id}:coordination:"))
    );
    assert_eq!(plan.script.name, "prodex_short_lived_lock_v1");
    assert!(plan.script.lua.contains("'NX'"));
    assert!(plan.script.lua.contains("'PX'"));
    assert!(script_uses_atomic_operations(SHORT_LIVED_LOCK_LUA));
    assert!(script_avoids_whole_map_json(SHORT_LIVED_LOCK_LUA));
}

#[test]
fn coordination_lock_rejects_cross_tenant_or_missing_ttl() {
    let tenant_id = TenantId::new();
    let other_tenant = TenantId::new();
    let storage_key = TenantStorageKey::tenant(other_tenant);

    assert_eq!(
        plan_short_lived_coordination_lock(tenant_id, storage_key, None, 30),
        Err(RedisPlanError::TenantMismatch {
            expected: tenant_id,
            actual: other_tenant,
        })
    );
    assert_eq!(
        plan_short_lived_coordination_lock(other_tenant, storage_key, None, 0),
        Err(RedisPlanError::MissingTtl)
    );
}

#[test]
fn recovery_lease_is_tenant_scoped_owner_token_based_and_atomic() {
    let tenant_id = TenantId::new();
    let storage_key = TenantStorageKey::tenant(tenant_id);

    let acquire = plan_recovery_lease_acquire(
        tenant_id,
        storage_key,
        "expired-reservations",
        "replica-a",
        45,
    )
    .unwrap();

    assert_eq!(acquire.tenant_id, tenant_id);
    assert_eq!(acquire.storage_key.tenant_id, tenant_id);
    assert_eq!(acquire.owner_token, "replica-a");
    assert_eq!(acquire.ttl_seconds, 45);
    assert!(
        acquire
            .key
            .as_str()
            .starts_with(&format!("prodex:tenant:{tenant_id}:recovery_lease:"))
    );
    assert_eq!(acquire.script.name, "prodex_recovery_lease_acquire_v1");
    assert!(RECOVERY_LEASE_ACQUIRE_LUA.lua.contains("'NX'"));
    assert!(RECOVERY_LEASE_ACQUIRE_LUA.lua.contains("'PX'"));
    assert!(RECOVERY_LEASE_ACQUIRE_LUA.lua.contains("PEXPIRE"));
    assert!(script_uses_atomic_operations(RECOVERY_LEASE_ACQUIRE_LUA));
    assert!(script_avoids_whole_map_json(RECOVERY_LEASE_ACQUIRE_LUA));

    let release =
        plan_recovery_lease_release(tenant_id, storage_key, "expired-reservations", "replica-a")
            .unwrap();
    assert_eq!(release.tenant_id, tenant_id);
    assert_eq!(release.owner_token, "replica-a");
    assert_eq!(release.key, acquire.key);
    assert_eq!(release.script.name, "prodex_recovery_lease_release_v1");
    assert!(RECOVERY_LEASE_RELEASE_LUA.lua.contains("GET"));
    assert!(RECOVERY_LEASE_RELEASE_LUA.lua.contains("PEXPIRE"));
    assert!(!RECOVERY_LEASE_RELEASE_LUA.lua.contains("DEL"));
    assert!(script_uses_atomic_operations(RECOVERY_LEASE_RELEASE_LUA));
    assert!(script_avoids_whole_map_json(RECOVERY_LEASE_RELEASE_LUA));
}

#[test]
fn recovery_lease_rejects_cross_tenant_missing_ttl_or_empty_owner() {
    let tenant_id = TenantId::new();
    let other_tenant = TenantId::new();
    let storage_key = TenantStorageKey::tenant(other_tenant);

    assert_eq!(
        plan_recovery_lease_acquire(tenant_id, storage_key, "expired", "replica-a", 30),
        Err(RedisPlanError::TenantMismatch {
            expected: tenant_id,
            actual: other_tenant,
        })
    );
    assert_eq!(
        plan_recovery_lease_acquire(other_tenant, storage_key, "expired", "replica-a", 0),
        Err(RedisPlanError::MissingTtl)
    );
    assert_eq!(
        plan_recovery_lease_acquire(other_tenant, storage_key, "expired", "   ", 30),
        Err(RedisPlanError::EmptyLeaseOwner)
    );
    assert_eq!(
        plan_recovery_lease_release(other_tenant, storage_key, "expired", "   "),
        Err(RedisPlanError::EmptyLeaseOwner)
    );
}

#[test]
fn script_guard_rejects_json_and_whole_list_rewrites() {
    for lua in [
        "redis.call('GET', KEYS[1]); cjson.decode(ARGV[1])",
        "redis.call('LRANGE', KEYS[1], 0, -1)",
        "redis.call('LSET', KEYS[1], 0, ARGV[1])",
        "redis.call('RPUSH', KEYS[1], ARGV[1])",
        "redis.call('DEL', KEYS[1])",
    ] {
        assert!(!script_avoids_whole_map_json(RedisScript {
            name: "bad",
            lua,
        }));
    }
}

#[test]
fn redis_plan_error_response_is_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let other_tenant = TenantId::new();
    let error = RedisPlanError::TenantMismatch {
        expected: tenant_id,
        actual: other_tenant,
    };
    assert!(!error.to_string().contains(&tenant_id.to_string()));
    assert!(!error.to_string().contains(&other_tenant.to_string()));

    let response = plan_redis_error_response(&error);

    assert_eq!(response.status, RedisPlanErrorStatus::ServiceUnavailable);
    assert_eq!(response.code, "redis_coordination_unavailable");
    assert_eq!(
        response.message,
        "redis coordination is temporarily unavailable"
    );
    assert!(!response.message.contains("Redis"));
    assert!(!response.message.contains(&tenant_id.to_string()));
    assert!(!response.message.contains(&other_tenant.to_string()));
}
