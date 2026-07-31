use super::{
    AUDIT_CHAIN_RETRIES, AUDIT_RECONCILIATION_RETRY_INTERVAL, AUDIT_WRITER_ACK_TIMEOUT,
    AUDIT_WRITER_QUEUE_LIMIT, AppendOnlyAuditCommand, Arc, AtomicBool, AtomicUsize, AuditAction,
    AuditEvent, AuditEventId, AuditOperation, AuditOutboxWriteCommand, AuditOutcome, AuditResource,
    AuditResourceId, AuditResult, Duration, GovernanceRepositoryError, Instant, Mutex, Ordering,
    PersistenceOperation, PersistenceResult, Principal, QueueDepthKind, Receiver, RecvTimeoutError,
    RuntimeGovernanceAuditContext, RuntimeGovernanceAuthority, RuntimeLocalRewriteProxyShared,
    SyncSender, TenantContext, TenantStorageKey, TrySendError, VecDeque,
    compute_audit_chain_digest, record_runtime_audit_metric, record_runtime_persistence_metric,
    record_runtime_queue_depth_metric, sync_channel, thread,
};

#[derive(Clone, Default)]
pub(in crate::runtime_launch::proxy_startup) struct RuntimeGovernanceAuditWriter {
    pub(super) queue: Arc<Mutex<Option<RuntimeGovernanceAuditQueue>>>,
    pub(super) reconciliation: Arc<Mutex<VecDeque<AuditEvent>>>,
    pub(super) reconciliation_overflowed: Arc<AtomicBool>,
    pub(super) available: Arc<AtomicBool>,
}

#[derive(Clone)]
pub(super) struct RuntimeGovernanceAuditQueue {
    pub(super) sender: SyncSender<RuntimeGovernanceAuditWrite>,
    pub(super) depth: Arc<AtomicUsize>,
}

pub(super) struct RuntimeGovernanceAuditWrite {
    event: AuditEvent,
    reconcile_on_failure: bool,
    acknowledge: SyncSender<Result<(), GovernanceRepositoryError>>,
}

pub(super) struct RuntimeGovernanceAuditPersistContext<'a> {
    pub(super) writer: &'a RuntimeGovernanceAuditWriter,
    pub(super) mandatory: bool,
    pub(super) audit_context: &'a RuntimeGovernanceAuditContext,
    pub(super) reconcile_on_failure: bool,
}

impl RuntimeGovernanceAuditWriter {
    pub(in crate::runtime_launch::proxy_startup) fn spawn(
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
        let recovery = (
            Arc::clone(&self.reconciliation),
            Arc::clone(&self.reconciliation_overflowed),
            Arc::clone(&self.available),
        );
        Ok(thread::spawn(move || {
            runtime_governance_audit_writer(authority, sqlite, receiver, depth, recovery, shutdown)
        }))
    }

    pub(in crate::runtime_launch::proxy_startup) fn is_available(&self) -> bool {
        self.available.load(Ordering::Acquire)
    }

    fn append(&self, event: AuditEvent) -> Result<(), GovernanceRepositoryError> {
        self.append_inner(event, false)
    }

    pub(super) fn append_reconciling(
        &self,
        event: AuditEvent,
    ) -> Result<(), GovernanceRepositoryError> {
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
                self.reconciliation_overflowed
                    .store(true, Ordering::Release);
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
            let (write, disconnected) = match error {
                TrySendError::Full(write) => (write, false),
                TrySendError::Disconnected(write) => (write, true),
            };
            if disconnected {
                self.reconciliation_overflowed
                    .store(true, Ordering::Release);
                self.available.store(false, Ordering::Release);
            }
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
        match response.recv_timeout(AUDIT_WRITER_ACK_TIMEOUT) {
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
                self.reconciliation_overflowed
                    .store(true, Ordering::Release);
                self.available.store(false, Ordering::Release);
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
        }
    }
}

pub(super) fn enqueue_audit_reconciliation(
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

pub(super) fn refresh_audit_reconciliation_availability(
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

pub(in crate::runtime_launch::proxy_startup) fn persist_runtime_control_plane_audit_event(
    shared: &RuntimeLocalRewriteProxyShared,
    event: AuditEvent,
) -> Result<(), GovernanceRepositoryError> {
    shared.governance_audit_writer.append(event)
}

pub(in crate::runtime_launch::proxy_startup) fn persist_runtime_governance_decision_audit(
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

pub(in crate::runtime_launch::proxy_startup) fn persist_runtime_material_governance_audit(
    shared: &RuntimeLocalRewriteProxyShared,
    context: &RuntimeGovernanceAuditContext,
    request_id: u64,
    action: &str,
    outcome: AuditOutcome,
    reason_code: &str,
) -> Result<(), GovernanceRepositoryError> {
    persist_runtime_material_governance_audit_with_writer(
        RuntimeGovernanceAuditPersistContext {
            writer: &shared.governance_audit_writer,
            mandatory: shared
                .runtime_shared
                .runtime_config
                .governance
                .mandatory_audit,
            audit_context: context,
            reconcile_on_failure: false,
        },
        request_id,
        action,
        outcome,
        reason_code,
    )
}

pub(in crate::runtime_launch::proxy_startup) fn persist_runtime_material_governance_audit_reconciling(
    shared: &RuntimeLocalRewriteProxyShared,
    context: &RuntimeGovernanceAuditContext,
    request_id: u64,
    action: &str,
    outcome: AuditOutcome,
    reason_code: &str,
) -> Result<(), GovernanceRepositoryError> {
    persist_runtime_material_governance_audit_with_writer(
        RuntimeGovernanceAuditPersistContext {
            writer: &shared.governance_audit_writer,
            mandatory: shared
                .runtime_shared
                .runtime_config
                .governance
                .mandatory_audit,
            audit_context: context,
            reconcile_on_failure: true,
        },
        request_id,
        action,
        outcome,
        reason_code,
    )
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_governance_audit_is_durable(
    shared: &RuntimeLocalRewriteProxyShared,
) -> bool {
    shared.governance_authority.is_some()
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_governance_audit_is_available(
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

pub(super) fn persist_runtime_material_governance_audit_with_writer(
    context: RuntimeGovernanceAuditPersistContext<'_>,
    request_id: u64,
    action: &str,
    outcome: AuditOutcome,
    reason_code: &str,
) -> Result<(), GovernanceRepositoryError> {
    let RuntimeGovernanceAuditPersistContext {
        writer,
        mandatory,
        audit_context,
        reconcile_on_failure,
    } = context;
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
        audit_context.tenant,
        &audit_context.principal,
        action,
        AuditResource::new_with_resource_id(
            "gateway_material_event",
            Some(resource_id),
            Some(audit_context.tenant.tenant_id),
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
    recovery: (
        Arc<Mutex<VecDeque<AuditEvent>>>,
        Arc<AtomicBool>,
        Arc<AtomicBool>,
    ),
    shutdown: Arc<AtomicBool>,
) {
    let (reconciliation, reconciliation_overflowed, available) = recovery;
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
