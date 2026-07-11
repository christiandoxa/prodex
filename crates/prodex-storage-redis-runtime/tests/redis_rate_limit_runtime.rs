use prodex_domain::{RateLimitBucketKey, RateLimitRule, TenantId};
use prodex_storage_redis::{RedisRateLimitDecision, plan_redis_rate_limit};
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
