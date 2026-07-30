use super::*;

#[test]
fn malformed_active_snapshot_keeps_lkg_and_rollback_is_atomic() {
    let tenant_id = TenantId::new();
    let database = TestDatabase::new(&[tenant_id]);
    let repository = database.repository();
    let maker = principal(tenant_id);
    let checker = principal(tenant_id);
    let mut audit = AuditCursor::default();
    let v1 = PolicyRevisionId::new().to_string();
    let approval_v1 = prepare_approved_revision(
        &repository,
        &mut audit,
        tenant_id,
        GovernanceArtifactKind::Policy,
        &v1,
        b"valid-v1",
        &maker,
        &checker,
        "policy/v1",
    );
    let active_v1 = repository
        .activate_revision(
            activation_request(
                tenant_id,
                GovernanceArtifactKind::Policy,
                &v1,
                &approval_v1,
                &checker,
                GovernanceActivationAction::Activate,
                None,
                "activate-v1",
                audit.next(tenant_id, &checker, "governance.revision.activate"),
            ),
            |_| true,
        )
        .unwrap();
    let v2 = PolicyRevisionId::new().to_string();
    let approval_v2 = prepare_approved_revision(
        &repository,
        &mut audit,
        tenant_id,
        GovernanceArtifactKind::Policy,
        &v2,
        b"malformed-v2",
        &maker,
        &checker,
        "policy/v2",
    );
    let active_v2 = repository
        .activate_revision(
            activation_request(
                tenant_id,
                GovernanceArtifactKind::Policy,
                &v2,
                &approval_v2,
                &checker,
                GovernanceActivationAction::Activate,
                Some(active_v1.etag),
                "activate-v2",
                audit.next(tenant_id, &checker, "governance.revision.activate"),
            ),
            |_| true,
        )
        .unwrap();
    let fallback = repository
        .load_snapshot(tenant_id, GovernanceArtifactKind::Policy, |input| {
            input.compiled_artifact != b"malformed-v2"
        })
        .unwrap();
    assert_eq!(fallback.revision_id, v1);
    assert_eq!(fallback.source, GovernanceSnapshotSource::LastKnownGood);

    let rollback_approval = prepare_approval_for_existing(
        &repository,
        &mut audit,
        tenant_id,
        GovernanceArtifactKind::Policy,
        ApprovalFingerprint::new(checksum(b"valid-v1")).unwrap(),
        &maker,
        &checker,
        "policy/rollback-v1",
    );
    let rollback = repository
        .activate_revision(
            activation_request(
                tenant_id,
                GovernanceArtifactKind::Policy,
                &v1,
                &rollback_approval,
                &checker,
                GovernanceActivationAction::Rollback,
                Some(active_v2.etag),
                "rollback-v1",
                audit.next(tenant_id, &checker, "governance.revision.rollback"),
            ),
            |_| true,
        )
        .unwrap();
    assert_eq!(rollback.revision_id, v1);
    assert_eq!(
        rollback.last_known_good_revision_id.as_deref(),
        Some(v1.as_str())
    );
}

#[test]
fn revocation_invalidates_active_lkg_and_future_activation() {
    let tenant_id = TenantId::new();
    let database = TestDatabase::new(&[tenant_id]);
    let repository = database.repository();
    let maker = principal(tenant_id);
    let checker = principal(tenant_id);
    let mut audit = AuditCursor::default();
    let v1 = PolicyRevisionId::new().to_string();
    let approval_v1 = prepare_approved_revision(
        &repository,
        &mut audit,
        tenant_id,
        GovernanceArtifactKind::Policy,
        &v1,
        b"policy-v1",
        &maker,
        &checker,
        "policy/revoke-v1",
    );
    let active_v1 = repository
        .activate_revision(
            activation_request(
                tenant_id,
                GovernanceArtifactKind::Policy,
                &v1,
                &approval_v1,
                &checker,
                GovernanceActivationAction::Activate,
                None,
                "activate-revoke-v1",
                audit.next(tenant_id, &checker, "governance.revision.activate"),
            ),
            |_| true,
        )
        .unwrap();
    let v2 = PolicyRevisionId::new().to_string();
    let approval_v2 = prepare_approved_revision(
        &repository,
        &mut audit,
        tenant_id,
        GovernanceArtifactKind::Policy,
        &v2,
        b"policy-v2",
        &maker,
        &checker,
        "policy/revoke-v2",
    );
    let active_v2 = repository
        .activate_revision(
            activation_request(
                tenant_id,
                GovernanceArtifactKind::Policy,
                &v2,
                &approval_v2,
                &checker,
                GovernanceActivationAction::Activate,
                Some(active_v1.etag),
                "activate-revoke-v2",
                audit.next(tenant_id, &checker, "governance.revision.activate"),
            ),
            |_| true,
        )
        .unwrap();

    let revoke_lkg = repository
        .activate_revision(
            revocation_request(
                tenant_id,
                GovernanceArtifactKind::Policy,
                &v1,
                &checker,
                Some(active_v2.etag),
                "revoke-lkg-v1",
                audit.next(tenant_id, &checker, "governance.revision.revoke"),
            ),
            |_| false,
        )
        .unwrap();
    assert_eq!(revoke_lkg.active_revision_id.as_deref(), Some(v2.as_str()));
    assert_eq!(revoke_lkg.last_known_good_revision_id, None);
    assert_eq!(
        repository
            .activate_revision(
                activation_request(
                    tenant_id,
                    GovernanceArtifactKind::Policy,
                    &v1,
                    &approval_v1,
                    &checker,
                    GovernanceActivationAction::Rollback,
                    Some(revoke_lkg.etag.clone()),
                    "rollback-revoked-v1",
                    audit.fork(tenant_id, &checker, "governance.revision.rollback", 97,),
                ),
                |_| true,
            )
            .unwrap_err(),
        GovernanceRepositoryError::Conflict
    );

    let v3 = PolicyRevisionId::new().to_string();
    let approval_v3 = prepare_approved_revision(
        &repository,
        &mut audit,
        tenant_id,
        GovernanceArtifactKind::Policy,
        &v3,
        b"policy-v3",
        &maker,
        &checker,
        "policy/revoke-approved-v3",
    );
    let revoked_v3 = repository
        .activate_revision(
            revocation_request(
                tenant_id,
                GovernanceArtifactKind::Policy,
                &v3,
                &checker,
                Some(revoke_lkg.etag),
                "revoke-approved-v3",
                audit.next(tenant_id, &checker, "governance.revision.revoke"),
            ),
            |_| true,
        )
        .unwrap();
    assert_eq!(revoked_v3.active_revision_id.as_deref(), Some(v2.as_str()));
    assert_eq!(revoked_v3.last_known_good_revision_id, None);
    assert_eq!(
        repository
            .activate_revision(
                activation_request(
                    tenant_id,
                    GovernanceArtifactKind::Policy,
                    &v3,
                    &approval_v3,
                    &checker,
                    GovernanceActivationAction::Activate,
                    Some(revoked_v3.etag.clone()),
                    "activate-revoked-v3",
                    audit.fork(tenant_id, &checker, "governance.revision.activate", 98,),
                ),
                |_| true,
            )
            .unwrap_err(),
        GovernanceRepositoryError::Conflict
    );

    let revoke_active = revocation_request(
        tenant_id,
        GovernanceArtifactKind::Policy,
        &v2,
        &checker,
        Some(revoked_v3.etag),
        "revoke-active-v2",
        audit.next(tenant_id, &checker, "governance.revision.revoke"),
    );
    let revoked = repository
        .activate_revision(revoke_active.clone(), |_| true)
        .unwrap();
    assert_eq!(revoked.active_revision_id, None);
    assert_eq!(revoked.last_known_good_revision_id, None);
    assert_eq!(
        repository
            .activate_revision(revoke_active, |_| true)
            .unwrap()
            .outcome,
        GovernanceWriteOutcome::Replayed
    );
    assert_eq!(
        repository.load_snapshot(tenant_id, GovernanceArtifactKind::Policy, |_| true),
        Err(GovernanceRepositoryError::SnapshotUnavailable)
    );
    assert_eq!(
        repository
            .activate_revision(
                activation_request(
                    tenant_id,
                    GovernanceArtifactKind::Policy,
                    &v2,
                    &approval_v2,
                    &checker,
                    GovernanceActivationAction::Activate,
                    Some(revoked.etag),
                    "reactivate-revoked-v2",
                    audit.next(tenant_id, &checker, "governance.revision.activate"),
                ),
                |_| true,
            )
            .unwrap_err(),
        GovernanceRepositoryError::Conflict
    );
}

#[test]
fn revocation_promotes_valid_lkg_and_replays_original_result_after_later_mutation() {
    let tenant_id = TenantId::new();
    let database = TestDatabase::new(&[tenant_id]);
    let repository = database.repository();
    let maker = principal(tenant_id);
    let checker = principal(tenant_id);
    let mut audit = AuditCursor::default();
    let v1 = PolicyRevisionId::new().to_string();
    let approval_v1 = prepare_approved_revision(
        &repository,
        &mut audit,
        tenant_id,
        GovernanceArtifactKind::Policy,
        &v1,
        b"promotion-v1",
        &maker,
        &checker,
        "policy/promotion-v1",
    );
    let active_v1 = repository
        .activate_revision(
            activation_request(
                tenant_id,
                GovernanceArtifactKind::Policy,
                &v1,
                &approval_v1,
                &checker,
                GovernanceActivationAction::Activate,
                None,
                "activate-promotion-v1",
                audit.next(tenant_id, &checker, "governance.revision.activate"),
            ),
            |_| true,
        )
        .unwrap();
    let v2 = PolicyRevisionId::new().to_string();
    let approval_v2 = prepare_approved_revision(
        &repository,
        &mut audit,
        tenant_id,
        GovernanceArtifactKind::Policy,
        &v2,
        b"promotion-v2",
        &maker,
        &checker,
        "policy/promotion-v2",
    );
    let active_v2 = repository
        .activate_revision(
            activation_request(
                tenant_id,
                GovernanceArtifactKind::Policy,
                &v2,
                &approval_v2,
                &checker,
                GovernanceActivationAction::Activate,
                Some(active_v1.etag),
                "activate-promotion-v2",
                audit.next(tenant_id, &checker, "governance.revision.activate"),
            ),
            |_| true,
        )
        .unwrap();
    let revoke_v2 = revocation_request(
        tenant_id,
        GovernanceArtifactKind::Policy,
        &v2,
        &checker,
        Some(active_v2.etag),
        "revoke-promote-v2",
        audit.next(tenant_id, &checker, "governance.revision.revoke"),
    );
    let promoted = repository
        .activate_revision(revoke_v2.clone(), |_| true)
        .unwrap();
    assert_eq!(promoted.active_revision_id.as_deref(), Some(v1.as_str()));
    assert_eq!(
        promoted.last_known_good_revision_id.as_deref(),
        Some(v1.as_str())
    );
    let revisions = repository
        .list_revisions(tenant_id, GovernanceArtifactKind::Policy)
        .unwrap();
    assert_eq!(
        revisions
            .iter()
            .find(|revision| revision.revision_id == v1)
            .unwrap()
            .lifecycle_state,
        "active"
    );

    let revoked_v1 = repository
        .activate_revision(
            revocation_request(
                tenant_id,
                GovernanceArtifactKind::Policy,
                &v1,
                &checker,
                Some(promoted.etag.clone()),
                "revoke-promoted-v1",
                audit.next(tenant_id, &checker, "governance.revision.revoke"),
            ),
            |_| true,
        )
        .unwrap();
    assert_eq!(revoked_v1.active_revision_id, None);
    let replay = repository.activate_revision(revoke_v2, |_| false).unwrap();
    assert_eq!(replay.outcome, GovernanceWriteOutcome::Replayed);
    assert_eq!(replay.etag, promoted.etag);
    assert_eq!(replay.active_revision_id, promoted.active_revision_id);
    assert_eq!(
        replay.last_known_good_revision_id,
        promoted.last_known_good_revision_id
    );

    let connection = Connection::open(database.path()).unwrap();
    let history: (Option<String>, Option<String>) = connection
        .query_row(
            "SELECT resulting_active_revision_id, promoted_revision_id
             FROM prodex_policy_activation_history
             WHERE tenant_id = ?1 AND revision_id = ?2 AND action = 'revoke'",
            rusqlite::params![tenant_id.to_string(), v2],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(history.0.as_deref(), Some(v1.as_str()));
    assert_eq!(history.1.as_deref(), Some(v1.as_str()));
    assert!(
        connection
            .execute(
                "UPDATE prodex_policy_revisions SET lifecycle_state = 'active'
                 WHERE tenant_id = ?1 AND revision_id = ?2",
                rusqlite::params![tenant_id.to_string(), v2],
            )
            .is_err()
    );
}

#[test]
fn revocation_refuses_non_pointer_lifecycle_fallback() {
    let tenant_id = TenantId::new();
    let database = TestDatabase::new(&[tenant_id]);
    let repository = database.repository();
    let maker = principal(tenant_id);
    let checker = principal(tenant_id);
    let mut audit = AuditCursor::default();
    let v1 = PolicyRevisionId::new().to_string();
    let approval_v1 = prepare_approved_revision(
        &repository,
        &mut audit,
        tenant_id,
        GovernanceArtifactKind::Policy,
        &v1,
        b"invalid-fallback-v1",
        &maker,
        &checker,
        "policy/invalid-fallback-v1",
    );
    let active_v1 = repository
        .activate_revision(
            activation_request(
                tenant_id,
                GovernanceArtifactKind::Policy,
                &v1,
                &approval_v1,
                &checker,
                GovernanceActivationAction::Activate,
                None,
                "activate-invalid-fallback-v1",
                audit.next(tenant_id, &checker, "governance.revision.activate"),
            ),
            |_| true,
        )
        .unwrap();
    let v2 = PolicyRevisionId::new().to_string();
    let approval_v2 = prepare_approved_revision(
        &repository,
        &mut audit,
        tenant_id,
        GovernanceArtifactKind::Policy,
        &v2,
        b"invalid-fallback-v2",
        &maker,
        &checker,
        "policy/invalid-fallback-v2",
    );
    let active_v2 = repository
        .activate_revision(
            activation_request(
                tenant_id,
                GovernanceArtifactKind::Policy,
                &v2,
                &approval_v2,
                &checker,
                GovernanceActivationAction::Activate,
                Some(active_v1.etag),
                "activate-invalid-fallback-v2",
                audit.next(tenant_id, &checker, "governance.revision.activate"),
            ),
            |_| true,
        )
        .unwrap();
    Connection::open(database.path())
        .unwrap()
        .execute(
            "UPDATE prodex_policy_revisions SET lifecycle_state = 'draft'
             WHERE tenant_id = ?1 AND revision_id = ?2",
            rusqlite::params![tenant_id.to_string(), v1],
        )
        .unwrap();

    let revoked = repository
        .activate_revision(
            revocation_request(
                tenant_id,
                GovernanceArtifactKind::Policy,
                &v2,
                &checker,
                Some(active_v2.etag),
                "revoke-invalid-fallback-v2",
                audit.next(tenant_id, &checker, "governance.revision.revoke"),
            ),
            |_| true,
        )
        .unwrap();
    assert_eq!(revoked.active_revision_id, None);
    assert_eq!(revoked.last_known_good_revision_id, None);
}

#[test]
fn approval_vote_cannot_revive_revoked_pending_revision() {
    let tenant_id = TenantId::new();
    let database = TestDatabase::new(&[tenant_id]);
    let repository = database.repository();
    let maker = principal(tenant_id);
    let checker = principal(tenant_id);
    let mut audit = AuditCursor::default();
    let revision_id = PolicyRevisionId::new().to_string();
    let command = revision_command(
        tenant_id,
        GovernanceArtifactKind::Policy,
        &revision_id,
        b"pending-then-revoked",
        maker.id,
    );
    repository
        .write_revision(
            command.clone(),
            audit.next(tenant_id, &maker, "governance.revision.write"),
        )
        .unwrap();
    let approval = ApprovalRecord::pending(
        ApprovalId::new("pending-then-revoked").unwrap(),
        tenant_id,
        ApprovalKind::PolicyRevision,
        ApprovalScope::new("policy/pending-then-revoked").unwrap(),
        command.fingerprint,
        maker.id,
        1,
        100_000,
    )
    .unwrap();
    repository
        .create_approval(
            approval.clone(),
            audit.next(tenant_id, &maker, "governance.approval.create"),
        )
        .unwrap();
    repository
        .activate_revision(
            revocation_request(
                tenant_id,
                GovernanceArtifactKind::Policy,
                &revision_id,
                &checker,
                None,
                "revoke-pending",
                audit.next(tenant_id, &checker, "governance.revision.revoke"),
            ),
            |_| true,
        )
        .unwrap();

    assert_eq!(
        repository
            .vote_approval(ApprovalVoteRequest {
                tenant_id,
                approval_id: approval.id.clone(),
                actor: checker.clone(),
                expected_version: 1,
                now_unix_ms: 2_000,
                reason: None,
                audit_outbox: audit.next(tenant_id, &checker, "governance.approval.vote"),
            })
            .unwrap_err(),
        GovernanceRepositoryError::Conflict
    );
    let stored_approval = repository.get_approval(tenant_id, &approval.id).unwrap();
    assert_eq!(stored_approval.state, ApprovalState::PendingApproval);
    assert_eq!(stored_approval.version, 1);
    let revisions = repository
        .list_revisions(tenant_id, GovernanceArtifactKind::Policy)
        .unwrap();
    assert_eq!(revisions[0].lifecycle_state, "revoked");
}
