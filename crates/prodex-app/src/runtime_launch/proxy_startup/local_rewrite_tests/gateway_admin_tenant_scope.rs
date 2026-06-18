use super::*;

#[test]
fn gateway_admin_token_tenant_scope_limits_admin_surfaces() {
    let root = temp_root("gateway-admin-tenant-scope");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(2);
    let tenant_a_token = "tenant-a-token";
    let tenant_b_token = "tenant-b-token";
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
                name: "tenant-a-admin".to_string(),
                token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
                    tenant_a_token,
                ),
                role: RuntimeGatewayAdminRole::Admin,
                allowed_key_prefixes: Vec::new(),
                tenant_id: Some("tenant-a".to_string()),
                team_id: None,
                project_id: None,
                user_id: None,
                budget_id: None,
            },
            RuntimeGatewayAdminToken {
                name: "tenant-b-admin".to_string(),
                token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
                    tenant_b_token,
                ),
                role: RuntimeGatewayAdminRole::Admin,
                allowed_key_prefixes: Vec::new(),
                tenant_id: Some("tenant-b".to_string()),
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

    let tenant_a_key = client
        .post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(tenant_a_token)
        .json(&serde_json::json!({"name": "shared-main"}))
        .send()
        .expect("tenant-a create key request should be sent");
    assert_eq!(tenant_a_key.status().as_u16(), 201);
    let tenant_a_key: serde_json::Value = tenant_a_key.json().expect("tenant-a key json");
    assert_eq!(tenant_a_key["key"]["tenant_id"], "tenant-a");
    let tenant_a_virtual_token = tenant_a_key["token"]
        .as_str()
        .expect("tenant-a generated token")
        .to_string();

    let tenant_b_key = client
        .post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(tenant_b_token)
        .json(&serde_json::json!({"name": "other-main"}))
        .send()
        .expect("tenant-b create key request should be sent");
    assert_eq!(tenant_b_key.status().as_u16(), 201);
    let tenant_b_key: serde_json::Value = tenant_b_key.json().expect("tenant-b key json");
    assert_eq!(tenant_b_key["key"]["tenant_id"], "tenant-b");
    let tenant_b_virtual_token = tenant_b_key["token"]
        .as_str()
        .expect("tenant-b generated token")
        .to_string();

    let tenant_a_cross_tenant_create = client
        .post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(tenant_a_token)
        .json(&serde_json::json!({"name": "tenant-b-forbidden", "tenant_id": "tenant-b"}))
        .send()
        .expect("tenant-a cross-tenant create request should be sent");
    assert_eq!(tenant_a_cross_tenant_create.status().as_u16(), 403);

    for (token, input) in [
        (&tenant_a_virtual_token, "tenant-a traffic"),
        (&tenant_b_virtual_token, "tenant-b traffic"),
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
        "other-main",
        200,
    );

    let tenant_a_keys = client
        .get(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(tenant_a_token)
        .send()
        .expect("tenant-a list keys request should be sent");
    assert_eq!(tenant_a_keys.status().as_u16(), 200);
    let tenant_a_keys: serde_json::Value = tenant_a_keys.json().expect("tenant-a keys json");
    let tenant_a_names = tenant_a_keys["keys"]
        .as_array()
        .unwrap()
        .iter()
        .map(|key| key["name"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(tenant_a_names, vec!["shared-main"]);

    let tenant_a_get_b = client
        .get(format!(
            "http://{}/v1/prodex/gateway/keys/other-main",
            proxy.listen_addr
        ))
        .bearer_auth(tenant_a_token)
        .send()
        .expect("tenant-a forbidden get key request should be sent");
    assert_eq!(tenant_a_get_b.status().as_u16(), 403);

    let tenant_a_scim_user = client
        .post(format!(
            "http://{}/v1/prodex/gateway/scim/v2/Users",
            proxy.listen_addr
        ))
        .bearer_auth(tenant_a_token)
        .json(&serde_json::json!({"userName": "alice@example.com"}))
        .send()
        .expect("tenant-a SCIM create request should be sent");
    assert_eq!(tenant_a_scim_user.status().as_u16(), 201);
    let tenant_a_scim_user: serde_json::Value =
        tenant_a_scim_user.json().expect("tenant-a SCIM user json");
    assert_eq!(tenant_a_scim_user["tenant_id"], "tenant-a");

    let tenant_b_scim_user = client
        .post(format!(
            "http://{}/v1/prodex/gateway/scim/v2/Users",
            proxy.listen_addr
        ))
        .bearer_auth(tenant_b_token)
        .json(&serde_json::json!({"userName": "bob@example.com"}))
        .send()
        .expect("tenant-b SCIM create request should be sent");
    assert_eq!(tenant_b_scim_user.status().as_u16(), 201);

    let tenant_a_users = client
        .get(format!(
            "http://{}/v1/prodex/gateway/scim/v2/Users",
            proxy.listen_addr
        ))
        .bearer_auth(tenant_a_token)
        .send()
        .expect("tenant-a SCIM list request should be sent");
    assert_eq!(tenant_a_users.status().as_u16(), 200);
    let tenant_a_users: serde_json::Value = tenant_a_users.json().expect("tenant-a SCIM list json");
    let tenant_a_user_names = tenant_a_users["Resources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|user| user["userName"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(tenant_a_user_names, vec!["alice@example.com"]);

    let tenant_a_ledger = client
        .get(format!(
            "http://{}/v1/prodex/gateway/ledger",
            proxy.listen_addr
        ))
        .bearer_auth(tenant_a_token)
        .send()
        .expect("tenant-a ledger request should be sent");
    assert_eq!(tenant_a_ledger.status().as_u16(), 200);
    let tenant_a_ledger: serde_json::Value = tenant_a_ledger.json().expect("tenant-a ledger json");
    assert_eq!(tenant_a_ledger["records"].as_array().unwrap().len(), 1);
    assert_eq!(tenant_a_ledger["records"][0]["key_name"], "shared-main");

    let tenant_a_metrics = client
        .get(format!(
            "http://{}/v1/prodex/gateway/metrics",
            proxy.listen_addr
        ))
        .bearer_auth(tenant_a_token)
        .send()
        .expect("tenant-a metrics request should be sent");
    assert_eq!(tenant_a_metrics.status().as_u16(), 200);
    let tenant_a_metrics = tenant_a_metrics.text().expect("tenant-a metrics text");
    assert!(tenant_a_metrics.contains("key=\"shared-main\""));
    assert!(!tenant_a_metrics.contains("key=\"other-main\""));
}
