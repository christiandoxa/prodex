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

pub(super) async fn load_revision_row(
    transaction: &Transaction<'_>,
    tenant_id: TenantId,
    kind: GovernanceArtifactKind,
    revision_id: &str,
) -> Result<Option<RevisionRow>, GovernanceRepositoryError> {
    let statement = transaction
        .prepare_cached(LOAD_GOVERNANCE_REVISION_ARTIFACT_STATEMENT.sql)
        .await
        .map_err(database_error)?;
    transaction
        .query_opt(
            &statement,
            &[
                &tenant_id.as_uuid(),
                &artifact_kind_label(kind),
                &revision_id,
            ],
        )
        .await
        .map_err(database_error)?
        .map(|row| {
            Ok(RevisionRow {
                checksum: row.get(0),
                compiled_artifact: row.get(1),
                created_by: PrincipalId::from_uuid(row.get::<_, Uuid>(2)),
                created_at_unix_ms: from_i64(row.get(3))?,
            })
        })
        .transpose()
}

pub(super) async fn load_verified_snapshot(
    transaction: &Transaction<'_>,
    tenant_id: TenantId,
    kind: GovernanceArtifactKind,
    revision_id: &str,
    source: GovernanceSnapshotSource,
    validate_artifact: &mut impl FnMut(&[u8]) -> bool,
) -> Result<Option<GovernanceSnapshot>, GovernanceRepositoryError> {
    let Some(revision) = load_revision_row(transaction, tenant_id, kind, revision_id).await? else {
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

pub(super) async fn revision_id_for_fingerprint(
    transaction: &Transaction<'_>,
    tenant_id: TenantId,
    kind: GovernanceArtifactKind,
    fingerprint: &str,
) -> Result<Option<String>, GovernanceRepositoryError> {
    transaction
        .query_opt(
            "SELECT revision_id FROM prodex_governance_revision_artifacts
             WHERE tenant_id = $1 AND artifact_kind = $2 AND artifact_checksum = $3",
            &[
                &tenant_id.as_uuid(),
                &artifact_kind_label(kind),
                &fingerprint,
            ],
        )
        .await
        .map_err(database_error)
        .map(|row| row.map(|row| row.get(0)))
}

pub(super) async fn insert_revision_metadata(
    transaction: &Transaction<'_>,
    command: &GovernanceRevisionWriteCommand,
    checksum: &str,
    created_at: i64,
) -> Result<(), GovernanceRepositoryError> {
    let metadata = serde_json::json!({ "artifact_checksum": checksum });
    let inserted = match command.kind {
        GovernanceArtifactKind::Policy => {
            let revision_id = policy_revision_id(&command.revision_id)?;
            transaction
                .query_opt(
                    "INSERT INTO prodex_policy_revisions (
                        tenant_id, revision_id, artifact_checksum, compiled_metadata,
                        lifecycle_state, created_by, created_at_unix_ms
                     ) VALUES ($1, $2, $3, $4, 'draft', $5, $6)
                     ON CONFLICT DO NOTHING RETURNING revision_id",
                    &[
                        &command.tenant_id.as_uuid(),
                        &revision_id.as_uuid(),
                        &checksum,
                        &metadata,
                        &command.created_by.as_uuid(),
                        &created_at,
                    ],
                )
                .await
        }
        GovernanceArtifactKind::ClassificationRules => {
            transaction
                .query_opt(
                    "INSERT INTO prodex_classification_rule_revisions (
                        tenant_id, revision_id, artifact_checksum, compiled_metadata,
                        lifecycle_state, created_at_unix_ms
                     ) VALUES ($1, $2, $3, $4, 'draft', $5)
                     ON CONFLICT DO NOTHING RETURNING revision_id",
                    &[
                        &command.tenant_id.as_uuid(),
                        &command.revision_id,
                        &checksum,
                        &metadata,
                        &created_at,
                    ],
                )
                .await
        }
        GovernanceArtifactKind::ProviderRegistry => {
            transaction
                .query_opt(
                    "INSERT INTO prodex_provider_registry_revisions (
                        tenant_id, revision_id, artifact_checksum, lifecycle_state,
                        created_at_unix_ms
                     ) VALUES ($1, $2, $3, 'draft', $4)
                     ON CONFLICT DO NOTHING RETURNING revision_id",
                    &[
                        &command.tenant_id.as_uuid(),
                        &command.revision_id,
                        &checksum,
                        &created_at,
                    ],
                )
                .await
        }
        GovernanceArtifactKind::RoutingScores => {
            transaction
                .query_opt(
                    "INSERT INTO prodex_routing_score_revisions (
                        tenant_id, revision_id, artifact_checksum, fixed_point_weights,
                        lifecycle_state, created_at_unix_ms
                     ) VALUES ($1, $2, $3, $4, 'draft', $5)
                     ON CONFLICT DO NOTHING RETURNING revision_id",
                    &[
                        &command.tenant_id.as_uuid(),
                        &command.revision_id,
                        &checksum,
                        &metadata,
                        &created_at,
                    ],
                )
                .await
        }
    }
    .map_err(database_error)?;
    if inserted.is_none() {
        return Err(GovernanceRepositoryError::Conflict);
    }
    Ok(())
}

pub(super) async fn update_revision_state(
    transaction: &Transaction<'_>,
    tenant_id: TenantId,
    kind: GovernanceArtifactKind,
    revision_id: &str,
    state: &str,
) -> Result<(), GovernanceRepositoryError> {
    let query = format!(
        "UPDATE {} SET lifecycle_state = $3 WHERE tenant_id = $1 AND revision_id = $2",
        revision_table(kind)
    );
    let changed = if kind == GovernanceArtifactKind::Policy {
        let revision_id = policy_revision_id(revision_id)?;
        transaction
            .execute(
                &query,
                &[&tenant_id.as_uuid(), &revision_id.as_uuid(), &state],
            )
            .await
    } else {
        transaction
            .execute(&query, &[&tenant_id.as_uuid(), &revision_id, &state])
            .await
    }
    .map_err(database_error)?;
    if changed != 1 {
        return Err(GovernanceRepositoryError::NotFound);
    }
    Ok(())
}

pub(super) fn revision_id_from_row(
    row: &Row,
    index: usize,
    kind: GovernanceArtifactKind,
) -> String {
    if kind == GovernanceArtifactKind::Policy {
        row.get::<_, Uuid>(index).to_string()
    } else {
        row.get(index)
    }
}

pub(super) async fn load_pointer_for_kind(
    transaction: &Transaction<'_>,
    tenant_id: TenantId,
    kind: GovernanceArtifactKind,
) -> Result<Option<GovernancePointer>, GovernanceRepositoryError> {
    let statement = transaction
        .prepare_cached(postgres_governance_pointer_statements(kind).load.sql)
        .await
        .map_err(database_error)?;
    transaction
        .query_opt(&statement, &[&tenant_id.as_uuid()])
        .await
        .map_err(database_error)
        .map(|row| {
            row.map(|row| {
                let (active_revision_id, last_known_good_revision_id) = match kind {
                    GovernanceArtifactKind::Policy => (
                        row.get::<_, Option<Uuid>>(0).map(|value| value.to_string()),
                        row.get::<_, Option<Uuid>>(1).map(|value| value.to_string()),
                    ),
                    GovernanceArtifactKind::ClassificationRules
                    | GovernanceArtifactKind::ProviderRegistry
                    | GovernanceArtifactKind::RoutingScores => (
                        row.get::<_, Option<String>>(0),
                        row.get::<_, Option<String>>(1),
                    ),
                };
                GovernancePointer {
                    active_revision_id,
                    last_known_good_revision_id,
                    etag: row.get(2),
                }
            })
        })
}

pub(super) async fn insert_activation_history(
    transaction: &Transaction<'_>,
    request: &GovernanceActivationRequest,
    previous_revision_id: Option<&str>,
    occurred_at: i64,
) -> Result<(), GovernanceRepositoryError> {
    let activation_id = request.audit_outbox.audit.event.id.as_uuid();
    if request.kind == GovernanceArtifactKind::Policy {
        let revision_id = policy_revision_id(&request.revision_id)?;
        let previous_revision_id = previous_revision_id
            .map(policy_revision_id)
            .transpose()?
            .map(PolicyRevisionId::as_uuid);
        transaction
            .execute(
                "INSERT INTO prodex_policy_activation_history (
                    tenant_id, activation_id, revision_id, previous_revision_id,
                    action, actor_id, occurred_at_unix_ms
                 ) VALUES ($1, $2, $3, $4, $5, $6, $7)",
                &[
                    &request.tenant_id.as_uuid(),
                    &activation_id,
                    &revision_id.as_uuid(),
                    &previous_revision_id,
                    &request.action.as_str(),
                    &request.actor.id.as_uuid(),
                    &occurred_at,
                ],
            )
            .await
    } else {
        transaction
            .execute(
                "INSERT INTO prodex_governance_activation_history (
                    tenant_id, activation_id, artifact_kind, revision_id,
                    previous_revision_id, action, actor_id, idempotency_key,
                    occurred_at_unix_ms
                 ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
                &[
                    &request.tenant_id.as_uuid(),
                    &activation_id,
                    &artifact_kind_label(request.kind),
                    &request.revision_id,
                    &previous_revision_id,
                    &request.action.as_str(),
                    &request.actor.id.as_uuid(),
                    &request.idempotency_key.as_str(),
                    &occurred_at,
                ],
            )
            .await
    }
    .map(|_| ())
    .map_err(database_error)
}

pub(super) async fn load_activation_replay(
    transaction: &Transaction<'_>,
    request: &GovernanceActivationRequest,
) -> Result<Option<GovernanceActivationResult>, GovernanceRepositoryError> {
    let row = transaction
        .query_opt(
            "SELECT request_fingerprint, action, revision_id, resulting_etag
             FROM prodex_governance_mutation_idempotency
             WHERE tenant_id = $1 AND artifact_kind = $2 AND idempotency_key = $3",
            &[
                &request.tenant_id.as_uuid(),
                &artifact_kind_label(request.kind),
                &request.idempotency_key.as_str(),
            ],
        )
        .await
        .map_err(database_error)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let fingerprint = row.get::<_, String>(0);
    let action = row.get::<_, String>(1);
    let revision_id = row.get::<_, String>(2);
    let etag = row.get::<_, String>(3);
    if fingerprint != request.request_fingerprint
        || action != request.action.as_str()
        || revision_id != request.revision_id
    {
        return Err(GovernanceRepositoryError::Conflict);
    }
    let pointer = load_pointer_for_kind(transaction, request.tenant_id, request.kind)
        .await?
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

pub(super) fn policy_revision_id(
    value: &str,
) -> Result<PolicyRevisionId, GovernanceRepositoryError> {
    PolicyRevisionId::from_str(value).map_err(|_| GovernanceRepositoryError::InvalidInput)
}
