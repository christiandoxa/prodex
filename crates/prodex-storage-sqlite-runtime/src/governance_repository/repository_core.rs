use super::*;

impl GovernanceSqliteRepository {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, GovernanceRepositoryError> {
        let connection = Connection::open(path).map_err(database_error)?;
        Self::from_connection(connection)
    }

    pub fn from_connection(connection: Connection) -> Result<Self, GovernanceRepositoryError> {
        connection
            .execute_batch("PRAGMA foreign_keys = ON;")
            .map_err(database_error)?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(database_error)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub fn governance_list_tenant_ids(
        &self,
        limit: u16,
    ) -> Result<Vec<TenantId>, GovernanceRepositoryError> {
        if limit == 0 {
            return Err(GovernanceRepositoryError::InvalidInput);
        }
        let connection = self.connection()?;
        let mut statement = connection
            .prepare("SELECT tenant_id FROM prodex_tenants ORDER BY tenant_id ASC LIMIT ?1")
            .map_err(database_error)?;
        statement
            .query_map([i64::from(limit)], |row| row.get::<_, String>(0))
            .map_err(database_error)?
            .map(|row| {
                row.map_err(database_error)?
                    .parse::<TenantId>()
                    .map_err(|_| GovernanceRepositoryError::Database)
            })
            .collect()
    }

    pub fn write_revision(
        &self,
        command: GovernanceRevisionWriteCommand,
        audit_outbox: AuditOutboxWriteCommand,
    ) -> Result<GovernanceWriteOutcome, GovernanceRepositoryError> {
        self.write_revision_inner(command, audit_outbox, None)
    }

    pub fn write_revision_idempotent(
        &self,
        command: GovernanceRevisionWriteCommand,
        audit_outbox: AuditOutboxWriteCommand,
        idempotency: GovernanceMutationIdempotency,
    ) -> Result<GovernanceWriteOutcome, GovernanceRepositoryError> {
        self.write_revision_inner(command, audit_outbox, Some(idempotency))
    }

    fn write_revision_inner(
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
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(database_error)?;
        if let Some(idempotency) = idempotency.as_ref() {
            match governance_idempotency_replay_sqlite(
                &transaction,
                command.tenant_id,
                idempotency,
            )? {
                IdempotencyReplayDecision::ExecuteAndRecordPending => {
                    insert_governance_idempotency_pending_sqlite(
                        &transaction,
                        command.tenant_id,
                        idempotency,
                    )?;
                }
                IdempotencyReplayDecision::AlreadyInProgress { .. } => {
                    return Err(GovernanceRepositoryError::Conflict);
                }
                IdempotencyReplayDecision::Replay(response) => {
                    transaction.commit().map_err(database_error)?;
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
        )? {
            if !revision_matches_command(&existing, &command, &checksum) {
                return Err(GovernanceRepositoryError::Conflict);
            }
            if let Some(idempotency) = idempotency.as_ref() {
                complete_governance_idempotency_sqlite(
                    &transaction,
                    command.tenant_id,
                    idempotency,
                    GOVERNANCE_REVISION_WRITE_IDEMPOTENCY_RESPONSE,
                    completed_at_unix_ms,
                )?;
            }
            transaction.commit().map_err(database_error)?;
            return Ok(GovernanceWriteOutcome::Replayed);
        }

        insert_revision_metadata(&transaction, &command, &checksum, created_at)?;
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
        transaction
            .execute(
                "INSERT INTO prodex_governance_revision_artifacts (
                    tenant_id, artifact_kind, revision_id, artifact_checksum,
                    compiled_artifact, signature_key_id, artifact_signature,
                    created_by, created_at_unix_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    command.tenant_id.to_string(),
                    artifact_kind_label(command.kind),
                    command.revision_id,
                    checksum,
                    command.compiled_artifact,
                    signature_key_id,
                    artifact_signature,
                    command.created_by.to_string(),
                    created_at,
                ],
            )
            .map_err(database_error)?;
        append_audit_outbox_tx(&transaction, audit_outbox)?;
        if let Some(idempotency) = idempotency.as_ref() {
            complete_governance_idempotency_sqlite(
                &transaction,
                command.tenant_id,
                idempotency,
                GOVERNANCE_REVISION_WRITE_IDEMPOTENCY_RESPONSE,
                completed_at_unix_ms,
            )?;
        }
        transaction.commit().map_err(database_error)?;
        Ok(GovernanceWriteOutcome::Applied)
    }

    pub fn create_approval(
        &self,
        approval: ApprovalRecord,
        audit_outbox: AuditOutboxWriteCommand,
    ) -> Result<GovernanceWriteOutcome, GovernanceRepositoryError> {
        self.create_approval_inner(approval, audit_outbox, None)
    }

    pub fn create_approval_idempotent(
        &self,
        approval: ApprovalRecord,
        audit_outbox: AuditOutboxWriteCommand,
        idempotency: GovernanceMutationIdempotency,
    ) -> Result<GovernanceWriteOutcome, GovernanceRepositoryError> {
        self.create_approval_inner(approval, audit_outbox, Some(idempotency))
    }

    fn create_approval_inner(
        &self,
        approval: ApprovalRecord,
        audit_outbox: AuditOutboxWriteCommand,
        idempotency: Option<GovernanceMutationIdempotency>,
    ) -> Result<GovernanceWriteOutcome, GovernanceRepositoryError> {
        if !approval_is_new(&approval) {
            return Err(GovernanceRepositoryError::InvalidInput);
        }
        let kind = approval_artifact_kind(approval.kind)?;
        let completed_at_unix_ms = audit_outbox.audit.event.occurred_at_unix_ms;
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(database_error)?;
        if let Some(idempotency) = idempotency.as_ref() {
            match governance_idempotency_replay_sqlite(
                &transaction,
                approval.tenant_id,
                idempotency,
            )? {
                IdempotencyReplayDecision::ExecuteAndRecordPending => {
                    insert_governance_idempotency_pending_sqlite(
                        &transaction,
                        approval.tenant_id,
                        idempotency,
                    )?;
                }
                IdempotencyReplayDecision::AlreadyInProgress { .. } => {
                    return Err(GovernanceRepositoryError::Conflict);
                }
                IdempotencyReplayDecision::Replay(response) => {
                    transaction.commit().map_err(database_error)?;
                    return if response == GOVERNANCE_APPROVAL_CREATE_IDEMPOTENCY_RESPONSE {
                        Ok(GovernanceWriteOutcome::Replayed)
                    } else {
                        Err(GovernanceRepositoryError::InvalidInput)
                    };
                }
            }
        }
        let revision_id = kind
            .map(|kind| {
                revision_id_for_fingerprint(
                    &transaction,
                    approval.tenant_id,
                    kind,
                    approval.fingerprint.as_str(),
                )?
                .ok_or(GovernanceRepositoryError::NotFound)
            })
            .transpose()?;

        if let Some(existing) = load_approval_tx(&transaction, approval.tenant_id, &approval.id)? {
            if existing != approval {
                return Err(GovernanceRepositoryError::Conflict);
            }
            if let Some(idempotency) = idempotency.as_ref() {
                complete_governance_idempotency_sqlite(
                    &transaction,
                    approval.tenant_id,
                    idempotency,
                    GOVERNANCE_APPROVAL_CREATE_IDEMPOTENCY_RESPONSE,
                    completed_at_unix_ms,
                )?;
            }
            transaction.commit().map_err(database_error)?;
            return Ok(GovernanceWriteOutcome::Replayed);
        }
        transaction
            .execute(
                "INSERT INTO prodex_approvals (
                    tenant_id, approval_id, approval_kind, approval_scope, fingerprint,
                    maker_id, lifecycle_state, required_quorum, expires_at_unix_ms,
                    activated_at_unix_ms, termination_reason, resource_version
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL, NULL, ?10)",
                params![
                    approval.tenant_id.to_string(),
                    approval.id.as_str(),
                    approval_kind_label(approval.kind),
                    approval.scope.as_str(),
                    approval.fingerprint.as_str(),
                    approval.maker.to_string(),
                    approval_state_label(approval.state),
                    i64::from(approval.required_quorum),
                    to_i64(approval.expires_at_unix_ms)?,
                    to_i64(approval.version)?,
                ],
            )
            .map_err(database_error)?;
        if let (Some(kind), Some(revision_id)) = (kind, revision_id.as_deref()) {
            update_revision_state(
                &transaction,
                approval.tenant_id,
                kind,
                revision_id,
                "pending_approval",
            )?;
        }
        append_audit_outbox_tx(&transaction, audit_outbox)?;
        if let Some(idempotency) = idempotency.as_ref() {
            complete_governance_idempotency_sqlite(
                &transaction,
                approval.tenant_id,
                idempotency,
                GOVERNANCE_APPROVAL_CREATE_IDEMPOTENCY_RESPONSE,
                completed_at_unix_ms,
            )?;
        }
        transaction.commit().map_err(database_error)?;
        Ok(GovernanceWriteOutcome::Applied)
    }

    pub fn vote_approval(
        &self,
        request: ApprovalVoteRequest,
    ) -> Result<ApprovalRecord, GovernanceRepositoryError> {
        self.transition_approval(request, ApprovalAction::Approve)
    }

    pub fn transition_approval(
        &self,
        request: ApprovalVoteRequest,
        action: ApprovalAction,
    ) -> Result<ApprovalRecord, GovernanceRepositoryError> {
        match self.transition_approval_inner(request, action, None)? {
            ApprovalVoteMutationOutcome::Applied(approval) => Ok(approval),
            ApprovalVoteMutationOutcome::Replayed(_) => {
                Err(GovernanceRepositoryError::InvalidInput)
            }
        }
    }

    pub fn transition_approval_idempotent(
        &self,
        request: ApprovalVoteRequest,
        action: ApprovalAction,
        idempotency: ApprovalVoteIdempotency,
    ) -> Result<ApprovalVoteMutationOutcome, GovernanceRepositoryError> {
        self.transition_approval_inner(request, action, Some(idempotency))
    }

    fn transition_approval_inner(
        &self,
        request: ApprovalVoteRequest,
        action: ApprovalAction,
        idempotency: Option<ApprovalVoteIdempotency>,
    ) -> Result<ApprovalVoteMutationOutcome, GovernanceRepositoryError> {
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(database_error)?;
        if let Some(idempotency) = idempotency.as_ref() {
            match governance_idempotency_replay_sqlite(
                &transaction,
                request.tenant_id,
                idempotency,
            )? {
                IdempotencyReplayDecision::ExecuteAndRecordPending => {
                    insert_governance_idempotency_pending_sqlite(
                        &transaction,
                        request.tenant_id,
                        idempotency,
                    )?;
                }
                IdempotencyReplayDecision::AlreadyInProgress { .. } => {
                    return Err(GovernanceRepositoryError::Conflict);
                }
                IdempotencyReplayDecision::Replay(response) => {
                    transaction.commit().map_err(database_error)?;
                    return ApprovalVoteStableOutcome::replay(&response);
                }
            }
        }
        let current = load_approval_tx(&transaction, request.tenant_id, &request.approval_id)?
            .ok_or(GovernanceRepositoryError::NotFound)?;
        let transition = match plan_approval_vote_transition(&current, &request, action)? {
            ApprovalVoteTransitionDecision::Transition(transition) => transition,
            ApprovalVoteTransitionDecision::Denied(denial) => {
                append_audit_outbox_tx(
                    &transaction,
                    denied_approval_audit_outbox(request.audit_outbox, denial),
                )?;
                if let Some(idempotency) = idempotency.as_ref() {
                    let response = ApprovalVoteStableOutcome::Denied(denial).encode();
                    complete_governance_idempotency_sqlite(
                        &transaction,
                        request.tenant_id,
                        idempotency,
                        &response,
                        request.now_unix_ms,
                    )?;
                }
                transaction.commit().map_err(database_error)?;
                return Err(denial.repository_error());
            }
        };
        if !transition.changed {
            if let Some(idempotency) = idempotency.as_ref() {
                let response = ApprovalVoteStableOutcome::Success(
                    ApprovalVoteSnapshot::from_record(&transition.record),
                )
                .encode();
                complete_governance_idempotency_sqlite(
                    &transaction,
                    request.tenant_id,
                    idempotency,
                    &response,
                    request.now_unix_ms,
                )?;
            }
            transaction.commit().map_err(database_error)?;
            return Ok(ApprovalVoteMutationOutcome::Applied(transition.record));
        }
        persist_approval_transition(&transaction, &current, &transition.record)?;
        if let Some(update) = plan_approval_revision_lifecycle_update(&transition.record)? {
            let revision_id = revision_id_for_fingerprint(
                &transaction,
                transition.record.tenant_id,
                update.kind,
                transition.record.fingerprint.as_str(),
            )?
            .ok_or(GovernanceRepositoryError::NotFound)?;
            update_revision_state(
                &transaction,
                transition.record.tenant_id,
                update.kind,
                &revision_id,
                update.state,
            )?;
        }
        append_audit_outbox_tx(&transaction, request.audit_outbox)?;
        if let Some(idempotency) = idempotency.as_ref() {
            let response = ApprovalVoteStableOutcome::Success(ApprovalVoteSnapshot::from_record(
                &transition.record,
            ))
            .encode();
            complete_governance_idempotency_sqlite(
                &transaction,
                request.tenant_id,
                idempotency,
                &response,
                request.now_unix_ms,
            )?;
        }
        transaction.commit().map_err(database_error)?;
        Ok(ApprovalVoteMutationOutcome::Applied(transition.record))
    }

    pub fn list_revisions(
        &self,
        tenant_id: TenantId,
        kind: GovernanceArtifactKind,
    ) -> Result<Vec<GovernanceRevisionSummary>, GovernanceRepositoryError> {
        let connection = self.connection()?;
        let table = revision_table(kind);
        let kind_label = artifact_kind_label(kind);
        let mut statement = connection
            .prepare(&format!(
                "SELECT revisions.revision_id, revisions.artifact_checksum,
                        revisions.lifecycle_state, revisions.created_at_unix_ms,
                        artifacts.signature_key_id
                 FROM {table} AS revisions
                 LEFT JOIN prodex_governance_revision_artifacts AS artifacts
                   ON artifacts.tenant_id = revisions.tenant_id
                  AND artifacts.artifact_kind = ?2
                  AND artifacts.revision_id = CAST(revisions.revision_id AS TEXT)
                 WHERE revisions.tenant_id = ?1
                 ORDER BY revisions.created_at_unix_ms DESC, revisions.revision_id DESC"
            ))
            .map_err(database_error)?;
        let rows = statement
            .query_map(params![tenant_id.to_string(), kind_label], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            })
            .map_err(database_error)?;
        let mut summaries = Vec::new();
        for row in rows {
            let (revision_id, fingerprint, lifecycle_state, created_at, signature_key_id) =
                row.map_err(database_error)?;
            summaries.push(GovernanceRevisionSummary {
                revision_id,
                fingerprint,
                lifecycle_state,
                signature_key_id,
                created_at_unix_ms: from_i64(created_at)?,
            });
        }
        Ok(summaries)
    }

    pub fn get_revision(
        &self,
        tenant_id: TenantId,
        kind: GovernanceArtifactKind,
        revision_id: &str,
    ) -> Result<GovernanceRevisionSummary, GovernanceRepositoryError> {
        validate_governance_revision_id(kind, revision_id)?;
        self.list_revisions(tenant_id, kind)?
            .into_iter()
            .find(|revision| revision.revision_id == revision_id)
            .ok_or(GovernanceRepositoryError::NotFound)
    }

    pub fn get_approval(
        &self,
        tenant_id: TenantId,
        approval_id: &ApprovalId,
    ) -> Result<ApprovalRecord, GovernanceRepositoryError> {
        let connection = self.connection()?;
        load_approval_tx(&connection, tenant_id, approval_id)?
            .ok_or(GovernanceRepositoryError::NotFound)
    }

    pub fn list_execution_approvals(
        &self,
        tenant_id: TenantId,
    ) -> Result<Vec<ApprovalRecord>, GovernanceRepositoryError> {
        self.list_approvals(tenant_id, prodex_domain::ApprovalKind::Execution)
    }

    pub fn list_approvals(
        &self,
        tenant_id: TenantId,
        kind: prodex_domain::ApprovalKind,
    ) -> Result<Vec<ApprovalRecord>, GovernanceRepositoryError> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(
                "SELECT approval_id FROM prodex_approvals
                 WHERE tenant_id = ?1 AND approval_kind = ?2
                 ORDER BY expires_at_unix_ms DESC, approval_id DESC",
            )
            .map_err(database_error)?;
        let ids = statement
            .query_map(
                rusqlite::params![
                    tenant_id.to_string(),
                    prodex_storage::governance_support::approval_kind_label(kind)
                ],
                |row| row.get::<_, String>(0),
            )
            .map_err(database_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(database_error)?;
        ids.into_iter()
            .map(|id| {
                let id = ApprovalId::new(id).map_err(|_| GovernanceRepositoryError::Database)?;
                load_approval_tx(&connection, tenant_id, &id)?
                    .ok_or(GovernanceRepositoryError::Database)
            })
            .collect()
    }

    pub fn status(
        &self,
        tenant_id: TenantId,
        kind: GovernanceArtifactKind,
    ) -> Result<GovernanceStatus, GovernanceRepositoryError> {
        let connection = self.connection()?;
        Ok(load_pointer(&connection, tenant_id, kind)?.map_or_else(
            GovernanceStatus::default,
            |pointer| GovernanceStatus {
                active_revision_id: pointer.active_revision_id,
                last_known_good_revision_id: pointer.last_known_good_revision_id,
                etag: Some(pointer.etag),
            },
        ))
    }

    pub fn latest_audit_digest(
        &self,
        tenant_id: TenantId,
    ) -> Result<Option<AuditDigest>, GovernanceRepositoryError> {
        let connection = self.connection()?;
        let value = connection
            .query_row(
                "SELECT COALESCE(
                    (
                        SELECT audit.event_digest
                        FROM prodex_audit_log audit
                        WHERE audit.tenant_id = ?1
                          AND NOT EXISTS (
                              SELECT 1 FROM prodex_audit_log child
                              WHERE child.tenant_id = audit.tenant_id
                                AND child.previous_digest = audit.event_digest
                          )
                        ORDER BY audit.occurred_at_unix_ms DESC, audit.audit_event_id DESC
                        LIMIT 1
                    ),
                    (
                        SELECT last_purged_digest
                        FROM prodex_audit_retention_anchors
                        WHERE tenant_id = ?1
                    )
                 )",
                [tenant_id.to_string()],
                |row| row.get::<_, Option<String>>(0),
            )
            .map_err(database_error)?;
        value
            .map(|digest| AuditDigest::new(digest).map_err(|_| GovernanceRepositoryError::Database))
            .transpose()
    }

    pub fn outbox_health(
        &self,
        tenant_id: TenantId,
    ) -> Result<GovernanceOutboxHealth, GovernanceRepositoryError> {
        let connection = self.connection()?;
        let (pending, oldest) = connection
            .query_row(
                "SELECT COUNT(*), MIN(created_at_unix_ms) FROM prodex_siem_outbox
                 WHERE tenant_id = ?1 AND delivered_at_unix_ms IS NULL",
                [tenant_id.to_string()],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<i64>>(1)?)),
            )
            .map_err(database_error)?;
        let dead_lettered = connection
            .query_row(
                "SELECT COUNT(*) FROM prodex_siem_dead_letters WHERE tenant_id = ?1",
                [tenant_id.to_string()],
                |row| row.get::<_, i64>(0),
            )
            .map_err(database_error)?;
        Ok(GovernanceOutboxHealth {
            pending: from_i64(pending)?,
            dead_lettered: from_i64(dead_lettered)?,
            oldest_pending_at_unix_ms: oldest.map(from_i64).transpose()?,
        })
    }

    pub fn aggregate_outbox_health(
        &self,
    ) -> Result<GovernanceOutboxHealth, GovernanceRepositoryError> {
        let connection = self.connection()?;
        let (pending, oldest) = connection
            .query_row(
                "SELECT COUNT(*), MIN(created_at_unix_ms) FROM prodex_siem_outbox
                 WHERE delivered_at_unix_ms IS NULL",
                [],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<i64>>(1)?)),
            )
            .map_err(database_error)?;
        let dead_lettered = connection
            .query_row("SELECT COUNT(*) FROM prodex_siem_dead_letters", [], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(database_error)?;
        Ok(GovernanceOutboxHealth {
            pending: from_i64(pending)?,
            dead_lettered: from_i64(dead_lettered)?,
            oldest_pending_at_unix_ms: oldest.map(from_i64).transpose()?,
        })
    }

    pub fn audit_integrity_health(
        &self,
        tenant_id: TenantId,
    ) -> Result<GovernanceAuditIntegrityHealth, GovernanceRepositoryError> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(
                "SELECT audit_event_id, occurred_at_unix_ms, principal_id, action,
                        resource_kind, resource_id, outcome, reason_code,
                        reason_detail, previous_digest, event_digest
                 FROM prodex_audit_log WHERE tenant_id = ?1",
            )
            .map_err(database_error)?;
        let records = statement
            .query_map([tenant_id.to_string()], governance_audit_export_record)
            .map_err(database_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(database_error)?;
        let anchor = connection
            .query_row(
                "SELECT last_purged_digest FROM prodex_audit_retention_anchors
                 WHERE tenant_id = ?1",
                [tenant_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(database_error)?
            .map(AuditDigest::new)
            .transpose()
            .map_err(|_| GovernanceRepositoryError::Database)?;
        Ok(verify_governance_audit_integrity_with_retention_anchor(
            tenant_id,
            &records,
            anchor.as_ref(),
        ))
    }

    pub fn governance_export_audit(
        &self,
        tenant_id: TenantId,
        limit: u16,
    ) -> Result<Vec<GovernanceAuditExportRecord>, GovernanceRepositoryError> {
        if limit == 0 || limit > 1_000 {
            return Err(GovernanceRepositoryError::InvalidInput);
        }
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(
                "SELECT audit_event_id, occurred_at_unix_ms, principal_id, action,
                        resource_kind, resource_id, outcome, reason_code,
                        reason_detail, previous_digest, event_digest
                 FROM prodex_audit_log WHERE tenant_id = ?1
                 ORDER BY occurred_at_unix_ms DESC, audit_event_id DESC LIMIT ?2",
            )
            .map_err(database_error)?;
        statement
            .query_map(
                params![tenant_id.to_string(), i64::from(limit)],
                governance_audit_export_record,
            )
            .map_err(database_error)?
            .map(|row| row.map_err(database_error))
            .collect()
    }
}

fn revision_matches_command(
    existing: &RevisionRow,
    command: &GovernanceRevisionWriteCommand,
    checksum: &str,
) -> bool {
    existing.checksum == checksum
        && existing.compiled_artifact == command.compiled_artifact
        && existing.authenticity == command.authenticity
        && existing.created_by == command.created_by
        && existing.created_at_unix_ms == command.created_at_unix_ms
}

fn approval_is_new(approval: &ApprovalRecord) -> bool {
    approval.state == ApprovalState::PendingApproval
        && approval.version == 1
        && approval.votes.is_empty()
}

fn governance_audit_export_record(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<GovernanceAuditExportRecord> {
    Ok(GovernanceAuditExportRecord {
        audit_event_id: row.get(0)?,
        occurred_at_unix_ms: from_i64(row.get(1)?)
            .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(1, i64::MAX))?,
        principal_id: row.get(2)?,
        action: row.get(3)?,
        resource_kind: row.get(4)?,
        resource_id: row.get(5)?,
        outcome: row.get(6)?,
        reason_code: row.get(7)?,
        reason_detail: row.get(8)?,
        previous_digest: row.get(9)?,
        event_digest: row.get(10)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connection_enables_foreign_keys() {
        let repository = GovernanceSqliteRepository::from_connection(
            Connection::open_in_memory().expect("in-memory SQLite connection"),
        )
        .expect("SQLite repository configuration");
        let connection = repository.connection().expect("SQLite connection lock");
        let enabled: i64 = connection
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .expect("foreign_keys pragma");
        assert_eq!(enabled, 1);
    }
}
