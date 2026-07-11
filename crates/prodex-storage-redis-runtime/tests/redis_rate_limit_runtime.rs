use prodex_domain::{RateLimitBucketKey, RateLimitRule, TenantId};
use prodex_storage_redis::{
    RedisDualRateLimitDecision, RedisRateLimitDecision, RedisRateLimitDimension,
    plan_redis_dual_rate_limit, plan_redis_rate_limit,
};
use prodex_storage_redis_runtime::{RedisRateLimitExecutor, RedisRuntimeConfig};
use std::time::{SystemTime, UNIX_EPOCH};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn independent_executors_share_atomic_allowance_without_overshoot() {
    let Some(url) = std::env::var("PRODEX_TEST_REDIS_URL").ok() else {
        eprintln!(
            "skipping independent_executors_share_atomic_allowance_without_overshoot: \
             PRODEX_TEST_REDIS_URL is not set"
        );
        return;
    };
    let config = RedisRuntimeConfig::new(url).expect("test Redis URL should be valid");
    let executor = RedisRateLimitExecutor::connect(&config)
        .await
        .expect("test Redis should connect");
    let second_executor = RedisRateLimitExecutor::connect(&config)
        .await
        .expect("second test Redis connection should connect");
    let tenant_id = TenantId::new();
    let now_unix_ms = u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis(),
    )
    .unwrap();
    let bucket = RateLimitBucketKey::new(tenant_id, None, now_unix_ms);
    let plan = plan_redis_rate_limit(bucket, RateLimitRule::new(5, 60), 1, now_unix_ms).unwrap();

    let mut tasks = Vec::new();
    for index in 0..20 {
        let executor = if index % 2 == 0 {
            executor.clone()
        } else {
            second_executor.clone()
        };
        let plan = plan.clone();
        tasks.push(tokio::spawn(async move { executor.execute(&plan).await }));
    }

    let mut allowed = 0;
    let mut limited = 0;
    for task in tasks {
        match task.await.unwrap().unwrap() {
            RedisRateLimitDecision::Allowed {
                current_requests, ..
            } => {
                allowed += 1;
                assert!(current_requests <= 5);
            }
            RedisRateLimitDecision::Limited {
                current_requests, ..
            } => {
                limited += 1;
                assert_eq!(current_requests, 5);
            }
        }
    }

    assert_eq!(allowed, 5);
    assert_eq!(limited, 15);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn independent_executors_enforce_dual_limits_all_or_nothing() {
    let Some(url) = std::env::var("PRODEX_TEST_REDIS_URL").ok() else {
        eprintln!(
            "skipping independent_executors_enforce_dual_limits_all_or_nothing: \
             PRODEX_TEST_REDIS_URL is not set"
        );
        return;
    };
    let config = RedisRuntimeConfig::new(url).expect("test Redis URL should be valid");
    let executor = RedisRateLimitExecutor::connect(&config)
        .await
        .expect("test Redis should connect");
    let second_executor = RedisRateLimitExecutor::connect(&config)
        .await
        .expect("second test Redis connection should connect");
    let tenant_id = TenantId::new();
    let now_unix_ms = unix_time_ms();

    let rollback_bucket = RateLimitBucketKey::new(tenant_id, None, now_unix_ms);
    let seed =
        plan_redis_dual_rate_limit(rollback_bucket, 60, Some(3), Some(10), 9, now_unix_ms).unwrap();
    match executor.execute_dual(&seed).await.unwrap() {
        RedisDualRateLimitDecision::Allowed {
            current_requests,
            current_tokens,
            ttl_ms,
        } => {
            assert_eq!(current_requests, 1);
            assert_eq!(current_tokens, 9);
            assert!(ttl_ms.is_some());
        }
        decision => panic!("expected initial allowance, got {decision:?}"),
    }

    let denied =
        plan_redis_dual_rate_limit(rollback_bucket, 60, Some(3), Some(10), 2, now_unix_ms).unwrap();
    match second_executor.execute_dual(&denied).await.unwrap() {
        RedisDualRateLimitDecision::Limited {
            dimension,
            current_requests,
            current_tokens,
            ..
        } => {
            assert_eq!(dimension, RedisRateLimitDimension::TokensPerMinute);
            assert_eq!(current_requests, 1);
            assert_eq!(current_tokens, 9);
        }
        decision => panic!("expected TPM denial, got {decision:?}"),
    }

    let fills_tokens =
        plan_redis_dual_rate_limit(rollback_bucket, 60, Some(3), Some(10), 1, now_unix_ms).unwrap();
    match executor.execute_dual(&fills_tokens).await.unwrap() {
        RedisDualRateLimitDecision::Allowed {
            current_requests,
            current_tokens,
            ..
        } => {
            assert_eq!(current_requests, 2);
            assert_eq!(current_tokens, 10);
        }
        decision => panic!("expected allowance after TPM denial, got {decision:?}"),
    }

    let concurrent_bucket = RateLimitBucketKey::new(tenant_id, None, now_unix_ms + 1);
    let concurrent_plan =
        plan_redis_dual_rate_limit(concurrent_bucket, 60, Some(5), Some(15), 3, now_unix_ms)
            .unwrap();
    let mut tasks = Vec::new();
    for index in 0..20 {
        let executor = if index % 2 == 0 {
            executor.clone()
        } else {
            second_executor.clone()
        };
        let plan = concurrent_plan.clone();
        tasks.push(tokio::spawn(
            async move { executor.execute_dual(&plan).await },
        ));
    }

    let mut allowed = 0;
    for task in tasks {
        match task.await.unwrap().unwrap() {
            RedisDualRateLimitDecision::Allowed {
                current_requests,
                current_tokens,
                ..
            } => {
                allowed += 1;
                assert!(current_requests <= 5);
                assert!(current_tokens <= 15);
            }
            RedisDualRateLimitDecision::Limited {
                current_requests,
                current_tokens,
                ..
            } => {
                assert_eq!(current_requests, 5);
                assert_eq!(current_tokens, 15);
            }
        }
    }
    assert_eq!(allowed, 5);
}

fn unix_time_ms() -> u64 {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis(),
    )
    .unwrap()
}
