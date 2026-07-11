use prodex_application::distributed_rate_limit::{
    ApplicationDistributedRateLimitErrorStatus, ApplicationDistributedRateLimitRequest,
    plan_application_distributed_rate_limit,
    plan_application_distributed_rate_limit_error_response,
};
use prodex_domain::{RateLimitBucketKey, TenantId, VirtualKeyId};

#[test]
fn application_plans_atomic_distributed_rpm_and_tpm() {
    let tenant_id = TenantId::new();
    let request = ApplicationDistributedRateLimitRequest {
        bucket: RateLimitBucketKey::new(tenant_id, Some(VirtualKeyId::new()), 1_900_000_000_000),
        window_seconds: 60,
        max_requests: Some(10),
        max_tokens: Some(2_000),
        increment_tokens: 250,
        now_unix_ms: 1_900_000_000_001,
    };

    let plan = plan_application_distributed_rate_limit(request.clone()).unwrap();
    assert_eq!(plan.redis.tenant_id, tenant_id);
    assert_ne!(plan.redis.request_key, plan.redis.token_key);
    assert_eq!(plan.redis.script.name, "prodex_atomic_dual_rate_limit_v1");

    let request_debug = format!("{request:?}");
    let plan_debug = format!("{plan:?}");
    for secret in [tenant_id.to_string(), "1900000000001".to_string()] {
        assert!(!request_debug.contains(&secret), "{request_debug}");
        assert!(!plan_debug.contains(&secret), "{plan_debug}");
    }
}

#[test]
fn application_distributed_rate_limit_errors_are_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let error = plan_application_distributed_rate_limit(ApplicationDistributedRateLimitRequest {
        bucket: RateLimitBucketKey::new(tenant_id, None, 1_900_000_000_000),
        window_seconds: 0,
        max_requests: Some(10),
        max_tokens: None,
        increment_tokens: 0,
        now_unix_ms: 1_900_000_000_001,
    })
    .unwrap_err();

    let response = plan_application_distributed_rate_limit_error_response(&error);
    assert_eq!(
        response.status,
        ApplicationDistributedRateLimitErrorStatus::ServiceUnavailable
    );
    assert_eq!(response.code, "distributed_rate_limit_unavailable");
    assert_eq!(
        response.message,
        "distributed rate limiting is temporarily unavailable"
    );
    assert!(!format!("{error:?}").contains(&tenant_id.to_string()));
    assert!(!error.to_string().contains(&tenant_id.to_string()));
}
