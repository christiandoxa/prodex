mod artifacts;
mod policy;

use super::*;
use crate::runtime_launch::proxy_startup::local_rewrite_gateway_backend_connection::runtime_gateway_sqlite_create_current_schema_for_tests;
use prodex_domain::{DataClassification, PrincipalId, SecretRef, TenantId};
use prodex_provider_core::{ProviderId, provider_adapter};
use rusqlite::Connection;
use sha2::{Digest, Sha256};

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
