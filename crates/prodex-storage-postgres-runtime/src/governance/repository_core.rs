use super::*;

impl PostgresRepository {
    pub async fn governance_list_tenant_ids(
        &self,
        limit: u16,
    ) -> Result<Vec<TenantId>, GovernanceRepositoryError> {
        if limit == 0 {
            return Err(GovernanceRepositoryError::InvalidInput);
        }
        self.governance_timeout(self.governance_list_tenant_ids_inner(limit))
            .await
    }

    async fn governance_list_tenant_ids_inner(
        &self,
        limit: u16,
    ) -> Result<Vec<TenantId>, GovernanceRepositoryError> {
        let client = self.pool.get().await.map_err(database_error)?;
        client
            .query(
                "SELECT tenant_id FROM prodex_tenants ORDER BY tenant_id ASC LIMIT $1",
                &[&i64::from(limit)],
            )
            .await
            .map_err(database_error)?
            .into_iter()
            .map(|row| {
                let tenant_id = row.try_get::<_, uuid::Uuid>(0).map_err(database_error)?;
                Ok(TenantId::from_uuid(tenant_id))
            })
            .collect()
    }

    pub async fn governance_write_revision(
        &self,
        command: GovernanceRevisionWriteCommand,
        audit_outbox: AuditOutboxWriteCommand,
    ) -> Result<GovernanceWriteOutcome, GovernanceRepositoryError> {
        self.governance_timeout(self.governance_write_revision_inner(command, audit_outbox, None))
            .await
    }

    pub async fn governance_write_revision_idempotent(
        &self,
        command: GovernanceRevisionWriteCommand,
        audit_outbox: AuditOutboxWriteCommand,
        idempotency: GovernanceMutationIdempotency,
    ) -> Result<GovernanceWriteOutcome, GovernanceRepositoryError> {
        self.governance_timeout(self.governance_write_revision_inner(
            command,
            audit_outbox,
            Some(idempotency),
        ))
        .await
    }

    async fn governance_write_revision_inner(
        &self,
        command: GovernanceRevisionWriteCommand,
        audit_outbox: AuditOutboxWriteCommand,
        idempotency: Option<GovernanceMutationIdempotency>,
    ) -> Result<GovernanceWriteOutcome, GovernanceRepositoryError> {
        plan_governance_revision_write(command.clone())
            .map_err(|_| GovernanceRepositoryError::InvalidInput)?;
        validate_governance_revision_id(command.kind, &command.revision_id)?;
        let checksum = artifact_checksum(&command.compiled_artifact);
        if command.fingerprint.as_str() != checksum {
            return Err(GovernanceRepositoryError::InvalidInput);
        }
        let created_at = to_i64(command.created_at_unix_ms)?;
        let completed_at_unix_ms = audit_outbox.audit.event.occurred_at_unix_ms;
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, command.tenant_id)
            .await
            .map_err(database_error)?;
        if let Some(idempotency) = idempotency.as_ref() {
            match governance_idempotency_replay_postgres(
                &transaction,
                command.tenant_id,
                idempotency,
            )
            .await?
            {
                IdempotencyReplayDecision::ExecuteAndRecordPending => {
                    insert_governance_idempotency_pending_postgres(
                        &transaction,
                        command.tenant_id,
                        idempotency,
                    )
                    .await?;
                }
                IdempotencyReplayDecision::AlreadyInProgress { .. } => {
                    return Err(GovernanceRepositoryError::Conflict);
                }
                IdempotencyReplayDecision::Replay(response) => {
                    transaction.commit().await.map_err(database_error)?;
                    return if response == GOVERNANCE_REVISION_WRITE_IDEMPOTENCY_RESPONSE {
                        Ok(GovernanceWriteOutcome::Replayed)
                    } else {
                        Err(GovernanceRepositoryError::InvalidInput)
                    };
                }
            }
        }

        if let Some(existing) = load_revision_row(
            &transaction,
            command.tenant_id,
            command.kind,
            &command.revision_id,
        )
        .await?
        {
            if existing.checksum == checksum
                && existing.compiled_artifact == command.compiled_artifact
                && existing.authenticity == command.authenticity
                && existing.created_by == command.created_by
                && existing.created_at_unix_ms == command.created_at_unix_ms
            {
                if let Some(idempotency) = idempotency.as_ref() {
                    complete_governance_idempotency_postgres(
                        &transaction,
                        command.tenant_id,
                        idempotency,
                        GOVERNANCE_REVISION_WRITE_IDEMPOTENCY_RESPONSE,
                        completed_at_unix_ms,
                    )
                    .await?;
                }
                transaction.commit().await.map_err(database_error)?;
                return Ok(GovernanceWriteOutcome::Replayed);
            }
            return Err(GovernanceRepositoryError::Conflict);
        }

        insert_revision_metadata(&transaction, &command, &checksum, created_at).await?;
        let (signature_key_id, artifact_signature) =
            command
                .authenticity
                .as_ref()
                .map_or((None, None), |authenticity| {
                    (
                        Some(authenticity.key_id.as_str()),
                        Some(authenticity.signature_base64.as_str()),
                    )
                });
        let statement = transaction
            .prepare_cached(INSERT_GOVERNANCE_REVISION_ARTIFACT_STATEMENT.sql)
            .await
            .map_err(database_error)?;
        let inserted = transaction
            .query_opt(
                &statement,
                &[
                    &command.tenant_id.as_uuid(),
                    &artifact_kind_label(command.kind),
                    &command.revision_id,
                    &checksum,
                    &command.compiled_artifact,
                    &signature_key_id,
                    &artifact_signature,
                    &command.created_by.as_uuid(),
                    &created_at,
                ],
            )
            .await
            .map_err(database_error)?;
        if inserted.is_none() {
            return Err(GovernanceRepositoryError::Conflict);
        }
        append_audit_outbox_tx(&transaction, audit_outbox).await?;
        if let Some(idempotency) = idempotency.as_ref() {
            complete_governance_idempotency_postgres(
                &transaction,
                command.tenant_id,
                idempotency,
                GOVERNANCE_REVISION_WRITE_IDEMPOTENCY_RESPONSE,
                completed_at_unix_ms,
            )
            .await?;
        }
        transaction.commit().await.map_err(database_error)?;
        Ok(GovernanceWriteOutcome::Applied)
    }

    pub async fn governance_create_approval(
        &self,
        approval: ApprovalRecord,
        audit_outbox: AuditOutboxWriteCommand,
    ) -> Result<GovernanceWriteOutcome, GovernanceRepositoryError> {
        self.governance_timeout(self.governance_create_approval_inner(approval, audit_outbox, None))
            .await
    }

    pub async fn governance_create_approval_idempotent(
        &self,
        approval: ApprovalRecord,
        audit_outbox: AuditOutboxWriteCommand,
        idempotency: GovernanceMutationIdempotency,
    ) -> Result<GovernanceWriteOutcome, GovernanceRepositoryError> {
        self.governance_timeout(self.governance_create_approval_inner(
            approval,
            audit_outbox,
            Some(idempotency),
        ))
        .await
    }

    async fn governance_create_approval_inner(
        &self,
        approval: ApprovalRecord,
        audit_outbox: AuditOutboxWriteCommand,
        idempotency: Option<GovernanceMutationIdempotency>,
    ) -> Result<GovernanceWriteOutcome, GovernanceRepositoryError> {
        if approval.state != ApprovalState::PendingApproval
            || approval.version != 1
            || !approval.votes.is_empty()
        {
            return Err(GovernanceRepositoryError::InvalidInput);
        }
        let expires_at = to_i64(approval.expires_at_unix_ms)?;
        let version = to_i64(approval.version)?;
        let completed_at_unix_ms = audit_outbox.audit.event.occurred_at_unix_ms;
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, approval.tenant_id)
            .await
            .map_err(database_error)?;
        if let Some(idempotency) = idempotency.as_ref() {
            match governance_idempotency_replay_postgres(
                &transaction,
                approval.tenant_id,
                idempotency,
            )
            .await?
            {
                IdempotencyReplayDecision::ExecuteAndRecordPending => {
                    insert_governance_idempotency_pending_postgres(
                        &transaction,
                        approval.tenant_id,
                        idempotency,
                    )
                    .await?;
                }
                IdempotencyReplayDecision::AlreadyInProgress { .. } => {
                    return Err(GovernanceRepositoryError::Conflict);
                }
                IdempotencyReplayDecision::Replay(response) => {
                    transaction.commit().await.map_err(database_error)?;
                    return if response == GOVERNANCE_APPROVAL_CREATE_IDEMPOTENCY_RESPONSE {
                        Ok(GovernanceWriteOutcome::Replayed)
                    } else {
                        Err(GovernanceRepositoryError::InvalidInput)
                    };
                }
            }
        }
        let kind = approval_artifact_kind(approval.kind)?;
        let revision_id = if let Some(kind) = kind {
            Some(
                revision_id_for_fingerprint(
                    &transaction,
                    approval.tenant_id,
                    kind,
                    approval.fingerprint.as_str(),
                )
                .await?
                .ok_or(GovernanceRepositoryError::NotFound)?,
            )
        } else {
            None
        };

        if let Some(existing) =
            load_approval_tx(&transaction, approval.tenant_id, &approval.id).await?
        {
            if existing == approval {
                if let Some(idempotency) = idempotency.as_ref() {
                    complete_governance_idempotency_postgres(
                        &transaction,
                        approval.tenant_id,
                        idempotency,
                        GOVERNANCE_APPROVAL_CREATE_IDEMPOTENCY_RESPONSE,
                        completed_at_unix_ms,
                    )
                    .await?;
                }
                transaction.commit().await.map_err(database_error)?;
                return Ok(GovernanceWriteOutcome::Replayed);
            }
            return Err(GovernanceRepositoryError::Conflict);
        }
        transaction
            .execute(
                "INSERT INTO prodex_approvals (
                    tenant_id, approval_id, approval_kind, approval_scope, fingerprint,
                    maker_id, lifecycle_state, required_quorum, expires_at_unix_ms,
                    activated_at_unix_ms, termination_reason, resource_version
                 ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NULL, NULL, $10)",
                &[
                    &approval.tenant_id.as_uuid(),
                    &approval.id.as_str(),
                    &approval_kind_label(approval.kind),
                    &approval.scope.as_str(),
                    &approval.fingerprint.as_str(),
                    &approval.maker.as_uuid(),
                    &approval_state_label(approval.state),
                    &i16::from(approval.required_quorum),
                    &expires_at,
                    &version,
                ],
            )
            .await
            .map_err(database_error)?;
        if let (Some(kind), Some(revision_id)) = (kind, revision_id.as_deref()) {
            update_revision_state(
                &transaction,
                approval.tenant_id,
                kind,
                revision_id,
                "pending_approval",
            )
            .await?;
        }
        append_audit_outbox_tx(&transaction, audit_outbox).await?;
        if let Some(idempotency) = idempotency.as_ref() {
            complete_governance_idempotency_postgres(
                &transaction,
                approval.tenant_id,
                idempotency,
                GOVERNANCE_APPROVAL_CREATE_IDEMPOTENCY_RESPONSE,
                completed_at_unix_ms,
            )
            .await?;
        }
        transaction.commit().await.map_err(database_error)?;
        Ok(GovernanceWriteOutcome::Applied)
    }

    pub async fn governance_transition_approval(
        &self,
        request: ApprovalVoteRequest,
        action: ApprovalAction,
    ) -> Result<ApprovalRecord, GovernanceRepositoryError> {
        match self
            .governance_timeout(self.governance_transition_approval_inner(request, action, None))
            .await?
        {
            ApprovalVoteMutationOutcome::Applied(approval) => Ok(approval),
            ApprovalVoteMutationOutcome::Replayed(_) => {
                Err(GovernanceRepositoryError::InvalidInput)
            }
        }
    }

    pub async fn governance_transition_approval_idempotent(
        &self,
        request: ApprovalVoteRequest,
        action: ApprovalAction,
        idempotency: ApprovalVoteIdempotency,
    ) -> Result<ApprovalVoteMutationOutcome, GovernanceRepositoryError> {
        self.governance_timeout(self.governance_transition_approval_inner(
            request,
            action,
            Some(idempotency),
        ))
        .await
    }

    async fn governance_transition_approval_inner(
        &self,
        request: ApprovalVoteRequest,
        action: ApprovalAction,
        idempotency: Option<ApprovalVoteIdempotency>,
    ) -> Result<ApprovalVoteMutationOutcome, GovernanceRepositoryError> {
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, request.tenant_id)
            .await
            .map_err(database_error)?;
        if let Some(idempotency) = idempotency.as_ref() {
            match governance_idempotency_replay_postgres(
                &transaction,
                request.tenant_id,
                idempotency,
            )
            .await?
            {
                IdempotencyReplayDecision::ExecuteAndRecordPending => {
                    insert_governance_idempotency_pending_postgres(
                        &transaction,
                        request.tenant_id,
                        idempotency,
                    )
                    .await?;
                }
                IdempotencyReplayDecision::AlreadyInProgress { .. } => {
                    return Err(GovernanceRepositoryError::Conflict);
                }
                IdempotencyReplayDecision::Replay(response) => {
                    transaction.commit().await.map_err(database_error)?;
                    return ApprovalVoteStableOutcome::replay(&response);
                }
            }
        }
        let current = load_approval_tx(&transaction, request.tenant_id, &request.approval_id)
            .await?
            .ok_or(GovernanceRepositoryError::NotFound)?;
        let transition = match plan_approval_vote_transition(&current, &request, action)? {
            ApprovalVoteTransitionDecision::Transition(transition) => transition,
            ApprovalVoteTransitionDecision::Denied(denial) => {
                append_audit_outbox_tx(
                    &transaction,
                    denied_approval_audit_outbox(request.audit_outbox, denial),
                )
                .await?;
                if let Some(idempotency) = idempotency.as_ref() {
                    let response = ApprovalVoteStableOutcome::Denied(denial).encode();
                    complete_governance_idempotency_postgres(
                        &transaction,
                        request.tenant_id,
                        idempotency,
                        &response,
                        request.now_unix_ms,
                    )
                    .await?;
                }
                transaction.commit().await.map_err(database_error)?;
                return Err(denial.repository_error());
            }
        };
        if !transition.changed {
            if let Some(idempotency) = idempotency.as_ref() {
                let response = ApprovalVoteStableOutcome::Success(
                    ApprovalVoteSnapshot::from_record(&transition.record),
                )
                .encode();
                complete_governance_idempotency_postgres(
                    &transaction,
                    request.tenant_id,
                    idempotency,
                    &response,
                    request.now_unix_ms,
                )
                .await?;
            }
            transaction.commit().await.map_err(database_error)?;
            return Ok(ApprovalVoteMutationOutcome::Applied(transition.record));
        }
        persist_approval_transition(&transaction, &current, &transition.record).await?;
        if let Some(update) = plan_approval_revision_lifecycle_update(&transition.record)? {
            let revision_id = revision_id_for_fingerprint(
                &transaction,
                transition.record.tenant_id,
                update.kind,
                transition.record.fingerprint.as_str(),
            )
            .await?
            .ok_or(GovernanceRepositoryError::NotFound)?;
            update_revision_state(
                &transaction,
                transition.record.tenant_id,
                update.kind,
                &revision_id,
                update.state,
            )
            .await?;
        }
        append_audit_outbox_tx(&transaction, request.audit_outbox).await?;
        if let Some(idempotency) = idempotency.as_ref() {
            let response = ApprovalVoteStableOutcome::Success(ApprovalVoteSnapshot::from_record(
                &transition.record,
            ))
            .encode();
            complete_governance_idempotency_postgres(
                &transaction,
                request.tenant_id,
                idempotency,
                &response,
                request.now_unix_ms,
            )
            .await?;
        }
        transaction.commit().await.map_err(database_error)?;
        Ok(ApprovalVoteMutationOutcome::Applied(transition.record))
    }

    pub async fn governance_list_revisions(
        &self,
        tenant_id: TenantId,
        kind: GovernanceArtifactKind,
    ) -> Result<Vec<GovernanceRevisionSummary>, GovernanceRepositoryError> {
        self.governance_timeout(self.governance_list_revisions_inner(tenant_id, kind))
            .await
    }

    async fn governance_list_revisions_inner(
        &self,
        tenant_id: TenantId,
        kind: GovernanceArtifactKind,
    ) -> Result<Vec<GovernanceRevisionSummary>, GovernanceRepositoryError> {
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, tenant_id)
            .await
            .map_err(database_error)?;
        let kind_label = artifact_kind_label(kind);
        let query = format!(
            "SELECT revisions.revision_id, revisions.artifact_checksum,
                    revisions.lifecycle_state, revisions.created_at_unix_ms,
                    artifacts.signature_key_id
             FROM {} AS revisions
             LEFT JOIN prodex_governance_revision_artifacts AS artifacts
               ON artifacts.tenant_id = revisions.tenant_id
              AND artifacts.artifact_kind = $2
              AND artifacts.revision_id = revisions.revision_id::text
             WHERE revisions.tenant_id = $1
             ORDER BY revisions.created_at_unix_ms DESC, revisions.revision_id DESC",
            revision_table(kind)
        );
        let rows = transaction
            .query(&query, &[&tenant_id.as_uuid(), &kind_label])
            .await
            .map_err(database_error)?;
        let summaries = rows
            .into_iter()
            .map(|row| {
                Ok(GovernanceRevisionSummary {
                    revision_id: revision_id_from_row(&row, 0, kind),
                    fingerprint: row.get(1),
                    lifecycle_state: row.get(2),
                    signature_key_id: row.get(4),
                    created_at_unix_ms: from_i64(row.get(3))?,
                })
            })
            .collect::<Result<Vec<_>, GovernanceRepositoryError>>()?;
        transaction.commit().await.map_err(database_error)?;
        Ok(summaries)
    }

    pub async fn governance_get_revision(
        &self,
        tenant_id: TenantId,
        kind: GovernanceArtifactKind,
        revision_id: &str,
    ) -> Result<GovernanceRevisionSummary, GovernanceRepositoryError> {
        if kind == GovernanceArtifactKind::Policy {
            policy_revision_id(revision_id)?;
        }
        self.governance_timeout(self.governance_get_revision_inner(
            tenant_id,
            kind,
            revision_id.to_string(),
        ))
        .await
    }

    async fn governance_get_revision_inner(
        &self,
        tenant_id: TenantId,
        kind: GovernanceArtifactKind,
        revision_id: String,
    ) -> Result<GovernanceRevisionSummary, GovernanceRepositoryError> {
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, tenant_id)
            .await
            .map_err(database_error)?;
        let kind_label = artifact_kind_label(kind);
        let query = format!(
            "SELECT revisions.revision_id, revisions.artifact_checksum,
                    revisions.lifecycle_state, revisions.created_at_unix_ms,
                    artifacts.signature_key_id
             FROM {} AS revisions
             LEFT JOIN prodex_governance_revision_artifacts AS artifacts
               ON artifacts.tenant_id = revisions.tenant_id
              AND artifacts.artifact_kind = $3
              AND artifacts.revision_id = revisions.revision_id::text
             WHERE revisions.tenant_id = $1 AND revisions.revision_id = $2",
            revision_table(kind)
        );
        let row = if kind == GovernanceArtifactKind::Policy {
            let revision_id = policy_revision_id(&revision_id)?;
            transaction
                .query_opt(
                    &query,
                    &[&tenant_id.as_uuid(), &revision_id.as_uuid(), &kind_label],
                )
                .await
        } else {
            transaction
                .query_opt(&query, &[&tenant_id.as_uuid(), &revision_id, &kind_label])
                .await
        }
        .map_err(database_error)?
        .ok_or(GovernanceRepositoryError::NotFound)?;
        let summary = GovernanceRevisionSummary {
            revision_id: revision_id_from_row(&row, 0, kind),
            fingerprint: row.get(1),
            lifecycle_state: row.get(2),
            signature_key_id: row.get(4),
            created_at_unix_ms: from_i64(row.get(3))?,
        };
        transaction.commit().await.map_err(database_error)?;
        Ok(summary)
    }

    pub async fn governance_get_approval(
        &self,
        tenant_id: TenantId,
        approval_id: ApprovalId,
    ) -> Result<ApprovalRecord, GovernanceRepositoryError> {
        self.governance_timeout(self.governance_get_approval_inner(tenant_id, approval_id))
            .await
    }

    async fn governance_get_approval_inner(
        &self,
        tenant_id: TenantId,
        approval_id: ApprovalId,
    ) -> Result<ApprovalRecord, GovernanceRepositoryError> {
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, tenant_id)
            .await
            .map_err(database_error)?;
        let approval = load_approval_tx(&transaction, tenant_id, &approval_id)
            .await?
            .ok_or(GovernanceRepositoryError::NotFound)?;
        transaction.commit().await.map_err(database_error)?;
        Ok(approval)
    }

    pub async fn governance_list_execution_approvals(
        &self,
        tenant_id: TenantId,
    ) -> Result<Vec<ApprovalRecord>, GovernanceRepositoryError> {
        self.governance_list_approvals(tenant_id, prodex_domain::ApprovalKind::Execution)
            .await
    }

    pub async fn governance_list_approvals(
        &self,
        tenant_id: TenantId,
        kind: prodex_domain::ApprovalKind,
    ) -> Result<Vec<ApprovalRecord>, GovernanceRepositoryError> {
        self.governance_timeout(self.governance_list_approvals_inner(tenant_id, kind))
            .await
    }

    async fn governance_list_approvals_inner(
        &self,
        tenant_id: TenantId,
        kind: prodex_domain::ApprovalKind,
    ) -> Result<Vec<ApprovalRecord>, GovernanceRepositoryError> {
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, tenant_id)
            .await
            .map_err(database_error)?;
        let rows = transaction
            .query(
                "SELECT approval_id FROM prodex_approvals
                 WHERE tenant_id = $1 AND approval_kind = $2
                 ORDER BY expires_at_unix_ms DESC, approval_id DESC",
                &[
                    &tenant_id.as_uuid(),
                    &prodex_storage::governance_support::approval_kind_label(kind),
                ],
            )
            .await
            .map_err(database_error)?;
        let mut approvals = Vec::with_capacity(rows.len());
        for row in rows {
            let id = ApprovalId::new(row.get::<_, String>(0))
                .map_err(|_| GovernanceRepositoryError::Database)?;
            approvals.push(
                load_approval_tx(&transaction, tenant_id, &id)
                    .await?
                    .ok_or(GovernanceRepositoryError::Database)?,
            );
        }
        transaction.commit().await.map_err(database_error)?;
        Ok(approvals)
    }

    pub async fn governance_status(
        &self,
        tenant_id: TenantId,
        kind: GovernanceArtifactKind,
    ) -> Result<GovernanceStatus, GovernanceRepositoryError> {
        self.governance_timeout(self.governance_status_inner(tenant_id, kind))
            .await
    }

    async fn governance_status_inner(
        &self,
        tenant_id: TenantId,
        kind: GovernanceArtifactKind,
    ) -> Result<GovernanceStatus, GovernanceRepositoryError> {
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, tenant_id)
            .await
            .map_err(database_error)?;
        let pointer = load_pointer_for_kind(&transaction, tenant_id, kind).await?;
        transaction.commit().await.map_err(database_error)?;
        Ok(
            pointer.map_or_else(GovernanceStatus::default, |pointer| GovernanceStatus {
                active_revision_id: pointer.active_revision_id,
                last_known_good_revision_id: pointer.last_known_good_revision_id,
                etag: Some(pointer.etag),
            }),
        )
    }

    pub async fn governance_load_snapshot<F>(
        &self,
        tenant_id: TenantId,
        kind: GovernanceArtifactKind,
        validate_artifact: F,
    ) -> Result<GovernanceSnapshot, GovernanceRepositoryError>
    where
        F: FnMut(&GovernanceArtifactValidationInput<'_>) -> bool,
    {
        self.governance_timeout(self.governance_load_snapshot_inner(
            tenant_id,
            kind,
            validate_artifact,
        ))
        .await
    }

    async fn governance_load_snapshot_inner<F>(
        &self,
        tenant_id: TenantId,
        kind: GovernanceArtifactKind,
        mut validate_artifact: F,
    ) -> Result<GovernanceSnapshot, GovernanceRepositoryError>
    where
        F: FnMut(&GovernanceArtifactValidationInput<'_>) -> bool,
    {
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, tenant_id)
            .await
            .map_err(database_error)?;
        let pointer = load_pointer_for_kind(&transaction, tenant_id, kind)
            .await?
            .ok_or(GovernanceRepositoryError::SnapshotUnavailable)?;
        let active = pointer
            .active_revision_id
            .as_deref()
            .ok_or(GovernanceRepositoryError::SnapshotUnavailable)?;
        if let Some(snapshot) = load_verified_snapshot(
            &transaction,
            tenant_id,
            kind,
            active,
            GovernanceSnapshotSource::Active,
            &mut validate_artifact,
        )
        .await?
        {
            transaction.commit().await.map_err(database_error)?;
            return Ok(snapshot);
        }
        let last_known_good = pointer
            .last_known_good_revision_id
            .as_deref()
            .ok_or(GovernanceRepositoryError::SnapshotUnavailable)?;
        if last_known_good == active {
            return Err(GovernanceRepositoryError::SnapshotUnavailable);
        }
        let snapshot = load_verified_snapshot(
            &transaction,
            tenant_id,
            kind,
            last_known_good,
            GovernanceSnapshotSource::LastKnownGood,
            &mut validate_artifact,
        )
        .await?
        .ok_or(GovernanceRepositoryError::SnapshotUnavailable)?;
        transaction.commit().await.map_err(database_error)?;
        Ok(snapshot)
    }

    pub async fn governance_latest_audit_digest(
        &self,
        tenant_id: TenantId,
    ) -> Result<Option<AuditDigest>, GovernanceRepositoryError> {
        self.governance_timeout(self.governance_latest_audit_digest_inner(tenant_id))
            .await
    }

    async fn governance_latest_audit_digest_inner(
        &self,
        tenant_id: TenantId,
    ) -> Result<Option<AuditDigest>, GovernanceRepositoryError> {
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, tenant_id)
            .await
            .map_err(database_error)?;
        let value = latest_audit_digest_tx(&transaction, tenant_id).await?;
        transaction.commit().await.map_err(database_error)?;
        value
            .map(|value| AuditDigest::new(value).map_err(|_| GovernanceRepositoryError::Database))
            .transpose()
    }

    pub async fn governance_outbox_health(
        &self,
        tenant_id: TenantId,
    ) -> Result<GovernanceOutboxHealth, GovernanceRepositoryError> {
        self.governance_timeout(self.governance_outbox_health_inner(tenant_id))
            .await
    }

    async fn governance_outbox_health_inner(
        &self,
        tenant_id: TenantId,
    ) -> Result<GovernanceOutboxHealth, GovernanceRepositoryError> {
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, tenant_id)
            .await
            .map_err(database_error)?;
        let row = transaction
            .query_one(
                "SELECT COUNT(*), MIN(created_at_unix_ms) FROM prodex_siem_outbox
                 WHERE tenant_id = $1 AND delivered_at_unix_ms IS NULL",
                &[&tenant_id.as_uuid()],
            )
            .await
            .map_err(database_error)?;
        let dead_lettered = transaction
            .query_one(
                "SELECT COUNT(*) FROM prodex_siem_dead_letters WHERE tenant_id = $1",
                &[&tenant_id.as_uuid()],
            )
            .await
            .map_err(database_error)?
            .get::<_, i64>(0);
        let health = GovernanceOutboxHealth {
            pending: from_i64(row.get(0))?,
            dead_lettered: from_i64(dead_lettered)?,
            oldest_pending_at_unix_ms: row.get::<_, Option<i64>>(1).map(from_i64).transpose()?,
        };
        transaction.commit().await.map_err(database_error)?;
        Ok(health)
    }

    pub async fn governance_audit_integrity_health(
        &self,
        tenant_id: TenantId,
    ) -> Result<GovernanceAuditIntegrityHealth, GovernanceRepositoryError> {
        self.governance_timeout(self.governance_audit_integrity_health_inner(tenant_id))
            .await
    }

    async fn governance_audit_integrity_health_inner(
        &self,
        tenant_id: TenantId,
    ) -> Result<GovernanceAuditIntegrityHealth, GovernanceRepositoryError> {
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, tenant_id)
            .await
            .map_err(database_error)?;
        let rows = transaction
            .query(
                "SELECT audit_event_id, occurred_at_unix_ms, principal_id, action,
                        resource_kind, resource_id, outcome, reason_code,
                        previous_digest, event_digest
                 FROM prodex_audit_log WHERE tenant_id = $1",
                &[&tenant_id.as_uuid()],
            )
            .await
            .map_err(database_error)?;
        let records = rows
            .into_iter()
            .map(|row| governance_audit_export_record(&row))
            .collect::<Result<Vec<_>, GovernanceRepositoryError>>()?;
        let anchor = transaction
            .query_opt(
                "SELECT last_purged_digest FROM prodex_audit_retention_anchors
                 WHERE tenant_id = $1",
                &[&tenant_id.as_uuid()],
            )
            .await
            .map_err(database_error)?
            .map(|row| AuditDigest::new(row.get::<_, String>(0)))
            .transpose()
            .map_err(|_| GovernanceRepositoryError::Database)?;
        let health = verify_governance_audit_integrity_with_retention_anchor(
            tenant_id,
            &records,
            anchor.as_ref(),
        );
        transaction.commit().await.map_err(database_error)?;
        Ok(health)
    }

    pub async fn governance_export_audit(
        &self,
        tenant_id: TenantId,
        limit: u16,
    ) -> Result<Vec<GovernanceAuditExportRecord>, GovernanceRepositoryError> {
        self.governance_timeout(self.governance_export_audit_inner(tenant_id, limit))
            .await
    }

    async fn governance_export_audit_inner(
        &self,
        tenant_id: TenantId,
        limit: u16,
    ) -> Result<Vec<GovernanceAuditExportRecord>, GovernanceRepositoryError> {
        if limit == 0 || limit > 1_000 {
            return Err(GovernanceRepositoryError::InvalidInput);
        }
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, tenant_id)
            .await
            .map_err(database_error)?;
        let rows = transaction
            .query(
                "SELECT audit_event_id, occurred_at_unix_ms, principal_id, action,
                        resource_kind, resource_id, outcome, reason_code,
                        previous_digest, event_digest
                 FROM prodex_audit_log WHERE tenant_id = $1
                 ORDER BY occurred_at_unix_ms DESC, audit_event_id DESC LIMIT $2",
                &[&tenant_id.as_uuid(), &i64::from(limit)],
            )
            .await
            .map_err(database_error)?;
        let records = rows
            .into_iter()
            .map(|row| governance_audit_export_record(&row))
            .collect::<Result<Vec<_>, GovernanceRepositoryError>>()?;
        transaction.commit().await.map_err(database_error)?;
        Ok(records)
    }
}

fn governance_audit_export_record(
    row: &Row,
) -> Result<GovernanceAuditExportRecord, GovernanceRepositoryError> {
    Ok(GovernanceAuditExportRecord {
        audit_event_id: row.get::<_, Uuid>(0).to_string(),
        occurred_at_unix_ms: from_i64(row.get(1))?,
        principal_id: row.get::<_, Uuid>(2).to_string(),
        action: row.get(3),
        resource_kind: row.get(4),
        resource_id: row.get(5),
        outcome: row.get(6),
        reason_code: row.get(7),
        previous_digest: row.get(8),
        event_digest: row.get(9),
    })
}
