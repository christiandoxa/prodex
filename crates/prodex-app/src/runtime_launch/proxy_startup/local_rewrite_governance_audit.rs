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

#[derive(Clone, Default)]
pub(super) struct RuntimeGovernanceAuditWriter {
    queue: Arc<Mutex<Option<RuntimeGovernanceAuditQueue>>>,
    reconciliation: Arc<Mutex<VecDeque<AuditEvent>>>,
    reconciliation_overflowed: Arc<AtomicBool>,
    available: Arc<AtomicBool>,
}

#[derive(Clone)]
struct RuntimeGovernanceAuditQueue {
    sender: SyncSender<RuntimeGovernanceAuditWrite>,
    depth: Arc<AtomicUsize>,
}

struct RuntimeGovernanceAuditWrite {
    event: AuditEvent,
    reconcile_on_failure: bool,
    acknowledge: SyncSender<Result<(), GovernanceRepositoryError>>,
}

impl RuntimeGovernanceAuditWriter {
    pub(super) fn spawn(
        &self,
        authority: RuntimeGovernanceAuthority,
        shutdown: Arc<AtomicBool>,
    ) -> Result<thread::JoinHandle<()>, GovernanceRepositoryError> {
        let sqlite = match &authority {
            RuntimeGovernanceAuthority::Sqlite { path, .. } => {
                Some(prodex_storage_sqlite_runtime::GovernanceSqliteRepository::open(path)?)
            }
            RuntimeGovernanceAuthority::Postgres { .. } => None,
        };
        let (sender, receiver) = sync_channel(AUDIT_WRITER_QUEUE_LIMIT);
        let depth = Arc::new(AtomicUsize::new(0));
        *self
            .queue
            .lock()
            .map_err(|_| GovernanceRepositoryError::Database)? =
            Some(RuntimeGovernanceAuditQueue {
                sender,
                depth: Arc::clone(&depth),
            });
        self.available.store(true, Ordering::Release);
        self.reconciliation_overflowed
            .store(false, Ordering::Release);
        record_audit_queue_depth(0);
        let available = Arc::clone(&self.available);
        let reconciliation = Arc::clone(&self.reconciliation);
        let reconciliation_overflowed = Arc::clone(&self.reconciliation_overflowed);
        Ok(thread::spawn(move || {
            runtime_governance_audit_writer(
                authority,
                sqlite,
                receiver,
                depth,
                reconciliation,
                reconciliation_overflowed,
                available,
                shutdown,
            )
        }))
    }

    pub(super) fn is_available(&self) -> bool {
        self.available.load(Ordering::Acquire)
    }

    fn append(&self, event: AuditEvent) -> Result<(), GovernanceRepositoryError> {
        self.append_inner(event, false)
    }

    fn append_reconciling(&self, event: AuditEvent) -> Result<(), GovernanceRepositoryError> {
        self.append_inner(event, true)
    }

    fn append_inner(
        &self,
        event: AuditEvent,
        reconcile_on_failure: bool,
    ) -> Result<(), GovernanceRepositoryError> {
        let started_at = Instant::now();
        let queue = match self.queue.lock() {
            Ok(queue) => queue.clone(),
            Err(_) => {
                self.available.store(false, Ordering::Release);
                if reconcile_on_failure {
                    enqueue_audit_reconciliation(
                        &self.reconciliation,
                        &self.reconciliation_overflowed,
                        &self.available,
                        event,
                    );
                }
                record_runtime_audit_metric(AuditOperation::Emit, AuditResult::Dropped, None);
                record_runtime_persistence_metric(
                    PersistenceOperation::Commit,
                    PersistenceResult::Unavailable,
                );
                return Err(GovernanceRepositoryError::Database);
            }
        };
        let Some(queue) = queue else {
            if reconcile_on_failure {
                enqueue_audit_reconciliation(
                    &self.reconciliation,
                    &self.reconciliation_overflowed,
                    &self.available,
                    event,
                );
            }
            record_runtime_audit_metric(AuditOperation::Emit, AuditResult::Dropped, None);
            record_runtime_persistence_metric(
                PersistenceOperation::Commit,
                PersistenceResult::Unavailable,
            );
            return Err(GovernanceRepositoryError::Unsupported);
        };
        let (acknowledge, response) = sync_channel(1);
        queue.depth.fetch_add(1, Ordering::AcqRel);
        if let Err(error) = queue.sender.try_send(RuntimeGovernanceAuditWrite {
            event,
            reconcile_on_failure,
            acknowledge,
        }) {
            let write = match error {
                TrySendError::Full(write) | TrySendError::Disconnected(write) => write,
            };
            if write.reconcile_on_failure {
                enqueue_audit_reconciliation(
                    &self.reconciliation,
                    &self.reconciliation_overflowed,
                    &self.available,
                    write.event,
                );
            }
            decrement_queue_depth(&queue.depth);
            record_runtime_audit_metric(AuditOperation::Emit, AuditResult::Dropped, None);
            record_runtime_persistence_metric(
                PersistenceOperation::Commit,
                PersistenceResult::Unavailable,
            );
            return Err(GovernanceRepositoryError::Database);
        }
        record_runtime_audit_metric(AuditOperation::Emit, AuditResult::Success, None);
        record_audit_queue_depth(queue.depth.load(Ordering::Acquire));
        let result = match response.recv_timeout(AUDIT_WRITER_ACK_TIMEOUT) {
            Ok(Ok(())) => {
                record_runtime_audit_metric(
                    AuditOperation::Persist,
                    AuditResult::Success,
                    Some(duration_millis(started_at)),
                );
                record_runtime_persistence_metric(
                    PersistenceOperation::Commit,
                    PersistenceResult::Success,
                );
                Ok(())
            }
            Ok(Err(error)) => {
                record_runtime_audit_metric(
                    AuditOperation::Persist,
                    AuditResult::Failure,
                    Some(duration_millis(started_at)),
                );
                record_runtime_persistence_metric(
                    PersistenceOperation::Commit,
                    persistence_result(&error),
                );
                Err(error)
            }
            Err(RecvTimeoutError::Timeout) => {
                record_runtime_audit_metric(
                    AuditOperation::Persist,
                    AuditResult::Failure,
                    Some(duration_millis(started_at)),
                );
                record_runtime_persistence_metric(
                    PersistenceOperation::Commit,
                    PersistenceResult::Timeout,
                );
                Err(GovernanceRepositoryError::Database)
            }
            Err(RecvTimeoutError::Disconnected) => {
                record_runtime_audit_metric(
                    AuditOperation::Persist,
                    AuditResult::Failure,
                    Some(duration_millis(started_at)),
                );
                record_runtime_persistence_metric(
                    PersistenceOperation::Commit,
                    PersistenceResult::Unavailable,
                );
                Err(GovernanceRepositoryError::Database)
            }
        };
        result
    }
}

fn enqueue_audit_reconciliation(
    reconciliation: &Mutex<VecDeque<AuditEvent>>,
    overflowed: &AtomicBool,
    available: &AtomicBool,
    event: AuditEvent,
) {
    match reconciliation.lock() {
        Ok(mut pending) if pending.len() < AUDIT_WRITER_QUEUE_LIMIT => {
            pending.push_back(event);
            available.store(false, Ordering::Release);
        }
        Ok(_) => {
            overflowed.store(true, Ordering::Release);
            available.store(false, Ordering::Release);
        }
        Err(_) => {
            overflowed.store(true, Ordering::Release);
            available.store(false, Ordering::Release);
        }
    }
}

fn refresh_audit_reconciliation_availability(
    reconciliation: &Mutex<VecDeque<AuditEvent>>,
    overflowed: &AtomicBool,
    available: &AtomicBool,
) {
    match reconciliation.lock() {
        Ok(pending) => {
            let recovered = !overflowed.load(Ordering::Acquire) && pending.is_empty();
            available.store(recovered, Ordering::Release);
        }
        Err(_) => {
            overflowed.store(true, Ordering::Release);
            available.store(false, Ordering::Release);
        }
    }
}

fn duration_millis(started_at: Instant) -> u64 {
    started_at
        .elapsed()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn persistence_result(error: &GovernanceRepositoryError) -> PersistenceResult {
    match error {
        GovernanceRepositoryError::Conflict
        | GovernanceRepositoryError::AuditChainConflict
        | GovernanceRepositoryError::EtagMismatch
        | GovernanceRepositoryError::StaleVersion => PersistenceResult::Conflict,
        GovernanceRepositoryError::Database | GovernanceRepositoryError::Unsupported => {
            PersistenceResult::Unavailable
        }
        _ => PersistenceResult::Failed,
    }
}

fn decrement_queue_depth(depth: &AtomicUsize) -> usize {
    depth
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
            Some(value.saturating_sub(1))
        })
        .unwrap_or_default()
        .saturating_sub(1)
}

fn record_audit_queue_depth(depth: usize) {
    record_runtime_queue_depth_metric(
        QueueDepthKind::Persistence,
        depth.try_into().unwrap_or(u64::MAX),
        AUDIT_WRITER_QUEUE_LIMIT.try_into().unwrap_or(u64::MAX),
    );
}

pub(super) fn persist_runtime_control_plane_audit_event(
    shared: &RuntimeLocalRewriteProxyShared,
    event: AuditEvent,
) -> Result<(), GovernanceRepositoryError> {
    shared.governance_audit_writer.append(event)
}

pub(super) fn persist_runtime_governance_decision_audit(
    shared: &RuntimeLocalRewriteProxyShared,
    tenant: TenantContext,
    principal: &Principal,
    action: &str,
    outcome: AuditOutcome,
    reason_code: &str,
    decision_context: &str,
) -> Result<(), GovernanceRepositoryError> {
    let action = AuditAction::try_new(format!("gateway.governance.{action}"))
        .map_err(|_| GovernanceRepositoryError::InvalidInput)?;
    if reason_code.is_empty()
        || reason_code.len() > 128
        || !reason_code.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':' | b'/')
        })
    {
        return Err(GovernanceRepositoryError::InvalidInput);
    }
    let resource_id = AuditResourceId::new(decision_context)
        .map_err(|_| GovernanceRepositoryError::InvalidInput)?;
    let occurred_at_unix_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .map_err(|_| GovernanceRepositoryError::InvalidInput)?;
    let event = AuditEvent::new(
        occurred_at_unix_ms,
        tenant,
        principal,
        action,
        AuditResource::new_with_resource_id(
            "gateway_governance_decision",
            Some(resource_id),
            Some(tenant.tenant_id),
        )
        .map_err(|_| GovernanceRepositoryError::InvalidInput)?,
        outcome,
        Some(reason_code.to_string()),
    );

    shared.governance_audit_writer.append(event)
}

pub(super) fn persist_runtime_material_governance_audit(
    shared: &RuntimeLocalRewriteProxyShared,
    context: &RuntimeGovernanceAuditContext,
    request_id: u64,
    action: &str,
    outcome: AuditOutcome,
    reason_code: &str,
) -> Result<(), GovernanceRepositoryError> {
    persist_runtime_material_governance_audit_with_writer(
        &shared.governance_audit_writer,
        shared
            .runtime_shared
            .runtime_config
            .governance
            .mandatory_audit,
        context,
        request_id,
        action,
        outcome,
        reason_code,
        false,
    )
}

pub(super) fn persist_runtime_material_governance_audit_reconciling(
    shared: &RuntimeLocalRewriteProxyShared,
    context: &RuntimeGovernanceAuditContext,
    request_id: u64,
    action: &str,
    outcome: AuditOutcome,
    reason_code: &str,
) -> Result<(), GovernanceRepositoryError> {
    persist_runtime_material_governance_audit_with_writer(
        &shared.governance_audit_writer,
        shared
            .runtime_shared
            .runtime_config
            .governance
            .mandatory_audit,
        context,
        request_id,
        action,
        outcome,
        reason_code,
        true,
    )
}

pub(super) fn runtime_governance_audit_is_durable(shared: &RuntimeLocalRewriteProxyShared) -> bool {
    shared.governance_authority.is_some()
}

pub(super) fn runtime_governance_audit_is_available(
    shared: &RuntimeLocalRewriteProxyShared,
) -> bool {
    !shared
        .runtime_shared
        .runtime_config
        .governance
        .mandatory_audit
        || (runtime_governance_audit_is_durable(shared)
            && shared.governance_audit_writer.is_available())
}

fn persist_runtime_material_governance_audit_with_writer(
    writer: &RuntimeGovernanceAuditWriter,
    mandatory: bool,
    context: &RuntimeGovernanceAuditContext,
    request_id: u64,
    action: &str,
    outcome: AuditOutcome,
    reason_code: &str,
    reconcile_on_failure: bool,
) -> Result<(), GovernanceRepositoryError> {
    let action = AuditAction::try_new(format!("gateway.governance.{action}"))
        .map_err(|_| GovernanceRepositoryError::InvalidInput)?;
    if reason_code.is_empty()
        || reason_code.len() > 128
        || !reason_code.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':' | b'/')
        })
    {
        return Err(GovernanceRepositoryError::InvalidInput);
    }
    let resource_id = AuditResourceId::new(format!("request:{request_id}"))
        .map_err(|_| GovernanceRepositoryError::InvalidInput)?;
    let occurred_at_unix_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .map_err(|_| GovernanceRepositoryError::InvalidInput)?;
    let event = AuditEvent::new(
        occurred_at_unix_ms,
        context.tenant,
        &context.principal,
        action,
        AuditResource::new_with_resource_id(
            "gateway_material_event",
            Some(resource_id),
            Some(context.tenant.tenant_id),
        )
        .map_err(|_| GovernanceRepositoryError::InvalidInput)?,
        outcome,
        Some(reason_code.to_string()),
    );
    let result = if reconcile_on_failure {
        writer.append_reconciling(event)
    } else {
        writer.append(event)
    };
    if mandatory { result } else { Ok(()) }
}

fn runtime_governance_audit_writer(
    authority: RuntimeGovernanceAuthority,
    sqlite: Option<prodex_storage_sqlite_runtime::GovernanceSqliteRepository>,
    receiver: Receiver<RuntimeGovernanceAuditWrite>,
    depth: Arc<AtomicUsize>,
    reconciliation: Arc<Mutex<VecDeque<AuditEvent>>>,
    reconciliation_overflowed: Arc<AtomicBool>,
    available: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
) {
    let mut reconciliation_attempted_at = Instant::now();
    loop {
        match receiver.recv_timeout(Duration::from_millis(100)) {
            Ok(write) => {
                record_audit_queue_depth(decrement_queue_depth(&depth));
                let event = write.event;
                let result = append_durable_audit(&authority, sqlite.as_ref(), event.clone());
                if result.is_err() && write.reconcile_on_failure {
                    enqueue_audit_reconciliation(
                        &reconciliation,
                        &reconciliation_overflowed,
                        &available,
                        event,
                    );
                }
                let _ = write.acknowledge.send(result);
            }
            Err(RecvTimeoutError::Timeout) if shutdown.load(Ordering::SeqCst) => break,
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        if reconciliation_attempted_at.elapsed() >= AUDIT_RECONCILIATION_RETRY_INTERVAL {
            reconcile_audit_failure(
                &authority,
                sqlite.as_ref(),
                &reconciliation,
                &reconciliation_overflowed,
                &available,
            );
            reconciliation_attempted_at = Instant::now();
        }
    }
}

fn reconcile_audit_failure(
    authority: &RuntimeGovernanceAuthority,
    sqlite: Option<&prodex_storage_sqlite_runtime::GovernanceSqliteRepository>,
    reconciliation: &Mutex<VecDeque<AuditEvent>>,
    overflowed: &AtomicBool,
    available: &AtomicBool,
) {
    let event = match reconciliation.lock() {
        Ok(mut pending) => pending.pop_front(),
        Err(_) => {
            overflowed.store(true, Ordering::Release);
            available.store(false, Ordering::Release);
            return;
        }
    };
    let Some(event) = event else {
        return;
    };
    if append_durable_audit(authority, sqlite, event.clone()).is_err() {
        match reconciliation.lock() {
            Ok(mut pending) => pending.push_front(event),
            Err(_) => overflowed.store(true, Ordering::Release),
        }
        available.store(false, Ordering::Release);
        return;
    }
    refresh_audit_reconciliation_availability(reconciliation, overflowed, available);
}

fn append_durable_audit(
    authority: &RuntimeGovernanceAuthority,
    sqlite: Option<&prodex_storage_sqlite_runtime::GovernanceSqliteRepository>,
    event: AuditEvent,
) -> Result<(), GovernanceRepositoryError> {
    for _ in 0..AUDIT_CHAIN_RETRIES {
        let previous_digest = match authority {
            RuntimeGovernanceAuthority::Sqlite { .. } => sqlite
                .ok_or(GovernanceRepositoryError::Database)?
                .latest_audit_digest(event.tenant_id)?,
            RuntimeGovernanceAuthority::Postgres {
                repository,
                runtime,
                ..
            } => runtime.block_on(repository.governance_latest_audit_digest(event.tenant_id))?,
        };
        let command = audit_command(event.clone(), previous_digest);
        let result = match authority {
            RuntimeGovernanceAuthority::Sqlite { .. } => sqlite
                .ok_or(GovernanceRepositoryError::Database)?
                .append_audit_outbox(command),
            RuntimeGovernanceAuthority::Postgres {
                repository,
                runtime,
                ..
            } => runtime.block_on(repository.governance_append_audit_outbox(command)),
        };
        match result {
            Ok(()) => return Ok(()),
            Err(GovernanceRepositoryError::AuditChainConflict) => continue,
            Err(error) => return Err(error),
        }
    }
    Err(GovernanceRepositoryError::AuditChainConflict)
}

fn audit_command(
    event: AuditEvent,
    previous_digest: Option<prodex_domain::AuditDigest>,
) -> AuditOutboxWriteCommand {
    let event_digest = compute_audit_chain_digest(previous_digest.as_ref(), &event);
    AuditOutboxWriteCommand {
        outbox_event_id: AuditEventId::new(),
        audit: AppendOnlyAuditCommand {
            storage_key: TenantStorageKey::tenant(event.tenant_id),
            event,
            previous_digest,
            event_digest,
        },
    }
}

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
            &RuntimeGovernanceAuditWriter::default(),
            true,
            &audit_context(),
            7,
            "request_transform",
            AuditOutcome::Success,
            "sensitive_fields_masked",
            false,
        )
        .unwrap_err();

        assert_eq!(error, GovernanceRepositoryError::Unsupported);
    }

    #[test]
    fn mandatory_response_precommit_audit_writer_failure_is_fail_closed() {
        let error = persist_runtime_material_governance_audit_with_writer(
            &RuntimeGovernanceAuditWriter::default(),
            true,
            &audit_context(),
            8,
            "response_precommit_block",
            AuditOutcome::Denied,
            "blocked_output_keyword",
            false,
        )
        .unwrap_err();

        assert_eq!(error, GovernanceRepositoryError::Unsupported);
    }

    #[test]
    fn observe_mode_attempts_but_does_not_fail_closed_on_writer_failure() {
        persist_runtime_material_governance_audit_with_writer(
            &RuntimeGovernanceAuditWriter::default(),
            false,
            &audit_context(),
            9,
            "request_transform",
            AuditOutcome::Success,
            "sensitive_fields_masked",
            false,
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
}
