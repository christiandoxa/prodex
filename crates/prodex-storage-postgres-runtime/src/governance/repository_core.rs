use super::*;

impl PostgresRepository {
    pub async fn governance_write_revision(
        &self,
        command: GovernanceRevisionWriteCommand,
        audit_outbox: AuditOutboxWriteCommand,
    ) -> Result<GovernanceWriteOutcome, GovernanceRepositoryError> {
        self.governance_timeout(self.governance_write_revision_inner(command, audit_outbox))
            .await
    }

    async fn governance_write_revision_inner(
        &self,
        command: GovernanceRevisionWriteCommand,
        audit_outbox: AuditOutboxWriteCommand,
    ) -> Result<GovernanceWriteOutcome, GovernanceRepositoryError> {
        plan_governance_revision_write(command.clone())
            .map_err(|_| GovernanceRepositoryError::InvalidInput)?;
        validate_governance_revision_id(command.kind, &command.revision_id)?;
        let checksum = artifact_checksum(&command.compiled_artifact);
        if command.fingerprint.as_str() != checksum {
            return Err(GovernanceRepositoryError::InvalidInput);
        }
        let created_at = to_i64(command.created_at_unix_ms)?;
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, command.tenant_id)
            .await
            .map_err(database_error)?;

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
                && existing.created_by == command.created_by
                && existing.created_at_unix_ms == command.created_at_unix_ms
            {
                transaction.commit().await.map_err(database_error)?;
                return Ok(GovernanceWriteOutcome::Replayed);
            }
            return Err(GovernanceRepositoryError::Conflict);
        }

        insert_revision_metadata(&transaction, &command, &checksum, created_at).await?;
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
        transaction.commit().await.map_err(database_error)?;
        Ok(GovernanceWriteOutcome::Applied)
    }

    pub async fn governance_create_approval(
        &self,
        approval: ApprovalRecord,
        audit_outbox: AuditOutboxWriteCommand,
    ) -> Result<GovernanceWriteOutcome, GovernanceRepositoryError> {
        self.governance_timeout(self.governance_create_approval_inner(approval, audit_outbox))
            .await
    }

    async fn governance_create_approval_inner(
        &self,
        approval: ApprovalRecord,
        audit_outbox: AuditOutboxWriteCommand,
    ) -> Result<GovernanceWriteOutcome, GovernanceRepositoryError> {
        if approval.state != ApprovalState::PendingApproval
            || approval.version != 1
            || !approval.votes.is_empty()
        {
            return Err(GovernanceRepositoryError::InvalidInput);
        }
        let expires_at = to_i64(approval.expires_at_unix_ms)?;
        let version = to_i64(approval.version)?;
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, approval.tenant_id)
            .await
            .map_err(database_error)?;
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
            match approval_idempotency_replay_postgres(&transaction, request.tenant_id, idempotency)
                .await?
            {
                IdempotencyReplayDecision::ExecuteAndRecordPending => {
                    insert_approval_idempotency_pending_postgres(
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
                    complete_approval_idempotency_postgres(
                        &transaction,
                        request.tenant_id,
                        idempotency,
                        ApprovalVoteStableOutcome::Denied(denial),
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
                complete_approval_idempotency_postgres(
                    &transaction,
                    request.tenant_id,
                    idempotency,
                    ApprovalVoteStableOutcome::Success(ApprovalVoteSnapshot::from_record(
                        &transition.record,
                    )),
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
            complete_approval_idempotency_postgres(
                &transaction,
                request.tenant_id,
                idempotency,
                ApprovalVoteStableOutcome::Success(ApprovalVoteSnapshot::from_record(
                    &transition.record,
                )),
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
        let query = format!(
            "SELECT revision_id, artifact_checksum, lifecycle_state, created_at_unix_ms
             FROM {} WHERE tenant_id = $1
             ORDER BY created_at_unix_ms DESC, revision_id DESC",
            revision_table(kind)
        );
        let rows = transaction
            .query(&query, &[&tenant_id.as_uuid()])
            .await
            .map_err(database_error)?;
        let summaries = rows
            .into_iter()
            .map(|row| {
                Ok(GovernanceRevisionSummary {
                    revision_id: revision_id_from_row(&row, 0, kind),
                    fingerprint: row.get(1),
                    lifecycle_state: row.get(2),
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
        let query = format!(
            "SELECT revision_id, artifact_checksum, lifecycle_state, created_at_unix_ms
             FROM {} WHERE tenant_id = $1 AND revision_id = $2",
            revision_table(kind)
        );
        let row = if kind == GovernanceArtifactKind::Policy {
            let revision_id = policy_revision_id(&revision_id)?;
            transaction
                .query_opt(&query, &[&tenant_id.as_uuid(), &revision_id.as_uuid()])
                .await
        } else {
            transaction
                .query_opt(&query, &[&tenant_id.as_uuid(), &revision_id])
                .await
        }
        .map_err(database_error)?
        .ok_or(GovernanceRepositoryError::NotFound)?;
        let summary = GovernanceRevisionSummary {
            revision_id: revision_id_from_row(&row, 0, kind),
            fingerprint: row.get(1),
            lifecycle_state: row.get(2),
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

    pub async fn governance_upsert_audit_legal_hold(
        &self,
        hold: AuditRetentionHold,
        created_by: PrincipalId,
        created_at_unix_ms: u64,
        audit: AuditOutboxWriteCommand,
    ) -> Result<(), GovernanceRepositoryError> {
        self.governance_timeout(self.governance_upsert_audit_legal_hold_inner(
            hold,
            created_by,
            created_at_unix_ms,
            audit,
        ))
        .await
    }

    async fn governance_upsert_audit_legal_hold_inner(
        &self,
        hold: AuditRetentionHold,
        created_by: PrincipalId,
        created_at_unix_ms: u64,
        audit: AuditOutboxWriteCommand,
    ) -> Result<(), GovernanceRepositoryError> {
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, hold.tenant_id)
            .await
            .map_err(database_error)?;
        let changed = transaction
            .execute(
                "INSERT INTO prodex_audit_legal_holds (
                    tenant_id, audit_event_id, reason_code, expires_at_unix_ms,
                    created_by, created_at_unix_ms
                 )
                 SELECT $1, $2, $3, $4, $5, $6
                 FROM prodex_audit_log
                 WHERE tenant_id = $1 AND audit_event_id = $2
                 ON CONFLICT (tenant_id, audit_event_id) DO UPDATE SET
                    reason_code = excluded.reason_code,
                    expires_at_unix_ms = excluded.expires_at_unix_ms,
                    created_by = excluded.created_by,
                    created_at_unix_ms = excluded.created_at_unix_ms",
                &[
                    &hold.tenant_id.as_uuid(),
                    &hold.event_id.as_uuid(),
                    &hold.reason_code.as_str(),
                    &hold
                        .expires_at
                        .map(AuditTimestamp::unix_ms)
                        .map(to_i64)
                        .transpose()?,
                    &created_by.as_uuid(),
                    &to_i64(created_at_unix_ms)?,
                ],
            )
            .await
            .map_err(database_error)?;
        if changed == 0 {
            return Err(GovernanceRepositoryError::NotFound);
        }
        append_audit_outbox_tx(&transaction, audit).await?;
        transaction.commit().await.map_err(database_error)?;
        Ok(())
    }

    pub async fn governance_list_audit_legal_holds(
        &self,
        tenant_id: TenantId,
    ) -> Result<Vec<AuditRetentionHold>, GovernanceRepositoryError> {
        self.governance_timeout(self.governance_list_audit_legal_holds_inner(tenant_id))
            .await
    }

    async fn governance_list_audit_legal_holds_inner(
        &self,
        tenant_id: TenantId,
    ) -> Result<Vec<AuditRetentionHold>, GovernanceRepositoryError> {
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, tenant_id)
            .await
            .map_err(database_error)?;
        let rows = transaction
            .query(
                "SELECT audit_event_id, reason_code, expires_at_unix_ms
                 FROM prodex_audit_legal_holds
                 WHERE tenant_id = $1
                 ORDER BY audit_event_id",
                &[&tenant_id.as_uuid()],
            )
            .await
            .map_err(database_error)?;
        let holds = rows
            .into_iter()
            .map(|row| {
                let expires_at_unix_ms = row.get::<_, Option<i64>>(2);
                Ok(AuditRetentionHold {
                    tenant_id,
                    event_id: AuditEventId::from_uuid(row.get(0)),
                    reason_code: AuditReasonCode::new(row.get::<_, String>(1))
                        .map_err(|_| GovernanceRepositoryError::Database)?,
                    expires_at: expires_at_unix_ms
                        .map(from_i64)
                        .transpose()?
                        .map(AuditTimestamp::new)
                        .transpose()
                        .map_err(|_| GovernanceRepositoryError::Database)?,
                })
            })
            .collect::<Result<Vec<_>, GovernanceRepositoryError>>()?;
        transaction.commit().await.map_err(database_error)?;
        Ok(holds)
    }

    pub async fn governance_delete_audit_legal_hold(
        &self,
        tenant_id: TenantId,
        event_id: AuditEventId,
        audit: AuditOutboxWriteCommand,
    ) -> Result<bool, GovernanceRepositoryError> {
        self.governance_timeout(
            self.governance_delete_audit_legal_hold_inner(tenant_id, event_id, audit),
        )
        .await
    }

    async fn governance_delete_audit_legal_hold_inner(
        &self,
        tenant_id: TenantId,
        event_id: AuditEventId,
        audit: AuditOutboxWriteCommand,
    ) -> Result<bool, GovernanceRepositoryError> {
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, tenant_id)
            .await
            .map_err(database_error)?;
        let changed = transaction
            .execute(
                "DELETE FROM prodex_audit_legal_holds
                 WHERE tenant_id = $1 AND audit_event_id = $2",
                &[&tenant_id.as_uuid(), &event_id.as_uuid()],
            )
            .await
            .map_err(database_error)?;
        if changed != 0 {
            append_audit_outbox_tx(&transaction, audit).await?;
        }
        transaction.commit().await.map_err(database_error)?;
        Ok(changed != 0)
    }

    pub async fn governance_purge_audit_events(
        &self,
        tenant_id: TenantId,
        event_ids: Vec<AuditEventId>,
        now_unix_ms: u64,
        cutoff_unix_ms: u64,
        audit: AuditOutboxWriteCommand,
    ) -> Result<Vec<AuditEventId>, GovernanceRepositoryError> {
        self.governance_timeout(self.governance_purge_audit_events_inner(
            tenant_id,
            event_ids,
            now_unix_ms,
            cutoff_unix_ms,
            audit,
        ))
        .await
    }

    async fn governance_purge_audit_events_inner(
        &self,
        tenant_id: TenantId,
        event_ids: Vec<AuditEventId>,
        now_unix_ms: u64,
        cutoff_unix_ms: u64,
        audit: AuditOutboxWriteCommand,
    ) -> Result<Vec<AuditEventId>, GovernanceRepositoryError> {
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, tenant_id)
            .await
            .map_err(database_error)?;
        transaction
            .execute(
                "DELETE FROM prodex_audit_legal_holds
                 WHERE tenant_id = $1
                   AND expires_at_unix_ms IS NOT NULL
                   AND expires_at_unix_ms < $2",
                &[&tenant_id.as_uuid(), &to_i64(now_unix_ms)?],
            )
            .await
            .map_err(database_error)?;
        transaction
            .execute(
                "SELECT set_config('prodex.audit_retention_purge_tenant', $1, true)",
                &[&tenant_id.to_string()],
            )
            .await
            .map_err(database_error)?;
        let ids = event_ids
            .iter()
            .map(|event_id| event_id.as_uuid())
            .collect::<Vec<_>>();
        let boundary_count = transaction
            .query_one(
                "SELECT COUNT(*)
                 FROM prodex_audit_log audit
                 WHERE audit.tenant_id = $1
                   AND (
                       audit.previous_digest IS NULL OR NOT EXISTS (
                           SELECT 1 FROM prodex_audit_log parent
                           WHERE parent.tenant_id = audit.tenant_id
                             AND parent.event_digest = audit.previous_digest
                       )
                   )",
                &[&tenant_id.as_uuid()],
            )
            .await
            .map_err(database_error)?
            .get::<_, i64>(0);
        if boundary_count > 1 {
            return Err(GovernanceRepositoryError::AuditChainConflict);
        }
        let chain = transaction
            .query(
                "WITH RECURSIVE chain(
                    audit_event_id, event_digest, occurred_at_unix_ms, depth
                 ) AS (
                    SELECT audit.audit_event_id, audit.event_digest,
                           audit.occurred_at_unix_ms, 1::BIGINT
                    FROM prodex_audit_log audit
                    WHERE audit.tenant_id = $1
                      AND (
                          audit.previous_digest IS NULL OR NOT EXISTS (
                              SELECT 1 FROM prodex_audit_log parent
                              WHERE parent.tenant_id = audit.tenant_id
                                AND parent.event_digest = audit.previous_digest
                          )
                      )
                    UNION ALL
                    SELECT child.audit_event_id, child.event_digest,
                           child.occurred_at_unix_ms, chain.depth + 1
                    FROM prodex_audit_log child
                    JOIN chain ON child.previous_digest = chain.event_digest
                    WHERE child.tenant_id = $1
                 )
                 SELECT chain.audit_event_id, chain.event_digest,
                        chain.occurred_at_unix_ms,
                        EXISTS (
                            SELECT 1 FROM prodex_audit_legal_holds hold
                            WHERE hold.tenant_id = $1
                              AND hold.audit_event_id = chain.audit_event_id
                        )
                 FROM chain
                 ORDER BY chain.depth
                 LIMIT $2",
                &[
                    &tenant_id.as_uuid(),
                    &i64::try_from(event_ids.len())
                        .map_err(|_| GovernanceRepositoryError::InvalidInput)?,
                ],
            )
            .await
            .map_err(database_error)?;
        let eligible_ids = chain
            .into_iter()
            .take_while(|row| {
                ids.contains(&row.get::<_, Uuid>(0))
                    && row.get::<_, i64>(2) >= 0
                    && u64::try_from(row.get::<_, i64>(2)).is_ok_and(|value| value < cutoff_unix_ms)
                    && !row.get::<_, bool>(3)
            })
            .map(|row| (row.get::<_, Uuid>(0), row.get::<_, String>(1)))
            .collect::<Vec<_>>();
        let last_purged_digest = eligible_ids.last().map(|(_, digest)| digest.clone());
        let eligible_ids = eligible_ids
            .into_iter()
            .map(|(event_id, _)| event_id)
            .collect::<Vec<_>>();
        transaction
            .execute(
                "DELETE FROM prodex_siem_outbox
                 WHERE tenant_id = $1 AND audit_event_id = ANY($2)",
                &[&tenant_id.as_uuid(), &eligible_ids],
            )
            .await
            .map_err(database_error)?;
        transaction
            .execute(
                "DELETE FROM prodex_siem_dead_letters
                 WHERE tenant_id = $1 AND audit_event_id = ANY($2)",
                &[&tenant_id.as_uuid(), &eligible_ids],
            )
            .await
            .map_err(database_error)?;
        let deleted = transaction
            .execute(
                "DELETE FROM prodex_audit_log audit
                 WHERE audit.tenant_id = $1
                   AND audit.audit_event_id = ANY($2)",
                &[&tenant_id.as_uuid(), &eligible_ids],
            )
            .await
            .map_err(database_error)?;
        if usize::try_from(deleted).ok() != Some(eligible_ids.len()) {
            return Err(GovernanceRepositoryError::AuditChainConflict);
        }
        let purged = eligible_ids
            .iter()
            .copied()
            .map(AuditEventId::from_uuid)
            .collect();
        if let Some(last_purged_digest) = last_purged_digest {
            transaction
                .execute(
                    "INSERT INTO prodex_audit_retention_anchors (
                        tenant_id, last_purged_digest, updated_at_unix_ms
                     ) VALUES ($1, $2, $3)
                     ON CONFLICT (tenant_id) DO UPDATE SET
                        last_purged_digest = excluded.last_purged_digest,
                        updated_at_unix_ms = excluded.updated_at_unix_ms",
                    &[
                        &tenant_id.as_uuid(),
                        &last_purged_digest,
                        &to_i64(now_unix_ms)?,
                    ],
                )
                .await
                .map_err(database_error)?;
        }
        append_audit_outbox_tx(&transaction, audit).await?;
        transaction.commit().await.map_err(database_error)?;
        Ok(purged)
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
        F: FnMut(&[u8]) -> bool,
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
        F: FnMut(&[u8]) -> bool,
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
