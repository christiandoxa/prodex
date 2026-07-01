use prodex_domain::{
    RateLimitAllowance, RateLimitAtomicUpdate, RateLimitAtomicUpdateError, RateLimitBucketKey,
    RateLimitDecision, RateLimitErrorStatus, RateLimitRejection, RateLimitRequest, RateLimitRule,
    RateLimitSnapshot, TenantId, VirtualKeyId, evaluate_rate_limit,
    plan_rate_limit_atomic_update_error_response, plan_rate_limit_decision_error_response,
};

#[test]
fn rate_limit_allows_when_remaining_capacity_is_sufficient() {
    let tenant_id = TenantId::new();
    let decision = evaluate_rate_limit(
        RateLimitRule::new(10, 60),
        RateLimitSnapshot {
            tenant_id,
            used_requests: 4,
            window_reset_unix_ms: 61_000,
        },
        RateLimitRequest {
            tenant_id,
            requested_requests: 3,
            now_unix_ms: 1_000,
        },
    );

    assert_eq!(
        decision,
        RateLimitDecision::Allow(RateLimitAllowance {
            remaining_after_admission: 3,
            reset_unix_ms: 61_000,
        })
    );
}

#[test]
fn rate_limit_rejects_with_retry_after_when_capacity_is_insufficient() {
    let tenant_id = TenantId::new();
    let decision = evaluate_rate_limit(
        RateLimitRule::new(10, 60),
        RateLimitSnapshot {
            tenant_id,
            used_requests: 9,
            window_reset_unix_ms: 61_000,
        },
        RateLimitRequest {
            tenant_id,
            requested_requests: 2,
            now_unix_ms: 1_001,
        },
    );

    assert_eq!(
        decision,
        RateLimitDecision::Reject(RateLimitRejection {
            retry_after_seconds: 60,
            reset_unix_ms: 61_000,
            remaining: 1,
        })
    );
}

#[test]
fn expired_rate_limit_window_resets_usage_before_evaluation() {
    let tenant_id = TenantId::new();
    let decision = evaluate_rate_limit(
        RateLimitRule::new(10, 60),
        RateLimitSnapshot {
            tenant_id,
            used_requests: 10,
            window_reset_unix_ms: 61_000,
        },
        RateLimitRequest {
            tenant_id,
            requested_requests: 1,
            now_unix_ms: 61_000,
        },
    );

    assert_eq!(
        decision,
        RateLimitDecision::Allow(RateLimitAllowance {
            remaining_after_admission: 9,
            reset_unix_ms: 121_000,
        })
    );
}

#[test]
fn tenant_mismatch_fails_closed() {
    let decision = evaluate_rate_limit(
        RateLimitRule::new(10, 60),
        RateLimitSnapshot {
            tenant_id: TenantId::new(),
            used_requests: 0,
            window_reset_unix_ms: 61_000,
        },
        RateLimitRequest {
            tenant_id: TenantId::new(),
            requested_requests: 1,
            now_unix_ms: 1_000,
        },
    );

    assert!(matches!(decision, RateLimitDecision::Reject(_)));
}

#[test]
fn zero_request_rate_limit_fails_closed() {
    let tenant_id = TenantId::new();
    let decision = evaluate_rate_limit(
        RateLimitRule::new(10, 60),
        RateLimitSnapshot {
            tenant_id,
            used_requests: 0,
            window_reset_unix_ms: 61_000,
        },
        RateLimitRequest {
            tenant_id,
            requested_requests: 0,
            now_unix_ms: 1_000,
        },
    );

    assert!(matches!(decision, RateLimitDecision::Reject(_)));
}

#[test]
fn invalid_rate_limit_rule_fails_closed() {
    let tenant_id = TenantId::new();
    let snapshot = RateLimitSnapshot {
        tenant_id,
        used_requests: 0,
        window_reset_unix_ms: 61_000,
    };
    let request = RateLimitRequest {
        tenant_id,
        requested_requests: 1,
        now_unix_ms: 1_000,
    };

    assert!(matches!(
        evaluate_rate_limit(RateLimitRule::new(0, 60), snapshot, request),
        RateLimitDecision::Reject(_)
    ));
    assert!(matches!(
        evaluate_rate_limit(RateLimitRule::new(10, 0), snapshot, request),
        RateLimitDecision::Reject(_)
    ));
    assert!(matches!(
        evaluate_rate_limit(
            RateLimitRule::new(10, u64::MAX / 1_000 + 1),
            snapshot,
            request
        ),
        RateLimitDecision::Reject(_)
    ));
}

#[test]
fn rate_limit_rule_debug_output_is_stable_and_redacted() {
    let rule = RateLimitRule::new(42, 60);
    let rendered = format!("{rule:?}");

    assert!(!rendered.contains("42"));
    assert!(!rendered.contains("60"));
    assert_eq!(
        rendered,
        "RateLimitRule { max_requests: \"<redacted>\", window_seconds: \"<redacted>\" }"
    );
}

#[test]
fn rate_limit_bucket_cache_key_includes_tenant_and_optional_virtual_key() {
    let tenant_id = TenantId::new();
    let virtual_key_id = VirtualKeyId::new();

    let tenant_key = RateLimitBucketKey::new(tenant_id, None, 60_000).cache_key();
    assert!(tenant_key.contains(&format!("tenant:{tenant_id}")));
    assert!(tenant_key.contains("window:60000"));
    assert!(!tenant_key.contains("virtual_key:"));

    let virtual_key = RateLimitBucketKey::new(tenant_id, Some(virtual_key_id), 60_000).cache_key();
    assert!(virtual_key.contains(&format!("tenant:{tenant_id}")));
    assert!(virtual_key.contains(&format!("virtual_key:{virtual_key_id}")));
}

#[test]
fn atomic_rate_limit_update_is_built_only_from_allowed_matching_request() {
    let tenant_id = TenantId::new();
    let key = RateLimitBucketKey::new(tenant_id, Some(VirtualKeyId::new()), 60_000);
    let request = RateLimitRequest {
        tenant_id,
        requested_requests: 2,
        now_unix_ms: 1_000,
    };
    let allowance = RateLimitAllowance {
        remaining_after_admission: 8,
        reset_unix_ms: 61_000,
    };

    let update = RateLimitAtomicUpdate::from_allowed_request(key, request, allowance).unwrap();

    assert_eq!(update.key, key);
    assert_eq!(update.increment_requests, 2);
    assert_eq!(update.expire_at_unix_ms, 61_000);
}

#[test]
fn atomic_rate_limit_update_rejects_tenant_mismatch_and_zero_increment() {
    let tenant_id = TenantId::new();
    let key = RateLimitBucketKey::new(tenant_id, None, 60_000);
    let allowance = RateLimitAllowance {
        remaining_after_admission: 8,
        reset_unix_ms: 61_000,
    };

    assert_eq!(
        RateLimitAtomicUpdate::from_allowed_request(
            key,
            RateLimitRequest {
                tenant_id: TenantId::new(),
                requested_requests: 1,
                now_unix_ms: 1_000,
            },
            allowance
        ),
        Err(RateLimitAtomicUpdateError::TenantMismatch)
    );
    assert_eq!(
        RateLimitAtomicUpdate::from_allowed_request(
            key,
            RateLimitRequest {
                tenant_id,
                requested_requests: 0,
                now_unix_ms: 1_000,
            },
            allowance
        ),
        Err(RateLimitAtomicUpdateError::ZeroIncrement)
    );
}

#[test]
fn atomic_rate_limit_update_rejects_expired_window() {
    let tenant_id = TenantId::new();
    let key = RateLimitBucketKey::new(tenant_id, None, 60_000);

    assert_eq!(
        RateLimitAtomicUpdate::from_allowed_request(
            key,
            RateLimitRequest {
                tenant_id,
                requested_requests: 1,
                now_unix_ms: 61_000,
            },
            RateLimitAllowance {
                remaining_after_admission: 8,
                reset_unix_ms: 61_000,
            },
        ),
        Err(RateLimitAtomicUpdateError::ExpiredWindow)
    );
}

#[test]
fn rate_limit_error_responses_are_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let decision = evaluate_rate_limit(
        RateLimitRule::new(10, 60),
        RateLimitSnapshot {
            tenant_id,
            used_requests: 10,
            window_reset_unix_ms: 61_000,
        },
        RateLimitRequest {
            tenant_id,
            requested_requests: 1,
            now_unix_ms: 1_001,
        },
    );

    let response = plan_rate_limit_decision_error_response(&decision).unwrap();
    assert_eq!(response.status, RateLimitErrorStatus::TooManyRequests);
    assert_eq!(response.code, "rate_limit_exceeded");
    assert_eq!(response.message, "rate limit exceeded");
    assert_eq!(response.retry_after_seconds, Some(60));

    let allow = RateLimitDecision::Allow(RateLimitAllowance {
        remaining_after_admission: 1,
        reset_unix_ms: 61_000,
    });
    assert!(plan_rate_limit_decision_error_response(&allow).is_none());

    let mismatch =
        plan_rate_limit_atomic_update_error_response(&RateLimitAtomicUpdateError::TenantMismatch);
    assert_eq!(mismatch.status, RateLimitErrorStatus::InvalidRequest);
    assert_eq!(mismatch.code, "rate_limit_tenant_mismatch");
    assert_eq!(mismatch.message, "rate limit update is invalid");
    assert_eq!(mismatch.retry_after_seconds, None);

    let zero =
        plan_rate_limit_atomic_update_error_response(&RateLimitAtomicUpdateError::ZeroIncrement);
    assert_eq!(zero.status, RateLimitErrorStatus::InvalidRequest);
    assert_eq!(zero.code, "rate_limit_increment_invalid");

    let expired =
        plan_rate_limit_atomic_update_error_response(&RateLimitAtomicUpdateError::ExpiredWindow);
    assert_eq!(expired.status, RateLimitErrorStatus::InvalidRequest);
    assert_eq!(expired.code, "rate_limit_window_invalid");

    let rendered = format!("{response:?} {mismatch:?} {zero:?} {expired:?}");
    for sensitive in [
        &tenant_id.to_string(),
        "61000",
        "remaining",
        "window_reset",
        "virtual_key",
        "used_requests",
        "60",
    ] {
        assert!(
            !rendered.contains(sensitive),
            "rate-limit response leaked sensitive token {sensitive}: {rendered}"
        );
    }
    assert!(rendered.contains("retry_after_seconds: Some(\"<redacted>\")"));
}

#[test]
fn rate_limit_debug_output_is_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let virtual_key_id = VirtualKeyId::new();
    let key = RateLimitBucketKey::new(tenant_id, Some(virtual_key_id), 60_000);
    let snapshot = RateLimitSnapshot {
        tenant_id,
        used_requests: 42,
        window_reset_unix_ms: 61_000,
    };
    let request = RateLimitRequest {
        tenant_id,
        requested_requests: 7,
        now_unix_ms: 1_000,
    };
    let allowance = RateLimitAllowance {
        remaining_after_admission: 3,
        reset_unix_ms: 61_000,
    };
    let rejection = RateLimitRejection {
        retry_after_seconds: 60,
        reset_unix_ms: 61_000,
        remaining: 3,
    };
    let update = RateLimitAtomicUpdate::from_allowed_request(key, request, allowance).unwrap();
    let allow = RateLimitDecision::Allow(allowance);
    let reject = RateLimitDecision::Reject(rejection);

    let rendered = format!(
        "{key:?} {snapshot:?} {request:?} {allowance:?} {rejection:?} {update:?} {allow:?} {reject:?}"
    );

    for sensitive in [
        &tenant_id.to_string(),
        &virtual_key_id.to_string(),
        "60000",
        "61000",
        "1000",
        "42",
        "7",
    ] {
        assert!(
            !rendered.contains(sensitive),
            "rate-limit debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
}
