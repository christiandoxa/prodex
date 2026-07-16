use super::*;

impl PostgresRepository {
    pub async fn governance_activate_revision<F>(
        &self,
        request: GovernanceActivationRequest,
        validate_artifact: F,
    ) -> Result<GovernanceActivationResult, GovernanceRepositoryError>
    where
        F: FnOnce(&[u8]) -> bool,
    {
        self.governance_timeout(self.governance_activate_revision_inner(request, validate_artifact))
            .await
    }

    async fn governance_activate_revision_inner<F>(
        &self,
        request: GovernanceActivationRequest,
        validate_artifact: F,
    ) -> Result<GovernanceActivationResult, GovernanceRepositoryError>
    where
        F: FnOnce(&[u8]) -> bool,
    {
        let activated_at = validate_governance_activation_request(&request)?;
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, request.tenant_id)
            .await
            .map_err(database_error)?;

        if let Some(replay) = load_activation_replay(&transaction, &request).await? {
            transaction.commit().await.map_err(database_error)?;
            return Ok(replay);
        }
        let revision = load_revision_row(
            &transaction,
            request.tenant_id,
            request.kind,
            &request.revision_id,
        )
        .await?
        .ok_or(GovernanceRepositoryError::NotFound)?;
        if revision.checksum != artifact_checksum(&revision.compiled_artifact)
            || !validate_artifact(&revision.compiled_artifact)
        {
            return Err(GovernanceRepositoryError::SnapshotUnavailable);
        }
        let approval = load_approval_tx(&transaction, request.tenant_id, &request.approval_id)
            .await?
            .ok_or(GovernanceRepositoryError::ApprovalRequired)?;
        let pointer = load_pointer_for_kind(&transaction, request.tenant_id, request.kind).await?;
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
            )
            .await?;
        }
        update_revision_state(
            &transaction,
            request.tenant_id,
            request.kind,
            &request.revision_id,
            "active",
        )
        .await?;
        persist_approval_transition(&transaction, &approval, &plan.activated_approval).await?;

        let pointer_statements = postgres_governance_pointer_statements(request.kind);
        let statement = transaction
            .prepare_cached(pointer_statements.compare_and_swap.sql)
            .await
            .map_err(database_error)?;
        let previous_etag = pointer.as_ref().map(|value| value.etag.clone());
        let stored = if request.kind == GovernanceArtifactKind::Policy {
            let revision_id = policy_revision_id(&request.revision_id)?;
            let last_known_good_id = policy_revision_id(&plan.last_known_good_revision_id)?;
            transaction
                .query_opt(
                    &statement,
                    &[
                        &request.tenant_id.as_uuid(),
                        &revision_id.as_uuid(),
                        &last_known_good_id.as_uuid(),
                        &plan.etag,
                        &activated_at,
                        &previous_etag,
                    ],
                )
                .await
        } else {
            transaction
                .query_opt(
                    &statement,
                    &[
                        &request.tenant_id.as_uuid(),
                        &request.revision_id,
                        &plan.last_known_good_revision_id,
                        &plan.etag,
                        &activated_at,
                        &previous_etag,
                    ],
                )
                .await
        }
        .map_err(database_error)?;
        if stored.is_none() {
            return Err(GovernanceRepositoryError::EtagMismatch);
        }
        insert_activation_history(
            &transaction,
            &request,
            plan.previous_active_revision_id.as_deref(),
            activated_at,
        )
        .await?;
        append_audit_outbox_tx(&transaction, request.audit_outbox.clone()).await?;
        transaction
            .execute(
                "INSERT INTO prodex_governance_mutation_idempotency (
                    tenant_id, artifact_kind, idempotency_key, request_fingerprint,
                    action, revision_id, resulting_etag, created_at_unix_ms
                 ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
                &[
                    &request.tenant_id.as_uuid(),
                    &artifact_kind_label(request.kind),
                    &request.idempotency_key.as_str(),
                    &request.request_fingerprint,
                    &request.action.as_str(),
                    &request.revision_id,
                    &plan.etag,
                    &activated_at,
                ],
            )
            .await
            .map_err(database_error)?;
        transaction.commit().await.map_err(database_error)?;
        Ok(GovernanceActivationResult {
            outcome: GovernanceWriteOutcome::Applied,
            kind: request.kind,
            revision_id: request.revision_id,
            etag: plan.etag,
            last_known_good_revision_id: plan.last_known_good_revision_id,
        })
    }

    pub async fn governance_upsert_session(
        &self,
        command: GovernanceSessionUpsertCommand,
    ) -> Result<GovernanceSessionUpsertOutcome, GovernanceRepositoryError> {
        self.governance_timeout(self.governance_upsert_session_inner(command))
            .await
    }

    async fn governance_upsert_session_inner(
        &self,
        command: GovernanceSessionUpsertCommand,
    ) -> Result<GovernanceSessionUpsertOutcome, GovernanceRepositoryError> {
        validate_governance_session_upsert(&command)?;
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, command.tenant_id)
            .await
            .map_err(database_error)?;
        let admission_lock = format!("{}:{}", command.tenant_id, command.principal_id);
        transaction
            .query_one(
                "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
                &[&admission_lock],
            )
            .await
            .map_err(database_error)?;
        let existing =
            load_governance_session_tx(&transaction, command.tenant_id, &command.session_id_hash)
                .await?;
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
            )
            .await?;
            if active >= u64::from(max) {
                transaction.commit().await.map_err(database_error)?;
                return Ok(GovernanceSessionUpsertOutcome::ConcurrentLimitReached);
            }
        }
        let created_at = to_i64(command.created_at_unix_ms)?;
        let last_seen_at = to_i64(command.last_seen_at_unix_ms)?;
        let absolute_expires_at = to_i64(command.absolute_expires_at_unix_ms)?;
        let idle_expires_at = to_i64(command.idle_expires_at_unix_ms)?;
        let provider_descriptor_revision = to_i64(command.provider_descriptor_revision)?;
        transaction
            .execute(
                "INSERT INTO prodex_governance_sessions (
                    tenant_id, session_id_hash, principal_id, channel, credential_scope,
                    classification, policy_revision_id, provider_registry_revision,
                    provider_descriptor_revision, provider_affinity,
                    created_at_unix_ms, last_seen_at_unix_ms, absolute_expires_at_unix_ms,
                    idle_expires_at_unix_ms
                 ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
                 ON CONFLICT (tenant_id, session_id_hash) DO UPDATE SET
                    classification = CASE
                        WHEN prodex_governance_sessions.classification = 'restricted'
                          OR EXCLUDED.classification = 'restricted' THEN 'restricted'
                        WHEN prodex_governance_sessions.classification = 'confidential'
                          OR EXCLUDED.classification = 'confidential' THEN 'confidential'
                        WHEN prodex_governance_sessions.classification = 'internal'
                          OR EXCLUDED.classification = 'internal' THEN 'internal'
                        ELSE 'public' END,
                    policy_revision_id = CASE
                        WHEN EXCLUDED.last_seen_at_unix_ms >= prodex_governance_sessions.last_seen_at_unix_ms
                        THEN EXCLUDED.policy_revision_id ELSE prodex_governance_sessions.policy_revision_id END,
                    provider_registry_revision = CASE
                        WHEN EXCLUDED.last_seen_at_unix_ms >= prodex_governance_sessions.last_seen_at_unix_ms
                        THEN EXCLUDED.provider_registry_revision
                        ELSE prodex_governance_sessions.provider_registry_revision END,
                    provider_descriptor_revision = CASE
                        WHEN EXCLUDED.last_seen_at_unix_ms >= prodex_governance_sessions.last_seen_at_unix_ms
                        THEN EXCLUDED.provider_descriptor_revision
                        ELSE prodex_governance_sessions.provider_descriptor_revision END,
                    provider_affinity = CASE
                        WHEN EXCLUDED.last_seen_at_unix_ms >= prodex_governance_sessions.last_seen_at_unix_ms
                        THEN EXCLUDED.provider_affinity ELSE prodex_governance_sessions.provider_affinity END,
                    last_seen_at_unix_ms = GREATEST(
                        prodex_governance_sessions.last_seen_at_unix_ms,
                        EXCLUDED.last_seen_at_unix_ms
                    ),
                    absolute_expires_at_unix_ms = LEAST(
                        prodex_governance_sessions.absolute_expires_at_unix_ms,
                        EXCLUDED.absolute_expires_at_unix_ms
                    ),
                    idle_expires_at_unix_ms = CASE
                        WHEN EXCLUDED.last_seen_at_unix_ms >= prodex_governance_sessions.last_seen_at_unix_ms
                        THEN EXCLUDED.idle_expires_at_unix_ms
                        ELSE prodex_governance_sessions.idle_expires_at_unix_ms END
                 WHERE prodex_governance_sessions.principal_id = EXCLUDED.principal_id
                   AND prodex_governance_sessions.channel = EXCLUDED.channel
                   AND prodex_governance_sessions.credential_scope = EXCLUDED.credential_scope",
                &[
                    &command.tenant_id.as_uuid(),
                    &command.session_id_hash,
                    &command.principal_id.as_uuid(),
                    &channel_label(command.channel),
                    &credential_scope_label(command.credential_scope),
                    &command.classification.as_str(),
                    &command.policy_revision_id.as_uuid(),
                    &command.provider_registry_revision,
                    &provider_descriptor_revision,
                    &command.provider_affinity,
                    &created_at,
                    &last_seen_at,
                    &absolute_expires_at,
                    &idle_expires_at,
                ],
            )
            .await
            .map_err(database_error)?;
        let stored =
            load_governance_session_tx(&transaction, command.tenant_id, &command.session_id_hash)
                .await?
                .ok_or(GovernanceRepositoryError::Database)?;
        transaction.commit().await.map_err(database_error)?;
        Ok(GovernanceSessionUpsertOutcome::Stored(Box::new(stored)))
    }

    pub async fn governance_load_sessions(
        &self,
        tenant_id: TenantId,
        now_unix_ms: u64,
        limit: u16,
    ) -> Result<Vec<GovernanceSessionRecord>, GovernanceRepositoryError> {
        self.governance_timeout(self.governance_load_sessions_inner(tenant_id, now_unix_ms, limit))
            .await
    }

    async fn governance_load_sessions_inner(
        &self,
        tenant_id: TenantId,
        now_unix_ms: u64,
        limit: u16,
    ) -> Result<Vec<GovernanceSessionRecord>, GovernanceRepositoryError> {
        if limit == 0 || limit > 4_096 {
            return Err(GovernanceRepositoryError::InvalidInput);
        }
        let now = to_i64(now_unix_ms)?;
        let limit = i64::from(limit);
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, tenant_id)
            .await
            .map_err(database_error)?;
        let rows = transaction
            .query(
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
                 WHERE session.tenant_id = $1
                   AND (revocation.session_id_hash IS NOT NULL
                        OR (session.absolute_expires_at_unix_ms > $2
                            AND session.idle_expires_at_unix_ms > $2))
                 ORDER BY session.last_seen_at_unix_ms DESC, session.session_id_hash
                 LIMIT $3",
                &[&tenant_id.as_uuid(), &now, &limit],
            )
            .await
            .map_err(database_error)?;
        let sessions = rows
            .into_iter()
            .map(|row| governance_session_from_row(tenant_id, &row))
            .collect::<Result<Vec<_>, GovernanceRepositoryError>>()?;
        transaction.commit().await.map_err(database_error)?;
        Ok(sessions)
    }

    pub async fn governance_count_concurrent_sessions(
        &self,
        tenant_id: TenantId,
        principal_id: PrincipalId,
        now_unix_ms: u64,
    ) -> Result<u64, GovernanceRepositoryError> {
        self.governance_timeout(self.governance_count_concurrent_sessions_inner(
            tenant_id,
            principal_id,
            now_unix_ms,
        ))
        .await
    }

    async fn governance_count_concurrent_sessions_inner(
        &self,
        tenant_id: TenantId,
        principal_id: PrincipalId,
        now_unix_ms: u64,
    ) -> Result<u64, GovernanceRepositoryError> {
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, tenant_id)
            .await
            .map_err(database_error)?;
        let count =
            count_concurrent_sessions_tx(&transaction, tenant_id, principal_id, now_unix_ms)
                .await?;
        transaction.commit().await.map_err(database_error)?;
        Ok(count)
    }

    pub async fn governance_session_revocation_epoch(
        &self,
        tenant_id: TenantId,
    ) -> Result<u64, GovernanceRepositoryError> {
        self.governance_timeout(self.governance_session_revocation_epoch_inner(tenant_id))
            .await
    }

    async fn governance_session_revocation_epoch_inner(
        &self,
        tenant_id: TenantId,
    ) -> Result<u64, GovernanceRepositoryError> {
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, tenant_id)
            .await
            .map_err(database_error)?;
        let row = transaction
            .query_opt(
                "SELECT session_revocation_epoch FROM prodex_tenants WHERE tenant_id = $1",
                &[&tenant_id.as_uuid()],
            )
            .await
            .map_err(database_error)?
            .ok_or(GovernanceRepositoryError::NotFound)?;
        let epoch = from_i64(row.get(0))?;
        transaction.commit().await.map_err(database_error)?;
        Ok(epoch)
    }

    pub async fn governance_revoke_session(
        &self,
        command: GovernanceSessionRevokeCommand,
    ) -> Result<GovernanceWriteOutcome, GovernanceRepositoryError> {
        self.governance_timeout(self.governance_revoke_session_inner(command))
            .await
    }

    async fn governance_revoke_session_inner(
        &self,
        command: GovernanceSessionRevokeCommand,
    ) -> Result<GovernanceWriteOutcome, GovernanceRepositoryError> {
        validate_governance_session_revoke(&command)?;
        let revoked_at = to_i64(command.revoked_at_unix_ms)?;
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, command.tenant_id)
            .await
            .map_err(database_error)?;
        if load_governance_session_tx(&transaction, command.tenant_id, &command.session_id_hash)
            .await?
            .is_none()
        {
            return Err(GovernanceRepositoryError::NotFound);
        }
        let existing = transaction
            .query_opt(
                "SELECT revoked_at_unix_ms, reason_code FROM prodex_session_revocations
                 WHERE tenant_id = $1 AND session_id_hash = $2 FOR UPDATE",
                &[&command.tenant_id.as_uuid(), &command.session_id_hash],
            )
            .await
            .map_err(database_error)?;
        if let Some(existing) = existing {
            if existing.get::<_, String>(1) != command.reason_code {
                return Err(GovernanceRepositoryError::Conflict);
            }
            transaction.commit().await.map_err(database_error)?;
            return Ok(GovernanceWriteOutcome::Replayed);
        }
        transaction
            .execute(
                "INSERT INTO prodex_session_revocations (
                    tenant_id, session_id_hash, revoked_at_unix_ms, reason_code
                 ) VALUES ($1, $2, $3, $4)",
                &[
                    &command.tenant_id.as_uuid(),
                    &command.session_id_hash,
                    &revoked_at,
                    &command.reason_code,
                ],
            )
            .await
            .map_err(database_error)?;
        let updated = transaction
            .execute(
                "UPDATE prodex_tenants
                 SET session_revocation_epoch = session_revocation_epoch + 1
                 WHERE tenant_id = $1",
                &[&command.tenant_id.as_uuid()],
            )
            .await
            .map_err(database_error)?;
        if updated != 1 {
            return Err(GovernanceRepositoryError::NotFound);
        }
        append_audit_outbox_tx(&transaction, command.audit_outbox).await?;
        transaction.commit().await.map_err(database_error)?;
        Ok(GovernanceWriteOutcome::Applied)
    }
}
