//! Durable, content-free data-plane governance audit writes.

use prodex_domain::{
    AuditAction, AuditEvent, AuditEventId, AuditOutcome, AuditResource, AuditResourceId, Principal,
    TenantContext, compute_audit_chain_digest,
};
use prodex_storage::{
    AppendOnlyAuditCommand, AuditOutboxWriteCommand, GovernanceRepositoryError, TenantStorageKey,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use super::local_rewrite::{RuntimeGovernanceAuthority, RuntimeLocalRewriteProxyShared};

const AUDIT_CHAIN_RETRIES: usize = 3;
const AUDIT_WRITER_QUEUE_LIMIT: usize = 128;
const AUDIT_WRITER_ACK_TIMEOUT: Duration = Duration::from_secs(5);

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
pub(super) struct RuntimeGovernanceAuditWriter(
    Arc<Mutex<Option<SyncSender<RuntimeGovernanceAuditWrite>>>>,
);

struct RuntimeGovernanceAuditWrite {
    event: AuditEvent,
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
        *self
            .0
            .lock()
            .map_err(|_| GovernanceRepositoryError::Database)? = Some(sender);
        Ok(thread::spawn(move || {
            runtime_governance_audit_writer(authority, sqlite, receiver, shutdown)
        }))
    }

    fn append(&self, event: AuditEvent) -> Result<(), GovernanceRepositoryError> {
        let sender = self
            .0
            .lock()
            .map_err(|_| GovernanceRepositoryError::Database)?
            .clone()
            .ok_or(GovernanceRepositoryError::Unsupported)?;
        let (acknowledge, response) = sync_channel(1);
        sender
            .try_send(RuntimeGovernanceAuditWrite { event, acknowledge })
            .map_err(|error| match error {
                TrySendError::Full(_) | TrySendError::Disconnected(_) => {
                    GovernanceRepositoryError::Database
                }
            })?;
        response
            .recv_timeout(AUDIT_WRITER_ACK_TIMEOUT)
            .map_err(|_| GovernanceRepositoryError::Database)?
    }
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
    )
}

pub(super) fn runtime_governance_audit_is_durable(shared: &RuntimeLocalRewriteProxyShared) -> bool {
    shared.governance_authority.is_some()
}

fn persist_runtime_material_governance_audit_with_writer(
    writer: &RuntimeGovernanceAuditWriter,
    mandatory: bool,
    context: &RuntimeGovernanceAuditContext,
    request_id: u64,
    action: &str,
    outcome: AuditOutcome,
    reason_code: &str,
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
    let result = writer.append(event);
    if mandatory { result } else { Ok(()) }
}

fn runtime_governance_audit_writer(
    authority: RuntimeGovernanceAuthority,
    sqlite: Option<prodex_storage_sqlite_runtime::GovernanceSqliteRepository>,
    receiver: Receiver<RuntimeGovernanceAuditWrite>,
    shutdown: Arc<AtomicBool>,
) {
    while !shutdown.load(Ordering::SeqCst) {
        let Ok(write) = receiver.recv_timeout(Duration::from_millis(100)) else {
            continue;
        };
        let result = append_durable_audit(&authority, sqlite.as_ref(), write.event);
        let _ = write.acknowledge.send(result);
    }
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
        )
        .unwrap();
    }
}
