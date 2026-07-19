use super::*;
use crate::runtime_launch::proxy_startup::local_rewrite_gateway_backend_connection::runtime_gateway_sqlite_create_current_schema_for_tests;
use prodex_domain::{DataClassification, PrincipalId, SecretRef, TenantId};
use prodex_provider_core::{ProviderId, provider_adapter};
use rusqlite::Connection;
use sha2::{Digest, Sha256};

#[test]
fn gateway_policy_http_enforces_maker_checker_replay_cas_tenant_and_lkg() {
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
        .json(&serde_json::json!({"artifact": {}}))
        .send()
        .unwrap();
    assert_eq!(valid_policy.status().as_u16(), 200);

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

    create_revision(
        &client,
        &base,
        maker_token,
        &revision_v1,
        "allow",
        "create-v1",
    );
    let approval_v1 = submit(&client, &base, maker_token, &revision_v1, "approval-v1");

    let self_vote = vote(
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
    let stale_vote = vote(
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
    assert_eq!(
        vote(
            &client,
            &base,
            checker_token,
            &revision_v1,
            &approval_v1,
            1,
            "approve",
            "approve-v1",
        )
        .status()
        .as_u16(),
        200
    );
    let active_v1 = activate(
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
    let replay = activate(
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

    create_revision(
        &client,
        &base,
        maker_token,
        &revision_v2,
        "deny",
        "create-v2",
    );
    let approval_v2 = submit(&client, &base, maker_token, &revision_v2, "approval-v2");
    assert_eq!(
        vote(
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
    let stale_activation = activate(
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
    let audit_failure = activate(
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

    let active_v2 = activate(
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
    let rollback_approval = submit(
        &client,
        &base,
        maker_token,
        &revision_v1,
        "approval-v1-rollback",
    );
    assert_eq!(
        vote(
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
    let rollback = activate(
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
    let rollback: serde_json::Value = rollback.json().unwrap();
    assert_eq!(rollback["revision_id"], revision_v1);
    assert_eq!(rollback["last_known_good_revision_id"], revision_v1);

    let rejected_revision = prodex_domain::PolicyRevisionId::new().to_string();
    create_revision(
        &client,
        &base,
        maker_token,
        &rejected_revision,
        "review",
        "create-rejected",
    );
    let rejected_approval = submit(
        &client,
        &base,
        maker_token,
        &rejected_revision,
        "approval-rejected",
    );
    let rejected = vote(
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

#[test]
fn gateway_execution_approval_http_lists_shows_and_reviews_without_payloads() {
    let root = temp_root("gateway-execution-approval");
    let paths = app_paths_for_root(root.clone());
    let database_path = root.join("gateway.sqlite");
    runtime_gateway_sqlite_create_current_schema_for_tests(&database_path).unwrap();
    let tenant_id = TenantId::new();
    let maker_id = PrincipalId::new();
    let checker_one_id = {
        let mut hasher = Sha256::new();
        hasher.update(b"prodex:gateway-admin-control-plane-principal:v1");
        let tenant = tenant_id.to_string();
        for part in [tenant.as_bytes(), b"checker-one"] {
            hasher.update([0]);
            hasher.update(part);
        }
        let digest = hasher.finalize();
        let mut bytes = [0_u8; 16];
        bytes.copy_from_slice(&digest[..16]);
        bytes[6] = (bytes[6] & 0x0f) | 0x80;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        PrincipalId::from_uuid(uuid::Uuid::from_bytes(bytes))
    };
    let connection = Connection::open(&database_path).unwrap();
    connection
        .execute(
            "INSERT INTO prodex_tenants (tenant_id, display_name, created_at_unix_ms, updated_at_unix_ms)
             VALUES (?1, 'test tenant', 1, 1)",
            [tenant_id.to_string()],
        )
        .unwrap();
    for (approval_id, fingerprint, maker, state, version) in [
        (
            "execution:sha256:http-approved",
            "sha256:http-approved",
            maker_id,
            "pending_approval",
            1,
        ),
        (
            "execution:sha256:http-rejected",
            "sha256:http-rejected",
            maker_id,
            "pending_approval",
            1,
        ),
        (
            "execution:sha256:http-self",
            "sha256:http-self",
            checker_one_id,
            "pending_approval",
            1,
        ),
        (
            "execution:sha256:http-stale",
            "sha256:http-stale",
            maker_id,
            "pending_approval",
            1,
        ),
        (
            "execution:sha256:http-invalid",
            "sha256:http-invalid",
            maker_id,
            "approved",
            2,
        ),
    ] {
        connection
            .execute(
                "INSERT INTO prodex_approvals (
                    tenant_id, approval_id, approval_kind, approval_scope, fingerprint,
                    maker_id, lifecycle_state, required_quorum, expires_at_unix_ms,
                    activated_at_unix_ms, termination_reason, resource_version
                 ) VALUES (?1, ?2, 'execution', 'execution', ?3, ?4,
                           ?5, 2, 9999999999999, NULL, NULL, ?6)",
                rusqlite::params![
                    tenant_id.to_string(),
                    approval_id,
                    fingerprint,
                    maker.to_string(),
                    state,
                    version,
                ],
            )
            .unwrap();
    }
    drop(connection);
    let admin = |name: &str, token: &str| RuntimeGatewayAdminToken {
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
            admin("checker-one", "checker-one-token"),
            admin("checker-two", "checker-two-token"),
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
    let base = format!(
        "http://{}/v1/prodex/gateway/execution-approvals",
        proxy.listen_addr
    );
    assert_eq!(client.get(&base).send().unwrap().status().as_u16(), 401);
    let listed: serde_json::Value = client
        .get(&base)
        .bearer_auth("checker-one-token")
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(listed["data"].as_array().unwrap().len(), 5);
    assert!(listed.to_string().contains("http-approved"));
    assert!(!listed.to_string().contains("prompt"));
    let shown: serde_json::Value = client
        .get(format!("{base}/execution:sha256:http-approved"))
        .bearer_auth("checker-one-token")
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(shown["state"], "pending_approval");

    let review = |token: &str, id: &str, version: u64, decision: &str, key: &str| {
        client
            .post(format!("{base}/{id}/votes"))
            .bearer_auth(token)
            .header("Idempotency-Key", key)
            .json(&serde_json::json!({
                "decision": decision,
                "expected_version": version
            }))
            .send()
            .unwrap()
    };
    let first = review(
        "checker-one-token",
        "execution:sha256:http-approved",
        1,
        "approve",
        "execution-vote-one",
    );
    assert_eq!(first.status().as_u16(), 200);
    let first_body = first.text().unwrap();
    let first: serde_json::Value = serde_json::from_str(&first_body).unwrap();
    assert_eq!(first["state"], "pending_approval");
    let audit_count_after_first = Connection::open(&database_path)
        .unwrap()
        .query_row(
            "SELECT COUNT(*) FROM prodex_audit_log WHERE tenant_id = ?1",
            [tenant_id.to_string()],
            |row| row.get::<_, i64>(0),
        )
        .unwrap();
    let replay = review(
        "checker-one-token",
        "execution:sha256:http-approved",
        1,
        "approve",
        "execution-vote-one",
    );
    assert_eq!(replay.status().as_u16(), 200);
    assert_eq!(replay.text().unwrap(), first_body);
    assert_eq!(
        Connection::open(&database_path)
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM prodex_audit_log WHERE tenant_id = ?1",
                [tenant_id.to_string()],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        audit_count_after_first
    );
    assert_eq!(
        review(
            "checker-one-token",
            "execution:sha256:http-approved",
            1,
            "reject",
            "execution-vote-one",
        )
        .status()
        .as_u16(),
        409
    );
    let second: serde_json::Value = review(
        "checker-two-token",
        "execution:sha256:http-approved",
        2,
        "approve",
        "execution-vote-two",
    )
    .json()
    .unwrap();
    assert_eq!(second["state"], "approved");
    let rejected: serde_json::Value = review(
        "checker-one-token",
        "execution:sha256:http-rejected",
        1,
        "reject",
        "execution-reject",
    )
    .json()
    .unwrap();
    assert_eq!(rejected["state"], "rejected");

    assert_eq!(
        review(
            "checker-one-token",
            "execution:sha256:http-self",
            1,
            "approve",
            "execution-self",
        )
        .status()
        .as_u16(),
        403
    );
    assert_eq!(
        review(
            "checker-one-token",
            "execution:sha256:http-stale",
            9,
            "approve",
            "execution-stale",
        )
        .status()
        .as_u16(),
        409
    );
    assert_eq!(
        review(
            "checker-one-token",
            "execution:sha256:http-invalid",
            2,
            "approve",
            "execution-invalid",
        )
        .status()
        .as_u16(),
        409
    );

    let connection = Connection::open(&database_path).unwrap();
    let counts = connection
        .query_row(
            "SELECT
                (SELECT COUNT(*) FROM prodex_approval_votes WHERE tenant_id = ?1),
                (SELECT COUNT(*) FROM prodex_audit_log WHERE tenant_id = ?1),
                (SELECT COUNT(*) FROM prodex_siem_outbox WHERE tenant_id = ?1),
                (SELECT COUNT(*) FROM prodex_idempotency_records WHERE tenant_id = ?1)",
            [tenant_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(counts.0, 2);
    assert_eq!(counts.1, counts.2);
    assert!(counts.1 >= 6);
    assert_eq!(counts.3, 6);
    let denial_reasons = connection
        .prepare(
            "SELECT reason_code FROM prodex_audit_log
             WHERE tenant_id = ?1 AND outcome = 'denied' ORDER BY reason_code",
        )
        .unwrap()
        .query_map([tenant_id.to_string()], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(
        denial_reasons,
        vec![
            "approval.invalid_transition",
            "approval.self_approval_denied",
            "approval.stale_version",
        ]
    );
    let stored_responses = connection
        .prepare(
            "SELECT response_body FROM prodex_idempotency_records
             WHERE tenant_id = ?1 ORDER BY idempotency_key",
        )
        .unwrap()
        .query_map([tenant_id.to_string()], |row| row.get::<_, Vec<u8>>(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    assert!(stored_responses.iter().all(|response| {
        let response = String::from_utf8_lossy(response);
        !response.contains("prompt") && !response.contains("decision")
    }));
}

#[test]
fn gateway_governance_artifacts_use_generic_maker_checker_lifecycle() {
    let root = temp_root("gateway-routing-scores-lifecycle");
    let paths = app_paths_for_root(root.clone());
    let database_path = root.join("gateway.sqlite");
    runtime_gateway_sqlite_create_current_schema_for_tests(&database_path).unwrap();
    let tenant = TenantId::new();
    let connection = Connection::open(&database_path).unwrap();
    connection
        .execute(
            "INSERT INTO prodex_tenants (tenant_id, display_name, created_at_unix_ms, updated_at_unix_ms)
             VALUES (?1, 'test tenant', 1, 1)",
            [tenant.to_string()],
        )
        .unwrap();
    drop(connection);

    let admin = |name: &str, token: &str| RuntimeGatewayAdminToken {
        name: name.to_string(),
        token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(token),
        role: RuntimeGatewayAdminRole::Admin,
        tenant_id: Some(tenant.to_string()),
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
            admin("maker", "routing-maker-token"),
            admin("checker", "routing-checker-token"),
        ],
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::sqlite(database_path),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: None,
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .unwrap();
    let client = reqwest::blocking::Client::new();
    let base = format!(
        "http://{}/v1/prodex/gateway/routing-scores",
        proxy.listen_addr
    );
    let artifact = serde_json::json!({
        "schema_version": 1,
        "revision": 7,
        "weights": {
            "health": 2_000,
            "load": 1_000,
            "cost": 3_000,
            "latency": 1_000,
            "risk": 1_000,
            "priority": 1_000,
            "affinity": 1_000
        }
    });
    assert_eq!(
        client
            .post(format!("{base}/validate"))
            .bearer_auth("routing-maker-token")
            .json(&serde_json::json!({"artifact": artifact.clone()}))
            .send()
            .unwrap()
            .status()
            .as_u16(),
        200
    );
    assert_eq!(
        client
            .post(format!("{base}/validate"))
            .bearer_auth("routing-maker-token")
            .json(&serde_json::json!({"artifact": {
                "schema_version": 1,
                "revision": 8,
                "weights": {
                    "health": 0,
                    "load": 0,
                    "cost": 10_001,
                    "latency": 0,
                    "risk": 0,
                    "priority": 0,
                    "affinity": 0
                }
            }}))
            .send()
            .unwrap()
            .status()
            .as_u16(),
        400
    );

    #[derive(serde::Serialize)]
    struct ClassificationChecksumInput {
        unsupported_coverage_floor: DataClassification,
        rules: Vec<serde_json::Value>,
    }
    let classification_checksum = Sha256::digest(
        serde_json::to_vec(&ClassificationChecksumInput {
            unsupported_coverage_floor: DataClassification::Restricted,
            rules: Vec::new(),
        })
        .unwrap(),
    )
    .iter()
    .map(|byte| format!("{byte:02x}"))
    .collect::<String>();
    let classification_artifact = serde_json::json!({
        "schema_version": 1,
        "detector_revision": "detector-v1",
        "patterns": [],
        "classification_revision": "classification-v1",
        "classification_checksum": classification_checksum,
        "unsupported_coverage_floor": "restricted",
        "classification_rules": []
    });
    let classification_base = format!(
        "http://{}/v1/prodex/gateway/classification-rules",
        proxy.listen_addr
    );
    let classification_valid = client
        .post(&classification_base)
        .bearer_auth("routing-maker-token")
        .header("Idempotency-Key", "classification-create-v1")
        .json(&serde_json::json!({
            "revision_id": "classification-v1",
            "artifact": classification_artifact.clone()
        }))
        .send()
        .unwrap();
    assert!(matches!(classification_valid.status().as_u16(), 200 | 201));
    assert_eq!(
        client
            .post(&classification_base)
            .bearer_auth("routing-maker-token")
            .header("Idempotency-Key", "classification-create-mismatched")
            .json(&serde_json::json!({
                "revision_id": "classification-v2",
                "artifact": classification_artifact
            }))
            .send()
            .unwrap()
            .status()
            .as_u16(),
        400
    );

    let adapter = provider_adapter(ProviderId::OpenAi);
    let endpoints = adapter
        .supported_endpoints()
        .iter()
        .copied()
        .filter(|endpoint| {
            crate::runtime_launch::proxy_startup::local_rewrite_application_data_plane::runtime_gateway_provider_capability_is_executable(
                adapter.capability_status(*endpoint),
            )
        })
        .collect::<Vec<_>>();
    let provider_artifact = serde_json::json!({
        "schema_version": 2,
        "revision": 7,
        "pricing_revision": 4,
        "descriptors": [{
            "revision": 9,
            "pricing_revision": 4,
            "provider": "openai",
            "credential_ref": SecretRef::new("runtime-provider", "openai", None::<String>),
            "enabled": true,
            "revoked": false,
            "executable": true,
            "endpoints": endpoints,
            "capabilities": crate::runtime_launch::proxy_startup::local_rewrite_application_data_plane::runtime_gateway_provider_executable_capabilities(ProviderId::OpenAi),
            "regions": ["*"],
            "local_execution": false,
            "trust_tier": "enterprise",
            "maximum_classification": "confidential",
            "retention_seconds": 0,
            "training_use": false,
            "model_costs": {
                "*": {
                    "input_cost_per_million_microusd": 1_000_000,
                    "output_cost_per_million_microusd": 2_000_000
                }
            },
            "cost": 2_000,
            "latency": 3_000,
            "risk": 1_000,
            "priority": 8_000
        }]
    });
    let provider_base = format!(
        "http://{}/v1/prodex/gateway/provider-registries",
        proxy.listen_addr
    );
    let provider_valid = client
        .post(&provider_base)
        .bearer_auth("routing-maker-token")
        .header("Idempotency-Key", "provider-registry-create-v7")
        .json(&serde_json::json!({
            "revision_id": "7",
            "artifact": provider_artifact.clone()
        }))
        .send()
        .unwrap();
    assert!(matches!(provider_valid.status().as_u16(), 200 | 201));
    assert_eq!(
        client
            .post(&provider_base)
            .bearer_auth("routing-maker-token")
            .header("Idempotency-Key", "provider-registry-create-mismatched")
            .json(&serde_json::json!({
                "revision_id": "8",
                "artifact": provider_artifact
            }))
            .send()
            .unwrap()
            .status()
            .as_u16(),
        400
    );

    let mismatched = client
        .post(&base)
        .bearer_auth("routing-maker-token")
        .header("Idempotency-Key", "routing-create-mismatched")
        .json(&serde_json::json!({"revision_id": "8", "artifact": artifact.clone()}))
        .send()
        .unwrap();
    assert_eq!(mismatched.status().as_u16(), 400);

    let revision = "7".to_string();
    let created = client
        .post(&base)
        .bearer_auth("routing-maker-token")
        .header("Idempotency-Key", "routing-create-v7")
        .json(&serde_json::json!({"revision_id": &revision, "artifact": artifact}))
        .send()
        .unwrap();
    assert!(matches!(created.status().as_u16(), 200 | 201));
    let approval = submit(
        &client,
        &base,
        "routing-maker-token",
        &revision,
        "routing-approval-v7",
    );
    assert_eq!(
        vote(
            &client,
            &base,
            "routing-checker-token",
            &revision,
            &approval,
            1,
            "approve",
            "routing-approve-v7",
        )
        .status()
        .as_u16(),
        200
    );
    let activated = activate(
        &client,
        &base,
        "routing-checker-token",
        &revision,
        &approval,
        "*",
        "routing-activate-v7",
        "activate",
    );
    assert_eq!(activated.status().as_u16(), 200);
    let status: serde_json::Value = client
        .get(format!("{base}/status"))
        .bearer_auth("routing-checker-token")
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(status["active_revision_id"], revision);
    assert_eq!(status["object"], "governance.routing_scores_status");
}

fn create_revision(
    client: &reqwest::blocking::Client,
    base: &str,
    token: &str,
    revision: &str,
    effect: &str,
    key: &str,
) -> serde_json::Value {
    let failure_mode = if effect == "allow" { "open" } else { "closed" };
    let response = client
        .post(base)
        .bearer_auth(token)
        .header("Idempotency-Key", key)
        .json(&serde_json::json!({
            "revision_id": revision,
            "artifact": {
                "policy_revision": revision,
                "policy_failure_mode": failure_mode
            }
        }))
        .send()
        .unwrap();
    let status = response.status().as_u16();
    let body = response.text().unwrap();
    assert!(matches!(status, 200 | 201), "status={status} body={body}");
    serde_json::from_str(&body).unwrap()
}

fn submit(
    client: &reqwest::blocking::Client,
    base: &str,
    token: &str,
    revision: &str,
    approval: &str,
) -> String {
    let response = client
        .post(format!("{base}/{revision}/submit"))
        .bearer_auth(token)
        .header("Idempotency-Key", format!("submit-{approval}"))
        .json(&serde_json::json!({"approval_id": approval, "required_quorum": 1}))
        .send()
        .unwrap();
    assert!(matches!(response.status().as_u16(), 200 | 201));
    response.json::<serde_json::Value>().unwrap()["approval_id"]
        .as_str()
        .unwrap()
        .to_string()
}

#[allow(clippy::too_many_arguments)]
fn vote(
    client: &reqwest::blocking::Client,
    base: &str,
    token: &str,
    revision: &str,
    approval: &str,
    version: u64,
    decision: &str,
    key: &str,
) -> reqwest::blocking::Response {
    client
        .post(format!("{base}/{revision}/approvals/{approval}/votes"))
        .bearer_auth(token)
        .header("Idempotency-Key", key)
        .json(&serde_json::json!({"decision": decision, "expected_version": version}))
        .send()
        .unwrap()
}

#[allow(clippy::too_many_arguments)]
fn activate(
    client: &reqwest::blocking::Client,
    base: &str,
    token: &str,
    revision: &str,
    approval: &str,
    etag: &str,
    key: &str,
    action: &str,
) -> reqwest::blocking::Response {
    client
        .post(format!("{base}/{revision}/{action}"))
        .bearer_auth(token)
        .header("Idempotency-Key", key)
        .header("If-Match", etag)
        .json(&serde_json::json!({"approval_id": approval}))
        .send()
        .unwrap()
}
