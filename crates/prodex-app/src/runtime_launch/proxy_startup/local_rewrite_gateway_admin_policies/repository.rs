use super::*;

pub(in crate::runtime_launch::proxy_startup) enum RuntimeGovernanceRepository<'a> {
    Sqlite(GovernanceSqliteRepository),
    Postgres {
        repository: &'a prodex_storage_postgres_runtime::PostgresRepository,
        runtime: tokio::runtime::Handle,
    },
}

impl RuntimeGovernanceRepository<'_> {
    fn write_revision_idempotent(
        &self,
        command: GovernanceRevisionWriteCommand,
        audit: AuditOutboxWriteCommand,
        idempotency: GovernanceMutationIdempotency,
    ) -> Result<GovernanceWriteOutcome, GovernanceRepositoryError> {
        match self {
            Self::Sqlite(repository) => {
                repository.write_revision_idempotent(command, audit, idempotency)
            }
            Self::Postgres {
                repository,
                runtime,
            } => runtime.block_on(repository.governance_write_revision_idempotent(
                command,
                audit,
                idempotency,
            )),
        }
    }

    pub(super) fn create_approval(
        &self,
        approval: ApprovalRecord,
        audit: AuditOutboxWriteCommand,
    ) -> Result<GovernanceWriteOutcome, GovernanceRepositoryError> {
        match self {
            Self::Sqlite(repository) => repository.create_approval(approval, audit),
            Self::Postgres {
                repository,
                runtime,
            } => runtime.block_on(repository.governance_create_approval(approval, audit)),
        }
    }

    pub(super) fn create_approval_idempotent(
        &self,
        approval: ApprovalRecord,
        audit: AuditOutboxWriteCommand,
        idempotency: GovernanceMutationIdempotency,
    ) -> Result<GovernanceWriteOutcome, GovernanceRepositoryError> {
        match self {
            Self::Sqlite(repository) => {
                repository.create_approval_idempotent(approval, audit, idempotency)
            }
            Self::Postgres {
                repository,
                runtime,
            } => runtime.block_on(repository.governance_create_approval_idempotent(
                approval,
                audit,
                idempotency,
            )),
        }
    }

    pub(super) fn transition_approval(
        &self,
        request: ApprovalVoteRequest,
        action: ApprovalAction,
    ) -> Result<ApprovalRecord, GovernanceRepositoryError> {
        match self {
            Self::Sqlite(repository) => repository.transition_approval(request, action),
            Self::Postgres {
                repository,
                runtime,
            } => runtime.block_on(repository.governance_transition_approval(request, action)),
        }
    }

    pub(super) fn transition_approval_idempotent(
        &self,
        request: ApprovalVoteRequest,
        action: ApprovalAction,
        idempotency: ApprovalVoteIdempotency,
    ) -> Result<ApprovalVoteMutationOutcome, GovernanceRepositoryError> {
        match self {
            Self::Sqlite(repository) => {
                repository.transition_approval_idempotent(request, action, idempotency)
            }
            Self::Postgres {
                repository,
                runtime,
            } => runtime.block_on(repository.governance_transition_approval_idempotent(
                request,
                action,
                idempotency,
            )),
        }
    }

    pub(super) fn list_revisions(
        &self,
        tenant_id: prodex_domain::TenantId,
        kind: GovernanceArtifactKind,
    ) -> Result<Vec<GovernanceRevisionSummary>, GovernanceRepositoryError> {
        match self {
            Self::Sqlite(repository) => repository.list_revisions(tenant_id, kind),
            Self::Postgres {
                repository,
                runtime,
            } => runtime.block_on(repository.governance_list_revisions(tenant_id, kind)),
        }
    }

    pub(super) fn get_revision(
        &self,
        tenant_id: prodex_domain::TenantId,
        kind: GovernanceArtifactKind,
        revision_id: &str,
    ) -> Result<GovernanceRevisionSummary, GovernanceRepositoryError> {
        match self {
            Self::Sqlite(repository) => repository.get_revision(tenant_id, kind, revision_id),
            Self::Postgres {
                repository,
                runtime,
            } => runtime.block_on(repository.governance_get_revision(tenant_id, kind, revision_id)),
        }
    }

    pub(super) fn get_approval(
        &self,
        tenant_id: prodex_domain::TenantId,
        approval_id: &ApprovalId,
    ) -> Result<ApprovalRecord, GovernanceRepositoryError> {
        match self {
            Self::Sqlite(repository) => repository.get_approval(tenant_id, approval_id),
            Self::Postgres {
                repository,
                runtime,
            } => {
                runtime.block_on(repository.governance_get_approval(tenant_id, approval_id.clone()))
            }
        }
    }

    pub(super) fn list_execution_approvals(
        &self,
        tenant_id: prodex_domain::TenantId,
    ) -> Result<Vec<ApprovalRecord>, GovernanceRepositoryError> {
        match self {
            Self::Sqlite(repository) => repository.list_execution_approvals(tenant_id),
            Self::Postgres {
                repository,
                runtime,
            } => runtime.block_on(repository.governance_list_execution_approvals(tenant_id)),
        }
    }

    pub(super) fn list_approvals(
        &self,
        tenant_id: prodex_domain::TenantId,
        kind: ApprovalKind,
    ) -> Result<Vec<ApprovalRecord>, GovernanceRepositoryError> {
        match self {
            Self::Sqlite(repository) => repository.list_approvals(tenant_id, kind),
            Self::Postgres {
                repository,
                runtime,
            } => runtime.block_on(repository.governance_list_approvals(tenant_id, kind)),
        }
    }

    pub(super) fn upsert_audit_legal_hold_idempotent(
        &self,
        hold: &AuditRetentionHold,
        created_by: prodex_domain::PrincipalId,
        created_at_unix_ms: u64,
        audit: AuditOutboxWriteCommand,
        idempotency: GovernanceMutationIdempotency,
    ) -> Result<(), GovernanceRepositoryError> {
        match self {
            Self::Sqlite(repository) => repository.upsert_audit_legal_hold_idempotent(
                hold,
                created_by,
                created_at_unix_ms,
                audit,
                idempotency,
            ),
            Self::Postgres {
                repository,
                runtime,
            } => runtime.block_on(repository.governance_upsert_audit_legal_hold_idempotent(
                hold.clone(),
                created_by,
                created_at_unix_ms,
                audit,
                idempotency,
            )),
        }
    }

    pub(super) fn list_audit_legal_holds(
        &self,
        tenant_id: prodex_domain::TenantId,
    ) -> Result<Vec<AuditRetentionHold>, GovernanceRepositoryError> {
        match self {
            Self::Sqlite(repository) => repository.list_audit_legal_holds(tenant_id),
            Self::Postgres {
                repository,
                runtime,
            } => runtime.block_on(repository.governance_list_audit_legal_holds(tenant_id)),
        }
    }

    pub(super) fn delete_audit_legal_hold_idempotent(
        &self,
        tenant_id: prodex_domain::TenantId,
        event_id: AuditEventId,
        audit: AuditOutboxWriteCommand,
        idempotency: GovernanceMutationIdempotency,
    ) -> Result<bool, GovernanceRepositoryError> {
        match self {
            Self::Sqlite(repository) => repository.delete_audit_legal_hold_idempotent(
                tenant_id,
                event_id,
                audit,
                idempotency,
            ),
            Self::Postgres {
                repository,
                runtime,
            } => runtime.block_on(repository.governance_delete_audit_legal_hold_idempotent(
                tenant_id,
                event_id,
                audit,
                idempotency,
            )),
        }
    }

    pub(super) fn purge_audit_events_idempotent(
        &self,
        tenant_id: prodex_domain::TenantId,
        event_ids: &[AuditEventId],
        now_unix_ms: u64,
        cutoff_unix_ms: u64,
        audit: AuditOutboxWriteCommand,
        idempotency: GovernanceMutationIdempotency,
    ) -> Result<Vec<AuditEventId>, GovernanceRepositoryError> {
        match self {
            Self::Sqlite(repository) => repository.purge_audit_events_idempotent(
                tenant_id,
                event_ids,
                now_unix_ms,
                cutoff_unix_ms,
                audit,
                idempotency,
            ),
            Self::Postgres {
                repository,
                runtime,
            } => runtime.block_on(repository.governance_purge_audit_events_idempotent(
                tenant_id,
                event_ids.to_vec(),
                now_unix_ms,
                cutoff_unix_ms,
                audit,
                idempotency,
            )),
        }
    }

    pub(super) fn status(
        &self,
        tenant_id: prodex_domain::TenantId,
        kind: GovernanceArtifactKind,
    ) -> Result<GovernanceStatus, GovernanceRepositoryError> {
        match self {
            Self::Sqlite(repository) => repository.status(tenant_id, kind),
            Self::Postgres {
                repository,
                runtime,
            } => runtime.block_on(repository.governance_status(tenant_id, kind)),
        }
    }

    pub(in crate::runtime_launch::proxy_startup) fn latest_audit_digest(
        &self,
        tenant_id: prodex_domain::TenantId,
    ) -> Result<Option<prodex_domain::AuditDigest>, GovernanceRepositoryError> {
        match self {
            Self::Sqlite(repository) => repository.latest_audit_digest(tenant_id),
            Self::Postgres {
                repository,
                runtime,
            } => runtime.block_on(repository.governance_latest_audit_digest(tenant_id)),
        }
    }

    fn activate_revision(
        &self,
        request: GovernanceActivationRequest,
        validate_artifact: impl FnOnce(&GovernanceArtifactValidationInput<'_>) -> bool,
    ) -> Result<GovernanceActivationResult, GovernanceRepositoryError> {
        match self {
            Self::Sqlite(repository) => repository.activate_revision(request, validate_artifact),
            Self::Postgres {
                repository,
                runtime,
            } => runtime
                .block_on(repository.governance_activate_revision(request, validate_artifact)),
        }
    }

    pub(super) fn append_audit_outbox(
        &self,
        command: AuditOutboxWriteCommand,
    ) -> Result<(), GovernanceRepositoryError> {
        match self {
            Self::Sqlite(repository) => repository.append_audit_outbox(command),
            Self::Postgres {
                repository,
                runtime,
            } => runtime.block_on(repository.governance_append_audit_outbox(command)),
        }
    }

    pub(super) fn outbox_health(
        &self,
        tenant_id: prodex_domain::TenantId,
    ) -> Result<GovernanceOutboxHealth, GovernanceRepositoryError> {
        match self {
            Self::Sqlite(repository) => repository.outbox_health(tenant_id),
            Self::Postgres {
                repository,
                runtime,
            } => runtime.block_on(repository.governance_outbox_health(tenant_id)),
        }
    }

    pub(super) fn audit_integrity_health(
        &self,
        tenant_id: prodex_domain::TenantId,
    ) -> Result<GovernanceAuditIntegrityHealth, GovernanceRepositoryError> {
        match self {
            Self::Sqlite(repository) => repository.audit_integrity_health(tenant_id),
            Self::Postgres {
                repository,
                runtime,
            } => runtime.block_on(repository.governance_audit_integrity_health(tenant_id)),
        }
    }

    pub(super) fn export_audit(
        &self,
        tenant_id: prodex_domain::TenantId,
        limit: u16,
    ) -> Result<Vec<GovernanceAuditExportRecord>, GovernanceRepositoryError> {
        match self {
            Self::Sqlite(repository) => repository.governance_export_audit(tenant_id, limit),
            Self::Postgres {
                repository,
                runtime,
            } => runtime.block_on(repository.governance_export_audit(tenant_id, limit)),
        }
    }
}

impl ApplicationGovernanceRepository for RuntimeGovernanceRepository<'_> {
    fn write_revision_idempotent(
        &self,
        command: GovernanceRevisionWriteCommand,
        audit_outbox: AuditOutboxWriteCommand,
        idempotency: GovernanceMutationIdempotency,
    ) -> Result<GovernanceWriteOutcome, GovernanceRepositoryError> {
        RuntimeGovernanceRepository::write_revision_idempotent(
            self,
            command,
            audit_outbox,
            idempotency,
        )
    }

    fn create_approval(
        &self,
        approval: ApprovalRecord,
        audit_outbox: AuditOutboxWriteCommand,
    ) -> Result<GovernanceWriteOutcome, GovernanceRepositoryError> {
        RuntimeGovernanceRepository::create_approval(self, approval, audit_outbox)
    }

    fn create_approval_idempotent(
        &self,
        approval: ApprovalRecord,
        audit_outbox: AuditOutboxWriteCommand,
        idempotency: GovernanceMutationIdempotency,
    ) -> Result<GovernanceWriteOutcome, GovernanceRepositoryError> {
        RuntimeGovernanceRepository::create_approval_idempotent(
            self,
            approval,
            audit_outbox,
            idempotency,
        )
    }

    fn transition_approval(
        &self,
        request: ApprovalVoteRequest,
        action: ApprovalAction,
    ) -> Result<ApprovalRecord, GovernanceRepositoryError> {
        RuntimeGovernanceRepository::transition_approval(self, request, action)
    }

    fn transition_approval_idempotent(
        &self,
        request: ApprovalVoteRequest,
        action: ApprovalAction,
        idempotency: ApprovalVoteIdempotency,
    ) -> Result<ApprovalVoteMutationOutcome, GovernanceRepositoryError> {
        RuntimeGovernanceRepository::transition_approval_idempotent(
            self,
            request,
            action,
            idempotency,
        )
    }

    fn get_approval(
        &self,
        tenant_id: prodex_domain::TenantId,
        approval_id: &ApprovalId,
    ) -> Result<ApprovalRecord, GovernanceRepositoryError> {
        RuntimeGovernanceRepository::get_approval(self, tenant_id, approval_id)
    }

    fn activate_revision(
        &self,
        request: GovernanceActivationRequest,
        validate_artifact: &mut dyn FnMut(&GovernanceArtifactValidationInput<'_>) -> bool,
    ) -> Result<GovernanceActivationResult, GovernanceRepositoryError> {
        RuntimeGovernanceRepository::activate_revision(self, request, validate_artifact)
    }
}

pub(in crate::runtime_launch::proxy_startup) fn repository(
    shared: &RuntimeLocalRewriteProxyShared,
) -> Result<RuntimeGovernanceRepository<'_>, tiny_http::ResponseBox> {
    runtime_governance_repository(shared).map_err(repository_error)
}
pub(in crate::runtime_launch::proxy_startup) fn runtime_governance_repository(
    shared: &RuntimeLocalRewriteProxyShared,
) -> Result<RuntimeGovernanceRepository<'_>, GovernanceRepositoryError> {
    match &shared.gateway_state_store {
        RuntimeGatewayStateStore::Sqlite { path } => GovernanceSqliteRepository::open(path)
            .map(RuntimeGovernanceRepository::Sqlite)
            .map_err(|_| GovernanceRepositoryError::Database),
        RuntimeGatewayStateStore::Postgres { .. } => shared
            .gateway_postgres_repository
            .as_ref()
            .map(|repository| RuntimeGovernanceRepository::Postgres {
                repository,
                runtime: shared.runtime_shared.async_runtime.handle().clone(),
            })
            .ok_or(GovernanceRepositoryError::Database),
        RuntimeGatewayStateStore::File { .. } | RuntimeGatewayStateStore::Redis { .. } => {
            Err(GovernanceRepositoryError::Unsupported)
        }
    }
}

pub(in crate::runtime_launch::proxy_startup) fn storage_unavailable() -> tiny_http::ResponseBox {
    build_runtime_proxy_json_error_response(
        503,
        "governance_policy_storage_unavailable",
        "policy governance storage is temporarily unavailable",
    )
}
