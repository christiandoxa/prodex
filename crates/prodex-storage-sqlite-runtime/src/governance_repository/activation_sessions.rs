use super::*;

impl GovernanceSqliteRepository {
    pub fn activate_revision(
        &self,
        request: GovernanceActivationRequest,
        validate_artifact: impl FnOnce(&[u8]) -> bool,
    ) -> Result<GovernanceActivationResult, GovernanceRepositoryError> {
        let activated_at = validate_governance_activation_request(&request)?;
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(database_error)?;

        if let Some(replay) = load_activation_replay(&transaction, &request)? {
            transaction.commit().map_err(database_error)?;
            return Ok(replay);
        }
        let revision = load_revision_row(
            &transaction,
            request.tenant_id,
            request.kind,
            &request.revision_id,
        )?
        .ok_or(GovernanceRepositoryError::NotFound)?;
        if revision.checksum != artifact_checksum(&revision.compiled_artifact)
            || !validate_artifact(&revision.compiled_artifact)
        {
            return Err(GovernanceRepositoryError::SnapshotUnavailable);
        }
        let approval = load_approval_tx(&transaction, request.tenant_id, &request.approval_id)?
            .ok_or(GovernanceRepositoryError::ApprovalRequired)?;
        let pointer = load_pointer(&transaction, request.tenant_id, request.kind)?;
        let plan = plan_governance_activation(
            &request,
            GovernanceActivationCurrent {
                revision_checksum: &revision.checksum,
                approval: &approval,
                active_revision_id: pointer
                    .as_ref()
                    .and_then(|value| value.active_revision_id.as_deref()),
                last_known_good_revision_id: pointer
                    .as_ref()
                    .and_then(|value| value.last_known_good_revision_id.as_deref()),
                etag: pointer.as_ref().map(|value| value.etag.as_str()),
            },
        )?;

        if let (Some(previous), Some(previous_state)) = (
            plan.previous_active_revision_id.as_deref(),
            plan.previous_revision_state,
        ) {
            update_revision_state(
                &transaction,
                request.tenant_id,
                request.kind,
                previous,
                previous_state,
            )?;
        }
        update_revision_state(
            &transaction,
            request.tenant_id,
            request.kind,
            &request.revision_id,
            "active",
        )?;
        persist_approval_transition(&transaction, &approval, &plan.activated_approval)?;
        store_pointer(
            &transaction,
            request.tenant_id,
            request.kind,
            &request.revision_id,
            &plan.last_known_good_revision_id,
            &plan.etag,
            activated_at,
            pointer.as_ref(),
        )?;
        insert_activation_history(
            &transaction,
            &request,
            plan.previous_active_revision_id.as_deref(),
            activated_at,
        )?;
        append_audit_outbox_tx(&transaction, request.audit_outbox.clone())?;
        transaction
            .execute(
                "INSERT INTO prodex_governance_mutation_idempotency (
                    tenant_id, artifact_kind, idempotency_key, request_fingerprint,
                    action, revision_id, resulting_etag, created_at_unix_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    request.tenant_id.to_string(),
                    artifact_kind_label(request.kind),
                    request.idempotency_key.as_str(),
                    request.request_fingerprint,
                    request.action.as_str(),
                    request.revision_id,
                    plan.etag,
                    activated_at,
                ],
            )
            .map_err(database_error)?;
        transaction.commit().map_err(database_error)?;
        Ok(GovernanceActivationResult {
            outcome: GovernanceWriteOutcome::Applied,
            kind: request.kind,
            revision_id: request.revision_id,
            etag: plan.etag,
            last_known_good_revision_id: plan.last_known_good_revision_id,
        })
    }

    pub fn load_snapshot(
        &self,
        tenant_id: TenantId,
        kind: GovernanceArtifactKind,
        mut validate_artifact: impl FnMut(&[u8]) -> bool,
    ) -> Result<GovernanceSnapshot, GovernanceRepositoryError> {
        let connection = self.connection()?;
        let pointer = load_pointer(&connection, tenant_id, kind)?
            .ok_or(GovernanceRepositoryError::SnapshotUnavailable)?;
        let active = pointer
            .active_revision_id
            .ok_or(GovernanceRepositoryError::SnapshotUnavailable)?;
        if let Some(snapshot) = load_verified_snapshot(
            &connection,
            tenant_id,
            kind,
            &active,
            GovernanceSnapshotSource::Active,
            &mut validate_artifact,
        )? {
            return Ok(snapshot);
        }
        let last_known_good = pointer
            .last_known_good_revision_id
            .ok_or(GovernanceRepositoryError::SnapshotUnavailable)?;
        if last_known_good == active {
            return Err(GovernanceRepositoryError::SnapshotUnavailable);
        }
        load_verified_snapshot(
            &connection,
            tenant_id,
            kind,
            &last_known_good,
            GovernanceSnapshotSource::LastKnownGood,
            &mut validate_artifact,
        )?
        .ok_or(GovernanceRepositoryError::SnapshotUnavailable)
    }

    pub fn append_audit_outbox(
        &self,
        command: AuditOutboxWriteCommand,
    ) -> Result<(), GovernanceRepositoryError> {
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(database_error)?;
        append_audit_outbox_tx(&transaction, command)?;
        transaction.commit().map_err(database_error)
    }

    pub fn governance_upsert_session(
        &self,
        command: GovernanceSessionUpsertCommand,
    ) -> Result<GovernanceSessionUpsertOutcome, GovernanceRepositoryError> {
        validate_governance_session_upsert(&command)?;
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(database_error)?;
        let existing =
            load_governance_session_tx(&transaction, command.tenant_id, &command.session_id_hash)?;
        if let Some(existing) = existing.as_ref() {
            if existing.principal_id != command.principal_id
                || existing.channel != command.channel
                || existing.credential_scope != command.credential_scope
                || existing.revoked_at_unix_ms.is_some()
            {
                return Err(GovernanceRepositoryError::Conflict);
            }
        } else if let Some(max) = command.max_concurrent {
            let active = count_concurrent_sessions_tx(
                &transaction,
                command.tenant_id,
                command.principal_id,
                command.last_seen_at_unix_ms,
            )?;
            if active >= u64::from(max) {
                transaction.commit().map_err(database_error)?;
                return Ok(GovernanceSessionUpsertOutcome::ConcurrentLimitReached);
            }
        }
        transaction
            .execute(
                "INSERT INTO prodex_governance_sessions (
                    tenant_id, session_id_hash, principal_id, channel, credential_scope,
                    classification, policy_revision_id, provider_registry_revision,
                    provider_descriptor_revision, provider_affinity,
                    created_at_unix_ms, last_seen_at_unix_ms, absolute_expires_at_unix_ms,
                    idle_expires_at_unix_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
                 ON CONFLICT (tenant_id, session_id_hash) DO UPDATE SET
                    classification = CASE
                        WHEN prodex_governance_sessions.classification = 'restricted'
                          OR excluded.classification = 'restricted' THEN 'restricted'
                        WHEN prodex_governance_sessions.classification = 'confidential'
                          OR excluded.classification = 'confidential' THEN 'confidential'
                        WHEN prodex_governance_sessions.classification = 'internal'
                          OR excluded.classification = 'internal' THEN 'internal'
                        ELSE 'public' END,
                    policy_revision_id = CASE
                        WHEN excluded.last_seen_at_unix_ms >= prodex_governance_sessions.last_seen_at_unix_ms
                        THEN excluded.policy_revision_id ELSE prodex_governance_sessions.policy_revision_id END,
                    provider_registry_revision = CASE
                        WHEN excluded.last_seen_at_unix_ms >= prodex_governance_sessions.last_seen_at_unix_ms
                        THEN excluded.provider_registry_revision
                        ELSE prodex_governance_sessions.provider_registry_revision END,
                    provider_descriptor_revision = CASE
                        WHEN excluded.last_seen_at_unix_ms >= prodex_governance_sessions.last_seen_at_unix_ms
                        THEN excluded.provider_descriptor_revision
                        ELSE prodex_governance_sessions.provider_descriptor_revision END,
                    provider_affinity = CASE
                        WHEN excluded.last_seen_at_unix_ms >= prodex_governance_sessions.last_seen_at_unix_ms
                        THEN excluded.provider_affinity ELSE prodex_governance_sessions.provider_affinity END,
                    last_seen_at_unix_ms = MAX(
                        prodex_governance_sessions.last_seen_at_unix_ms,
                        excluded.last_seen_at_unix_ms
                    ),
                    absolute_expires_at_unix_ms = MIN(
                        prodex_governance_sessions.absolute_expires_at_unix_ms,
                        excluded.absolute_expires_at_unix_ms
                    ),
                    idle_expires_at_unix_ms = CASE
                        WHEN excluded.last_seen_at_unix_ms >= prodex_governance_sessions.last_seen_at_unix_ms
                        THEN excluded.idle_expires_at_unix_ms
                        ELSE prodex_governance_sessions.idle_expires_at_unix_ms END",
                params![
                    command.tenant_id.to_string(),
                    command.session_id_hash,
                    command.principal_id.to_string(),
                    channel_label(command.channel),
                    credential_scope_label(command.credential_scope),
                    command.classification.as_str(),
                    command.policy_revision_id.to_string(),
                    command.provider_registry_revision,
                    to_i64(command.provider_descriptor_revision)?,
                    command.provider_affinity,
                    to_i64(command.created_at_unix_ms)?,
                    to_i64(command.last_seen_at_unix_ms)?,
                    to_i64(command.absolute_expires_at_unix_ms)?,
                    to_i64(command.idle_expires_at_unix_ms)?,
                ],
            )
            .map_err(database_error)?;
        let stored =
            load_governance_session_tx(&transaction, command.tenant_id, &command.session_id_hash)?
                .ok_or(GovernanceRepositoryError::Database)?;
        transaction.commit().map_err(database_error)?;
        Ok(GovernanceSessionUpsertOutcome::Stored(Box::new(stored)))
    }

    pub fn governance_load_sessions(
        &self,
        tenant_id: TenantId,
        now_unix_ms: u64,
        limit: u16,
    ) -> Result<Vec<GovernanceSessionRecord>, GovernanceRepositoryError> {
        if limit == 0 || limit > 4_096 {
            return Err(GovernanceRepositoryError::InvalidInput);
        }
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(
                "SELECT session.session_id_hash, session.principal_id, session.channel,
                        session.credential_scope, session.classification,
                        session.policy_revision_id, session.provider_registry_revision,
                        session.provider_descriptor_revision, session.provider_affinity,
                        session.created_at_unix_ms,
                        session.last_seen_at_unix_ms, session.absolute_expires_at_unix_ms,
                        session.idle_expires_at_unix_ms, revocation.revoked_at_unix_ms,
                        revocation.reason_code
                 FROM prodex_governance_sessions session
                 LEFT JOIN prodex_session_revocations revocation
                   ON revocation.tenant_id = session.tenant_id
                  AND revocation.session_id_hash = session.session_id_hash
                 WHERE session.tenant_id = ?1
                   AND (revocation.session_id_hash IS NOT NULL
                        OR (session.absolute_expires_at_unix_ms > ?2
                            AND session.idle_expires_at_unix_ms > ?2))
                 ORDER BY session.last_seen_at_unix_ms DESC, session.session_id_hash
                 LIMIT ?3",
            )
            .map_err(database_error)?;
        let rows = statement
            .query_map(
                params![
                    tenant_id.to_string(),
                    to_i64(now_unix_ms)?,
                    i64::from(limit)
                ],
                governance_session_row,
            )
            .map_err(database_error)?;
        rows.map(|row| {
            row.map_err(database_error)
                .and_then(|row| governance_session_from_values(tenant_id, row))
        })
        .collect()
    }

    pub fn governance_count_concurrent_sessions(
        &self,
        tenant_id: TenantId,
        principal_id: PrincipalId,
        now_unix_ms: u64,
    ) -> Result<u64, GovernanceRepositoryError> {
        let connection = self.connection()?;
        count_concurrent_sessions_tx(&connection, tenant_id, principal_id, now_unix_ms)
    }

    pub fn governance_session_revocation_epoch(
        &self,
        tenant_id: TenantId,
    ) -> Result<u64, GovernanceRepositoryError> {
        let connection = self.connection()?;
        let epoch = connection
            .query_row(
                "SELECT session_revocation_epoch FROM prodex_tenants WHERE tenant_id = ?1",
                params![tenant_id.to_string()],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(database_error)?
            .ok_or(GovernanceRepositoryError::NotFound)?;
        from_i64(epoch)
    }

    pub fn governance_revoke_session(
        &self,
        command: GovernanceSessionRevokeCommand,
    ) -> Result<GovernanceWriteOutcome, GovernanceRepositoryError> {
        validate_governance_session_revoke(&command)?;
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(database_error)?;
        if load_governance_session_tx(&transaction, command.tenant_id, &command.session_id_hash)?
            .is_none()
        {
            return Err(GovernanceRepositoryError::NotFound);
        }
        let existing = transaction
            .query_row(
                "SELECT revoked_at_unix_ms, reason_code FROM prodex_session_revocations
                 WHERE tenant_id = ?1 AND session_id_hash = ?2",
                params![command.tenant_id.to_string(), command.session_id_hash],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(database_error)?;
        if let Some((_revoked_at, reason)) = existing {
            if reason != command.reason_code {
                return Err(GovernanceRepositoryError::Conflict);
            }
            transaction.commit().map_err(database_error)?;
            return Ok(GovernanceWriteOutcome::Replayed);
        }
        transaction
            .execute(
                "INSERT INTO prodex_session_revocations (
                    tenant_id, session_id_hash, revoked_at_unix_ms, reason_code
                 ) VALUES (?1, ?2, ?3, ?4)",
                params![
                    command.tenant_id.to_string(),
                    &command.session_id_hash,
                    to_i64(command.revoked_at_unix_ms)?,
                    &command.reason_code,
                ],
            )
            .map_err(database_error)?;
        let updated = transaction
            .execute(
                "UPDATE prodex_tenants
                 SET session_revocation_epoch = session_revocation_epoch + 1
                 WHERE tenant_id = ?1",
                params![command.tenant_id.to_string()],
            )
            .map_err(database_error)?;
        if updated != 1 {
            return Err(GovernanceRepositoryError::NotFound);
        }
        append_audit_outbox_tx(&transaction, command.audit_outbox)?;
        transaction.commit().map_err(database_error)?;
        Ok(GovernanceWriteOutcome::Applied)
    }
}
