#![cfg(test)]

use super::{
    BudgetLimit, BudgetSnapshot, CallId, IdempotencyKey, RUNTIME_GATEWAY_RESERVATION_TTL_MS,
    RequestId, ReservationRequest, RuntimeGatewayDurableReservationError, RuntimeGatewayStateStore,
    RuntimeGatewayVirtualKeyStoreFile, TenantId, UsageAmount, calculate_cost_microusd,
    estimate_request_input_tokens, runtime_gateway_conversation_namespace,
    runtime_gateway_sqlite_open, runtime_gateway_sqlite_reserve_usage,
    runtime_gateway_virtual_key_store_load_strict,
};
use crate::runtime_launch::proxy_startup::local_rewrite_gateway_backend_connection::runtime_gateway_sqlite_create_current_schema_for_tests;
use crate::runtime_launch::proxy_startup::local_rewrite_gateway_store_types::{
    RuntimeGatewayScimUser, runtime_gateway_virtual_key_store_version,
};
use std::sync::{Arc, Barrier};

#[test]
fn conversation_namespace_is_stable_opaque_and_tenant_bound() {
    let tenant_a = TenantId::new();
    let tenant_b = TenantId::new();
    let namespace = runtime_gateway_conversation_namespace(&tenant_a, "virtual-key", "shared-key");

    assert_eq!(
        namespace,
        runtime_gateway_conversation_namespace(&tenant_a, "virtual-key", "shared-key")
    );
    assert_ne!(
        namespace,
        runtime_gateway_conversation_namespace(&tenant_b, "virtual-key", "shared-key")
    );
    assert_ne!(
        namespace,
        runtime_gateway_conversation_namespace(&tenant_a, "principal", "shared-key")
    );
    assert!(!namespace.contains("shared-key"));
    assert!(!namespace.contains(&tenant_a.to_string()));
}

#[test]
fn key_store_load_failure_is_fail_closed_and_logs_without_path_details() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("prodex-key-store-load-log-redaction-{nonce}"));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("test root should be created");
    let key_store_path = root.join("gateway-virtual-keys.json");
    std::fs::create_dir_all(&key_store_path).expect("key store path directory should exist");
    let log_path = root.join("runtime.log");
    crate::runtime_core_shared::prepare_runtime_proxy_test_log_path(&log_path);
    let state_store = RuntimeGatewayStateStore::File {
        key_store_path: key_store_path.clone(),
        usage_path: root.join("gateway-virtual-key-usage.json"),
        ledger_path: root.join("gateway-billing-ledger.jsonl"),
    };

    assert!(runtime_gateway_virtual_key_store_load_strict(&state_store, &log_path).is_err());
    let runtime_log = std::fs::read_to_string(&log_path).expect("runtime log should exist");
    assert!(runtime_log.contains("gateway_virtual_key_store_load_failed"));
    assert!(runtime_log.contains("error_kind=gateway_key_store_persistence_failed"));
    assert!(!runtime_log.contains("gateway-virtual-keys.json"));
    assert!(!runtime_log.contains(&root.display().to_string()));
    assert!(!runtime_log.contains("Is a directory"));
}

#[test]
fn key_store_load_filters_malformed_scim_rows_from_active_state() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("prodex-key-store-scim-filter-{nonce}"));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("test root should be created");
    let key_store_path = root.join("gateway-virtual-keys.json");
    let log_path = root.join("runtime.log");
    crate::runtime_core_shared::prepare_runtime_proxy_test_log_path(&log_path);
    let valid = RuntimeGatewayScimUser {
        id: prodex_domain::PrincipalId::new().to_string(),
        user_name: "alice@example.com".to_string(),
        external_id: None,
        display_name: None,
        active: true,
        role: Some("admin".to_string()),
        tenant_id: Some(String::new()),
        team_id: None,
        project_id: None,
        user_id: Some(prodex_domain::PrincipalId::new().to_string()),
        group_ids: Vec::new(),
        department_id: None,
        budget_id: None,
        allowed_key_prefixes: vec!["team-a-".to_string()],
        created_at_epoch: 1,
        updated_at_epoch: 2,
    };
    let mut malformed = valid.clone();
    malformed.id = prodex_domain::PrincipalId::new().to_string();
    malformed.user_name = "mallory@example.com".to_string();
    malformed.tenant_id = Some(" tenant-a ".to_string());
    let store = RuntimeGatewayVirtualKeyStoreFile {
        version: runtime_gateway_virtual_key_store_version(),
        keys: Vec::new(),
        scim_users: vec![malformed, valid.clone()],
        ..RuntimeGatewayVirtualKeyStoreFile::default()
    };
    std::fs::write(&key_store_path, serde_json::to_vec_pretty(&store).unwrap()).unwrap();
    let state_store = RuntimeGatewayStateStore::File {
        key_store_path,
        usage_path: root.join("gateway-virtual-key-usage.json"),
        ledger_path: root.join("gateway-billing-ledger.jsonl"),
    };

    let loaded = runtime_gateway_virtual_key_store_load_strict(&state_store, &log_path).unwrap();

    assert_eq!(loaded.scim_users.len(), 1);
    assert_eq!(loaded.scim_users[0].user_name, valid.user_name);
    assert_eq!(loaded.scim_users[0].tenant_id, None);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn admission_cost_estimate_includes_reserved_output_tokens() {
    let body = br#"{"model":"gpt-5.4","input":"hello from prodex","max_output_tokens":17}"#;
    let input_tokens = estimate_request_input_tokens(body);
    let reserved_tokens = runtime_proxy_crate::runtime_gateway_estimated_tokens(body);
    let reserved_output_tokens = reserved_tokens.saturating_sub(input_tokens);
    let cost = prodex_provider_core::ProviderModelCost {
        input_cost_per_million_microusd: Some(1_000_000),
        output_cost_per_million_microusd: Some(2_000_000),
    };

    let estimated_cost =
        calculate_cost_microusd(Some(input_tokens), Some(reserved_output_tokens), cost);

    assert_eq!(input_tokens, 5);
    assert_eq!(reserved_tokens, 22);
    assert_eq!(reserved_output_tokens, 17);
    assert_eq!(estimated_cost, Some(39));
}

#[test]
fn sqlite_durable_reservation_rejects_second_concurrent_claim_on_same_budget_scope() {
    let root = std::env::temp_dir().join(format!(
        "prodex-sqlite-durable-reserve-{}",
        RequestId::new()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("test root should be created");
    let path = root.join("state.sqlite");
    runtime_gateway_sqlite_create_current_schema_for_tests(&path)
        .expect("sqlite schema fixture should be created");

    let tenant_id = TenantId::new();
    let virtual_key_id = prodex_domain::VirtualKeyId::new();
    let tenant_id_text = tenant_id.to_string();
    let conn = runtime_gateway_sqlite_open(&path).expect("sqlite database should open");
    conn.execute(
        "INSERT INTO prodex_tenants (tenant_id, display_name, created_at_unix_ms, updated_at_unix_ms) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![tenant_id_text, "tenant", 1_i64, 1_i64],
    )
    .expect("tenant row should insert");
    drop(conn);

    let make_command = move || {
        let call_id = CallId::new();
        let reservation_id = prodex_domain::ReservationId::new();
        prodex_storage::AtomicReservationCommand {
            storage_key: prodex_storage::TenantStorageKey::virtual_key(tenant_id, virtual_key_id),
            idempotency_key: IdempotencyKey::from_call_reservation(call_id, reservation_id),
            snapshot: BudgetSnapshot::default(),
            limit: BudgetLimit::new(u64::MAX, 42),
            request: ReservationRequest {
                tenant_id,
                call_id,
                reservation_id,
                estimate: UsageAmount::new(22, 42),
            },
            created_at_unix_ms: 1_000,
            ttl_ms: RUNTIME_GATEWAY_RESERVATION_TTL_MS,
        }
    };
    let barrier = Arc::new(Barrier::new(2));
    let run = |barrier: Arc<Barrier>| {
        let path = path.clone();
        std::thread::spawn(move || {
            let command = make_command();
            let plan = prodex_application::plan_application_atomic_reservation(
                prodex_application::ApplicationAtomicReservationRequest {
                    durable_store: prodex_storage::DurableStoreKind::Sqlite,
                    reservation: command.clone(),
                },
            )
            .expect("sqlite reservation plan should build");
            let prodex_application::ApplicationAtomicReservationStoragePlan::Sqlite(storage) =
                plan.storage
            else {
                panic!("expected sqlite storage plan");
            };
            barrier.wait();
            runtime_gateway_sqlite_reserve_usage(&path, &storage, &command)
        })
    };

    let first = run(Arc::clone(&barrier));
    let second = run(barrier);
    let results = [
        first.join().expect("first thread should finish"),
        second.join().expect("second thread should finish"),
    ];
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(
                result,
                Err(RuntimeGatewayDurableReservationError::Rejected(
                    runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::BudgetExceeded
                ))
            ))
            .count(),
        1
    );

    let conn = runtime_gateway_sqlite_open(&path).expect("sqlite database should reopen");
    let reservation_rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM prodex_reservations", [], |row| {
            row.get(0)
        })
        .expect("reservation count should load");
    let reserved_rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM prodex_usage_ledger WHERE event_kind = 'reserved'",
            [],
            |row| row.get(0),
        )
        .expect("reserved ledger count should load");
    let (reserved_tokens, reserved_cost_micros): (i64, i64) = conn
        .query_row(
            "SELECT reserved_tokens, reserved_cost_micros FROM prodex_budget_counters",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("budget counters should load");
    assert_eq!(reservation_rows, 1);
    assert_eq!(reserved_rows, 1);
    assert_eq!(reserved_tokens, 22);
    assert_eq!(reserved_cost_micros, 42);

    let _ = std::fs::remove_dir_all(root);
}
