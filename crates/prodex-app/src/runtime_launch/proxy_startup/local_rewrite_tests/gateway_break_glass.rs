use super::{TestUpstream, app_paths_for_root, temp_root};
use crate::AppState;
use crate::runtime_launch::proxy_startup::local_rewrite::{
    RuntimeGatewayAdminRole, RuntimeGatewayAdminToken, RuntimeGatewayGuardrailWebhookConfig,
    RuntimeGatewayObservabilityConfig, RuntimeGatewaySsoConfig, RuntimeGatewayStateStore,
    RuntimeLocalRewriteProviderOptions, RuntimeLocalRewriteProxyStartOptions,
    start_runtime_local_rewrite_proxy,
};
use crate::runtime_launch::proxy_startup::local_rewrite_gateway_backend_connection::runtime_gateway_sqlite_create_current_schema_for_tests;
use prodex_domain::TenantId;
use rusqlite::Connection;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn gateway_break_glass_http_enforces_scope_expiry_revocation_and_audit() {
    let root = temp_root("gateway-break-glass-lifecycle");
    let paths = app_paths_for_root(root.clone());
    let database_path = root.join("gateway.sqlite");
    runtime_gateway_sqlite_create_current_schema_for_tests(&database_path).unwrap();
    let tenant_id = TenantId::new();
    Connection::open(&database_path)
        .unwrap()
        .execute(
            "INSERT INTO prodex_tenants (tenant_id, display_name, created_at_unix_ms, updated_at_unix_ms)
             VALUES (?1, 'test tenant', 1, 1)",
            [tenant_id.to_string()],
        )
        .unwrap();

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
            admin("maker", "maker-token"),
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
    let approval_id = "break-glass:retention-test";
    let approval_base = format!(
        "http://{}/v1/prodex/gateway/break-glass-approvals",
        proxy.listen_addr
    );
    let purge_url = format!(
        "http://{}/v1/prodex/gateway/audit/retention/purge",
        proxy.listen_addr
    );
    let expires_at_unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
        + 600_000;
    let created: serde_json::Value = client
        .post(&approval_base)
        .bearer_auth("maker-token")
        .header("Idempotency-Key", "break-glass-create")
        .json(&serde_json::json!({
            "approval_id": approval_id,
            "reason_code": "incident.retention",
            "expires_at_unix_ms": expires_at_unix_ms
        }))
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(created["approval"]["scope"], "audit_retention");
    assert_eq!(created["approval"]["state"], "pending_approval");

    let audit_event_id = Connection::open(&database_path)
        .unwrap()
        .query_row(
            "SELECT audit_event_id FROM prodex_audit_log WHERE tenant_id = ?1 ORDER BY occurred_at_unix_ms LIMIT 1",
            [tenant_id.to_string()],
            |row| row.get::<_, String>(0),
        )
        .unwrap();
    let purge = |key: &str| {
        client
            .delete(&purge_url)
            .bearer_auth("checker-one-token")
            .header("Idempotency-Key", key)
            .json(&serde_json::json!({
                "approval_id": approval_id,
                "audit_event_ids": [&audit_event_id],
                "retention_days": 30
            }))
            .send()
            .unwrap()
    };
    assert_eq!(purge("break-glass-pending-purge").status().as_u16(), 403);

    let transition =
        |token: &str, suffix: &str, expected_version: u64, decision: Option<&str>, key: &str| {
            let mut body = serde_json::json!({"expected_version": expected_version});
            if let Some(decision) = decision {
                body["decision"] = serde_json::Value::String(decision.to_string());
            }
            client
                .post(format!("{approval_base}/{approval_id}/{suffix}"))
                .bearer_auth(token)
                .header("Idempotency-Key", key)
                .json(&body)
                .send()
                .unwrap()
        };
    assert_eq!(
        transition(
            "maker-token",
            "votes",
            1,
            Some("approve"),
            "break-glass-self-vote"
        )
        .status()
        .as_u16(),
        403
    );
    let first_vote: serde_json::Value = transition(
        "checker-one-token",
        "votes",
        1,
        Some("approve"),
        "break-glass-vote-one",
    )
    .json()
    .unwrap();
    assert_eq!(first_vote["state"], "pending_approval");
    let second_vote = transition(
        "checker-two-token",
        "votes",
        2,
        Some("approve"),
        "break-glass-vote-two",
    );
    let second_vote_status = second_vote.status().as_u16();
    let second_vote_body = second_vote.text().unwrap();
    assert_eq!(second_vote_status, 200, "{second_vote_body}");
    let second_vote: serde_json::Value = serde_json::from_str(&second_vote_body).unwrap();
    assert_eq!(second_vote["state"], "approved");
    let active: serde_json::Value = transition(
        "checker-one-token",
        "activate",
        3,
        None,
        "break-glass-activate",
    )
    .json()
    .unwrap();
    assert_eq!(active["state"], "active");
    assert_eq!(purge("break-glass-active-purge").status().as_u16(), 200);

    let connection = Connection::open(&database_path).unwrap();
    connection
        .execute(
            "UPDATE prodex_approvals SET approval_scope = 'execution'
             WHERE tenant_id = ?1 AND approval_id = ?2",
            rusqlite::params![tenant_id.to_string(), approval_id],
        )
        .unwrap();
    drop(connection);
    assert_eq!(purge("break-glass-wrong-scope").status().as_u16(), 403);
    let connection = Connection::open(&database_path).unwrap();
    connection
        .execute(
            "UPDATE prodex_approvals SET approval_scope = 'audit_retention:incident.retention', expires_at_unix_ms = 1
             WHERE tenant_id = ?1 AND approval_id = ?2",
            rusqlite::params![tenant_id.to_string(), approval_id],
        )
        .unwrap();
    drop(connection);
    assert_eq!(purge("break-glass-expired").status().as_u16(), 403);
    Connection::open(&database_path)
        .unwrap()
        .execute(
            "UPDATE prodex_approvals SET expires_at_unix_ms = ?1
             WHERE tenant_id = ?2 AND approval_id = ?3",
            rusqlite::params![
                i64::try_from(expires_at_unix_ms).unwrap(),
                tenant_id.to_string(),
                approval_id
            ],
        )
        .unwrap();

    let revoked: serde_json::Value =
        transition("checker-two-token", "revoke", 4, None, "break-glass-revoke")
            .json()
            .unwrap();
    assert_eq!(revoked["state"], "superseded");
    assert_eq!(purge("break-glass-revoked").status().as_u16(), 403);

    let connection = Connection::open(&database_path).unwrap();
    let lifecycle_audits = connection
        .query_row(
            "SELECT COUNT(*) FROM prodex_audit_log
             WHERE tenant_id = ?1 AND action LIKE 'governance.break_glass_approval.%'",
            [tenant_id.to_string()],
            |row| row.get::<_, i64>(0),
        )
        .unwrap();
    assert_eq!(lifecycle_audits, 6);
    drop(connection);
    let integrity: serde_json::Value = client
        .get(format!(
            "http://{}/v1/prodex/gateway/governance/audit/integrity",
            proxy.listen_addr
        ))
        .bearer_auth("checker-one-token")
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(integrity["chain_valid"], true);
}
