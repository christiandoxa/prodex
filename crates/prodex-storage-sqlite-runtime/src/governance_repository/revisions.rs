use super::*;

#[derive(Clone)]
pub(super) struct RevisionRow {
    pub(super) checksum: String,
    pub(super) compiled_artifact: Vec<u8>,
    pub(super) created_by: PrincipalId,
    pub(super) created_at_unix_ms: u64,
}

#[derive(Clone)]
pub(super) struct GovernancePointer {
    pub(super) active_revision_id: Option<String>,
    pub(super) last_known_good_revision_id: Option<String>,
    pub(super) etag: String,
}

pub(super) fn insert_revision_metadata(
    transaction: &Transaction<'_>,
    command: &GovernanceRevisionWriteCommand,
    checksum: &str,
    created_at: i64,
) -> Result<(), GovernanceRepositoryError> {
    let common = params![
        command.tenant_id.to_string(),
        command.revision_id,
        checksum,
        command.created_by.to_string(),
        created_at,
    ];
    match command.kind {
        GovernanceArtifactKind::Policy => transaction.execute(
            "INSERT INTO prodex_policy_revisions (
                tenant_id, revision_id, artifact_checksum, compiled_metadata,
                lifecycle_state, created_by, created_at_unix_ms
             ) VALUES (?1, ?2, ?3, '{}', 'draft', ?4, ?5)",
            common,
        ),
        GovernanceArtifactKind::ClassificationRules => transaction.execute(
            "INSERT INTO prodex_classification_rule_revisions (
                tenant_id, revision_id, artifact_checksum, compiled_metadata,
                lifecycle_state, created_at_unix_ms
             ) VALUES (?1, ?2, ?3, '{}', 'draft', ?5)",
            common,
        ),
        GovernanceArtifactKind::ProviderRegistry => transaction.execute(
            "INSERT INTO prodex_provider_registry_revisions (
                tenant_id, revision_id, artifact_checksum, lifecycle_state, created_at_unix_ms
             ) VALUES (?1, ?2, ?3, 'draft', ?5)",
            common,
        ),
        GovernanceArtifactKind::RoutingScores => transaction.execute(
            "INSERT INTO prodex_routing_score_revisions (
                tenant_id, revision_id, artifact_checksum, fixed_point_weights,
                lifecycle_state, created_at_unix_ms
             ) VALUES (?1, ?2, ?3, '{}', 'draft', ?5)",
            common,
        ),
    }
    .map(|_| ())
    .map_err(database_error)
}

pub(super) fn update_revision_state(
    transaction: &Transaction<'_>,
    tenant_id: TenantId,
    kind: GovernanceArtifactKind,
    revision_id: &str,
    state: &str,
) -> Result<(), GovernanceRepositoryError> {
    let table = revision_table(kind);
    let changed = transaction
        .execute(
            &format!(
                "UPDATE {table} SET lifecycle_state = ?3 WHERE tenant_id = ?1 AND revision_id = ?2"
            ),
            params![tenant_id.to_string(), revision_id, state],
        )
        .map_err(database_error)?;
    if changed != 1 {
        return Err(GovernanceRepositoryError::NotFound);
    }
    Ok(())
}

pub(super) fn load_revision_row(
    connection: &Connection,
    tenant_id: TenantId,
    kind: GovernanceArtifactKind,
    revision_id: &str,
) -> Result<Option<RevisionRow>, GovernanceRepositoryError> {
    let row = connection
        .query_row(
            "SELECT artifact_checksum, compiled_artifact, created_by, created_at_unix_ms
             FROM prodex_governance_revision_artifacts
             WHERE tenant_id = ?1 AND artifact_kind = ?2 AND revision_id = ?3",
            params![
                tenant_id.to_string(),
                artifact_kind_label(kind),
                revision_id
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            },
        )
        .optional()
        .map_err(database_error)?;
    row.map(
        |(checksum, compiled_artifact, created_by, created_at_unix_ms)| {
            Ok(RevisionRow {
                checksum,
                compiled_artifact,
                created_by: PrincipalId::from_str(&created_by)
                    .map_err(|_| GovernanceRepositoryError::Database)?,
                created_at_unix_ms: from_i64(created_at_unix_ms)?,
            })
        },
    )
    .transpose()
}

pub(super) fn revision_id_for_fingerprint(
    connection: &Connection,
    tenant_id: TenantId,
    kind: GovernanceArtifactKind,
    fingerprint: &str,
) -> Result<Option<String>, GovernanceRepositoryError> {
    connection
        .query_row(
            "SELECT revision_id FROM prodex_governance_revision_artifacts
             WHERE tenant_id = ?1 AND artifact_kind = ?2 AND artifact_checksum = ?3",
            params![
                tenant_id.to_string(),
                artifact_kind_label(kind),
                fingerprint
            ],
            |row| row.get(0),
        )
        .optional()
        .map_err(database_error)
}

pub(super) fn load_pointer(
    connection: &Connection,
    tenant_id: TenantId,
    kind: GovernanceArtifactKind,
) -> Result<Option<GovernancePointer>, GovernanceRepositoryError> {
    let table = pointer_table(kind);
    connection
        .query_row(
            &format!(
                "SELECT active_revision_id, last_known_good_revision_id, etag
                 FROM {table} WHERE tenant_id = ?1"
            ),
            [tenant_id.to_string()],
            |row| {
                Ok(GovernancePointer {
                    active_revision_id: row.get(0)?,
                    last_known_good_revision_id: row.get(1)?,
                    etag: row.get(2)?,
                })
            },
        )
        .optional()
        .map_err(database_error)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn store_pointer(
    transaction: &Transaction<'_>,
    tenant_id: TenantId,
    kind: GovernanceArtifactKind,
    active_revision_id: &str,
    last_known_good_revision_id: &str,
    etag: &str,
    updated_at: i64,
    previous: Option<&GovernancePointer>,
) -> Result<(), GovernanceRepositoryError> {
    let table = pointer_table(kind);
    let changed = if let Some(previous) = previous {
        transaction
            .execute(
                &format!(
                    "UPDATE {table}
                     SET active_revision_id = ?2, last_known_good_revision_id = ?3,
                         etag = ?4, updated_at_unix_ms = ?5
                     WHERE tenant_id = ?1 AND etag = ?6"
                ),
                params![
                    tenant_id.to_string(),
                    active_revision_id,
                    last_known_good_revision_id,
                    etag,
                    updated_at,
                    previous.etag,
                ],
            )
            .map_err(database_error)?
    } else {
        transaction
            .execute(
                &format!(
                    "INSERT INTO {table} (
                        tenant_id, active_revision_id, last_known_good_revision_id,
                        etag, updated_at_unix_ms
                     ) VALUES (?1, ?2, ?3, ?4, ?5)"
                ),
                params![
                    tenant_id.to_string(),
                    active_revision_id,
                    last_known_good_revision_id,
                    etag,
                    updated_at,
                ],
            )
            .map_err(database_error)?
    };
    if changed != 1 {
        return Err(GovernanceRepositoryError::EtagMismatch);
    }
    Ok(())
}

pub(super) fn insert_activation_history(
    transaction: &Transaction<'_>,
    request: &GovernanceActivationRequest,
    previous_revision_id: Option<&str>,
    occurred_at: i64,
) -> Result<(), GovernanceRepositoryError> {
    let event_id = request.audit_outbox.audit.event.id.to_string();
    if request.kind == GovernanceArtifactKind::Policy {
        transaction.execute(
            "INSERT INTO prodex_policy_activation_history (
                    tenant_id, activation_id, revision_id, previous_revision_id,
                    action, actor_id, occurred_at_unix_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                request.tenant_id.to_string(),
                event_id,
                request.revision_id,
                previous_revision_id,
                request.action.as_str(),
                request.actor.id.to_string(),
                occurred_at,
            ],
        )
    } else {
        transaction.execute(
            "INSERT INTO prodex_governance_activation_history (
                    tenant_id, activation_id, artifact_kind, revision_id,
                    previous_revision_id, action, actor_id, idempotency_key,
                    occurred_at_unix_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                request.tenant_id.to_string(),
                event_id,
                artifact_kind_label(request.kind),
                request.revision_id,
                previous_revision_id,
                request.action.as_str(),
                request.actor.id.to_string(),
                request.idempotency_key.as_str(),
                occurred_at,
            ],
        )
    }
    .map(|_| ())
    .map_err(database_error)
}

pub(super) fn load_activation_replay(
    connection: &Connection,
    request: &GovernanceActivationRequest,
) -> Result<Option<GovernanceActivationResult>, GovernanceRepositoryError> {
    let row = connection
        .query_row(
            "SELECT request_fingerprint, action, revision_id, resulting_etag
             FROM prodex_governance_mutation_idempotency
             WHERE tenant_id = ?1 AND artifact_kind = ?2 AND idempotency_key = ?3",
            params![
                request.tenant_id.to_string(),
                artifact_kind_label(request.kind),
                request.idempotency_key.as_str(),
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional()
        .map_err(database_error)?;
    let Some((fingerprint, action, revision_id, etag)) = row else {
        return Ok(None);
    };
    if fingerprint != request.request_fingerprint
        || action != request.action.as_str()
        || revision_id != request.revision_id
    {
        return Err(GovernanceRepositoryError::Conflict);
    }
    let pointer = load_pointer(connection, request.tenant_id, request.kind)?
        .ok_or(GovernanceRepositoryError::Database)?;
    Ok(Some(GovernanceActivationResult {
        outcome: GovernanceWriteOutcome::Replayed,
        kind: request.kind,
        revision_id,
        etag,
        last_known_good_revision_id: pointer
            .last_known_good_revision_id
            .ok_or(GovernanceRepositoryError::Database)?,
    }))
}

pub(super) fn load_verified_snapshot(
    connection: &Connection,
    tenant_id: TenantId,
    kind: GovernanceArtifactKind,
    revision_id: &str,
    source: GovernanceSnapshotSource,
    validate_artifact: &mut impl FnMut(&[u8]) -> bool,
) -> Result<Option<GovernanceSnapshot>, GovernanceRepositoryError> {
    let Some(revision) = load_revision_row(connection, tenant_id, kind, revision_id)? else {
        return Ok(None);
    };
    if revision.checksum != artifact_checksum(&revision.compiled_artifact)
        || !validate_artifact(&revision.compiled_artifact)
    {
        return Ok(None);
    }
    Ok(Some(GovernanceSnapshot {
        tenant_id,
        kind,
        revision_id: revision_id.to_string(),
        compiled_artifact: revision.compiled_artifact,
        source,
    }))
}

fn pointer_table(kind: GovernanceArtifactKind) -> &'static str {
    match kind {
        GovernanceArtifactKind::Policy => "prodex_policy_pointers",
        GovernanceArtifactKind::ClassificationRules => "prodex_classification_rule_pointers",
        GovernanceArtifactKind::ProviderRegistry => "prodex_provider_registry_pointers",
        GovernanceArtifactKind::RoutingScores => "prodex_routing_score_pointers",
    }
}

pub(super) fn validate_revision_id(
    kind: GovernanceArtifactKind,
    revision_id: &str,
) -> Result<(), GovernanceRepositoryError> {
    if kind == GovernanceArtifactKind::Policy {
        prodex_domain::PolicyRevisionId::from_str(revision_id)
            .map_err(|_| GovernanceRepositoryError::InvalidInput)?;
    }
    Ok(())
}
