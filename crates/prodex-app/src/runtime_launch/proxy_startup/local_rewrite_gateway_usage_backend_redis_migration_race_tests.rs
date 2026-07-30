use std::collections::BTreeMap;

use redis::Commands;

use super::{
    RUNTIME_GATEWAY_REDIS_LEGACY_USAGE_BEGIN_SCRIPT,
    RUNTIME_GATEWAY_REDIS_LEGACY_USAGE_FINALIZE_SCRIPT,
    RUNTIME_GATEWAY_REDIS_LEGACY_USAGE_MIGRATE_SCRIPT,
    RUNTIME_GATEWAY_REDIS_LEGACY_USAGE_MIGRATION_MARKER_VALUE, runtime_gateway_redis_connection,
    runtime_gateway_redis_legacy_usage_fingerprint, runtime_gateway_redis_usage_hash_key,
    runtime_gateway_redis_usage_index_key, runtime_gateway_redis_usage_load,
    runtime_gateway_redis_usage_migrated_keys_key,
    runtime_gateway_redis_usage_migration_in_progress_key,
    runtime_gateway_redis_usage_migration_marker_key,
};

#[test]
#[ignore = "requires PRODEX_TEST_REDIS_URL"]
fn redis_legacy_usage_finalize_preserves_concurrent_replacement() {
    let url = std::env::var("PRODEX_TEST_REDIS_URL")
        .expect("PRODEX_TEST_REDIS_URL must point to the test Redis instance");
    let suffix = prodex_domain::RequestId::new();
    let usage_key = format!("prodex:test:gateway:usage-migration-race:{suffix}");
    let marker_key = runtime_gateway_redis_usage_migration_marker_key(&usage_key);
    let in_progress_key = runtime_gateway_redis_usage_migration_in_progress_key(&usage_key);
    let migrated_keys_key = runtime_gateway_redis_usage_migrated_keys_key(&usage_key);
    let index_key = runtime_gateway_redis_usage_index_key(&usage_key);
    let hash_key = runtime_gateway_redis_usage_hash_key(&usage_key, "team-a");
    let legacy_usage = |requests_total| {
        BTreeMap::from([(
            "team-a".to_string(),
            runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage {
                minute_epoch: 100,
                requests_this_minute: 5,
                tokens_this_minute: 50,
                requests_total,
                spend_microusd: 200,
            },
        )])
    };
    let first_payload = serde_json::to_string(&legacy_usage(20)).unwrap();
    let replacement_payload = serde_json::to_string(&legacy_usage(21)).unwrap();
    let fingerprint = runtime_gateway_redis_legacy_usage_fingerprint(&first_payload);
    let mut conn = runtime_gateway_redis_connection(&url).unwrap();
    let _: () = conn.set(&usage_key, &first_payload).unwrap();

    let started: i32 = redis::cmd("EVAL")
        .arg(RUNTIME_GATEWAY_REDIS_LEGACY_USAGE_BEGIN_SCRIPT)
        .arg(3)
        .arg(&usage_key)
        .arg(&marker_key)
        .arg(&in_progress_key)
        .arg(RUNTIME_GATEWAY_REDIS_LEGACY_USAGE_MIGRATION_MARKER_VALUE)
        .arg(&first_payload)
        .arg(&fingerprint)
        .query(&mut conn)
        .unwrap();
    assert_eq!(started, 1);
    let _: i32 = redis::cmd("EVAL")
        .arg(RUNTIME_GATEWAY_REDIS_LEGACY_USAGE_MIGRATE_SCRIPT)
        .arg(3)
        .arg(&index_key)
        .arg(&hash_key)
        .arg(&migrated_keys_key)
        .arg("team-a")
        .arg(100)
        .arg(5)
        .arg(50)
        .arg(20)
        .arg(200)
        .query(&mut conn)
        .unwrap();
    let _: () = conn.set(&usage_key, &replacement_payload).unwrap();

    let err = redis::cmd("EVAL")
        .arg(RUNTIME_GATEWAY_REDIS_LEGACY_USAGE_FINALIZE_SCRIPT)
        .arg(3)
        .arg(&usage_key)
        .arg(&marker_key)
        .arg(&in_progress_key)
        .arg(RUNTIME_GATEWAY_REDIS_LEGACY_USAGE_MIGRATION_MARKER_VALUE)
        .arg(&first_payload)
        .arg(&fingerprint)
        .query::<i32>(&mut conn)
        .unwrap_err();
    assert!(
        err.to_string().contains("changed during migration"),
        "{err:?}"
    );
    assert_eq!(
        conn.get::<_, String>(&usage_key).unwrap(),
        replacement_payload
    );
    assert!(!conn.exists::<_, bool>(&marker_key).unwrap());
    let hash_before_retry: BTreeMap<String, String> = conn.hgetall(&hash_key).unwrap();

    let err = runtime_gateway_redis_usage_load(&url, &usage_key).unwrap_err();
    assert!(
        err.to_string()
            .contains("changed while migration was in progress"),
        "{err:?}"
    );
    assert_eq!(
        conn.get::<_, String>(&usage_key).unwrap(),
        replacement_payload
    );
    assert!(!conn.exists::<_, bool>(&marker_key).unwrap());
    assert_eq!(
        conn.get::<_, String>(&in_progress_key).unwrap(),
        fingerprint
    );
    assert!(
        conn.sismember::<_, _, bool>(&migrated_keys_key, "team-a")
            .unwrap()
    );
    assert_eq!(
        conn.hgetall::<_, BTreeMap<String, String>>(&hash_key)
            .unwrap(),
        hash_before_retry
    );

    assert!(runtime_gateway_redis_usage_load(&url, &usage_key).is_err());
    assert_eq!(
        conn.get::<_, String>(&usage_key).unwrap(),
        replacement_payload
    );
    assert!(!conn.exists::<_, bool>(&marker_key).unwrap());
    assert_eq!(
        conn.hgetall::<_, BTreeMap<String, String>>(&hash_key)
            .unwrap(),
        hash_before_retry
    );

    let keys = [
        usage_key,
        marker_key,
        in_progress_key,
        migrated_keys_key,
        index_key,
        hash_key,
    ];
    let _: usize = redis::cmd("DEL").arg(&keys).query(&mut conn).unwrap();
}
