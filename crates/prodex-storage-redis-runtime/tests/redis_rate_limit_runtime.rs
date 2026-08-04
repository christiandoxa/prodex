use prodex_domain::{RateLimitBucketKey, RateLimitRule, TenantId};
use prodex_storage_redis::{
    RedisDualRateLimitDecision, RedisRateLimitDecision, RedisRateLimitDimension,
    plan_redis_dual_rate_limit, plan_redis_rate_limit,
};
use prodex_storage_redis_runtime::{RedisRateLimitExecutor, RedisRuntimeConfig, RedisRuntimeError};
use std::time::{SystemTime, UNIX_EPOCH};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires PRODEX_TEST_REDIS_URL"]
async fn independent_executors_share_atomic_allowance_without_overshoot() {
    let url = std::env::var("PRODEX_TEST_REDIS_URL")
        .expect("PRODEX_TEST_REDIS_URL must point to the test Redis instance");
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
#[ignore = "requires PRODEX_TEST_REDIS_URL"]
async fn independent_executors_enforce_dual_limits_all_or_nothing() {
    let url = std::env::var("PRODEX_TEST_REDIS_URL")
        .expect("PRODEX_TEST_REDIS_URL must point to the test Redis instance");
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires PRODEX_TEST_REDIS_URL"]
async fn externally_seeded_oversized_ephemeral_values_are_rejected_without_take_delete() {
    let url = std::env::var("PRODEX_TEST_REDIS_URL")
        .expect("PRODEX_TEST_REDIS_URL must point to the test Redis instance");
    let config = RedisRuntimeConfig::new(url.clone()).expect("test Redis URL should be valid");
    let executor = RedisRateLimitExecutor::connect(&config)
        .await
        .expect("test Redis should connect");
    let client = redis::Client::open(url.as_str()).expect("test Redis URL should open");
    let mut connection = client
        .get_multiplexed_async_connection()
        .await
        .expect("test Redis should accept a direct connection");
    let key = format!(
        "prodex:test:oversized:{}:{}",
        std::process::id(),
        unix_time_ms()
    );
    let value = vec![b'x'; 64 * 1_024 + 1];
    let _: String = redis::cmd("SET")
        .arg(&key)
        .arg(&value)
        .query_async(&mut connection)
        .await
        .expect("test Redis should accept the seeded value");

    assert_eq!(
        executor.get_ephemeral(&key).await,
        Err(RedisRuntimeError::InvalidResponse)
    );
    assert_eq!(
        executor.take_ephemeral(&key).await,
        Err(RedisRuntimeError::InvalidResponse)
    );
    let exists: bool = redis::cmd("EXISTS")
        .arg(&key)
        .query_async(&mut connection)
        .await
        .expect("test Redis should report the seeded value");
    assert!(exists, "oversized take must not delete the seeded value");
    let _: i64 = redis::cmd("DEL")
        .arg(&key)
        .query_async(&mut connection)
        .await
        .expect("test Redis should clean up the seeded value");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires PRODEX_TEST_REDIS_URL"]
async fn ephemeral_member_take_rejects_unsafe_sets_without_delete_and_takes_valid_sets() {
    let url = std::env::var("PRODEX_TEST_REDIS_URL")
        .expect("PRODEX_TEST_REDIS_URL must point to the test Redis instance");
    let config = RedisRuntimeConfig::new(url.clone()).expect("test Redis URL should be valid");
    let executor = RedisRateLimitExecutor::connect(&config)
        .await
        .expect("test Redis should connect");
    let client = redis::Client::open(url.as_str()).expect("test Redis URL should open");
    let mut connection = client
        .get_multiplexed_async_connection()
        .await
        .expect("test Redis should accept a direct connection");

    let cases = [
        ("malformed", vec!["contains/slash".to_owned()]),
        ("oversized", vec!["x".repeat(129)]),
        ("storage", vec!["x".repeat(1_048_576)]),
        (
            "aggregate",
            (0..513)
                .map(|index| format!("m{index:0127}"))
                .collect::<Vec<_>>(),
        ),
    ];
    for (name, members) in cases {
        let key = format!(
            "prodex:test:ephemeral-set:{name}:{}:{}",
            std::process::id(),
            unix_time_ms()
        );
        let _: i64 = redis::cmd("SADD")
            .arg(&key)
            .arg(&members)
            .query_async(&mut connection)
            .await
            .expect("test Redis should accept the seeded set");

        assert_eq!(
            executor.take_ephemeral_members(&key).await,
            Err(RedisRuntimeError::InvalidResponse)
        );
        let exists: bool = redis::cmd("EXISTS")
            .arg(&key)
            .query_async(&mut connection)
            .await
            .expect("test Redis should report the seeded set");
        assert!(exists, "unsafe take must not delete the seeded set: {name}");
        let _: i64 = redis::cmd("DEL")
            .arg(&key)
            .query_async(&mut connection)
            .await
            .expect("test Redis should clean up the seeded set");
    }

    let key = format!(
        "prodex:test:ephemeral-set:valid:{}:{}",
        std::process::id(),
        unix_time_ms()
    );
    let expected = vec!["session_a".to_owned(), "session_b".to_owned()];
    let _: i64 = redis::cmd("SADD")
        .arg(&key)
        .arg(&expected)
        .query_async(&mut connection)
        .await
        .expect("test Redis should accept the valid set");

    let mut actual = executor
        .take_ephemeral_members(&key)
        .await
        .expect("valid set take should succeed");
    actual.sort();
    assert_eq!(actual, expected);
    let exists: bool = redis::cmd("EXISTS")
        .arg(&key)
        .query_async(&mut connection)
        .await
        .expect("test Redis should report the taken set");
    assert!(!exists, "valid atomic take must delete the set");
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
