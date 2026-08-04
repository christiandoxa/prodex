use super::*;

#[test]
fn retention_purge_can_replace_the_entire_chain_with_an_anchor() {
    let tenant_id = TenantId::new();
    let database = TestDatabase::new(&[tenant_id]);
    let repository = database.repository();
    let admin = principal(tenant_id);
    let mut audit = AuditCursor::default();
    let fixture = audit.next(tenant_id, &admin, "governance.audit.fixture");
    let fixture_id = fixture.audit.event.id;
    repository.append_audit_outbox(fixture).unwrap();

    assert_eq!(
        repository
            .purge_audit_events(
                tenant_id,
                &[fixture_id],
                10_000,
                5_000,
                audit.next(tenant_id, &admin, "governance.audit_retention.purge"),
            )
            .unwrap(),
        vec![fixture_id]
    );
    let records = repository.governance_export_audit(tenant_id, 10).unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].action, "governance.audit_retention.purge");
    assert!(
        repository
            .audit_integrity_health(tenant_id)
            .unwrap()
            .chain_valid
    );
}

#[test]
fn audit_export_preserves_reason_detail() {
    let tenant_id = TenantId::new();
    let database = TestDatabase::new(&[tenant_id]);
    let repository = database.repository();
    let admin = principal(tenant_id);
    let mut audit = AuditCursor::default();
    let mut command = audit.next(
        tenant_id,
        &admin,
        "control_plane.provider_credential.rotate",
    );
    command.audit.event.reason_detail = Some(
        prodex_domain::AuditReasonDetail::new("incident\u{2003}response api_key=fixture-secret")
            .unwrap(),
    );
    command.audit.event_digest = prodex_domain::compute_audit_chain_digest(
        command.audit.previous_digest.as_ref(),
        &command.audit.event,
    );
    repository.append_audit_outbox(command).unwrap();

    let records = repository.governance_export_audit(tenant_id, 10).unwrap();
    assert_eq!(
        records[0].reason_detail.as_deref(),
        Some("incident response api_key=<redacted>")
    );
}

#[test]
fn legal_hold_persists_and_blocks_retention_purge_atomically() {
    let tenant_id = TenantId::new();
    let database = TestDatabase::new(&[tenant_id]);
    let repository = database.repository();
    let admin = principal(tenant_id);
    let mut audit = AuditCursor::default();

    let protected = audit.next(tenant_id, &admin, "governance.audit.fixture");
    let protected_id = protected.audit.event.id;
    repository.append_audit_outbox(protected).unwrap();
    let purgeable = audit.next(tenant_id, &admin, "governance.audit.fixture");
    let purgeable_id = purgeable.audit.event.id;
    repository.append_audit_outbox(purgeable).unwrap();

    let hold = AuditRetentionHold::new(
        TenantContext { tenant_id },
        protected_id,
        AuditReasonCode::new("legal.investigation").unwrap(),
        None,
    );
    repository
        .upsert_audit_legal_hold(
            &hold,
            admin.id,
            2_000,
            audit.next(tenant_id, &admin, "governance.audit_legal_hold.upsert"),
        )
        .unwrap();
    assert_eq!(
        repository.list_audit_legal_holds(tenant_id).unwrap(),
        vec![hold]
    );

    let purged = repository
        .purge_audit_events(
            tenant_id,
            &[protected_id, purgeable_id],
            10_000,
            5_000,
            audit.next(tenant_id, &admin, "governance.audit_retention.purge"),
        )
        .unwrap();
    assert!(purged.is_empty());

    let connection = Connection::open(database.path()).unwrap();
    let protected_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM prodex_audit_log
             WHERE tenant_id = ?1 AND audit_event_id = ?2",
            rusqlite::params![tenant_id.to_string(), protected_id.to_string()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(protected_count, 1);
}

#[test]
fn retention_purge_requires_a_contiguous_chain_prefix() {
    let tenant_id = TenantId::new();
    let database = TestDatabase::new(&[tenant_id]);
    let repository = database.repository();
    let admin = principal(tenant_id);
    let mut audit = AuditCursor::default();
    let first = audit.next(tenant_id, &admin, "governance.audit.fixture");
    let first_id = first.audit.event.id;
    repository.append_audit_outbox(first).unwrap();
    let second = audit.next(tenant_id, &admin, "governance.audit.fixture");
    let second_id = second.audit.event.id;
    repository.append_audit_outbox(second).unwrap();

    let skipped = repository
        .purge_audit_events(
            tenant_id,
            &[second_id],
            10_000,
            5_000,
            audit.next(tenant_id, &admin, "governance.audit_retention.purge"),
        )
        .unwrap();
    assert!(skipped.is_empty());

    let operation = IdempotentOperation::new(
        tenant_id,
        IdempotencyKey::new("retention-purge-prefix").unwrap(),
        "sha256:retention-purge-prefix",
    )
    .unwrap();
    let purged = repository
        .purge_audit_events_idempotent(
            tenant_id,
            &[first_id, second_id],
            10_001,
            5_000,
            audit.next(tenant_id, &admin, "governance.audit_retention.purge"),
            ApprovalVoteIdempotency {
                operation: operation.clone(),
                started_at_unix_ms: 10_000,
            },
        )
        .unwrap();
    assert_eq!(purged, vec![first_id, second_id]);
    assert_eq!(
        repository
            .purge_audit_events_idempotent(
                tenant_id,
                &[first_id, second_id],
                10_001,
                5_000,
                audit.next(tenant_id, &admin, "governance.audit_retention.purge"),
                ApprovalVoteIdempotency {
                    operation,
                    started_at_unix_ms: 10_000,
                },
            )
            .unwrap(),
        purged
    );
    assert!(
        repository
            .audit_integrity_health(tenant_id)
            .unwrap()
            .chain_valid
    );
}
