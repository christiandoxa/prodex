use super::*;
use prodex_domain::AuditEventId;
use prodex_storage::{
    decode_governance_audit_retention_purge_idempotency_response,
    encode_governance_audit_retention_purge_idempotency_response,
};

#[test]
fn audit_retention_purge_plan_uses_tenant_scoped_batch() {
    let tenant_id = TenantId::new();
    let command = retention_purge_command(tenant_id);
    let expected_event_ids = command.batch.event_ids().collect::<Vec<_>>();

    let plan = plan_audit_retention_purge(command).unwrap();

    assert_eq!(plan.storage_key.tenant_id, tenant_id);
    assert_eq!(plan.batch.tenant_id, tenant_id);
    assert_eq!(
        plan.batch.event_ids().collect::<Vec<_>>(),
        expected_event_ids
    );
}

#[test]
fn audit_retention_purge_idempotency_response_round_trips() {
    let event_ids = (0..AuditRetentionBatchLimit::MAX)
        .map(|_| AuditEventId::new())
        .collect::<Vec<_>>();
    let encoded = encode_governance_audit_retention_purge_idempotency_response(&event_ids);

    assert_eq!(
        decode_governance_audit_retention_purge_idempotency_response(&encoded).unwrap(),
        event_ids
    );
    assert!(decode_governance_audit_retention_purge_idempotency_response(b"invalid").is_err());
}
