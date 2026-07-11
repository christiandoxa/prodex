use super::*;
use crate::TestEnvVarGuard;
use std::fs;

#[test]
fn gateway_admin_token_key_prefix_scope_limits_key_access() {
    let root = temp_root("gateway-admin-key-scope");
    let audit_dir = root.join("audit");
    let _audit_env = TestEnvVarGuard::set("PRODEX_AUDIT_LOG_DIR", audit_dir.to_str().unwrap());
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(2);
    let admin_token = "admin-token";
    let scoped_token = "scoped-token";
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
            RuntimeGatewayAdminToken {
                name: "admin".to_string(),
                token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
                    admin_token,
                ),
                role: RuntimeGatewayAdminRole::Admin,
                allowed_key_prefixes: Vec::new(),
                tenant_id: None,
                team_id: None,
                project_id: None,
                user_id: None,
                budget_id: None,
            },
            RuntimeGatewayAdminToken {
                name: "team-a-admin".to_string(),
                token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
                    scoped_token,
                ),
                role: RuntimeGatewayAdminRole::Admin,
                allowed_key_prefixes: vec!["team-a-".to_string()],
                tenant_id: None,
                team_id: None,
                project_id: None,
                user_id: None,
                budget_id: None,
            },
        ],
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("gateway proxy should start");
    let client = reqwest::blocking::Client::new();

    let team_a = client
        .post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .header("Idempotency-Key", "scope-admin-create-team-a")
        .json(&serde_json::json!({"name": "team-a-main"}))
        .send()
        .expect("admin create team-a key request should be sent");
    assert_eq!(team_a.status().as_u16(), 201);
    let team_a: serde_json::Value = team_a
        .json()
        .expect("team-a create response should be json");
    let team_a_token = team_a["token"]
        .as_str()
        .expect("team-a generated token should be returned")
        .to_string();

    let team_b = client
        .post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .header("Idempotency-Key", "scope-admin-create-team-b")
        .json(&serde_json::json!({"name": "team-b-main"}))
        .send()
        .expect("admin create team-b key request should be sent");
    assert_eq!(team_b.status().as_u16(), 201);
    let team_b: serde_json::Value = team_b
        .json()
        .expect("team-b create response should be json");
    let team_b_token = team_b["token"]
        .as_str()
        .expect("team-b generated token should be returned")
        .to_string();

    for (token, input) in [
        (&team_a_token, "team-a traffic"),
        (&team_b_token, "team-b traffic"),
    ] {
        let response = client
            .post(format!("http://{}/v1/responses", proxy.listen_addr))
            .bearer_auth(token)
            .json(&serde_json::json!({"model": "gpt-5.4", "input": input}))
            .send()
            .expect("gateway request should be sent");
        assert_eq!(response.status().as_u16(), 200);
        let _ = upstream
            .body_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("upstream should receive gateway request");
    }
    wait_for_ledger_file_key_response_status(
        &paths.root.join("gateway-billing-ledger.jsonl"),
        "team-b-main",
        200,
    );

    let scoped_keys = client
        .get(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(scoped_token)
        .send()
        .expect("scoped key list request should be sent");
    assert_eq!(scoped_keys.status().as_u16(), 200);
    let scoped_keys: serde_json::Value = scoped_keys
        .json()
        .expect("scoped key list response should be json");
    let scoped_names = scoped_keys["keys"]
        .as_array()
        .unwrap()
        .iter()
        .map(|key| key["name"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(scoped_names, vec!["team-a-main"]);

    let scoped_get_allowed = client
        .get(format!(
            "http://{}/v1/prodex/gateway/keys/team-a-main",
            proxy.listen_addr
        ))
        .bearer_auth(scoped_token)
        .send()
        .expect("scoped get allowed key request should be sent");
    assert_eq!(scoped_get_allowed.status().as_u16(), 200);

    let scoped_get_forbidden = client
        .get(format!(
            "http://{}/v1/prodex/gateway/keys/team-b-main",
            proxy.listen_addr
        ))
        .bearer_auth(scoped_token)
        .send()
        .expect("scoped get forbidden key request should be sent");
    assert_eq!(scoped_get_forbidden.status().as_u16(), 403);
    assert_eq!(
        scoped_get_forbidden.json::<serde_json::Value>().unwrap()["error"]["code"],
        "gateway_admin_key_scope_forbidden"
    );

    let scoped_create_forbidden = client
        .post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(scoped_token)
        .header("Idempotency-Key", "scope-create-team-b-forbidden")
        .json(&serde_json::json!({"name": "team-b-new"}))
        .send()
        .expect("scoped forbidden create key request should be sent");
    assert_eq!(scoped_create_forbidden.status().as_u16(), 403);

    let scoped_create_allowed = client
        .post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(scoped_token)
        .header("Idempotency-Key", "scope-create-team-a")
        .json(&serde_json::json!({"name": "team-a-new"}))
        .send()
        .expect("scoped allowed create key request should be sent");
    assert_eq!(scoped_create_allowed.status().as_u16(), 201);

    let scoped_patch_forbidden = client
        .patch(format!(
            "http://{}/v1/prodex/gateway/keys/team-b-main",
            proxy.listen_addr
        ))
        .bearer_auth(scoped_token)
        .header("Idempotency-Key", "scope-patch-team-b-forbidden")
        .json(&serde_json::json!({"disabled": true}))
        .send()
        .expect("scoped forbidden patch key request should be sent");
    assert_eq!(scoped_patch_forbidden.status().as_u16(), 403);

    let scoped_delete_forbidden = client
        .delete(format!(
            "http://{}/v1/prodex/gateway/keys/team-b-main",
            proxy.listen_addr
        ))
        .bearer_auth(scoped_token)
        .header("Idempotency-Key", "scope-delete-team-b-forbidden")
        .send()
        .expect("scoped forbidden delete key request should be sent");
    assert_eq!(scoped_delete_forbidden.status().as_u16(), 403);

    let audit_log = fs::read_to_string(audit_dir.join("prodex-audit.log"))
        .expect("gateway admin audit log should be written");
    assert!(audit_log.contains(r#""action":"authorization_denied""#));
    assert!(audit_log.contains(r#""reason":"scope_forbidden""#));
    assert!(audit_log.contains(r#""resource":"key""#));
    assert!(audit_log.contains(r#""resource_name":"team-b-main""#));
    assert!(!audit_log.contains(admin_token));
    assert!(!audit_log.contains(scoped_token));

    let scoped_ledger = client
        .get(format!(
            "http://{}/v1/prodex/gateway/ledger",
            proxy.listen_addr
        ))
        .bearer_auth(scoped_token)
        .send()
        .expect("scoped ledger request should be sent");
    assert_eq!(scoped_ledger.status().as_u16(), 200);
    let scoped_ledger: serde_json::Value = scoped_ledger
        .json()
        .expect("scoped ledger response should be json");
    assert_eq!(scoped_ledger["records"].as_array().unwrap().len(), 1);
    assert_eq!(scoped_ledger["records"][0]["key_name"], "team-a-main");

    let scoped_summary = client
        .get(format!(
            "http://{}/v1/prodex/gateway/ledger/summary",
            proxy.listen_addr
        ))
        .bearer_auth(scoped_token)
        .send()
        .expect("scoped billing summary request should be sent");
    assert_eq!(scoped_summary.status().as_u16(), 200);
    let scoped_summary: serde_json::Value = scoped_summary
        .json()
        .expect("scoped billing summary response should be json");
    assert_eq!(scoped_summary["totals"]["requests"], 1);
    assert_eq!(scoped_summary["by_key"][0]["key_name"], "team-a-main");

    let scoped_metrics = client
        .get(format!(
            "http://{}/v1/prodex/gateway/metrics",
            proxy.listen_addr
        ))
        .bearer_auth(scoped_token)
        .send()
        .expect("scoped metrics request should be sent");
    assert_eq!(scoped_metrics.status().as_u16(), 200);
    let scoped_metrics = scoped_metrics
        .text()
        .expect("scoped metrics should be text");
    assert_eq!(
        scoped_metrics
            .matches("prodex_gateway_virtual_key_requests_total{")
            .count(),
        2
    );
    assert!(!scoped_metrics.contains("team-a-main"));
    assert!(!scoped_metrics.contains("team-a-new"));
    assert!(!scoped_metrics.contains("team-b-main"));
}

#[test]
fn gateway_admin_token_governance_scope_limits_admin_surfaces() {
    let root = temp_root("gateway-admin-governance-scope");
    let audit_dir = root.join("audit");
    let _audit_env = TestEnvVarGuard::set("PRODEX_AUDIT_LOG_DIR", audit_dir.to_str().unwrap());
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(3);
    let admin_token = "admin-token";
    let team_a_token = "team-a-token";
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
            RuntimeGatewayAdminToken {
                name: "admin".to_string(),
                token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
                    admin_token,
                ),
                role: RuntimeGatewayAdminRole::Admin,
                allowed_key_prefixes: Vec::new(),
                tenant_id: None,
                team_id: None,
                project_id: None,
                user_id: None,
                budget_id: None,
            },
            RuntimeGatewayAdminToken {
                name: "team-a-admin".to_string(),
                token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
                    team_a_token,
                ),
                role: RuntimeGatewayAdminRole::Admin,
                allowed_key_prefixes: Vec::new(),
                tenant_id: None,
                team_id: Some("team-a".to_string()),
                project_id: None,
                user_id: None,
                budget_id: None,
            },
        ],
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("gateway proxy should start");
    let client = reqwest::blocking::Client::new();

    let alpha = client
        .post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .header("Idempotency-Key", "team-scope-admin-create-alpha")
        .json(&serde_json::json!({"name": "alpha-main", "team_id": "team-a"}))
        .send()
        .expect("admin create alpha key request should be sent");
    assert_eq!(alpha.status().as_u16(), 201);
    let alpha: serde_json::Value = alpha.json().expect("alpha key json");
    let alpha_token = alpha["token"].as_str().unwrap().to_string();

    let beta = client
        .post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .header("Idempotency-Key", "team-scope-admin-create-beta")
        .json(&serde_json::json!({"name": "beta-main", "team_id": "team-b"}))
        .send()
        .expect("admin create beta key request should be sent");
    assert_eq!(beta.status().as_u16(), 201);
    let beta: serde_json::Value = beta.json().expect("beta key json");
    let beta_token = beta["token"].as_str().unwrap().to_string();

    let team_created = client
        .post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(team_a_token)
        .header("Idempotency-Key", "team-scope-create-gamma")
        .json(&serde_json::json!({"name": "gamma-main"}))
        .send()
        .expect("team-scoped create key request should be sent");
    assert_eq!(team_created.status().as_u16(), 201);
    let team_created: serde_json::Value = team_created.json().expect("team-created key json");
    assert_eq!(team_created["key"]["team_id"], "team-a");
    let gamma_token = team_created["token"].as_str().unwrap().to_string();

    let forbidden_create = client
        .post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(team_a_token)
        .header("Idempotency-Key", "team-scope-create-delta-forbidden")
        .json(&serde_json::json!({"name": "delta-main", "team_id": "team-b"}))
        .send()
        .expect("team-scoped forbidden create key request should be sent");
    assert_eq!(forbidden_create.status().as_u16(), 403);

    let forbidden_patch = client
        .patch(format!(
            "http://{}/v1/prodex/gateway/keys/alpha-main",
            proxy.listen_addr
        ))
        .bearer_auth(team_a_token)
        .header("Idempotency-Key", "team-scope-patch-alpha-forbidden")
        .json(&serde_json::json!({"team_id": "team-b"}))
        .send()
        .expect("team-scoped forbidden patch key request should be sent");
    assert_eq!(forbidden_patch.status().as_u16(), 403);

    let forbidden_scope_clear = client
        .patch(format!(
            "http://{}/v1/prodex/gateway/keys/alpha-main",
            proxy.listen_addr
        ))
        .bearer_auth(team_a_token)
        .header("Idempotency-Key", "team-scope-clear-alpha-forbidden")
        .json(&serde_json::json!({"team_id": null}))
        .send()
        .expect("team-scoped scope-clearing patch key request should be sent");
    assert_eq!(forbidden_scope_clear.status().as_u16(), 403);

    let team_scim_user = client
        .post(format!(
            "http://{}/v1/prodex/gateway/scim/v2/Users",
            proxy.listen_addr
        ))
        .bearer_auth(team_a_token)
        .header("Idempotency-Key", "team-scope-scim-create-alice")
        .json(&serde_json::json!({"userName": "alice-team@example.com"}))
        .send()
        .expect("team-scoped SCIM create request should be sent");
    assert_eq!(team_scim_user.status().as_u16(), 201);
    let team_scim_user: serde_json::Value =
        team_scim_user.json().expect("team-scoped SCIM user json");
    assert_eq!(team_scim_user["team_id"], "team-a");
    assert_eq!(
        team_scim_user["urn:prodex:params:scim:schemas:gateway:2.0:User"]["team_id"],
        "team-a"
    );

    let team_b_scim_user = client
        .post(format!(
            "http://{}/v1/prodex/gateway/scim/v2/Users",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .header("Idempotency-Key", "team-scope-scim-create-bob")
        .json(&serde_json::json!({"userName": "bob-team@example.com", "team_id": "team-b"}))
        .send()
        .expect("admin SCIM team-b create request should be sent");
    assert_eq!(team_b_scim_user.status().as_u16(), 201);
    let team_b_scim_user: serde_json::Value = team_b_scim_user
        .json()
        .expect("team-b SCIM user response should be json");
    let team_b_scim_user_id = team_b_scim_user["id"]
        .as_str()
        .expect("team-b SCIM user should have id")
        .to_string();

    let forbidden_scim_user = client
        .post(format!(
            "http://{}/v1/prodex/gateway/scim/v2/Users",
            proxy.listen_addr
        ))
        .bearer_auth(team_a_token)
        .header(
            "Idempotency-Key",
            "team-scope-scim-create-charlie-forbidden",
        )
        .json(&serde_json::json!({
            "userName": "charlie-team@example.com",
            "team_id": "team-b"
        }))
        .send()
        .expect("team-scoped forbidden SCIM create request should be sent");
    assert_eq!(forbidden_scim_user.status().as_u16(), 403);

    let forbidden_scim_patch = client
        .patch(format!(
            "http://{}/v1/prodex/gateway/scim/v2/Users/{}",
            proxy.listen_addr, team_b_scim_user_id
        ))
        .bearer_auth(team_a_token)
        .header("Idempotency-Key", "team-scope-scim-patch-bob-forbidden")
        .json(&serde_json::json!({"displayName": "blocked"}))
        .send()
        .expect("team-scoped forbidden SCIM patch request should be sent");
    assert_eq!(forbidden_scim_patch.status().as_u16(), 403);

    let forbidden_scim_delete = client
        .delete(format!(
            "http://{}/v1/prodex/gateway/scim/v2/Users/{}",
            proxy.listen_addr, team_b_scim_user_id
        ))
        .bearer_auth(team_a_token)
        .header("Idempotency-Key", "team-scope-scim-delete-bob-forbidden")
        .send()
        .expect("team-scoped forbidden SCIM delete request should be sent");
    assert_eq!(forbidden_scim_delete.status().as_u16(), 403);

    let audit_log = fs::read_to_string(audit_dir.join("prodex-audit.log"))
        .expect("gateway admin audit log should be written");
    assert!(audit_log.contains(r#""action":"authorization_denied""#));
    assert!(audit_log.contains(r#""resource":"scim_user""#));
    assert!(audit_log.contains(r#""action":"update_scim_user""#));
    assert!(audit_log.contains(r#""action":"delete_scim_user""#));
    assert!(audit_log.contains(&team_b_scim_user_id));
    assert!(!audit_log.contains(admin_token));
    assert!(!audit_log.contains(team_a_token));

    for (token, input) in [
        (&alpha_token, "alpha traffic"),
        (&beta_token, "beta traffic"),
        (&gamma_token, "gamma traffic"),
    ] {
        let response = client
            .post(format!("http://{}/v1/responses", proxy.listen_addr))
            .bearer_auth(token)
            .json(&serde_json::json!({"model": "gpt-5.4", "input": input}))
            .send()
            .expect("gateway request should be sent");
        assert_eq!(response.status().as_u16(), 200);
        let _ = upstream
            .body_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("upstream should receive gateway request");
    }
    wait_for_ledger_file_key_response_status(
        &paths.root.join("gateway-billing-ledger.jsonl"),
        "gamma-main",
        200,
    );

    let scoped_keys = client
        .get(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(team_a_token)
        .send()
        .expect("team-scoped list keys request should be sent");
    assert_eq!(scoped_keys.status().as_u16(), 200);
    let scoped_keys: serde_json::Value = scoped_keys.json().expect("scoped keys json");
    let names = scoped_keys["keys"]
        .as_array()
        .unwrap()
        .iter()
        .map(|key| key["name"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(names, vec!["alpha-main", "gamma-main"]);

    let hidden_beta = client
        .get(format!(
            "http://{}/v1/prodex/gateway/keys/beta-main",
            proxy.listen_addr
        ))
        .bearer_auth(team_a_token)
        .send()
        .expect("team-scoped forbidden get key request should be sent");
    assert_eq!(hidden_beta.status().as_u16(), 403);

    let scoped_summary = client
        .get(format!(
            "http://{}/v1/prodex/gateway/ledger/summary",
            proxy.listen_addr
        ))
        .bearer_auth(team_a_token)
        .send()
        .expect("team-scoped summary request should be sent");
    assert_eq!(scoped_summary.status().as_u16(), 200);
    let scoped_summary: serde_json::Value = scoped_summary.json().expect("scoped summary json");
    assert_eq!(scoped_summary["totals"]["requests"], 2);
    assert_eq!(scoped_summary["by_team"][0]["team_id"], "team-a");

    let scoped_metrics = client
        .get(format!(
            "http://{}/v1/prodex/gateway/metrics",
            proxy.listen_addr
        ))
        .bearer_auth(team_a_token)
        .send()
        .expect("team-scoped metrics request should be sent");
    assert_eq!(scoped_metrics.status().as_u16(), 200);
    let scoped_metrics = scoped_metrics
        .text()
        .expect("scoped metrics should be text");
    assert_eq!(
        scoped_metrics
            .matches("prodex_gateway_virtual_key_requests_total{")
            .count(),
        2
    );
    assert!(!scoped_metrics.contains("alpha-main"));
    assert!(!scoped_metrics.contains("gamma-main"));
    assert!(!scoped_metrics.contains("beta-main"));
}
