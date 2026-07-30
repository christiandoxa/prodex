use super::{
    AppState, RuntimeGatewayAdminRole, RuntimeGatewayAdminToken,
    RuntimeGatewayGuardrailWebhookConfig, RuntimeGatewayObservabilityConfig,
    RuntimeGatewaySsoConfig, RuntimeGatewayStateStore, RuntimeLocalRewriteProviderOptions,
    RuntimeLocalRewriteProxyStartOptions, TestUpstream, app_paths_for_root, artifacts,
    runtime_gateway_sqlite_create_current_schema_for_tests, start_runtime_local_rewrite_proxy,
    temp_root,
};
use prodex_domain::TenantId;
use rusqlite::Connection;

#[test]
fn gateway_policy_http_revocation_invalidates_cache_and_lkg() {
    let root = temp_root("gateway-policy-lifecycle");
    let paths = app_paths_for_root(root.clone());
    let database_path = root.join("gateway.sqlite");
    runtime_gateway_sqlite_create_current_schema_for_tests(&database_path).unwrap();
    let tenant_a = TenantId::new();
    let tenant_b = TenantId::new();
    let connection = Connection::open(&database_path).unwrap();
    for tenant in [tenant_a, tenant_b] {
        connection
            .execute(
                "INSERT INTO prodex_tenants (tenant_id, display_name, created_at_unix_ms, updated_at_unix_ms)
                 VALUES (?1, 'test tenant', 1, 1)",
                [tenant.to_string()],
            )
            .unwrap();
    }
    drop(connection);

    let maker_token = "maker-token";
    let checker_token = "checker-token";
    let other_token = "other-token";
    let admin = |name: &str, token: &str, tenant_id: TenantId| RuntimeGatewayAdminToken {
        name: name.to_string(),
        token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(token),
        role: RuntimeGatewayAdminRole::Admin,
        tenant_id: Some(tenant_id.to_string()),
        team_id: None,
        project_id: None,
        user_id: None,
        budget_id: None,
        allowed_key_prefixes: Vec::new(),
    };
    let upstream = TestUpstream::start_n(0);
    let proxy = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths: &paths,
        state: &AppState::default(),
        upstream_base_url: format!("http://{}/v1", upstream.addr),
        provider: RuntimeLocalRewriteProviderOptions::OpenAiResponses {
            api_keys: vec!["upstream-key".to_string()],
        },
        upstream_no_proxy: false,
        smart_context_enabled: false,
        presidio_redaction_enabled: false,
        model_context_window_tokens: None,
        preferred_listen_addr: Some("127.0.0.1:0"),
        gateway_auth_token_hash: None,
        gateway_admin_tokens: vec![
            admin("maker", maker_token, tenant_a),
            admin("checker", checker_token, tenant_a),
            admin("other", other_token, tenant_b),
        ],
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::sqlite(database_path.clone()),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: None,
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .unwrap();
    let client = reqwest::blocking::Client::new();
    let base = format!("http://{}/v1/prodex/gateway/policies", proxy.listen_addr);
    let revision_v1 = prodex_domain::PolicyRevisionId::new().to_string();
    let revision_v2 = prodex_domain::PolicyRevisionId::new().to_string();

    let unknown_field = client
        .post(format!("{base}/validate"))
        .bearer_auth(maker_token)
        .json(&serde_json::json!({"artifact": {"unknown_rule": true}}))
        .send()
        .unwrap();
    assert_eq!(unknown_field.status().as_u16(), 400);
    let invalid_semantics = client
        .post(format!("{base}/validate"))
        .bearer_auth(maker_token)
        .json(&serde_json::json!({"artifact": {"config_version": 2}}))
        .send()
        .unwrap();
    assert_eq!(invalid_semantics.status().as_u16(), 400);
    let valid_policy = client
        .post(format!("{base}/validate"))
        .bearer_auth(maker_token)
        .json(&serde_json::json!({
            "revision_id": revision_v1,
            "artifact": {"policy_revision": revision_v1}
        }))
        .send()
        .unwrap();
    assert_eq!(valid_policy.status().as_u16(), 200);
    let valid_policy = valid_policy.json::<serde_json::Value>().unwrap();
    assert_eq!(valid_policy["signing"]["algorithm"], "ed25519");
    assert!(
        valid_policy["signing"]["payload_base64"]
            .as_str()
            .is_some_and(|payload| !payload.is_empty())
    );

    let unknown_signing_key = client
        .post(&base)
        .bearer_auth(maker_token)
        .header("Idempotency-Key", "unknown-governance-signing-key")
        .json(&serde_json::json!({
            "revision_id": revision_v1,
            "artifact": {"policy_revision": revision_v1},
            "authenticity": {"key_id": "unknown-key", "signature": "AQID"}
        }))
        .send()
        .unwrap();
    assert_eq!(unknown_signing_key.status().as_u16(), 400);

    let mismatched_revision = client
        .post(&base)
        .bearer_auth(maker_token)
        .header("Idempotency-Key", "mismatched-policy-revision")
        .json(&serde_json::json!({
            "revision_id": revision_v1,
            "artifact": {"policy_revision": revision_v2}
        }))
        .send()
        .unwrap();
    assert_eq!(mismatched_revision.status().as_u16(), 400);

    let unauthenticated = client.get(&base).send().unwrap();
    assert_eq!(unauthenticated.status().as_u16(), 401);
    let missing_idempotency = client
        .post(&base)
        .bearer_auth(maker_token)
        .json(&serde_json::json!({"revision_id": revision_v1, "artifact": {"effect": "allow"}}))
        .send()
        .unwrap();
    assert_eq!(missing_idempotency.status().as_u16(), 400);

    let created = artifacts::create_revision(
        &client,
        &base,
        maker_token,
        &revision_v1,
        "allow",
        "create-v1",
    );
    assert_eq!(created["replayed"], false);
    assert_eq!(
        artifacts::create_revision(
            &client,
            &base,
            maker_token,
            &revision_v1,
            "allow",
            "create-v1",
        )["replayed"],
        true
    );
    let conflicting_create = client
        .post(&base)
        .bearer_auth(maker_token)
        .header("Idempotency-Key", "create-v1")
        .json(&serde_json::json!({
            "revision_id": revision_v2,
            "artifact": {
                "policy_revision": revision_v2,
                "policy_failure_mode": "open"
            }
        }))
        .send()
        .unwrap();
    assert_eq!(conflicting_create.status().as_u16(), 409);
    let approval_v1 = artifacts::submit(&client, &base, maker_token, &revision_v1, "approval-v1");
    let submit_replay = client
        .post(format!("{base}/{revision_v1}/submit"))
        .bearer_auth(maker_token)
        .header("Idempotency-Key", "submit-approval-v1")
        .json(&serde_json::json!({"approval_id": "approval-v1", "required_quorum": 1}))
        .send()
        .unwrap();
    assert_eq!(submit_replay.status().as_u16(), 200);
    assert_eq!(
        submit_replay.json::<serde_json::Value>().unwrap()["replayed"],
        true
    );
    let generated_approval = || {
        client
            .post(format!("{base}/{revision_v1}/submit"))
            .bearer_auth(maker_token)
            .header("Idempotency-Key", "submit-generated-approval")
            .json(&serde_json::json!({"required_quorum": 1}))
            .send()
            .unwrap()
    };
    let generated_first = generated_approval();
    assert_eq!(generated_first.status().as_u16(), 201);
    let generated_first = generated_first.json::<serde_json::Value>().unwrap();
    let generated_replay = generated_approval();
    assert_eq!(generated_replay.status().as_u16(), 200);
    let generated_replay = generated_replay.json::<serde_json::Value>().unwrap();
    assert_eq!(
        generated_replay["approval_id"],
        generated_first["approval_id"]
    );
    assert_eq!(generated_replay["replayed"], true);

    let self_vote = artifacts::vote(
        &client,
        &base,
        maker_token,
        &revision_v1,
        &approval_v1,
        1,
        "approve",
        "self-vote",
    );
    assert_eq!(self_vote.status().as_u16(), 403);
    assert_eq!(
        self_vote.json::<serde_json::Value>().unwrap()["error"]["code"],
        "governance_policy_self_approval_forbidden"
    );
    let stale_vote = artifacts::vote(
        &client,
        &base,
        checker_token,
        &revision_v1,
        &approval_v1,
        0,
        "approve",
        "stale-vote",
    );
    assert_eq!(stale_vote.status().as_u16(), 409);
    assert_eq!(
        stale_vote.json::<serde_json::Value>().unwrap()["error"]["code"],
        "governance_policy_version_stale"
    );
    let approved = artifacts::vote(
        &client,
        &base,
        checker_token,
        &revision_v1,
        &approval_v1,
        1,
        "approve",
        "approve-v1",
    );
    assert_eq!(approved.status().as_u16(), 200);
    let approved = approved.json::<serde_json::Value>().unwrap();
    let approved_replay = artifacts::vote(
        &client,
        &base,
        checker_token,
        &revision_v1,
        &approval_v1,
        1,
        "approve",
        "approve-v1",
    );
    assert_eq!(approved_replay.status().as_u16(), 200);
    assert_eq!(
        approved_replay.json::<serde_json::Value>().unwrap(),
        approved
    );
    assert_eq!(
        artifacts::vote(
            &client,
            &base,
            checker_token,
            &revision_v1,
            &approval_v1,
            1,
            "reject",
            "approve-v1",
        )
        .status()
        .as_u16(),
        409
    );
    let active_v1 = artifacts::activate(
        &client,
        &base,
        checker_token,
        &revision_v1,
        &approval_v1,
        "*",
        "activate-v1",
        "activate",
    );
    assert_eq!(active_v1.status().as_u16(), 200);
    let etag_v1 = active_v1.headers()["etag"].to_str().unwrap().to_string();
    let replay = artifacts::activate(
        &client,
        &base,
        checker_token,
        &revision_v1,
        &approval_v1,
        "*",
        "activate-v1",
        "activate",
    );
    assert_eq!(replay.status().as_u16(), 200);
    assert_eq!(
        replay.json::<serde_json::Value>().unwrap()["replayed"],
        true
    );

    let cross_tenant = client
        .get(format!("{base}/{revision_v1}"))
        .bearer_auth(other_token)
        .send()
        .unwrap();
    assert_eq!(cross_tenant.status().as_u16(), 404);

    artifacts::create_revision(
        &client,
        &base,
        maker_token,
        &revision_v2,
        "deny",
        "create-v2",
    );
    let approval_v2 = artifacts::submit(&client, &base, maker_token, &revision_v2, "approval-v2");
    assert_eq!(
        artifacts::vote(
            &client,
            &base,
            checker_token,
            &revision_v2,
            &approval_v2,
            1,
            "approve",
            "approve-v2",
        )
        .status()
        .as_u16(),
        200
    );
    let stale_activation = artifacts::activate(
        &client,
        &base,
        checker_token,
        &revision_v2,
        &approval_v2,
        "stale-etag",
        "activate-v2-stale",
        "activate",
    );
    assert_eq!(stale_activation.status().as_u16(), 412);

    let connection = Connection::open(&database_path).unwrap();
    connection
        .execute_batch(
            "CREATE TRIGGER fail_policy_audit BEFORE INSERT ON prodex_audit_log
             BEGIN SELECT RAISE(ABORT, 'audit unavailable'); END;",
        )
        .unwrap();
    let audit_failure = artifacts::activate(
        &client,
        &base,
        checker_token,
        &revision_v2,
        &approval_v2,
        &etag_v1,
        "activate-v2-audit-failure",
        "activate",
    );
    assert_eq!(audit_failure.status().as_u16(), 503);
    let status: serde_json::Value = client
        .get(format!("{base}/status"))
        .bearer_auth(checker_token)
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(status["active_revision_id"], revision_v1);
    connection
        .execute_batch("DROP TRIGGER fail_policy_audit;")
        .unwrap();
    drop(connection);

    let active_v2 = artifacts::activate(
        &client,
        &base,
        checker_token,
        &revision_v2,
        &approval_v2,
        &etag_v1,
        "activate-v2",
        "activate",
    );
    assert_eq!(active_v2.status().as_u16(), 200);
    let etag_v2 = active_v2.headers()["etag"].to_str().unwrap().to_string();
    let rollback_approval = artifacts::submit(
        &client,
        &base,
        maker_token,
        &revision_v1,
        "approval-v1-rollback",
    );
    assert_eq!(
        artifacts::vote(
            &client,
            &base,
            checker_token,
            &revision_v1,
            &rollback_approval,
            1,
            "approve",
            "approve-v1-rollback",
        )
        .status()
        .as_u16(),
        200
    );
    let rollback = artifacts::activate(
        &client,
        &base,
        checker_token,
        &revision_v1,
        &rollback_approval,
        &etag_v2,
        "rollback-v1",
        "rollback",
    );
    assert_eq!(rollback.status().as_u16(), 200);
    let rollback_etag = rollback.headers()["etag"].to_str().unwrap().to_string();
    let rollback: serde_json::Value = rollback.json().unwrap();
    assert_eq!(rollback["revision_id"], revision_v1);
    assert_eq!(rollback["last_known_good_revision_id"], revision_v1);

    let revoked = artifacts::activate(
        &client,
        &base,
        checker_token,
        &revision_v1,
        &rollback_approval,
        &rollback_etag,
        "revoke-v1",
        "revoke",
    );
    assert_eq!(revoked.status().as_u16(), 200);
    let revoked_etag = revoked.headers()["etag"].to_str().unwrap().to_string();
    let revoked: serde_json::Value = revoked.json().unwrap();
    assert_eq!(revoked["object"], "governance.policy_revocation");
    assert_eq!(revoked["active_revision_id"], serde_json::Value::Null);
    assert_eq!(
        revoked["last_known_good_revision_id"],
        serde_json::Value::Null
    );
    let revoke_replay = artifacts::activate(
        &client,
        &base,
        checker_token,
        &revision_v1,
        &rollback_approval,
        &rollback_etag,
        "revoke-v1",
        "revoke",
    );
    assert_eq!(revoke_replay.status().as_u16(), 200);
    assert_eq!(
        revoke_replay.json::<serde_json::Value>().unwrap()["replayed"],
        true
    );
    let status: serde_json::Value = client
        .get(format!("{base}/status"))
        .bearer_auth(checker_token)
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(status["active_revision_id"], serde_json::Value::Null);
    assert_eq!(
        status["last_known_good_revision_id"],
        serde_json::Value::Null
    );
    assert_eq!(
        artifacts::activate(
            &client,
            &base,
            checker_token,
            &revision_v1,
            &rollback_approval,
            &revoked_etag,
            "reactivate-revoked-v1",
            "activate",
        )
        .status()
        .as_u16(),
        409
    );
    let connection = Connection::open(&database_path).unwrap();
    assert_eq!(
        connection
            .query_row(
                "SELECT lifecycle_state FROM prodex_policy_revisions
                 WHERE tenant_id = ?1 AND revision_id = ?2",
                rusqlite::params![tenant_a.to_string(), revision_v1],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
        "revoked"
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM prodex_policy_activation_history
                 WHERE tenant_id = ?1 AND revision_id = ?2 AND action = 'revoke'",
                rusqlite::params![tenant_a.to_string(), revision_v1],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        1
    );
    drop(connection);
    assert_eq!(
        prodex_storage_sqlite_runtime::GovernanceSqliteRepository::open(&database_path)
            .unwrap()
            .load_snapshot(
                tenant_a,
                prodex_storage::GovernanceArtifactKind::Policy,
                |_| true,
            ),
        Err(prodex_storage::GovernanceRepositoryError::SnapshotUnavailable)
    );

    let rejected_revision = prodex_domain::PolicyRevisionId::new().to_string();
    artifacts::create_revision(
        &client,
        &base,
        maker_token,
        &rejected_revision,
        "review",
        "create-rejected",
    );
    let rejected_approval = artifacts::submit(
        &client,
        &base,
        maker_token,
        &rejected_revision,
        "approval-rejected",
    );
    let rejected = artifacts::vote(
        &client,
        &base,
        checker_token,
        &rejected_revision,
        &rejected_approval,
        1,
        "reject",
        "reject-policy",
    );
    assert_eq!(rejected.status().as_u16(), 200);
    assert_eq!(
        rejected.json::<serde_json::Value>().unwrap()["state"],
        "rejected"
    );

    let session_registry = "session-registry-v1";
    let raw_current_session = "opaque-current-session";
    let current_hash = crate::runtime_launch::proxy_startup::local_rewrite_governance_session::runtime_gateway_governance_session_hash(
        &runtime_proxy_crate::RuntimeProxyRequest {
            method: "POST".to_string(),
            path_and_query: "/v1/responses".to_string(),
            headers: vec![("session_id".to_string(), raw_current_session.to_string())],
            body: Vec::new(),
        },
    )
    .unwrap();
    let admin_hash = "a".repeat(64);
    let connection = Connection::open(&database_path).unwrap();
    connection
        .execute(
            "INSERT INTO prodex_provider_registry_revisions (
                tenant_id, revision_id, artifact_checksum, lifecycle_state, created_at_unix_ms
             ) VALUES (?1, ?2, 'sha256:session-registry-v1', 'active', 1)",
            rusqlite::params![tenant_a.to_string(), session_registry],
        )
        .unwrap();
    for session_hash in [&admin_hash, &current_hash] {
        connection
            .execute(
                "INSERT INTO prodex_governance_sessions (
                    tenant_id, session_id_hash, principal_id, channel, credential_scope,
                    classification, policy_revision_id, provider_registry_revision,
                    provider_descriptor_revision, provider_affinity,
                    created_at_unix_ms, last_seen_at_unix_ms,
                    absolute_expires_at_unix_ms, idle_expires_at_unix_ms
                 ) VALUES (?1, ?2, ?3, 'api', 'data_plane', 'confidential', ?4, ?5,
                           1, 'openai', 1, 1, ?6, ?6)",
                rusqlite::params![
                    tenant_a.to_string(),
                    session_hash,
                    prodex_domain::PrincipalId::new().to_string(),
                    revision_v2,
                    session_registry,
                    i64::MAX,
                ],
            )
            .unwrap();
    }
    drop(connection);
    let session_base = format!("http://{}/v1/prodex/gateway/sessions", proxy.listen_addr);
    for _ in 0..2 {
        let response = client
            .post(format!("{session_base}/{admin_hash}/revoke"))
            .bearer_auth(maker_token)
            .header("Idempotency-Key", "revoke-admin-hash")
            .json(&serde_json::json!({"reason_code": "session.admin_revoke"}))
            .send()
            .unwrap();
        assert_eq!(response.status().as_u16(), 204);
        assert!(response.bytes().unwrap().is_empty());
    }
    let current = client
        .post(format!("{session_base}/current/revoke"))
        .bearer_auth(maker_token)
        .header("session_id", raw_current_session)
        .header("Idempotency-Key", "revoke-current-session")
        .send()
        .unwrap();
    assert_eq!(current.status().as_u16(), 204);
    let connection = Connection::open(&database_path).unwrap();
    let revoked = connection
        .query_row(
            "SELECT COUNT(*) FROM prodex_session_revocations WHERE tenant_id = ?1",
            [tenant_a.to_string()],
            |row| row.get::<_, i64>(0),
        )
        .unwrap();
    assert_eq!(revoked, 2);
    drop(connection);

    let export: serde_json::Value = client
        .post(format!(
            "http://{}/v1/prodex/gateway/audit/exports",
            proxy.listen_addr
        ))
        .bearer_auth(checker_token)
        .json(&serde_json::json!({"limit": 10}))
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(export["object"], "governance.audit_export");
    assert!(!export["data"].as_array().unwrap().is_empty());

    let outbox: serde_json::Value = client
        .get(format!(
            "http://{}/v1/prodex/gateway/governance/outbox",
            proxy.listen_addr
        ))
        .bearer_auth(checker_token)
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert!(outbox["pending"].as_u64().unwrap() > 0);
    let integrity: serde_json::Value = client
        .get(format!(
            "http://{}/v1/prodex/gateway/governance/audit/integrity",
            proxy.listen_addr
        ))
        .bearer_auth(checker_token)
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(integrity["chain_valid"], true);
    assert_eq!(integrity.as_object().unwrap().len(), 3);
    let claim = client
        .post(format!(
            "http://{}/v1/prodex/gateway/governance/outbox/claim",
            proxy.listen_addr
        ))
        .bearer_auth(checker_token)
        .header("Idempotency-Key", "claim-unsupported")
        .send()
        .unwrap();
    assert_eq!(claim.status().as_u16(), 503);

    let connection = Connection::open(&database_path).unwrap();
    connection
        .execute(
            "UPDATE prodex_audit_log SET outcome = 'failed'
             WHERE tenant_id = ?1 AND audit_event_id = (
                 SELECT audit_event_id FROM prodex_audit_log
                 WHERE tenant_id = ?1 LIMIT 1
             )",
            [tenant_a.to_string()],
        )
        .unwrap();
    drop(connection);
    let tampered: serde_json::Value = client
        .get(format!(
            "http://{}/v1/prodex/gateway/governance/audit/integrity",
            proxy.listen_addr
        ))
        .bearer_auth(checker_token)
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(tampered["chain_valid"], false);
    assert_eq!(tampered.as_object().unwrap().len(), 3);
}
