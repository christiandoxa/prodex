//! Durable, content-free data-plane governance audit writes.

use prodex_domain::{
    AuditAction, AuditEvent, AuditEventId, AuditOutcome, AuditResource, AuditResourceId, Principal,
    TenantContext, compute_audit_chain_digest,
};
use prodex_observability::{
    AuditOperation, AuditResult, PersistenceOperation, PersistenceResult, QueueDepthKind,
};
use prodex_storage::{
    AppendOnlyAuditCommand, AuditOutboxWriteCommand, GovernanceRepositoryError, TenantStorageKey,
};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use super::local_rewrite::{RuntimeGovernanceAuthority, RuntimeLocalRewriteProxyShared};
use crate::runtime_operational_metrics::{
    record_runtime_audit_metric, record_runtime_persistence_metric,
    record_runtime_queue_depth_metric,
};

const AUDIT_CHAIN_RETRIES: usize = 3;
const AUDIT_WRITER_QUEUE_LIMIT: usize = 128;
const AUDIT_WRITER_ACK_TIMEOUT: Duration = Duration::from_secs(5);
const AUDIT_RECONCILIATION_RETRY_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone)]
pub(super) struct RuntimeGovernanceAuditContext {
    pub(super) tenant: TenantContext,
    pub(super) principal: Principal,
}

impl RuntimeGovernanceAuditContext {
    pub(super) fn new(tenant: TenantContext, principal: Principal) -> Self {
        Self { tenant, principal }
    }

    pub(super) fn from_authorized(
        authorized: &prodex_application::ApplicationAuthorizedRequestContext<'_>,
    ) -> Option<Self> {
        Some(Self::new(
            authorized.tenant_context()?,
            authorized.principal()?.clone(),
        ))
    }
}

#[path = "local_rewrite_governance_audit/writer.rs"]
mod writer;
#[cfg(test)]
use writer::{
    RuntimeGovernanceAuditPersistContext, RuntimeGovernanceAuditQueue,
    enqueue_audit_reconciliation, persist_runtime_material_governance_audit_with_writer,
    refresh_audit_reconciliation_availability,
};
pub(super) use writer::{
    RuntimeGovernanceAuditWriter, persist_runtime_control_plane_audit_event,
    persist_runtime_governance_decision_audit, persist_runtime_material_governance_audit,
    persist_runtime_material_governance_audit_reconciling, runtime_governance_audit_is_available,
    runtime_governance_audit_is_durable,
};

#[cfg(test)]
mod tests {
    use super::*;
    use prodex_domain::{CredentialScope, PrincipalId, PrincipalKind, Role, TenantId};

    fn audit_context() -> RuntimeGovernanceAuditContext {
        let tenant_id = TenantId::from_uuid(uuid::Uuid::from_u128(1));
        RuntimeGovernanceAuditContext::new(
            TenantContext { tenant_id },
            Principal::new(
                PrincipalId::from_uuid(uuid::Uuid::from_u128(2)),
                Some(tenant_id),
                PrincipalKind::VirtualKey,
                Role::Operator,
                CredentialScope::DataPlane,
            ),
        )
    }

    fn audit_event(request_id: u64) -> AuditEvent {
        let context = audit_context();
        AuditEvent::new(
            1_000 + request_id,
            context.tenant,
            &context.principal,
            AuditAction::try_new("gateway.governance.test").unwrap(),
            AuditResource::new(
                "gateway_test",
                Some(format!("request:{request_id}")),
                Some(context.tenant.tenant_id),
            ),
            AuditOutcome::Denied,
            Some("test"),
        )
    }

    #[test]
    fn decision_context_is_bounded_explainable_and_content_free() {
        let context = "p:018f77e0-7b5d-7000-8000-000000000001:r:7:s:9:c:restricted:v:openai:q:018f77e0-7b5d-7000-8000-000000000002:i:full:e:allow";
        assert!(AuditResourceId::new(context).is_ok());
        assert!(context.contains(":c:restricted:"));
        assert!(context.contains(":v:openai:"));
        assert!(!context.contains("prompt"));
        assert!(context.len() <= 256);
    }

    #[test]
    fn mandatory_transform_audit_writer_failure_is_fail_closed_and_content_free() {
        let error = persist_runtime_material_governance_audit_with_writer(
            RuntimeGovernanceAuditPersistContext {
                writer: &RuntimeGovernanceAuditWriter::default(),
                mandatory: true,
                audit_context: &audit_context(),
                reconcile_on_failure: false,
            },
            7,
            "request_transform",
            AuditOutcome::Success,
            "sensitive_fields_masked",
        )
        .unwrap_err();

        assert_eq!(error, GovernanceRepositoryError::Unsupported);
    }

    #[test]
    fn mandatory_response_precommit_audit_writer_failure_is_fail_closed() {
        let error = persist_runtime_material_governance_audit_with_writer(
            RuntimeGovernanceAuditPersistContext {
                writer: &RuntimeGovernanceAuditWriter::default(),
                mandatory: true,
                audit_context: &audit_context(),
                reconcile_on_failure: false,
            },
            8,
            "response_precommit_block",
            AuditOutcome::Denied,
            "blocked_output_keyword",
        )
        .unwrap_err();

        assert_eq!(error, GovernanceRepositoryError::Unsupported);
    }

    #[test]
    fn observe_mode_attempts_but_does_not_fail_closed_on_writer_failure() {
        persist_runtime_material_governance_audit_with_writer(
            RuntimeGovernanceAuditPersistContext {
                writer: &RuntimeGovernanceAuditWriter::default(),
                mandatory: false,
                audit_context: &audit_context(),
                reconcile_on_failure: false,
            },
            9,
            "request_transform",
            AuditOutcome::Success,
            "sensitive_fields_masked",
        )
        .unwrap();
    }

    #[test]
    fn reconciliation_queue_overflow_never_reports_recovery() {
        let reconciliation = Mutex::new(VecDeque::new());
        let overflowed = AtomicBool::new(false);
        let available = AtomicBool::new(true);

        for request_id in 0..=AUDIT_WRITER_QUEUE_LIMIT {
            enqueue_audit_reconciliation(
                &reconciliation,
                &overflowed,
                &available,
                audit_event(request_id as u64),
            );
        }

        assert_eq!(
            reconciliation.lock().unwrap().len(),
            AUDIT_WRITER_QUEUE_LIMIT
        );
        assert!(overflowed.load(Ordering::Acquire));
        assert!(!available.load(Ordering::Acquire));

        reconciliation.lock().unwrap().clear();
        refresh_audit_reconciliation_availability(&reconciliation, &overflowed, &available);
        assert!(!available.load(Ordering::Acquire));
    }

    #[test]
    fn poisoned_writer_queue_never_reports_recovery() {
        let writer = RuntimeGovernanceAuditWriter::default();
        writer.available.store(true, Ordering::Release);
        let _ = std::panic::catch_unwind(|| {
            let _queue = writer.queue.lock().unwrap();
            panic!("poison audit writer queue");
        });

        assert_eq!(
            writer.append_reconciling(audit_event(1)).unwrap_err(),
            GovernanceRepositoryError::Database
        );
        assert_eq!(writer.reconciliation.lock().unwrap().len(), 1);
        assert!(writer.reconciliation_overflowed.load(Ordering::Acquire));
        assert!(!writer.is_available());

        writer.reconciliation.lock().unwrap().clear();
        refresh_audit_reconciliation_availability(
            &writer.reconciliation,
            &writer.reconciliation_overflowed,
            &writer.available,
        );
        assert!(!writer.is_available());
    }

    #[test]
    fn disconnected_writer_queue_never_reports_recovery() {
        let writer = RuntimeGovernanceAuditWriter::default();
        let (sender, receiver) = sync_channel(AUDIT_WRITER_QUEUE_LIMIT);
        drop(receiver);
        *writer.queue.lock().unwrap() = Some(RuntimeGovernanceAuditQueue {
            sender,
            depth: Arc::new(AtomicUsize::new(0)),
        });
        writer.available.store(true, Ordering::Release);

        assert_eq!(
            writer.append_reconciling(audit_event(1)).unwrap_err(),
            GovernanceRepositoryError::Database
        );
        assert_eq!(writer.reconciliation.lock().unwrap().len(), 1);
        assert!(writer.reconciliation_overflowed.load(Ordering::Acquire));
        assert!(!writer.is_available());
    }
}
