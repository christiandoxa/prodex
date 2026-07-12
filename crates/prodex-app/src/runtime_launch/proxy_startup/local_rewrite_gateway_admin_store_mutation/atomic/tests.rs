use super::*;
use crate::runtime_launch::proxy_startup::local_rewrite_gateway_backend_connection::runtime_gateway_sqlite_create_current_schema_for_tests;
use crate::runtime_launch::proxy_startup::local_rewrite_gateway_store_types::RuntimeGatewayStoredVirtualKey;
use prodex_domain::{
    AuditAction, AuditDigest, AuditOutcome, AuditResource, CredentialScope, IdempotencyKey,
    Principal, PrincipalId, PrincipalKind, Role, TenantContext, TenantId,
};

fn atomic_write(
    tenant_id: TenantId,
    key: &str,
    fingerprint: &str,
) -> RuntimeGatewayAdminAtomicWrite {
    let operation =
        IdempotentOperation::new(tenant_id, IdempotencyKey::new(key).unwrap(), fingerprint)
            .unwrap();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::User,
        Role::Admin,
        CredentialScope::ControlPlane,
    );
    let event = prodex_domain::AuditEvent::new(
        10,
        TenantContext { tenant_id },
        &principal,
        AuditAction::new("virtual_key.create"),
        AuditResource::new("virtual_key", Some("key-a"), Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    RuntimeGatewayAdminAtomicWrite {
        operation,
        started_at_unix_ms: 9,
        completed_at_unix_ms: 10,
        audit_event: event,
    }
}

fn stored_key(tenant_id: TenantId) -> RuntimeGatewayStoredVirtualKey {
    RuntimeGatewayStoredVirtualKey {
        name: "key-a".to_string(),
        token_hash_base64: "hash-only".to_string(),
        virtual_key_id: Some(prodex_domain::VirtualKeyId::new().to_string()),
        tenant_id: Some(tenant_id.to_string()),
        team_id: None,
        project_id: None,
        user_id: None,
        budget_id: None,
        allowed_models: Vec::new(),
        budget_microusd: None,
        request_budget: None,
        rpm_limit: None,
        tpm_limit: None,
        disabled: Some(false),
        created_at_epoch: 1,
        updated_at_epoch: 1,
    }
}

fn sqlite_audit_envelope(
    tx: &rusqlite::Transaction<'_>,
    event: &prodex_domain::AuditEvent,
) -> AuditEnvelope {
    match runtime_gateway_sqlite_audit_envelope(tx, event) {
        Ok(envelope) => envelope,
        Err(_) => panic!("SQLite audit head should resolve"),
    }
}

#[test]
fn file_history_is_bounded_and_never_contains_response_material() {
    let tenant_id = TenantId::new();
    let write = atomic_write(tenant_id, "request-1", "fingerprint-1");
    let record = RuntimeGatewayAdminIdempotencyRecord {
        entry: IdempotencyEntry::completed(IdempotencyRecord {
            operation: write.operation.clone(),
            response: (),
        }),
        completed_at_unix_ms: write.completed_at_unix_ms,
    };
    let digest = compute_audit_chain_digest(None, &write.audit_event);
    let envelope = AuditEnvelope::new(write.audit_event, None, digest);
    let mut store = RuntimeGatewayVirtualKeyStoreFile {
        admin_idempotency: vec![record; 4_100],
        admin_audit: vec![envelope; 4_100],
        ..RuntimeGatewayVirtualKeyStoreFile::default()
    };

    store.bound_admin_history();

    assert_eq!(store.admin_idempotency.len(), 4_096);
    assert_eq!(store.admin_audit.len(), 4_096);
    let payload = serde_json::to_string(&store).unwrap();
    assert!(!payload.contains("raw-token-must-never-persist"));
}

#[test]
fn file_duplicate_keys_are_rejected_and_unique_writes_chain_under_lock() {
    let tenant_id = TenantId::new();
    let write = atomic_write(tenant_id, "request-1", "fingerprint-1");
    let mut store = RuntimeGatewayVirtualKeyStoreFile::default();
    runtime_gateway_record_file_atomic_write(&mut store, write);
    let duplicate = atomic_write(tenant_id, "request-1", "fingerprint-1");
    assert!(runtime_gateway_file_idempotency_absent(&store, &duplicate.operation).is_err());

    let second = atomic_write(tenant_id, "request-2", "fingerprint-2");
    runtime_gateway_record_file_atomic_write(&mut store, second);
    assert_eq!(store.admin_audit.len(), 2);
    assert_eq!(
        store.admin_audit[1].previous_digest.as_ref(),
        Some(&store.admin_audit[0].event_digest)
    );
    assert_eq!(
        store.admin_audit[1].event_digest,
        compute_audit_chain_digest(
            Some(&store.admin_audit[0].event_digest),
            &store.admin_audit[1].event
        )
    );
}

#[test]
fn sqlite_compatibility_write_idempotency_and_audit_share_one_transaction() {
    let root = std::env::temp_dir().join(format!(
        "prodex-admin-atomic-sqlite-{}",
        prodex_domain::RequestId::new()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("state.sqlite");
    runtime_gateway_sqlite_create_current_schema_for_tests(&path).unwrap();
    let tenant_id = TenantId::new();
    let store = RuntimeGatewayVirtualKeyStoreFile {
        version: runtime_gateway_virtual_key_store_version(),
        keys: vec![stored_key(tenant_id)],
        ..RuntimeGatewayVirtualKeyStoreFile::default()
    };

    {
        let mut conn = runtime_gateway_sqlite_open(&path).unwrap();
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        runtime_gateway_sqlite_save_key_store_in_tx(&tx, &store).unwrap();
        let write = atomic_write(tenant_id, "request-1", "fingerprint-1");
        let audit = sqlite_audit_envelope(&tx, &write.audit_event);
        runtime_gateway_sqlite_write_metadata(&tx, &write, &audit).unwrap();
    }
    let conn = runtime_gateway_sqlite_open(&path).unwrap();
    for table in [
        "prodex_gateway_virtual_keys",
        "prodex_idempotency_records",
        "prodex_audit_log",
    ] {
        let count: i64 = conn
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 0, "rollback must cover {table}");
    }
    drop(conn);

    let mut conn = runtime_gateway_sqlite_open(&path).unwrap();
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    runtime_gateway_sqlite_save_key_store_in_tx(&tx, &store).unwrap();
    let write = atomic_write(tenant_id, "request-1", "fingerprint-1");
    let audit = sqlite_audit_envelope(&tx, &write.audit_event);
    runtime_gateway_sqlite_write_metadata(&tx, &write, &audit).unwrap();
    tx.commit().unwrap();
    let marker: (String, Option<Vec<u8>>) = conn
        .query_row(
            "SELECT entry_status, response_body FROM prodex_idempotency_records",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(marker, ("completed".to_string(), None));
    let audit_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM prodex_audit_log", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(audit_count, 1);

    let first_digest: String = conn
        .query_row("SELECT event_digest FROM prodex_audit_log", [], |row| {
            row.get(0)
        })
        .unwrap();
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    let mut second = atomic_write(tenant_id, "request-2", "fingerprint-2");
    second.audit_event.occurred_at_unix_ms = 5;
    let second_audit = sqlite_audit_envelope(&tx, &second.audit_event);
    assert_eq!(
        second_audit
            .previous_digest
            .as_ref()
            .map(AuditDigest::as_str),
        Some(first_digest.as_str())
    );
    runtime_gateway_sqlite_save_key_store_in_tx(&tx, &store).unwrap();
    runtime_gateway_sqlite_write_metadata(&tx, &second, &second_audit).unwrap();
    tx.commit().unwrap();
    let audit_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM prodex_audit_log", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(audit_count, 2);

    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    let third = atomic_write(tenant_id, "request-3", "fingerprint-3");
    let third_audit = sqlite_audit_envelope(&tx, &third.audit_event);
    assert_eq!(
        third_audit.previous_digest.as_ref(),
        Some(&second_audit.event_digest)
    );
    drop(tx);

    drop(conn);
    std::fs::remove_dir_all(root).unwrap();
}
