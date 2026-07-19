mod self_service;

use crate::RuntimeProxyRequest;
use prodex_domain::{
    AuditAction, AuditEvent, AuditEventId, AuditOutcome, AuditResource, Channel, CredentialScope,
    DataClassification, PolicyRevisionId, Principal, PrincipalId, SessionPolicyContext,
    TenantContext, TenantId, compute_audit_chain_digest,
};
use prodex_provider_core::ProviderId;
use prodex_storage::{
    AppendOnlyAuditCommand, AuditOutboxWriteCommand, GovernanceRepositoryError,
    GovernanceSessionRecord, GovernanceSessionRevokeCommand, GovernanceSessionUpsertCommand,
    GovernanceSessionUpsertOutcome, GovernanceWriteOutcome, TenantStorageKey,
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use super::local_rewrite::RuntimeGovernanceAuthority;

const RUNTIME_GATEWAY_GOVERNANCE_SESSION_LIMIT: usize = 4_096;
const RUNTIME_GATEWAY_GOVERNANCE_SESSION_BANK_LIMIT: usize = 128;
const RUNTIME_GATEWAY_GOVERNANCE_SESSION_ACK_TIMEOUT: Duration = Duration::from_secs(5);
const RUNTIME_GATEWAY_GOVERNANCE_SESSION_REFRESH_INTERVAL: Duration = Duration::from_secs(5);
const RUNTIME_GATEWAY_GOVERNANCE_REVOCATION_POLL_INTERVAL: Duration = Duration::from_millis(250);
const RUNTIME_GATEWAY_GOVERNANCE_SESSION_POLL_INTERVAL: Duration = Duration::from_millis(100);
const RUNTIME_GATEWAY_GOVERNANCE_SESSION_NEVER_EXPIRES_UNIX_MS: u64 = i64::MAX as u64;

#[derive(Clone, Default)]
pub(super) struct RuntimeGatewayGovernanceSessionStore(Arc<RuntimeGatewayGovernanceSessionInner>);

#[derive(Default)]
struct RuntimeGatewayGovernanceSessionInner {
    state: Mutex<RuntimeGatewayGovernanceSessionState>,
}

#[derive(Default)]
struct RuntimeGatewayGovernanceSessionState {
    sessions: BTreeMap<[u8; 32], RuntimeGatewayGovernanceSessionRecord>,
    pending_touches: BTreeMap<[u8; 32], GovernanceSessionUpsertCommand>,
    hydrated_tenants: BTreeSet<TenantId>,
    bank: Option<SyncSender<RuntimeGatewayGovernanceSessionBankCommand>>,
    durable: bool,
}

#[derive(Clone, Copy)]
struct RuntimeGatewayGovernanceSessionRecord {
    session_id_hash: [u8; 32],
    tenant_id: TenantId,
    principal_id: PrincipalId,
    credential_scope: CredentialScope,
    channel: Channel,
    created_at_seconds: u64,
    last_seen_seconds: u64,
    revoked: bool,
    classification: DataClassification,
    policy_revision: PolicyRevisionId,
    registry_revision: u64,
    provider_descriptor_revision: u64,
    provider: ProviderId,
}

enum RuntimeGatewayGovernanceSessionBankCommand {
    Admit {
        command: GovernanceSessionUpsertCommand,
        acknowledge: SyncSender<Result<GovernanceSessionUpsertOutcome, GovernanceRepositoryError>>,
    },
    Revoke {
        tenant: TenantContext,
        session_id_hash: String,
        actor: Principal,
        reason_code: String,
        acknowledge: SyncSender<Result<GovernanceWriteOutcome, GovernanceRepositoryError>>,
    },
    Wake,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RuntimeGatewayGovernanceSessionPersistError {
    ConcurrentLimitReached,
    Unavailable,
}

#[derive(Clone, Copy)]
pub(super) struct RuntimeGatewayGovernanceSessionSnapshot {
    key: Option<[u8; 32]>,
    binding_valid: bool,
    existing: bool,
    store_ready: bool,
    pub(super) policy: SessionPolicyContext,
    pub(super) affinity_provider: Option<ProviderId>,
    pub(super) pinned_policy_revision: Option<PolicyRevisionId>,
    pub(super) pinned_registry_revision: Option<u64>,
    pub(super) pinned_provider_descriptor_revision: Option<u64>,
}

impl RuntimeGatewayGovernanceSessionSnapshot {
    pub(super) fn session_id_hash(&self) -> Option<String> {
        self.key.as_ref().map(session_hash_hex)
    }
}

impl RuntimeGatewayGovernanceSessionStore {
    pub(super) fn spawn_durable_bank(
        &self,
        authority: RuntimeGovernanceAuthority,
        fail_closed_on_revocation_error: bool,
        shutdown: Arc<AtomicBool>,
    ) -> Result<thread::JoinHandle<()>, GovernanceRepositoryError> {
        let sqlite = match &authority {
            RuntimeGovernanceAuthority::Sqlite { path, .. } => {
                Some(prodex_storage_sqlite_runtime::GovernanceSqliteRepository::open(path)?)
            }
            RuntimeGovernanceAuthority::Postgres { .. } => None,
        };
        let (sender, receiver) = sync_channel(RUNTIME_GATEWAY_GOVERNANCE_SESSION_BANK_LIMIT);
        {
            let mut state = self
                .0
                .state
                .lock()
                .map_err(|_| GovernanceRepositoryError::Database)?;
            state.durable = true;
            state.bank = Some(sender);
        }
        let store = self.clone();
        Ok(thread::spawn(move || {
            runtime_gateway_governance_session_bank(
                store,
                authority,
                sqlite,
                receiver,
                fail_closed_on_revocation_error,
                shutdown,
            )
        }))
    }

    pub(super) fn snapshot(
        &self,
        request: &RuntimeProxyRequest,
        tenant: TenantContext,
        principal: &Principal,
        channel: Channel,
        now_seconds: u64,
    ) -> RuntimeGatewayGovernanceSessionSnapshot {
        let key = runtime_gateway_governance_session_key(request);
        let Some(key) = key else {
            return RuntimeGatewayGovernanceSessionSnapshot::new(None);
        };
        let Ok(state) = self.0.state.lock() else {
            let mut snapshot = RuntimeGatewayGovernanceSessionSnapshot::new(Some(key));
            snapshot.binding_valid = false;
            snapshot.store_ready = false;
            snapshot.policy.revoked = true;
            return snapshot;
        };
        let store_ready = !state.durable || state.hydrated_tenants.contains(&tenant.tenant_id);
        let Some(record) = state.sessions.get(&key) else {
            let mut snapshot = RuntimeGatewayGovernanceSessionSnapshot::new(Some(key));
            snapshot.store_ready = store_ready;
            if !store_ready {
                snapshot.policy.revoked = true;
            }
            return snapshot;
        };
        let binding_valid = record.tenant_id == tenant.tenant_id
            && record.principal_id == principal.id
            && record.credential_scope == principal.credential_scope
            && record.channel == channel;
        RuntimeGatewayGovernanceSessionSnapshot {
            key: Some(key),
            binding_valid,
            existing: true,
            store_ready,
            policy: SessionPolicyContext {
                age_seconds: now_seconds.saturating_sub(record.created_at_seconds),
                idle_seconds: now_seconds.saturating_sub(record.last_seen_seconds),
                revoked: record.revoked || !binding_valid,
                // Data-plane virtual-key/bearer evidence has no verified MFA claim. Keep this
                // false so MFA obligations fail closed until trusted auth evidence carries it.
                mfa_satisfied: false,
                retained_classification: record.classification,
            },
            affinity_provider: binding_valid.then_some(record.provider),
            pinned_policy_revision: Some(record.policy_revision),
            pinned_registry_revision: Some(record.registry_revision),
            pinned_provider_descriptor_revision: Some(record.provider_descriptor_revision),
        }
    }

    pub(super) fn revoke_by_hash(
        &self,
        tenant: TenantContext,
        session_id_hash: &str,
        actor: &Principal,
        reason_code: &str,
    ) -> Result<GovernanceWriteOutcome, GovernanceRepositoryError> {
        let key =
            parse_session_hash(session_id_hash).ok_or(GovernanceRepositoryError::InvalidInput)?;
        if actor.tenant_id != Some(tenant.tenant_id)
            || reason_code.is_empty()
            || reason_code.len() > 64
        {
            return Err(GovernanceRepositoryError::InvalidInput);
        }
        let bank = self
            .0
            .state
            .lock()
            .map_err(|_| GovernanceRepositoryError::Database)?
            .bank
            .clone()
            .ok_or(GovernanceRepositoryError::Unsupported)?;
        let (acknowledge, response) = sync_channel(1);
        bank.try_send(RuntimeGatewayGovernanceSessionBankCommand::Revoke {
            tenant,
            session_id_hash: session_id_hash.to_string(),
            actor: actor.clone(),
            reason_code: reason_code.to_string(),
            acknowledge,
        })
        .map_err(|_| GovernanceRepositoryError::Database)?;
        match response.recv_timeout(RUNTIME_GATEWAY_GOVERNANCE_SESSION_ACK_TIMEOUT) {
            Ok(Ok(outcome)) => {
                if let Ok(mut state) = self.0.state.lock()
                    && let Some(record) = state.sessions.get_mut(&key)
                {
                    record.revoked = true;
                }
                Ok(outcome)
            }
            Ok(Err(error)) => Err(error),
            Err(_) => Err(GovernanceRepositoryError::Database),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn remember(
        &self,
        snapshot: RuntimeGatewayGovernanceSessionSnapshot,
        tenant: TenantContext,
        principal: &Principal,
        channel: Channel,
        now_seconds: u64,
        classification: DataClassification,
        policy_revision: PolicyRevisionId,
        registry_revision: u64,
        provider_descriptor_revision: u64,
        provider: ProviderId,
        config: prodex_config::GovernanceSessionConfig,
    ) -> Result<(), RuntimeGatewayGovernanceSessionPersistError> {
        let Some(key) = snapshot.key else {
            return Ok(());
        };
        if !snapshot.binding_valid || !snapshot.store_ready {
            return Err(RuntimeGatewayGovernanceSessionPersistError::Unavailable);
        }
        let now_unix_ms = now_seconds.saturating_mul(1_000);
        let absolute_expires_at_unix_ms = config
            .absolute_timeout_seconds
            .map(|seconds| now_unix_ms.saturating_add(u64::from(seconds).saturating_mul(1_000)))
            .unwrap_or(RUNTIME_GATEWAY_GOVERNANCE_SESSION_NEVER_EXPIRES_UNIX_MS);
        let idle_expires_at_unix_ms = config
            .idle_timeout_seconds
            .map(|seconds| now_unix_ms.saturating_add(u64::from(seconds).saturating_mul(1_000)))
            .unwrap_or(RUNTIME_GATEWAY_GOVERNANCE_SESSION_NEVER_EXPIRES_UNIX_MS);
        let command = GovernanceSessionUpsertCommand {
            tenant_id: tenant.tenant_id,
            session_id_hash: session_hash_hex(&key),
            principal_id: principal.id,
            channel,
            credential_scope: principal.credential_scope,
            classification,
            policy_revision_id: policy_revision,
            provider_registry_revision: registry_revision.to_string(),
            provider_descriptor_revision,
            provider_affinity: Some(provider.label().to_string()),
            created_at_unix_ms: now_unix_ms,
            last_seen_at_unix_ms: now_unix_ms,
            absolute_expires_at_unix_ms,
            idle_expires_at_unix_ms,
            max_concurrent: config.max_concurrent,
        };
        let bank = self
            .0
            .state
            .lock()
            .map_err(|_| RuntimeGatewayGovernanceSessionPersistError::Unavailable)?
            .bank
            .clone();
        let security_relevant_change = snapshot.existing
            && (classification > snapshot.policy.retained_classification
                || snapshot.pinned_policy_revision != Some(policy_revision)
                || snapshot.pinned_registry_revision != Some(registry_revision)
                || snapshot.pinned_provider_descriptor_revision
                    != Some(provider_descriptor_revision)
                || snapshot.affinity_provider != Some(provider));
        if snapshot.existing && !security_relevant_change {
            self.remember_memory(command_record(&command, key, false));
            if let Some(bank) = bank {
                let mut state = self
                    .0
                    .state
                    .lock()
                    .map_err(|_| RuntimeGatewayGovernanceSessionPersistError::Unavailable)?;
                state.pending_touches.insert(key, command);
                drop(state);
                let _ = bank.try_send(RuntimeGatewayGovernanceSessionBankCommand::Wake);
            }
            return Ok(());
        }
        let Some(bank) = bank else {
            self.remember_memory(command_record(&command, key, false));
            return Ok(());
        };
        let (acknowledge, response) = sync_channel(1);
        bank.try_send(RuntimeGatewayGovernanceSessionBankCommand::Admit {
            command,
            acknowledge,
        })
        .map_err(|error| match error {
            TrySendError::Full(_) | TrySendError::Disconnected(_) => {
                RuntimeGatewayGovernanceSessionPersistError::Unavailable
            }
        })?;
        match response.recv_timeout(RUNTIME_GATEWAY_GOVERNANCE_SESSION_ACK_TIMEOUT) {
            Ok(Ok(GovernanceSessionUpsertOutcome::Stored(record))) => {
                self.remember_memory(stored_record(*record)?);
                Ok(())
            }
            Ok(Ok(GovernanceSessionUpsertOutcome::ConcurrentLimitReached)) => {
                Err(RuntimeGatewayGovernanceSessionPersistError::ConcurrentLimitReached)
            }
            Ok(Err(_)) | Err(_) => Err(RuntimeGatewayGovernanceSessionPersistError::Unavailable),
        }
    }

    fn remember_memory(&self, incoming: RuntimeGatewayGovernanceSessionRecord) {
        let Ok(mut state) = self.0.state.lock() else {
            return;
        };
        state
            .sessions
            .entry(incoming.session_id_hash)
            .and_modify(|record| {
                record.last_seen_seconds = record.last_seen_seconds.max(incoming.last_seen_seconds);
                record.classification = record.classification.raised_to(incoming.classification);
                if incoming.last_seen_seconds >= record.last_seen_seconds {
                    record.policy_revision = incoming.policy_revision;
                    record.registry_revision = incoming.registry_revision;
                    record.provider_descriptor_revision = incoming.provider_descriptor_revision;
                    record.provider = incoming.provider;
                }
                record.revoked |= incoming.revoked;
            })
            .or_insert(incoming);
        while state.sessions.len() > RUNTIME_GATEWAY_GOVERNANCE_SESSION_LIMIT {
            let Some(stale) = state
                .sessions
                .iter()
                .min_by_key(|(_, record)| record.last_seen_seconds)
                .map(|(key, _)| *key)
            else {
                break;
            };
            state.sessions.remove(&stale);
        }
    }

    pub(super) fn configured_violation(
        &self,
        snapshot: RuntimeGatewayGovernanceSessionSnapshot,
        tenant: TenantContext,
        principal: &Principal,
        now_seconds: u64,
        config: prodex_config::GovernanceSessionConfig,
    ) -> Option<&'static str> {
        if !snapshot.store_ready {
            return Some("session_store_unavailable");
        }
        if config
            .absolute_timeout_seconds
            .is_some_and(|limit| snapshot.policy.age_seconds > u64::from(limit))
        {
            return Some("session_absolute_timeout");
        }
        if config
            .idle_timeout_seconds
            .is_some_and(|limit| snapshot.policy.idle_seconds > u64::from(limit))
        {
            return Some("session_idle_timeout");
        }
        let max = usize::try_from(config.max_concurrent?).unwrap_or(usize::MAX);
        if snapshot.existing || snapshot.key.is_none() {
            return None;
        }
        let state = self.0.state.lock().ok()?;
        let active = state
            .sessions
            .values()
            .filter(|record| {
                record.tenant_id == tenant.tenant_id
                    && record.principal_id == principal.id
                    && !record.revoked
                    && config.absolute_timeout_seconds.is_none_or(|limit| {
                        now_seconds.saturating_sub(record.created_at_seconds) <= u64::from(limit)
                    })
                    && config.idle_timeout_seconds.is_none_or(|limit| {
                        now_seconds.saturating_sub(record.last_seen_seconds) <= u64::from(limit)
                    })
            })
            .count();
        (active >= max).then_some("session_concurrency_limit")
    }
}

impl RuntimeGatewayGovernanceSessionSnapshot {
    pub(super) fn policy_revision_mismatch(self, current: PolicyRevisionId) -> bool {
        self.pinned_policy_revision
            .is_some_and(|pinned| pinned != current)
    }

    pub(super) fn provider_revision_mismatch(
        self,
        registry_revision: u64,
        descriptor_revision: u64,
    ) -> bool {
        self.pinned_registry_revision
            .zip(self.pinned_provider_descriptor_revision)
            .is_some_and(|(registry, descriptor)| {
                registry != registry_revision || descriptor != descriptor_revision
            })
    }

    fn new(key: Option<[u8; 32]>) -> Self {
        Self {
            key,
            binding_valid: true,
            existing: false,
            store_ready: true,
            policy: SessionPolicyContext {
                age_seconds: 0,
                idle_seconds: 0,
                revoked: false,
                mfa_satisfied: false,
                retained_classification: DataClassification::Public,
            },
            affinity_provider: None,
            pinned_policy_revision: None,
            pinned_registry_revision: None,
            pinned_provider_descriptor_revision: None,
        }
    }
}

fn runtime_gateway_governance_session_bank(
    store: RuntimeGatewayGovernanceSessionStore,
    authority: RuntimeGovernanceAuthority,
    sqlite: Option<prodex_storage_sqlite_runtime::GovernanceSqliteRepository>,
    receiver: Receiver<RuntimeGatewayGovernanceSessionBankCommand>,
    fail_closed_on_revocation_error: bool,
    shutdown: Arc<AtomicBool>,
) {
    runtime_gateway_governance_session_refresh(&store, &authority, sqlite.as_ref());
    let mut revocation_epochs = BTreeMap::new();
    if runtime_gateway_governance_session_revocation_changed(
        &authority,
        sqlite.as_ref(),
        &mut revocation_epochs,
    )
    .is_err()
        && fail_closed_on_revocation_error
    {
        runtime_gateway_governance_sessions_mark_unavailable(&store);
    }
    let mut refreshed_at = std::time::Instant::now();
    let mut revocations_polled_at = std::time::Instant::now();
    while !shutdown.load(Ordering::SeqCst) {
        match receiver.recv_timeout(RUNTIME_GATEWAY_GOVERNANCE_SESSION_POLL_INTERVAL) {
            Ok(RuntimeGatewayGovernanceSessionBankCommand::Admit {
                command,
                acknowledge,
            }) => {
                let result =
                    runtime_gateway_governance_session_upsert(&authority, sqlite.as_ref(), command);
                let _ = acknowledge.send(result);
            }
            Ok(RuntimeGatewayGovernanceSessionBankCommand::Revoke {
                tenant,
                session_id_hash,
                actor,
                reason_code,
                acknowledge,
            }) => {
                let result = runtime_gateway_governance_session_revoke(
                    &authority,
                    sqlite.as_ref(),
                    tenant,
                    session_id_hash,
                    actor,
                    reason_code,
                );
                let _ = acknowledge.send(result);
            }
            Ok(RuntimeGatewayGovernanceSessionBankCommand::Wake)
            | Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        let touches = {
            let Ok(mut state) = store.0.state.lock() else {
                continue;
            };
            std::mem::take(&mut state.pending_touches)
        };
        for (key, command) in touches {
            if runtime_gateway_governance_session_upsert(
                &authority,
                sqlite.as_ref(),
                command.clone(),
            )
            .is_err()
                && let Ok(mut state) = store.0.state.lock()
                && state.pending_touches.len() < RUNTIME_GATEWAY_GOVERNANCE_SESSION_LIMIT
            {
                state.pending_touches.insert(key, command);
            }
        }
        if revocations_polled_at.elapsed() >= RUNTIME_GATEWAY_GOVERNANCE_REVOCATION_POLL_INTERVAL {
            match runtime_gateway_governance_session_revocation_changed(
                &authority,
                sqlite.as_ref(),
                &mut revocation_epochs,
            ) {
                Ok(true) => {
                    if fail_closed_on_revocation_error {
                        runtime_gateway_governance_sessions_mark_unavailable(&store);
                    }
                    runtime_gateway_governance_session_refresh(&store, &authority, sqlite.as_ref());
                    refreshed_at = std::time::Instant::now();
                }
                Ok(false) => {}
                Err(_) if fail_closed_on_revocation_error => {
                    runtime_gateway_governance_sessions_mark_unavailable(&store);
                }
                Err(_) => {}
            }
            revocations_polled_at = std::time::Instant::now();
        }
        if refreshed_at.elapsed() >= RUNTIME_GATEWAY_GOVERNANCE_SESSION_REFRESH_INTERVAL {
            if fail_closed_on_revocation_error {
                runtime_gateway_governance_sessions_mark_unavailable(&store);
            }
            runtime_gateway_governance_session_refresh(&store, &authority, sqlite.as_ref());
            refreshed_at = std::time::Instant::now();
        }
    }
}

fn runtime_gateway_governance_session_revocation_changed(
    authority: &RuntimeGovernanceAuthority,
    sqlite: Option<&prodex_storage_sqlite_runtime::GovernanceSqliteRepository>,
    epochs: &mut BTreeMap<TenantId, u64>,
) -> Result<bool, GovernanceRepositoryError> {
    let tenant_ids = authority.tenant_ids()?;
    let mut changed = false;
    for tenant_id in &tenant_ids {
        let epoch = match authority {
            RuntimeGovernanceAuthority::Sqlite { .. } => sqlite
                .ok_or(GovernanceRepositoryError::Database)
                .and_then(|repository| repository.governance_session_revocation_epoch(*tenant_id)),
            RuntimeGovernanceAuthority::Postgres {
                repository,
                runtime,
                ..
            } => runtime.block_on(repository.governance_session_revocation_epoch(*tenant_id)),
        };
        let epoch = epoch?;
        changed |= epochs
            .insert(*tenant_id, epoch)
            .is_none_or(|prior| prior != epoch);
    }
    Ok(changed)
}

fn runtime_gateway_governance_sessions_mark_unavailable(
    store: &RuntimeGatewayGovernanceSessionStore,
) {
    if let Ok(mut state) = store.0.state.lock() {
        state.hydrated_tenants.clear();
    }
}

fn runtime_gateway_governance_session_revoke(
    authority: &RuntimeGovernanceAuthority,
    sqlite: Option<&prodex_storage_sqlite_runtime::GovernanceSqliteRepository>,
    tenant: TenantContext,
    session_id_hash: String,
    actor: Principal,
    reason_code: String,
) -> Result<GovernanceWriteOutcome, GovernanceRepositoryError> {
    for _ in 0..3 {
        let previous_digest = match authority {
            RuntimeGovernanceAuthority::Sqlite { .. } => sqlite
                .ok_or(GovernanceRepositoryError::Database)?
                .latest_audit_digest(tenant.tenant_id)?,
            RuntimeGovernanceAuthority::Postgres {
                repository,
                runtime,
                ..
            } => runtime.block_on(repository.governance_latest_audit_digest(tenant.tenant_id))?,
        };
        let occurred_at_unix_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .try_into()
            .map_err(|_| GovernanceRepositoryError::InvalidInput)?;
        let event = AuditEvent::new(
            occurred_at_unix_ms,
            tenant,
            &actor,
            AuditAction::try_new("governance.session.revoke")
                .map_err(|_| GovernanceRepositoryError::InvalidInput)?,
            AuditResource::new(
                "governance_session",
                Some(session_id_hash.clone()),
                Some(tenant.tenant_id),
            ),
            AuditOutcome::Success,
            Some(reason_code.clone()),
        );
        let event_digest = compute_audit_chain_digest(previous_digest.as_ref(), &event);
        let command = GovernanceSessionRevokeCommand {
            tenant_id: tenant.tenant_id,
            session_id_hash: session_id_hash.clone(),
            revoked_at_unix_ms: occurred_at_unix_ms,
            reason_code: reason_code.clone(),
            audit_outbox: AuditOutboxWriteCommand {
                outbox_event_id: AuditEventId::new(),
                audit: AppendOnlyAuditCommand {
                    storage_key: TenantStorageKey::tenant(tenant.tenant_id),
                    event,
                    previous_digest,
                    event_digest,
                },
            },
        };
        let result = match authority {
            RuntimeGovernanceAuthority::Sqlite { .. } => sqlite
                .ok_or(GovernanceRepositoryError::Database)?
                .governance_revoke_session(command),
            RuntimeGovernanceAuthority::Postgres {
                repository,
                runtime,
                ..
            } => runtime.block_on(repository.governance_revoke_session(command)),
        };
        match result {
            Err(GovernanceRepositoryError::AuditChainConflict) => continue,
            result => return result,
        }
    }
    Err(GovernanceRepositoryError::AuditChainConflict)
}

fn runtime_gateway_governance_session_upsert(
    authority: &RuntimeGovernanceAuthority,
    sqlite: Option<&prodex_storage_sqlite_runtime::GovernanceSqliteRepository>,
    command: GovernanceSessionUpsertCommand,
) -> Result<GovernanceSessionUpsertOutcome, GovernanceRepositoryError> {
    match authority {
        RuntimeGovernanceAuthority::Sqlite { .. } => sqlite
            .ok_or(GovernanceRepositoryError::Database)?
            .governance_upsert_session(command),
        RuntimeGovernanceAuthority::Postgres {
            repository,
            runtime,
            ..
        } => runtime.block_on(repository.governance_upsert_session(command)),
    }
}

fn runtime_gateway_governance_session_refresh(
    store: &RuntimeGatewayGovernanceSessionStore,
    authority: &RuntimeGovernanceAuthority,
    sqlite: Option<&prodex_storage_sqlite_runtime::GovernanceSqliteRepository>,
) {
    let Ok(tenant_ids) = authority.tenant_ids() else {
        return;
    };
    let now_unix_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX);
    for tenant_id in &tenant_ids {
        let loaded = match authority {
            RuntimeGovernanceAuthority::Sqlite { .. } => sqlite
                .ok_or(GovernanceRepositoryError::Database)
                .and_then(|repository| {
                    repository.governance_load_sessions(
                        *tenant_id,
                        now_unix_ms,
                        RUNTIME_GATEWAY_GOVERNANCE_SESSION_LIMIT as u16,
                    )
                }),
            RuntimeGovernanceAuthority::Postgres {
                repository,
                runtime,
                ..
            } => runtime.block_on(repository.governance_load_sessions(
                *tenant_id,
                now_unix_ms,
                RUNTIME_GATEWAY_GOVERNANCE_SESSION_LIMIT as u16,
            )),
        };
        let Ok(records) = loaded else {
            continue;
        };
        let records = records
            .into_iter()
            .map(stored_record)
            .collect::<Result<Vec<_>, _>>();
        let Ok(records) = records else {
            continue;
        };
        let Ok(state) = store.0.state.lock() else {
            continue;
        };
        drop(state);
        for record in records {
            store.remember_memory(record);
        }
        let Ok(mut state) = store.0.state.lock() else {
            continue;
        };
        state.hydrated_tenants.insert(*tenant_id);
        while state.sessions.len() > RUNTIME_GATEWAY_GOVERNANCE_SESSION_LIMIT {
            let Some(stale) = state
                .sessions
                .iter()
                .min_by_key(|(_, record)| record.last_seen_seconds)
                .map(|(key, _)| *key)
            else {
                break;
            };
            state.sessions.remove(&stale);
        }
    }
}

fn command_record(
    command: &GovernanceSessionUpsertCommand,
    session_id_hash: [u8; 32],
    revoked: bool,
) -> RuntimeGatewayGovernanceSessionRecord {
    let registry_revision = command
        .provider_registry_revision
        .parse()
        .unwrap_or_default();
    RuntimeGatewayGovernanceSessionRecord {
        session_id_hash,
        tenant_id: command.tenant_id,
        principal_id: command.principal_id,
        credential_scope: command.credential_scope,
        channel: command.channel,
        created_at_seconds: command.created_at_unix_ms / 1_000,
        last_seen_seconds: command.last_seen_at_unix_ms / 1_000,
        revoked,
        classification: command.classification,
        policy_revision: command.policy_revision_id,
        registry_revision,
        provider_descriptor_revision: command.provider_descriptor_revision,
        provider: command
            .provider_affinity
            .as_deref()
            .and_then(ProviderId::parse)
            .unwrap_or(ProviderId::OpenAi),
    }
}

fn stored_record(
    record: GovernanceSessionRecord,
) -> Result<RuntimeGatewayGovernanceSessionRecord, RuntimeGatewayGovernanceSessionPersistError> {
    let session_id_hash = parse_session_hash(&record.session_id_hash)
        .ok_or(RuntimeGatewayGovernanceSessionPersistError::Unavailable)?;
    let registry_revision = record
        .provider_registry_revision
        .parse()
        .map_err(|_| RuntimeGatewayGovernanceSessionPersistError::Unavailable)?;
    if record.provider_descriptor_revision == 0 {
        return Err(RuntimeGatewayGovernanceSessionPersistError::Unavailable);
    }
    let provider = record
        .provider_affinity
        .as_deref()
        .and_then(ProviderId::parse)
        .ok_or(RuntimeGatewayGovernanceSessionPersistError::Unavailable)?;
    Ok(RuntimeGatewayGovernanceSessionRecord {
        session_id_hash,
        tenant_id: record.tenant_id,
        principal_id: record.principal_id,
        credential_scope: record.credential_scope,
        channel: record.channel,
        created_at_seconds: record.created_at_unix_ms / 1_000,
        last_seen_seconds: record.last_seen_at_unix_ms / 1_000,
        revoked: record.revoked_at_unix_ms.is_some(),
        classification: record.classification,
        policy_revision: record.policy_revision_id,
        registry_revision,
        provider_descriptor_revision: record.provider_descriptor_revision,
        provider,
    })
}

fn session_hash_hex(hash: &[u8; 32]) -> String {
    hash.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn parse_session_hash(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return None;
    }
    let mut hash = [0_u8; 32];
    for (index, byte) in hash.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).ok()?;
    }
    Some(hash)
}

fn runtime_gateway_governance_session_key(request: &RuntimeProxyRequest) -> Option<[u8; 32]> {
    let session_id = runtime_proxy_crate::runtime_request_session_id(request)?;
    let mut hasher = Sha256::new();
    hasher.update(b"prodex:gateway-governance-session:v1");
    hasher.update([0]);
    hasher.update(session_id.as_bytes());
    Some(hasher.finalize().into())
}

pub(super) fn runtime_gateway_governance_session_hash(
    request: &RuntimeProxyRequest,
) -> Option<String> {
    runtime_gateway_governance_session_key(request).map(|hash| session_hash_hex(&hash))
}

#[cfg(test)]
mod tests {
    use super::*;
    use prodex_domain::{PrincipalKind, Role};

    fn principal(tenant_id: TenantId) -> Principal {
        Principal::new(
            PrincipalId::new(),
            Some(tenant_id),
            PrincipalKind::VirtualKey,
            Role::Operator,
            CredentialScope::DataPlane,
        )
    }

    fn request() -> RuntimeProxyRequest {
        RuntimeProxyRequest {
            method: "POST".to_string(),
            path_and_query: "/v1/responses".to_string(),
            headers: vec![("session_id".to_string(), "opaque-session".to_string())],
            body: Vec::new(),
        }
    }

    #[test]
    fn session_context_propagates_age_idle_classification_and_affinity() {
        let store = RuntimeGatewayGovernanceSessionStore::default();
        let tenant = TenantContext {
            tenant_id: TenantId::new(),
        };
        let principal = principal(tenant.tenant_id);
        let initial = store.snapshot(&request(), tenant, &principal, Channel::Api, 100);
        let policy_revision = PolicyRevisionId::new();
        store
            .remember(
                initial,
                tenant,
                &principal,
                Channel::Api,
                100,
                DataClassification::Confidential,
                policy_revision,
                7,
                9,
                ProviderId::Gemini,
                prodex_config::GovernanceSessionConfig::default(),
            )
            .unwrap();

        let resumed = store.snapshot(&request(), tenant, &principal, Channel::Api, 112);
        assert_eq!(resumed.policy.age_seconds, 12);
        assert_eq!(resumed.policy.idle_seconds, 12);
        assert_eq!(
            resumed.policy.retained_classification,
            DataClassification::Confidential
        );
        assert_eq!(resumed.affinity_provider, Some(ProviderId::Gemini));
        assert_eq!(resumed.pinned_registry_revision, Some(7));
        assert_eq!(resumed.pinned_provider_descriptor_revision, Some(9));
        assert_eq!(resumed.pinned_policy_revision, Some(policy_revision));
        assert!(!resumed.policy_revision_mismatch(policy_revision));
        assert!(resumed.policy_revision_mismatch(PolicyRevisionId::new()));
        assert!(!resumed.provider_revision_mismatch(7, 9));
        assert!(resumed.provider_revision_mismatch(8, 9));
        assert!(!resumed.policy.revoked);
    }

    #[test]
    fn durable_session_round_trips_separate_provider_revisions() {
        let tenant_id = TenantId::new();
        let principal = principal(tenant_id);
        let policy_revision = PolicyRevisionId::new();
        let record = GovernanceSessionRecord {
            tenant_id,
            session_id_hash: "a".repeat(64),
            principal_id: principal.id,
            channel: Channel::Api,
            credential_scope: CredentialScope::DataPlane,
            classification: DataClassification::Restricted,
            policy_revision_id: policy_revision,
            provider_registry_revision: "7".to_string(),
            provider_descriptor_revision: 9,
            provider_affinity: Some("gemini".to_string()),
            created_at_unix_ms: 100_000,
            last_seen_at_unix_ms: 112_000,
            absolute_expires_at_unix_ms: 200_000,
            idle_expires_at_unix_ms: 150_000,
            revoked_at_unix_ms: None,
            revocation_reason_code: None,
        };

        let stored = stored_record(record).unwrap();
        assert_eq!(stored.registry_revision, 7);
        assert_eq!(stored.provider_descriptor_revision, 9);
        assert_eq!(stored.provider, ProviderId::Gemini);
        assert_eq!(stored.classification, DataClassification::Restricted);
        assert_eq!(stored.policy_revision, policy_revision);
    }

    #[test]
    fn session_reuse_with_another_principal_is_revoked() {
        let store = RuntimeGatewayGovernanceSessionStore::default();
        let tenant = TenantContext {
            tenant_id: TenantId::new(),
        };
        let owner = principal(tenant.tenant_id);
        let initial = store.snapshot(&request(), tenant, &owner, Channel::Api, 100);
        store
            .remember(
                initial,
                tenant,
                &owner,
                Channel::Api,
                100,
                DataClassification::Internal,
                PolicyRevisionId::new(),
                1,
                1,
                ProviderId::OpenAi,
                prodex_config::GovernanceSessionConfig::default(),
            )
            .unwrap();

        let other = principal(tenant.tenant_id);
        let resumed = store.snapshot(&request(), tenant, &other, Channel::Api, 101);
        assert!(resumed.policy.revoked);
        assert_eq!(resumed.affinity_provider, None);
    }

    #[test]
    fn configured_timeouts_and_concurrency_fail_closed() {
        let store = RuntimeGatewayGovernanceSessionStore::default();
        let tenant = TenantContext {
            tenant_id: TenantId::new(),
        };
        let principal = principal(tenant.tenant_id);
        let first_request = request();
        let initial = store.snapshot(&first_request, tenant, &principal, Channel::Api, 100);
        store
            .remember(
                initial,
                tenant,
                &principal,
                Channel::Api,
                100,
                DataClassification::Internal,
                PolicyRevisionId::new(),
                1,
                1,
                ProviderId::OpenAi,
                prodex_config::GovernanceSessionConfig::default(),
            )
            .unwrap();
        let resumed = store.snapshot(&first_request, tenant, &principal, Channel::Api, 112);
        assert_eq!(
            store.configured_violation(
                resumed,
                tenant,
                &principal,
                112,
                prodex_config::GovernanceSessionConfig {
                    absolute_timeout_seconds: Some(10),
                    idle_timeout_seconds: None,
                    max_concurrent: None,
                },
            ),
            Some("session_absolute_timeout")
        );

        let mut second_request = request();
        second_request.headers[0].1 = "second-session".to_string();
        let second = store.snapshot(&second_request, tenant, &principal, Channel::Api, 101);
        assert_eq!(
            store.configured_violation(
                second,
                tenant,
                &principal,
                101,
                prodex_config::GovernanceSessionConfig {
                    absolute_timeout_seconds: None,
                    idle_timeout_seconds: None,
                    max_concurrent: Some(1),
                },
            ),
            Some("session_concurrency_limit")
        );
    }

    #[test]
    fn cross_replica_revocation_epoch_invalidates_cached_sessions_promptly() {
        let tenant_id = TenantId::new();
        let root = std::env::temp_dir().join(format!(
            "prodex-session-revocation-epoch-{}",
            AuditEventId::new()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("state.sqlite");
        let connection = rusqlite::Connection::open(&path).unwrap();
        for migration in prodex_storage_sqlite::SQLITE_MIGRATIONS {
            connection.execute_batch(migration.sql).unwrap();
        }
        connection
            .execute(
                "INSERT INTO prodex_tenants (
                    tenant_id, display_name, created_at_unix_ms, updated_at_unix_ms
                 ) VALUES (?1, 'tenant', 1, 1)",
                rusqlite::params![tenant_id.to_string()],
            )
            .unwrap();
        drop(connection);

        let authority = RuntimeGovernanceAuthority::Sqlite {
            path: path.clone(),
            tenant_ids: Arc::new(Mutex::new(BTreeSet::from([tenant_id]))),
        };
        let repository =
            prodex_storage_sqlite_runtime::GovernanceSqliteRepository::open(&path).unwrap();
        let mut epochs = BTreeMap::new();
        assert!(
            runtime_gateway_governance_session_revocation_changed(
                &authority,
                Some(&repository),
                &mut epochs,
            )
            .unwrap()
        );
        assert!(
            !runtime_gateway_governance_session_revocation_changed(
                &authority,
                Some(&repository),
                &mut epochs,
            )
            .unwrap()
        );

        let second_replica = rusqlite::Connection::open(&path).unwrap();
        second_replica
            .execute(
                "UPDATE prodex_tenants SET session_revocation_epoch = 1 WHERE tenant_id = ?1",
                rusqlite::params![tenant_id.to_string()],
            )
            .unwrap();
        drop(second_replica);
        assert!(
            runtime_gateway_governance_session_revocation_changed(
                &authority,
                Some(&repository),
                &mut epochs,
            )
            .unwrap()
        );
        assert_eq!(epochs.get(&tenant_id), Some(&1));

        assert!(
            runtime_gateway_governance_session_revocation_changed(&authority, None, &mut epochs)
                .is_err()
        );

        let store = RuntimeGatewayGovernanceSessionStore::default();
        {
            let mut state = store.0.state.lock().unwrap();
            state.durable = true;
            state.hydrated_tenants.insert(tenant_id);
        }
        runtime_gateway_governance_sessions_mark_unavailable(&store);
        let principal = principal(tenant_id);
        let snapshot = store.snapshot(
            &request(),
            TenantContext { tenant_id },
            &principal,
            Channel::Api,
            1,
        );
        assert_eq!(
            store.configured_violation(
                snapshot,
                TenantContext { tenant_id },
                &principal,
                1,
                prodex_config::GovernanceSessionConfig::default(),
            ),
            Some("session_store_unavailable")
        );

        drop(repository);
        std::fs::remove_dir_all(root).unwrap();
    }
}
