use super::super::super::provider_bridge::RuntimeProviderGatewaySpendEvent;
use super::*;

#[test]
fn redis_usage_hash_helpers_round_trip_usage_fields() {
    let mut fields = BTreeMap::new();
    fields.insert("minute_epoch".to_string(), "42".to_string());
    fields.insert("requests_this_minute".to_string(), "3".to_string());
    fields.insert("tokens_this_minute".to_string(), "144".to_string());
    fields.insert("requests_total".to_string(), "9".to_string());
    fields.insert("spend_microusd".to_string(), "1700".to_string());

    let usage = runtime_gateway_redis_usage_from_hash(&fields).unwrap();

    assert_eq!(usage.minute_epoch, 42);
    assert_eq!(usage.requests_this_minute, 3);
    assert_eq!(usage.tokens_this_minute, 144);
    assert_eq!(usage.requests_total, 9);
    assert_eq!(usage.spend_microusd, 1700);
}

#[test]
fn redis_usage_hash_rejects_malformed_counter_fields() {
    for (field, value, message) in [
        (
            "requests_total",
            "not-a-number",
            "gateway redis usage field requests_total must be an unsigned integer",
        ),
        (
            "spend_microusd",
            " 1700 ",
            "gateway redis usage field spend_microusd must not contain whitespace",
        ),
    ] {
        let mut fields = BTreeMap::new();
        fields.insert("minute_epoch".to_string(), "42".to_string());
        fields.insert("requests_this_minute".to_string(), "3".to_string());
        fields.insert("tokens_this_minute".to_string(), "144".to_string());
        fields.insert("requests_total".to_string(), "9".to_string());
        fields.insert("spend_microusd".to_string(), "1700".to_string());
        fields.insert(field.to_string(), value.to_string());

        let err = runtime_gateway_redis_usage_from_hash(&fields).unwrap_err();

        assert!(err.to_string().contains(message), "{err:?}");
    }
}

#[test]
fn redis_usage_hash_keys_are_per_virtual_key() {
    assert_eq!(
        runtime_gateway_redis_usage_index_key("prodex:gateway:virtual_key_usage"),
        "prodex:gateway:virtual_key_usage:keys"
    );
    assert_eq!(
        runtime_gateway_redis_usage_hash_key("prodex:gateway:virtual_key_usage", "team-a"),
        "prodex:gateway:virtual_key_usage:key:team-a"
    );
    assert_eq!(
        runtime_gateway_redis_usage_migration_marker_key("prodex:gateway:virtual_key_usage"),
        "prodex:gateway:virtual_key_usage:legacy_usage_migrated_v1"
    );
    assert_eq!(
        runtime_gateway_redis_usage_migrated_keys_key("prodex:gateway:virtual_key_usage"),
        "prodex:gateway:virtual_key_usage:legacy_usage_migrated_keys_v1"
    );
    assert_eq!(
        runtime_gateway_redis_usage_migration_in_progress_key("prodex:gateway:virtual_key_usage"),
        "prodex:gateway:virtual_key_usage:legacy_usage_migration_v1"
    );
}

#[test]
fn redis_usage_backend_does_not_write_whole_usage_json_blob() {
    let source = include_str!("local_rewrite_gateway_usage_backend.rs");
    let set_blob = ["conn.set", "(redis_key"].join("");
    let whole_usage_json = ["serde_json::to_string", "(usage"].join("");

    assert!(!source.contains(&set_blob));
    assert!(!source.contains(&whole_usage_json));
    assert!(RUNTIME_GATEWAY_REDIS_USAGE_APPLY_SCRIPT.contains("EXISTS"));
    assert!(RUNTIME_GATEWAY_REDIS_USAGE_APPLY_SCRIPT.contains("RPUSH"));
    assert!(RUNTIME_GATEWAY_REDIS_USAGE_APPLY_SCRIPT.contains("HSET"));
    assert!(RUNTIME_GATEWAY_REDIS_USAGE_APPLY_SCRIPT.contains("overflow"));
    assert!(RUNTIME_GATEWAY_REDIS_USAGE_APPLY_SCRIPT.contains("SISMEMBER', KEYS[6]"));
    assert!(RUNTIME_GATEWAY_REDIS_USAGE_APPLY_SCRIPT.contains("SADD', KEYS[6]"));
    assert!(!RUNTIME_GATEWAY_REDIS_USAGE_APPLY_SCRIPT.contains("LPOS"));
    assert!(RUNTIME_GATEWAY_REDIS_LEGACY_USAGE_MIGRATE_SCRIPT.contains("HSET"));
    assert!(RUNTIME_GATEWAY_REDIS_LEGACY_USAGE_MIGRATE_SCRIPT.contains("SISMEMBER', KEYS[3]"));
    assert!(RUNTIME_GATEWAY_REDIS_LEGACY_USAGE_MIGRATE_SCRIPT.contains("SADD', KEYS[3]"));
    assert!(RUNTIME_GATEWAY_REDIS_LEGACY_USAGE_MIGRATE_SCRIPT.contains("add_decimal"));
}

#[test]
#[ignore = "requires PRODEX_TEST_REDIS_URL"]
fn redis_usage_and_ledger_commit_atomically_under_concurrent_retry() {
    use super::super::super::local_rewrite_gateway_redis_ledger::{
        runtime_gateway_redis_ledger_id_index_key,
        runtime_gateway_redis_ledger_id_index_marker_key, runtime_gateway_redis_ledger_load,
    };

    let url = std::env::var("PRODEX_TEST_REDIS_URL")
        .expect("PRODEX_TEST_REDIS_URL must point to the test Redis instance");
    let suffix = prodex_domain::RequestId::new();
    let usage_key = format!("prodex:test:gateway:usage:{suffix}");
    let ledger_key = format!("prodex:test:gateway:ledger:{suffix}");
    let delta = RuntimeGatewayVirtualKeyUsageDelta {
        request_id: 42,
        typed_request_id: format!("prodex-{}", prodex_domain::RequestId::new()),
        call_id: format!("prodex-{}", prodex_domain::CallId::new()),
        key_name: "team-a".to_string(),
        tenant_id: None,
        team_id: None,
        project_id: None,
        user_id: None,
        budget_id: None,
        model: "gpt-5.4".to_string(),
        minute_epoch: 100,
        input_tokens: 3,
        reserved_tokens: 13,
        estimated_cost_microusd: Some(17),
        created_at_epoch: 1_700_000_001,
    };
    let legacy_usage = BTreeMap::from([(
        "team-a".to_string(),
        runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage {
            minute_epoch: 100,
            requests_this_minute: 5,
            tokens_this_minute: 50,
            requests_total: 20,
            spend_microusd: 200,
        },
    )]);
    let mut conn = runtime_gateway_redis_connection(&url).unwrap();
    let _: () = conn
        .set(&usage_key, serde_json::to_string(&legacy_usage).unwrap())
        .unwrap();
    let workers = (0..8)
        .map(|_| {
            let url = url.clone();
            let usage_key = usage_key.clone();
            let ledger_key = ledger_key.clone();
            let delta = delta.clone();
            std::thread::spawn(move || {
                runtime_gateway_redis_usage_apply_deltas(&url, &usage_key, &ledger_key, &[delta])
            })
        })
        .collect::<Vec<_>>();
    for worker in workers {
        worker.join().unwrap().unwrap();
    }

    let usage = runtime_gateway_redis_usage_load(&url, &usage_key).unwrap();
    assert_eq!(usage["team-a"].requests_total, 21);
    assert_eq!(usage["team-a"].tokens_this_minute, 63);
    assert_eq!(usage["team-a"].spend_microusd, 217);
    assert!(!conn.exists::<_, bool>(&usage_key).unwrap());
    assert_eq!(
        conn.get::<_, String>(runtime_gateway_redis_usage_migration_marker_key(&usage_key))
            .unwrap(),
        RUNTIME_GATEWAY_REDIS_LEGACY_USAGE_MIGRATION_MARKER_VALUE
    );
    let ledger = runtime_gateway_redis_ledger_load(&url, &ledger_key, usize::MAX).unwrap();
    assert_eq!(ledger.len(), 1);
    assert_eq!(ledger[0].reserved_tokens, Some(13));

    let entry_id = runtime_gateway_redis_ledger_entry_id(&ledger[0]);
    let entry_key = runtime_gateway_redis_ledger_entry_key(&ledger_key, &entry_id);
    let call_index_key = runtime_gateway_redis_ledger_call_index_key(&ledger_key, &delta.call_id);
    let id_index_key = runtime_gateway_redis_ledger_id_index_key(&ledger_key);
    assert!(
        conn.sismember::<_, _, bool>(&id_index_key, &entry_id)
            .unwrap()
    );
    let _: usize = redis::cmd("DEL")
        .arg(&[entry_key.as_str(), call_index_key.as_str()])
        .query(&mut conn)
        .unwrap();
    assert!(runtime_gateway_redis_ledger_load(&url, &ledger_key, usize::MAX).is_err());
    assert!(
        runtime_gateway_redis_usage_apply_deltas(
            &url,
            &usage_key,
            &ledger_key,
            std::slice::from_ref(&delta),
        )
        .is_err()
    );
    let keys = [
        usage_key.clone(),
        runtime_gateway_redis_usage_migration_marker_key(&usage_key),
        runtime_gateway_redis_usage_migrated_keys_key(&usage_key),
        runtime_gateway_redis_usage_index_key(&usage_key),
        runtime_gateway_redis_usage_hash_key(&usage_key, "team-a"),
        runtime_gateway_redis_ledger_index_key(&ledger_key),
        runtime_gateway_redis_ledger_id_index_key(&ledger_key),
        runtime_gateway_redis_ledger_id_index_marker_key(&ledger_key),
        call_index_key,
        entry_key,
    ];
    let _: usize = redis::cmd("DEL").arg(&keys).query(&mut conn).unwrap();
}

#[test]
#[ignore = "requires PRODEX_TEST_REDIS_URL"]
fn redis_legacy_usage_merges_into_newer_hash_once() {
    let url = std::env::var("PRODEX_TEST_REDIS_URL")
        .expect("PRODEX_TEST_REDIS_URL must point to the test Redis instance");
    let suffix = prodex_domain::RequestId::new();
    let usage_key = format!("prodex:test:gateway:mixed-usage:{suffix}");
    let index_key = runtime_gateway_redis_usage_index_key(&usage_key);
    let newer_hash_key = runtime_gateway_redis_usage_hash_key(&usage_key, "newer-minute");
    let same_hash_key = runtime_gateway_redis_usage_hash_key(&usage_key, "same-minute");
    let migrated_keys_key = runtime_gateway_redis_usage_migrated_keys_key(&usage_key);
    let marker_key = runtime_gateway_redis_usage_migration_marker_key(&usage_key);
    let legacy_usage = BTreeMap::from([
        (
            "newer-minute".to_string(),
            runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage {
                minute_epoch: 100,
                requests_this_minute: 5,
                tokens_this_minute: 50,
                requests_total: 20,
                spend_microusd: 200,
            },
        ),
        (
            "same-minute".to_string(),
            runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage {
                minute_epoch: 100,
                requests_this_minute: 6,
                tokens_this_minute: 60,
                requests_total: 30,
                spend_microusd: 300,
            },
        ),
    ]);
    let mut conn = runtime_gateway_redis_connection(&url).unwrap();
    let _: () = conn
        .set(&usage_key, serde_json::to_string(&legacy_usage).unwrap())
        .unwrap();
    for (key_name, hash_key, minute) in [
        ("newer-minute", &newer_hash_key, 101_u64),
        ("same-minute", &same_hash_key, 100_u64),
    ] {
        let _: usize = conn.sadd(&index_key, key_name).unwrap();
        let _: usize = redis::cmd("HSET")
            .arg(hash_key)
            .arg("minute_epoch")
            .arg(minute)
            .arg("requests_this_minute")
            .arg(2)
            .arg("tokens_this_minute")
            .arg(30)
            .arg("requests_total")
            .arg(4)
            .arg("spend_microusd")
            .arg(40)
            .query(&mut conn)
            .unwrap();
    }

    let usage = runtime_gateway_redis_usage_load(&url, &usage_key).unwrap();
    assert_eq!(usage["newer-minute"].minute_epoch, 101);
    assert_eq!(usage["newer-minute"].requests_this_minute, 2);
    assert_eq!(usage["newer-minute"].tokens_this_minute, 30);
    assert_eq!(usage["newer-minute"].requests_total, 24);
    assert_eq!(usage["newer-minute"].spend_microusd, 240);
    assert_eq!(usage["same-minute"].minute_epoch, 100);
    assert_eq!(usage["same-minute"].requests_this_minute, 8);
    assert_eq!(usage["same-minute"].tokens_this_minute, 90);
    assert_eq!(usage["same-minute"].requests_total, 34);
    assert_eq!(usage["same-minute"].spend_microusd, 340);
    assert!(!conn.exists::<_, bool>(&usage_key).unwrap());
    assert!(
        conn.sismember::<_, _, bool>(&migrated_keys_key, "newer-minute")
            .unwrap()
    );
    assert!(
        conn.sismember::<_, _, bool>(&migrated_keys_key, "same-minute")
            .unwrap()
    );

    let retry_result: i32 = redis::cmd("EVAL")
        .arg(RUNTIME_GATEWAY_REDIS_LEGACY_USAGE_MIGRATE_SCRIPT)
        .arg(3)
        .arg(&index_key)
        .arg(&newer_hash_key)
        .arg(&migrated_keys_key)
        .arg("newer-minute")
        .arg(100)
        .arg(5)
        .arg(50)
        .arg(20)
        .arg(200)
        .query(&mut conn)
        .unwrap();
    assert_eq!(retry_result, 0);
    let usage = runtime_gateway_redis_usage_load(&url, &usage_key).unwrap();
    assert_eq!(usage["newer-minute"].requests_total, 24);
    assert_eq!(usage["newer-minute"].spend_microusd, 240);

    let keys = [
        usage_key,
        index_key,
        newer_hash_key,
        same_hash_key,
        migrated_keys_key,
        marker_key,
    ];
    let _: usize = redis::cmd("DEL").arg(&keys).query(&mut conn).unwrap();
}

#[test]
#[ignore = "requires PRODEX_TEST_REDIS_URL"]
fn redis_legacy_usage_blob_survives_failed_migration() {
    let url = std::env::var("PRODEX_TEST_REDIS_URL")
        .expect("PRODEX_TEST_REDIS_URL must point to the test Redis instance");
    let suffix = prodex_domain::RequestId::new();
    let usage_key = format!("prodex:test:gateway:failed-usage-migration:{suffix}");
    let index_key = runtime_gateway_redis_usage_index_key(&usage_key);
    let hash_key = runtime_gateway_redis_usage_hash_key(&usage_key, "team-a");
    let marker_key = runtime_gateway_redis_usage_migration_marker_key(&usage_key);
    let legacy_usage = BTreeMap::from([(
        "team-a".to_string(),
        runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage {
            minute_epoch: 100,
            requests_this_minute: 5,
            tokens_this_minute: 50,
            requests_total: 20,
            spend_microusd: 200,
        },
    )]);
    let payload = serde_json::to_string(&legacy_usage).unwrap();
    let mut conn = runtime_gateway_redis_connection(&url).unwrap();
    let _: () = conn.set(&usage_key, &payload).unwrap();
    let _: usize = conn.sadd(&index_key, "team-a").unwrap();

    assert!(runtime_gateway_redis_usage_load(&url, &usage_key).is_err());
    assert_eq!(conn.get::<_, String>(&usage_key).unwrap(), payload);
    assert!(!conn.exists::<_, bool>(&marker_key).unwrap());
    assert!(!conn.exists::<_, bool>(&hash_key).unwrap());

    let _: () = conn
        .set(
            &marker_key,
            RUNTIME_GATEWAY_REDIS_LEGACY_USAGE_MIGRATION_MARKER_VALUE,
        )
        .unwrap();
    let err = runtime_gateway_redis_usage_load(&url, &usage_key).unwrap_err();
    assert!(
        err.to_string().contains("retained its legacy blob"),
        "{err:?}"
    );
    assert_eq!(conn.get::<_, String>(&usage_key).unwrap(), payload);

    let keys = [
        usage_key.clone(),
        index_key,
        hash_key,
        marker_key,
        runtime_gateway_redis_usage_migrated_keys_key(&usage_key),
        runtime_gateway_redis_usage_migration_in_progress_key(&usage_key),
    ];
    let _: usize = redis::cmd("DEL").arg(&keys).query(&mut conn).unwrap();
}

#[test]
#[ignore = "requires PRODEX_TEST_REDIS_URL"]
fn redis_existing_ledger_index_backfills_global_ids_once() {
    use super::super::super::local_rewrite_gateway_redis_ledger::{
        runtime_gateway_redis_ledger_id_index_key,
        runtime_gateway_redis_ledger_id_index_marker_key, runtime_gateway_redis_ledger_load,
    };

    let url = std::env::var("PRODEX_TEST_REDIS_URL")
        .expect("PRODEX_TEST_REDIS_URL must point to the test Redis instance");
    let suffix = prodex_domain::RequestId::new();
    let ledger_key = format!("prodex:test:gateway:ledger-index-backfill:{suffix}");
    let delta = RuntimeGatewayVirtualKeyUsageDelta {
        request_id: 42,
        typed_request_id: format!("prodex-{}", prodex_domain::RequestId::new()),
        call_id: format!("prodex-{}", prodex_domain::CallId::new()),
        key_name: "team-a".to_string(),
        tenant_id: None,
        team_id: None,
        project_id: None,
        user_id: None,
        budget_id: None,
        model: "gpt-5.4".to_string(),
        minute_epoch: 100,
        input_tokens: 3,
        reserved_tokens: 13,
        estimated_cost_microusd: Some(17),
        created_at_epoch: 1_700_000_001,
    };
    let entry = runtime_gateway_billing_ledger_entry_from_delta(&delta);
    let entry_id = runtime_gateway_redis_ledger_entry_id(&entry);
    let index_key = runtime_gateway_redis_ledger_index_key(&ledger_key);
    let entry_key = runtime_gateway_redis_ledger_entry_key(&ledger_key, &entry_id);
    let call_index_key = runtime_gateway_redis_ledger_call_index_key(&ledger_key, &delta.call_id);
    let id_index_key = runtime_gateway_redis_ledger_id_index_key(&ledger_key);
    let marker_key = runtime_gateway_redis_ledger_id_index_marker_key(&ledger_key);
    let mut conn = runtime_gateway_redis_connection(&url).unwrap();
    let _: usize = conn.rpush(&index_key, &entry_id).unwrap();
    let _: () = conn
        .set(&entry_key, serde_json::to_string(&entry).unwrap())
        .unwrap();
    let _: usize = conn.sadd(&call_index_key, &entry_id).unwrap();

    let first = runtime_gateway_redis_ledger_load(&url, &ledger_key, usize::MAX).unwrap();
    assert_eq!(first.len(), 1);
    assert!(
        conn.sismember::<_, _, bool>(&id_index_key, &entry_id)
            .unwrap()
    );
    assert_eq!(conn.get::<_, String>(&marker_key).unwrap(), "1");
    assert_eq!(
        runtime_gateway_redis_ledger_load(&url, &ledger_key, usize::MAX)
            .unwrap()
            .len(),
        1
    );

    let _: usize = conn.rpush(&index_key, &entry_id).unwrap();
    assert!(runtime_gateway_redis_ledger_load(&url, &ledger_key, usize::MAX).is_err());

    let keys = [
        ledger_key,
        index_key,
        entry_key,
        call_index_key,
        id_index_key,
        marker_key,
    ];
    let _: usize = redis::cmd("DEL").arg(&keys).query(&mut conn).unwrap();
}

#[test]
#[ignore = "requires PRODEX_TEST_REDIS_URL"]
fn redis_legacy_ledger_stays_visible_and_corruption_fails_closed() {
    use super::super::super::local_rewrite_gateway_redis_ledger::{
        runtime_gateway_redis_ledger_id_index_key,
        runtime_gateway_redis_ledger_id_index_marker_key, runtime_gateway_redis_ledger_load,
        runtime_gateway_redis_ledger_reconcile_response,
    };

    let url = std::env::var("PRODEX_TEST_REDIS_URL")
        .expect("PRODEX_TEST_REDIS_URL must point to the test Redis instance");
    let suffix = prodex_domain::RequestId::new();
    let usage_key = format!("prodex:test:gateway:legacy-usage:{suffix}");
    let ledger_key = format!("prodex:test:gateway:legacy-ledger:{suffix}");
    let legacy_call_id = format!("prodex-{}", prodex_domain::CallId::new());
    let new_call_id = format!("prodex-{}", prodex_domain::CallId::new());
    let legacy_delta = RuntimeGatewayVirtualKeyUsageDelta {
        request_id: 41,
        typed_request_id: format!("prodex-{}", prodex_domain::RequestId::new()),
        call_id: legacy_call_id.clone(),
        key_name: "team-a".to_string(),
        tenant_id: None,
        team_id: None,
        project_id: None,
        user_id: None,
        budget_id: None,
        model: "gpt-5.4".to_string(),
        minute_epoch: 100,
        input_tokens: 3,
        reserved_tokens: 11,
        estimated_cost_microusd: Some(7),
        created_at_epoch: 1_700_000_000,
    };
    let new_delta = RuntimeGatewayVirtualKeyUsageDelta {
        request_id: 42,
        typed_request_id: format!("prodex-{}", prodex_domain::RequestId::new()),
        call_id: new_call_id.clone(),
        reserved_tokens: 13,
        estimated_cost_microusd: Some(17),
        created_at_epoch: 1_700_000_001,
        ..legacy_delta.clone()
    };
    let legacy_entry = runtime_gateway_billing_ledger_entry_from_delta(&legacy_delta);
    let legacy_usage = BTreeMap::from([(
        "team-a".to_string(),
        runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage {
            minute_epoch: 100,
            requests_this_minute: 1,
            tokens_this_minute: 11,
            requests_total: 1,
            spend_microusd: 7,
        },
    )]);
    let mut conn = runtime_gateway_redis_connection(&url).unwrap();
    let _: () = conn
        .set(&usage_key, serde_json::to_string(&legacy_usage).unwrap())
        .unwrap();
    runtime_gateway_redis_usage_apply_deltas(
        &url,
        &usage_key,
        &ledger_key,
        std::slice::from_ref(&new_delta),
    )
    .unwrap();
    assert!(!conn.exists::<_, bool>(&usage_key).unwrap());
    let _: usize = conn
        .rpush(&ledger_key, serde_json::to_string(&legacy_entry).unwrap())
        .unwrap();

    let ledger = runtime_gateway_redis_ledger_load(&url, &ledger_key, usize::MAX).unwrap();
    assert_eq!(
        ledger
            .iter()
            .map(|entry| entry.call_id.as_str())
            .collect::<Vec<_>>(),
        vec![legacy_call_id.as_str(), new_call_id.as_str()]
    );
    assert!(!conn.exists::<_, bool>(&ledger_key).unwrap());
    assert_eq!(
        runtime_gateway_redis_ledger_load(&url, &ledger_key, usize::MAX)
            .unwrap()
            .iter()
            .map(|entry| entry.call_id.as_str())
            .collect::<Vec<_>>(),
        vec![legacy_call_id.as_str(), new_call_id.as_str()]
    );
    assert_eq!(
        runtime_gateway_redis_ledger_load(&url, &ledger_key, 1).unwrap()[0].call_id,
        new_call_id
    );

    runtime_gateway_redis_usage_apply_deltas(
        &url,
        &usage_key,
        &ledger_key,
        std::slice::from_ref(&legacy_delta),
    )
    .unwrap();

    let usage = runtime_gateway_redis_usage_load(&url, &usage_key).unwrap();
    assert_eq!(usage["team-a"].requests_total, 2);
    assert_eq!(usage["team-a"].tokens_this_minute, 24);
    let ledger = runtime_gateway_redis_ledger_load(&url, &ledger_key, usize::MAX).unwrap();
    assert_eq!(ledger.len(), 2);

    let event = RuntimeProviderGatewaySpendEvent {
        event: "gateway_spend",
        phase: "response",
        request: legacy_delta.request_id,
        key_name: Some(legacy_delta.key_name.clone()),
        tenant_id: None,
        request_id: legacy_delta.typed_request_id.clone(),
        legacy_request_sequence: legacy_delta.request_id,
        call_id: legacy_call_id.clone(),
        provider: "openai".to_string(),
        path: "/v1/responses".to_string(),
        model: legacy_delta.model.clone(),
        status: 200,
        elapsed_ms: 1,
        request_bytes: 1,
        response_bytes: Some(2),
        input_tokens: Some(3),
        output_tokens: Some(5),
        cost_usd: Some(0.000_007),
        reconciliation_reason: Some(prodex_domain::ReservationReconciliationReason::Completed),
        sink: "runtime-log".to_string(),
    };
    assert!(
        runtime_gateway_redis_ledger_reconcile_response(
            &url,
            &ledger_key,
            "unused",
            || Ok("unused".to_string()),
            &event,
            1_700_000_002,
        )
        .unwrap()
    );
    let ledger = runtime_gateway_redis_ledger_load(&url, &ledger_key, usize::MAX).unwrap();
    assert_eq!(
        ledger
            .iter()
            .find(|entry| entry.call_id == legacy_call_id)
            .unwrap()
            .response_status,
        Some(200)
    );

    let new_entry = ledger
        .iter()
        .find(|entry| entry.call_id == new_call_id)
        .unwrap();
    let new_entry_id = runtime_gateway_redis_ledger_entry_id(new_entry);
    let new_entry_key = runtime_gateway_redis_ledger_entry_key(&ledger_key, &new_entry_id);
    let original_payload: String = conn.get(&new_entry_key).unwrap();
    let mut tampered_entry = new_entry.clone();
    tampered_entry.key_name = "tampered".to_string();
    let _: () = conn
        .set(
            &new_entry_key,
            serde_json::to_string(&tampered_entry).unwrap(),
        )
        .unwrap();
    assert!(runtime_gateway_redis_ledger_load(&url, &ledger_key, usize::MAX).is_err());
    assert!(
        runtime_gateway_redis_usage_apply_deltas(
            &url,
            &usage_key,
            &ledger_key,
            std::slice::from_ref(&new_delta),
        )
        .is_err()
    );
    let _: () = conn.set(&new_entry_key, original_payload).unwrap();

    let usage_hash_key = runtime_gateway_redis_usage_hash_key(&usage_key, "team-a");
    let usage_index_key = runtime_gateway_redis_usage_index_key(&usage_key);
    let _: usize = redis::cmd("DEL")
        .arg(&usage_hash_key)
        .query(&mut conn)
        .unwrap();
    let _: usize = conn.srem(&usage_index_key, "team-a").unwrap();
    assert!(
        runtime_gateway_redis_usage_apply_deltas(
            &url,
            &usage_key,
            &ledger_key,
            std::slice::from_ref(&new_delta),
        )
        .is_err()
    );
    let _: usize = conn.sadd(&usage_index_key, "team-a").unwrap();
    assert!(runtime_gateway_redis_usage_load(&url, &usage_key).is_err());
    let missing_hash_delta = RuntimeGatewayVirtualKeyUsageDelta {
        request_id: 43,
        typed_request_id: format!("prodex-{}", prodex_domain::RequestId::new()),
        call_id: format!("prodex-{}", prodex_domain::CallId::new()),
        ..new_delta
    };
    assert!(
        runtime_gateway_redis_usage_apply_deltas(
            &url,
            &usage_key,
            &ledger_key,
            std::slice::from_ref(&missing_hash_delta),
        )
        .is_err()
    );
    assert_eq!(
        runtime_gateway_redis_ledger_load(&url, &ledger_key, usize::MAX)
            .unwrap()
            .len(),
        2
    );

    let legacy_entry_id = runtime_gateway_redis_ledger_entry_id(&legacy_entry);
    let missing_hash_entry = runtime_gateway_billing_ledger_entry_from_delta(&missing_hash_delta);
    let keys = [
        usage_key.clone(),
        runtime_gateway_redis_usage_migration_marker_key(&usage_key),
        runtime_gateway_redis_usage_migrated_keys_key(&usage_key),
        runtime_gateway_redis_usage_index_key(&usage_key),
        usage_hash_key,
        ledger_key.clone(),
        runtime_gateway_redis_ledger_index_key(&ledger_key),
        runtime_gateway_redis_ledger_id_index_key(&ledger_key),
        runtime_gateway_redis_ledger_id_index_marker_key(&ledger_key),
        runtime_gateway_redis_ledger_call_index_key(&ledger_key, &legacy_call_id),
        runtime_gateway_redis_ledger_call_index_key(&ledger_key, &new_call_id),
        runtime_gateway_redis_ledger_call_index_key(&ledger_key, &missing_hash_delta.call_id),
        runtime_gateway_redis_ledger_entry_key(&ledger_key, &legacy_entry_id),
        new_entry_key,
        runtime_gateway_redis_ledger_entry_key(
            &ledger_key,
            &runtime_gateway_redis_ledger_entry_id(&missing_hash_entry),
        ),
    ];
    let _: usize = redis::cmd("DEL").arg(&keys).query(&mut conn).unwrap();
}
