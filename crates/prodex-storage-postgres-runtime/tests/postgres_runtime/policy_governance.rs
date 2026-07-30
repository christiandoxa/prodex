use super::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires PRODEX_TEST_POSTGRES_URL"]
async fn postgres_policy_governance_activates_and_replays_idempotently() {
    let url = std::env::var("PRODEX_TEST_POSTGRES_URL")
        .expect("PRODEX_TEST_POSTGRES_URL must point to the test PostgreSQL instance");
    let config = PostgresRuntimeConfig::new(url, 2).expect("test config should be valid");
    let pool = config
        .create_pool_explicit_no_tls()
        .expect("test pool should build");
    let repository = PostgresRepository::from_pool_with_config(pool.clone(), &config);
    let tenant_id = TenantId::new();
    create_tenant(&pool, tenant_id).await;
    let maker = governance_principal(tenant_id);
    let checker = governance_principal(tenant_id);
    let revision_id = PolicyRevisionId::new();
    let artifact = br#"{"version":1}"#.to_vec();
    let fingerprint = governance_checksum(&artifact);
    let (write_audit, digest) = governance_audit(
        tenant_id,
        &maker,
        "governance.policy.revision.write",
        1_800_000_000_001,
        None,
    );
    let write = GovernanceRevisionWriteCommand {
        storage_key: TenantStorageKey::tenant(tenant_id),
        tenant_id,
        kind: GovernanceArtifactKind::Policy,
        revision_id: revision_id.to_string(),
        fingerprint: ApprovalFingerprint::new(fingerprint.clone()).unwrap(),
        compiled_artifact: artifact,
        authenticity: Some(GovernanceArtifactAuthenticity {
            key_id: "release-2026-01".to_string(),
            signature_base64: "AQID".to_string(),
        }),
        created_by: maker.id,
        created_at_unix_ms: 1_800_000_000_001,
    };
    let write_idempotency = GovernanceMutationIdempotency {
        operation: IdempotentOperation::new(
            tenant_id,
            IdempotencyKey::new("write-policy-v1").unwrap(),
            "sha256:write-policy-v1",
        )
        .unwrap(),
        started_at_unix_ms: 1_800_000_000_000,
    };
    assert_eq!(
        repository
            .governance_write_revision_idempotent(
                write.clone(),
                write_audit.clone(),
                write_idempotency.clone(),
            )
            .await
            .unwrap(),
        GovernanceWriteOutcome::Applied
    );
    assert_eq!(
        repository
            .governance_write_revision_idempotent(write.clone(), write_audit, write_idempotency)
            .await
            .unwrap(),
        GovernanceWriteOutcome::Replayed
    );

    let approval_id = ApprovalId::new(format!("approval-{revision_id}")).unwrap();
    let approval = ApprovalRecord::pending(
        approval_id.clone(),
        tenant_id,
        ApprovalKind::PolicyRevision,
        ApprovalScope::new(format!("policy/{revision_id}")).unwrap(),
        ApprovalFingerprint::new(fingerprint).unwrap(),
        maker.id,
        1,
        1_900_000_000_000,
    )
    .unwrap();
    let (approval_audit, digest) = governance_audit(
        tenant_id,
        &maker,
        "governance.policy.approval.create",
        1_800_000_000_002,
        Some(digest),
    );
    let approval_idempotency = GovernanceMutationIdempotency {
        operation: IdempotentOperation::new(
            tenant_id,
            IdempotencyKey::new("submit-policy-v1").unwrap(),
            "sha256:submit-policy-v1",
        )
        .unwrap(),
        started_at_unix_ms: 1_800_000_000_001,
    };
    assert_eq!(
        repository
            .governance_create_approval_idempotent(
                approval.clone(),
                approval_audit.clone(),
                approval_idempotency.clone(),
            )
            .await
            .unwrap(),
        GovernanceWriteOutcome::Applied
    );
    assert_eq!(
        repository
            .governance_create_approval_idempotent(approval, approval_audit, approval_idempotency,)
            .await
            .unwrap(),
        GovernanceWriteOutcome::Replayed
    );
    let (vote_audit, digest) = governance_audit(
        tenant_id,
        &checker,
        "governance.policy.approval.approve",
        1_800_000_000_003,
        Some(digest),
    );
    let vote_idempotency = GovernanceMutationIdempotency {
        operation: IdempotentOperation::new(
            tenant_id,
            IdempotencyKey::new("approve-policy-v1").unwrap(),
            "sha256:approve-policy-v1",
        )
        .unwrap(),
        started_at_unix_ms: 1_800_000_000_002,
    };
    let vote = || ApprovalVoteRequest {
        tenant_id,
        approval_id: approval_id.clone(),
        actor: checker.clone(),
        expected_version: 1,
        now_unix_ms: 1_800_000_000_003,
        reason: None,
        audit_outbox: vote_audit.clone(),
    };
    let approved = repository
        .governance_transition_approval_idempotent(
            vote(),
            ApprovalAction::Approve,
            vote_idempotency.clone(),
        )
        .await
        .unwrap();
    let ApprovalVoteMutationOutcome::Applied(approved) = approved else {
        panic!("first vote must be applied");
    };
    assert_eq!(approved.version, 2);
    assert!(matches!(
        repository
            .governance_transition_approval_idempotent(
                vote(),
                ApprovalAction::Approve,
                vote_idempotency,
            )
            .await
            .unwrap(),
        ApprovalVoteMutationOutcome::Replayed(snapshot) if snapshot.version == 2
    ));

    let (activation_audit, _) = governance_audit(
        tenant_id,
        &maker,
        "governance.policy.revision.activate",
        1_800_000_000_004,
        Some(digest),
    );
    let request = GovernanceActivationRequest {
        tenant_id,
        kind: GovernanceArtifactKind::Policy,
        revision_id: revision_id.to_string(),
        approval_id: Some(approval_id),
        actor: maker.clone(),
        action: GovernanceActivationAction::Activate,
        expected_etag: None,
        idempotency_key: IdempotencyKey::new(format!("activate-{revision_id}")).unwrap(),
        request_fingerprint: format!("request-{revision_id}"),
        audit_outbox: activation_audit,
        activated_at_unix_ms: 1_800_000_000_004,
    };
    let activated = repository
        .governance_activate_revision(request.clone(), |input| {
            input.authenticity.is_some_and(|authenticity| {
                authenticity.key_id == "release-2026-01" && authenticity.signature_base64 == "AQID"
            })
        })
        .await
        .unwrap();
    assert_eq!(activated.outcome, GovernanceWriteOutcome::Applied);
    let replayed = repository
        .governance_activate_revision(request, |_| true)
        .await
        .unwrap();
    assert_eq!(replayed.outcome, GovernanceWriteOutcome::Replayed);
    assert_eq!(replayed.etag, activated.etag);
    let snapshot = repository
        .governance_load_snapshot(tenant_id, GovernanceArtifactKind::Policy, |input| {
            input.authenticity.is_some_and(|authenticity| {
                authenticity.key_id == "release-2026-01" && authenticity.signature_base64 == "AQID"
            })
        })
        .await
        .unwrap();
    assert_eq!(snapshot.revision_id, revision_id.to_string());
    assert_eq!(
        repository
            .governance_get_revision(
                tenant_id,
                GovernanceArtifactKind::Policy,
                &revision_id.to_string(),
            )
            .await
            .unwrap()
            .signature_key_id
            .as_deref(),
        Some("release-2026-01")
    );
    assert_eq!(
        repository
            .governance_status(tenant_id, GovernanceArtifactKind::Policy)
            .await
            .unwrap()
            .active_revision_id
            .as_deref(),
        Some(revision_id.to_string().as_str())
    );
    let exported = repository
        .governance_export_audit(tenant_id, 10)
        .await
        .unwrap();
    assert_eq!(exported.len(), 4);
    assert_eq!(exported[0].action, "governance.policy.revision.activate");
    assert_eq!(
        repository.governance_export_audit(tenant_id, 0).await,
        Err(prodex_storage::GovernanceRepositoryError::InvalidInput)
    );
    let claims = repository
        .governance_claim_siem_outbox_batch(tenant_id, 1_800_000_000_005, 2, 60_000)
        .await
        .unwrap();
    assert_eq!(claims.len(), 2);
    let other_claims = repository
        .governance_claim_siem_outbox_batch(tenant_id, 1_800_000_000_005, 4, 60_000)
        .await
        .unwrap();
    assert!(claims.iter().all(|claim| {
        other_claims
            .iter()
            .all(|other| other.event_id != claim.event_id)
    }));
    assert_eq!(
        repository
            .governance_finalize_siem_outbox_claim(
                &claims[0],
                true,
                1_800_000_000_006,
                prodex_storage::SiemOutboxRetryPolicy::bounded(3, 1_000, 10_000).unwrap(),
            )
            .await
            .unwrap(),
        prodex_storage::SiemOutboxDeliveryDecision::Delivered
    );

    let registry_revision = "registry-v1";
    let mut client = pool.get().await.unwrap();
    let transaction = client.transaction().await.unwrap();
    transaction
        .query_one(SET_TENANT_STATEMENT.sql, &[&tenant_id.to_string()])
        .await
        .unwrap();
    transaction
        .execute(
            "INSERT INTO prodex_provider_registry_revisions (
                tenant_id, revision_id, artifact_checksum, lifecycle_state, created_at_unix_ms
             ) VALUES ($1, $2, $3, 'active', $4)",
            &[
                &tenant_id.as_uuid(),
                &registry_revision,
                &"sha256:registry-v1",
                &1_800_000_000_007_i64,
            ],
        )
        .await
        .unwrap();
    transaction.commit().await.unwrap();
    let repository_two = PostgresRepository::from_pool_with_config(pool.clone(), &config);
    let principal_id = PrincipalId::new();
    let session_command = |session_id_hash: String| GovernanceSessionUpsertCommand {
        tenant_id,
        session_id_hash,
        principal_id,
        channel: Channel::Api,
        credential_scope: CredentialScope::DataPlane,
        classification: DataClassification::Confidential,
        policy_revision_id: revision_id,
        provider_registry_revision: registry_revision.to_string(),
        provider_descriptor_revision: 1,
        provider_affinity: Some("provider-v1".to_string()),
        created_at_unix_ms: 1_800_000_000_007,
        last_seen_at_unix_ms: 1_800_000_000_007,
        absolute_expires_at_unix_ms: 1_800_000_100_000,
        idle_expires_at_unix_ms: 1_800_000_010_000,
        max_concurrent: Some(1),
    };
    let session_a = "a".repeat(64);
    let session_b = "b".repeat(64);
    let (first, second) = tokio::join!(
        repository.governance_upsert_session(session_command(session_a.clone())),
        repository_two.governance_upsert_session(session_command(session_b.clone()))
    );
    let outcomes = [first.unwrap(), second.unwrap()];
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| matches!(outcome, GovernanceSessionUpsertOutcome::Stored(_)))
            .count(),
        1
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| matches!(
                outcome,
                GovernanceSessionUpsertOutcome::ConcurrentLimitReached
            ))
            .count(),
        1
    );
    let winning_session = outcomes
        .iter()
        .find_map(|outcome| match outcome {
            GovernanceSessionUpsertOutcome::Stored(record) => Some(record.session_id_hash.clone()),
            GovernanceSessionUpsertOutcome::ConcurrentLimitReached => None,
        })
        .unwrap();
    let previous_digest = repository
        .governance_latest_audit_digest(tenant_id)
        .await
        .unwrap();
    let (revoke_audit, _) = governance_audit(
        tenant_id,
        &maker,
        "governance.session.revoke",
        1_800_000_000_008,
        previous_digest,
    );
    let revoke = GovernanceSessionRevokeCommand {
        tenant_id,
        session_id_hash: winning_session,
        revoked_at_unix_ms: 1_800_000_000_008,
        reason_code: "session.revoked".to_string(),
        audit_outbox: revoke_audit,
    };
    assert_eq!(
        repository
            .governance_session_revocation_epoch(tenant_id)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        repository_two
            .governance_revoke_session(revoke.clone())
            .await
            .unwrap(),
        GovernanceWriteOutcome::Applied
    );
    assert_eq!(
        repository
            .governance_session_revocation_epoch(tenant_id)
            .await
            .unwrap(),
        1
    );
    let mut replay = revoke;
    replay.revoked_at_unix_ms += 1;
    assert_eq!(
        repository.governance_revoke_session(replay).await.unwrap(),
        GovernanceWriteOutcome::Replayed
    );
    assert_eq!(
        repository
            .governance_session_revocation_epoch(tenant_id)
            .await
            .unwrap(),
        1,
        "idempotent replay must not publish another invalidation"
    );
    assert_eq!(
        repository
            .governance_count_concurrent_sessions(tenant_id, principal_id, 1_800_000_000_008,)
            .await
            .unwrap(),
        0
    );
    let loaded = repository
        .governance_load_sessions(tenant_id, 1_800_000_000_008, 16)
        .await
        .unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(
        loaded[0].revocation_reason_code.as_deref(),
        Some("session.revoked")
    );
}
