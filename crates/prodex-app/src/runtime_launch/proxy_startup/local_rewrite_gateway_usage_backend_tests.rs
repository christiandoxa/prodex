use super::super::local_rewrite_gateway_backend_connection::runtime_gateway_sqlite_create_current_schema_for_tests;
use super::super::local_rewrite_gateway_sqlite_utils::runtime_gateway_sqlite_u64_to_i64;
use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

#[path = "local_rewrite_gateway_usage_backend_redis_migration_race_tests.rs"]
mod redis_migration_race_tests;
#[path = "local_rewrite_gateway_usage_backend_redis_tests.rs"]
mod redis_tests;

fn temp_dir(name: &str) -> std::path::PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("prodex-gateway-usage-{name}-{stamp}"))
}

#[test]
fn sqlite_usage_load_reads_usage_rows() {
    let root = temp_dir("sqlite");
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("state.sqlite");
    runtime_gateway_sqlite_create_current_schema_for_tests(&path).unwrap();
    let conn = runtime_gateway_sqlite_open(&path).unwrap();
    conn.execute(
        r#"
            INSERT INTO prodex_gateway_virtual_key_usage (
                key_name, minute_epoch, requests_this_minute, tokens_this_minute,
                requests_total, spend_microusd
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        rusqlite::params![
            "alpha",
            runtime_gateway_sqlite_u64_to_i64(10),
            runtime_gateway_sqlite_u64_to_i64(1),
            runtime_gateway_sqlite_u64_to_i64(20),
            runtime_gateway_sqlite_u64_to_i64(2),
            runtime_gateway_sqlite_u64_to_i64(300),
        ],
    )
    .unwrap();

    let usage = runtime_gateway_sqlite_usage_load(&path).unwrap();
    assert_eq!(usage["alpha"].minute_epoch, 10);
    assert_eq!(usage["alpha"].spend_microusd, 300);

    drop(conn);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn usage_delta_debug_output_redacts_accounting_fields() {
    let delta = RuntimeGatewayVirtualKeyUsageDelta {
        request_id: 42,
        typed_request_id: "prodex-request-delta-secret".to_string(),
        call_id: "prodex-call-delta-secret".to_string(),
        key_name: "sk-delta-secret".to_string(),
        tenant_id: Some("tenant-delta-secret".to_string()),
        team_id: Some("team-delta-secret".to_string()),
        project_id: Some("project-delta-secret".to_string()),
        user_id: Some("user-delta-secret".to_string()),
        budget_id: Some("budget-delta-secret".to_string()),
        model: "gpt-delta-secret".to_string(),
        minute_epoch: 1_700_000_000,
        input_tokens: 123,
        reserved_tokens: 456,
        estimated_cost_microusd: Some(789),
        created_at_epoch: 1_700_000_001,
    };
    let rendered = format!("{delta:?}");

    assert!(rendered.contains("RuntimeGatewayVirtualKeyUsageDelta"));
    assert!(rendered.contains("<redacted>"));
    for raw in [
        "prodex-request-delta-secret",
        "prodex-call-delta-secret",
        "sk-delta-secret",
        "tenant-delta-secret",
        "team-delta-secret",
        "project-delta-secret",
        "user-delta-secret",
        "budget-delta-secret",
        "gpt-delta-secret",
        "1700000000",
        "123",
        "456",
        "789",
    ] {
        assert!(!rendered.contains(raw), "{rendered}");
    }
}

#[test]
fn postgres_usage_upsert_increments_counters_atomically() {
    assert!(RUNTIME_GATEWAY_POSTGRES_USAGE_UPSERT_SQL.contains("ON CONFLICT(key_name)"));
    assert!(RUNTIME_GATEWAY_POSTGRES_USAGE_UPSERT_SQL.contains("requests_total + 1"));
    assert!(
        RUNTIME_GATEWAY_POSTGRES_USAGE_UPSERT_SQL
            .contains("spend_microusd + EXCLUDED.spend_microusd")
    );
    assert!(!RUNTIME_GATEWAY_POSTGRES_USAGE_UPSERT_SQL.contains("FOR UPDATE"));
    let source = include_str!("local_rewrite_gateway_usage_backend.rs");
    let postgres_apply = source
        .split("pub(super) fn runtime_gateway_postgres_usage_apply_deltas")
        .nth(1)
        .unwrap()
        .split("const RUNTIME_GATEWAY_POSTGRES_USAGE_UPSERT_SQL")
        .next()
        .unwrap();
    assert!(postgres_apply.contains("delta.reserved_tokens"));
    assert!(!postgres_apply.contains("delta.input_tokens"));
}

#[test]
fn postgres_ledger_insert_conflict_target_uses_call_id() {
    for column in [
        "typed_request_id",
        "tenant_id",
        "team_id",
        "project_id",
        "user_id",
        "budget_id",
        "reserved_tokens",
    ] {
        assert!(RUNTIME_GATEWAY_POSTGRES_LEDGER_INSERT_SQL.contains(column));
    }
    assert!(
        RUNTIME_GATEWAY_POSTGRES_LEDGER_INSERT_SQL
            .contains("ON CONFLICT(call_id, key_name, phase) DO NOTHING")
    );
    assert!(
        !RUNTIME_GATEWAY_POSTGRES_LEDGER_INSERT_SQL
            .contains("ON CONFLICT(request_id, key_name, phase)")
    );
    assert!(
        include_str!("local_rewrite_gateway_usage_backend.rs")
            .contains("runtime_gateway_sqlite_optional_u64_to_i64(ledger.reserved_tokens)")
    );
}

#[test]
fn sqlite_usage_load_rejects_negative_usage_rows() {
    let root = temp_dir("sqlite-negative");
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("state.sqlite");
    runtime_gateway_sqlite_create_current_schema_for_tests(&path).unwrap();
    let conn = runtime_gateway_sqlite_open(&path).unwrap();
    conn.execute(
        r#"
            INSERT INTO prodex_gateway_virtual_key_usage (
                key_name, minute_epoch, requests_this_minute, tokens_this_minute,
                requests_total, spend_microusd
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        rusqlite::params!["alpha", 10_i64, 1_i64, 20_i64, -1_i64, 300_i64],
    )
    .unwrap();

    assert!(runtime_gateway_sqlite_usage_load(&path).is_err());

    drop(conn);
    std::fs::remove_dir_all(root).unwrap();
}
