use super::*;
use crate::TestEnvVarGuard;
use std::fs;

#[test]
fn gateway_admin_can_create_rotate_disable_and_delete_virtual_keys() {
    let root = temp_root("gateway-admin-crud");
    let audit_dir = root.join("audit");
    let _audit_env = TestEnvVarGuard::set("PRODEX_AUDIT_LOG_DIR", audit_dir.to_str().unwrap());
    let paths = app_paths_for_root(root.clone());
    let upstream = TestUpstream::start_n(2);
    let admin_token = "admin-token";
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
        gateway_admin_tokens: vec![runtime_gateway_test_admin_token(admin_token)],
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig {
            blocked_keywords: vec!["secret".to_string()],
            blocked_output_keywords: vec!["leak".to_string()],
            allowed_models: vec!["gpt-5.4".to_string()],
            prompt_injection_detection: true,
            pii_redaction: false,
        },
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig {
            url: Some("https://guardrails.example.test/check".to_string()),
            phases: vec!["request".to_string()],
            bearer_token: Some("webhook-secret".to_string()),
            fail_closed: true,
        },
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
        gateway_observability: RuntimeGatewayObservabilityConfig {
            sinks: vec!["jsonl".to_string(), "http".to_string()],
            jsonl_path: Some(root.join("gateway-observability.jsonl")),
            http_endpoint: Some("https://telemetry.example.test/ingest".to_string()),
            http_schema: "prodex.gateway.event.v1".to_string(),
            http_bearer_token: Some("telemetry-secret".to_string()),
        },
    })
    .expect("gateway proxy should start");
    let client = reqwest::blocking::Client::new();
    let openapi = client
        .get(format!(
            "http://{}/v1/prodex/gateway/openapi.json",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .send()
        .expect("OpenAPI request should be sent");
    assert_eq!(openapi.status().as_u16(), 200);
    assert_eq!(
        openapi
            .headers()
            .get("cache-control")
            .and_then(|value| value.to_str().ok()),
        Some("no-store")
    );
    assert_eq!(
        openapi
            .headers()
            .get("x-content-type-options")
            .and_then(|value| value.to_str().ok()),
        Some("nosniff")
    );
    let wrong_method = client
        .post(format!(
            "http://{}/v1/prodex/gateway/ledger",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .send()
        .expect("wrong method admin request should be sent");
    assert_eq!(wrong_method.status().as_u16(), 405);
    assert_eq!(
        wrong_method.json::<serde_json::Value>().unwrap()["error"]["code"],
        "control_plane_method_not_allowed"
    );
    let openapi: serde_json::Value = openapi.json().expect("OpenAPI response should be json");
    assert_eq!(openapi["openapi"], "3.1.0");
    assert!(openapi["paths"]["/v1/responses"].is_object());
    assert!(openapi["paths"]["/v1/prodex/gateway/keys"].is_object());
    assert!(openapi["paths"]["/v1/prodex/gateway/keys/{name}"].is_object());
    assert!(openapi["paths"]["/v1/prodex/gateway/scim/v2/Users"].is_object());
    assert!(openapi["paths"]["/v1/prodex/gateway/scim/v2/Users/{id}"].is_object());
    assert!(openapi["paths"]["/v1/prodex/gateway/ledger"].is_object());
    assert!(openapi["paths"]["/v1/prodex/gateway/ledger.csv"].is_object());
    assert!(openapi["paths"]["/v1/prodex/gateway/ledger/summary"].is_object());
    assert!(openapi["paths"]["/v1/prodex/gateway/ledger/summary.csv"].is_object());
    assert!(openapi["paths"]["/v1/prodex/gateway/metrics"].is_object());
    assert!(openapi["paths"]["/v1/prodex/gateway/providers"].is_object());
    assert!(openapi["paths"]["/v1/prodex/gateway/observability"].is_object());
    assert!(openapi["paths"]["/v1/prodex/gateway/guardrails"].is_object());
    assert!(openapi["paths"]["/v1/prodex/gateway/admin"].is_object());
    assert_eq!(
        openapi["paths"]["/v1/prodex/gateway/admin"]["get"]["security"][0]["GatewayBearerAuth"],
        serde_json::json!([])
    );
    assert!(openapi["paths"]["/v1/prodex/gateway/admin"]["get"]["responses"]["401"].is_object());
    assert!(openapi["paths"]["/livez"].is_object());
    assert!(openapi["paths"]["/readyz"]["get"]["responses"]["503"].is_object());
    assert!(openapi["paths"]["/readyz"]["get"]["responses"]["405"].is_object());
    assert!(openapi["paths"]["/startupz"]["head"]["responses"]["200"].is_object());
    assert!(openapi["paths"]["/readyz"]["head"]["responses"]["200"]["content"].is_null());
    assert!(openapi["paths"]["/readyz"]["head"]["responses"]["503"]["content"].is_null());
    assert!(
        openapi["components"]["schemas"]["GatewayHealth"]["required"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == "draining")
    );
    assert!(
        openapi["components"]["schemas"]["GatewayHealth"]["properties"]["status"]["enum"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == "draining")
    );
    assert_eq!(
        openapi["paths"]["/readyz"]["get"]["responses"]["405"]["headers"]["Allow"]["schema"]["type"],
        "string"
    );
    assert_eq!(
        openapi["paths"]["/v1/prodex/gateway/keys"]["post"]["parameters"][0]["name"],
        "Idempotency-Key"
    );
    assert_eq!(
        openapi["paths"]["/v1/prodex/gateway/keys/{name}"]["get"]["responses"]["200"]["headers"]["ETag"]
            ["schema"]["type"],
        "string"
    );
    assert_eq!(
        openapi["paths"]["/v1/prodex/gateway/keys/{name}"]["patch"]["parameters"][1]["name"],
        "If-Match"
    );
    assert!(
        openapi["paths"]["/v1/prodex/gateway/keys/{name}"]["patch"]["responses"]["412"].is_object()
    );
    assert_eq!(
        openapi["paths"]["/v1/prodex/gateway/scim/v2/Users"]["post"]["parameters"][0]["name"],
        "Idempotency-Key"
    );
    assert!(openapi["components"]["schemas"]["GatewayHealth"].is_object());
    assert!(
        openapi["components"]["schemas"]["GatewayHealth"]["required"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("status"))
    );
    assert!(
        openapi["components"]["schemas"]["GatewayHealth"]["required"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("policy_version"))
    );
    assert!(openapi["components"]["schemas"]["GatewayHealth"]["properties"]["ok"].is_null());
    assert_eq!(
        openapi["components"]["schemas"]["GatewayHealth"]["properties"]["status"]["enum"],
        serde_json::json!(["ok", "overloaded", "draining", "method_not_allowed"])
    );
    assert_eq!(
        openapi["components"]["schemas"]["GatewayHealth"]["properties"]["policy_version"]["type"],
        serde_json::json!(["integer", "null"])
    );

    let providers = client
        .get(format!(
            "http://{}/v1/prodex/gateway/providers",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .send()
        .expect("providers request should be sent");
    assert_eq!(providers.status().as_u16(), 200);
    let providers: serde_json::Value = providers.json().expect("providers response should be json");
    assert_eq!(providers["object"], "gateway.providers");
    let provider_entries = providers["providers"].as_array().unwrap();
    assert_eq!(provider_entries.len(), 7);
    assert_eq!(provider_entries[0]["provider"], "openai");
    assert!(
        provider_entries
            .iter()
            .any(|provider| provider["provider"] == serde_json::json!("kiro"))
    );
    assert_eq!(
        providers["providers"][0]["client_request_format"],
        "openai-responses"
    );
    assert!(
        providers["providers"][0]["supported_endpoints"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("responses"))
    );
    assert_eq!(providers["providers"][0]["replay_case_count"], 1);

    let observability = client
        .get(format!(
            "http://{}/v1/prodex/gateway/observability",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .send()
        .expect("observability request should be sent");
    assert_eq!(observability.status().as_u16(), 200);
    let observability: serde_json::Value = observability
        .json()
        .expect("observability response should be json");
    assert_eq!(observability["object"], "gateway.observability");
    assert_eq!(observability["call_id_header"], "x-prodex-call-id");
    assert_eq!(observability["sinks"], serde_json::json!(["jsonl", "http"]));
    assert_eq!(
        observability["http_endpoint"],
        "https://telemetry.example.test/ingest"
    );
    assert_eq!(observability["http_bearer_token_configured"], true);
    assert!(!observability.to_string().contains("telemetry-secret"));

    let guardrails = client
        .get(format!(
            "http://{}/v1/prodex/gateway/guardrails",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .send()
        .expect("guardrails request should be sent");
    assert_eq!(guardrails.status().as_u16(), 200);
    let guardrails: serde_json::Value = guardrails
        .json()
        .expect("guardrails response should be json");
    assert_eq!(guardrails["object"], "gateway.guardrails");
    assert_eq!(guardrails["blocked_keywords_count"], 1);
    assert_eq!(guardrails["blocked_output_keywords_count"], 1);
    assert_eq!(guardrails["allowed_models"], serde_json::json!(["gpt-5.4"]));
    assert_eq!(guardrails["prompt_injection_detection"], true);
    assert_eq!(guardrails["pii_redaction"], false);
    assert_eq!(guardrails["webhook"]["configured"], true);
    assert_eq!(guardrails["webhook"]["bearer_token_configured"], true);
    assert!(!guardrails.to_string().contains("webhook-secret"));

    let dashboard_url = format!("http://{}/v1/prodex/gateway/admin", proxy.listen_addr);
    let unauthenticated_dashboard = client
        .get(&dashboard_url)
        .send()
        .expect("unauthenticated admin dashboard request should be sent");
    assert_eq!(unauthenticated_dashboard.status().as_u16(), 401);
    assert_eq!(
        unauthenticated_dashboard
            .headers()
            .get("cache-control")
            .and_then(|value| value.to_str().ok()),
        Some("no-store")
    );
    assert_eq!(
        unauthenticated_dashboard
            .headers()
            .get("x-content-type-options")
            .and_then(|value| value.to_str().ok()),
        Some("nosniff")
    );

    let dashboard = client
        .get(&dashboard_url)
        .bearer_auth(admin_token)
        .send()
        .expect("admin dashboard request should be sent");
    assert_eq!(dashboard.status().as_u16(), 200);
    assert!(
        dashboard
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .contains("text/html")
    );
    assert_eq!(
        dashboard
            .headers()
            .get("cache-control")
            .and_then(|value| value.to_str().ok()),
        Some("no-store")
    );
    assert_eq!(
        dashboard
            .headers()
            .get("x-content-type-options")
            .and_then(|value| value.to_str().ok()),
        Some("nosniff")
    );
    assert_eq!(
        dashboard
            .headers()
            .get("x-frame-options")
            .and_then(|value| value.to_str().ok()),
        Some("DENY")
    );
    let csp = dashboard
        .headers()
        .get("content-security-policy")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert!(csp.contains("default-src 'none'"));
    assert!(csp.contains("frame-ancestors 'none'"));
    let dashboard = dashboard.text().expect("dashboard should be text");
    assert!(dashboard.contains("Prodex Gateway Admin"));
    assert!(dashboard.contains("SSO Users"));
    assert!(dashboard.contains("/scim/v2/Users"));
    assert!(dashboard.contains("userTeamId"));
    assert!(dashboard.contains("userBudgetId"));
    assert!(dashboard.contains("/v1/prodex/gateway"));

    let unauthenticated_keys = client
        .get(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .send()
        .expect("unauthenticated admin API request should be sent");
    assert_eq!(unauthenticated_keys.status().as_u16(), 401);

    let missing_idempotency_key = client
        .post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({"name": "team-missing-idempotency"}))
        .send()
        .expect("create key request without idempotency should be sent");
    assert_eq!(missing_idempotency_key.status().as_u16(), 400);
    let missing_idempotency_key: serde_json::Value = missing_idempotency_key
        .json()
        .expect("missing idempotency response should be json");
    assert_eq!(
        missing_idempotency_key["error"]["code"],
        "control_plane_idempotency_key_required"
    );
    assert_eq!(
        missing_idempotency_key["error"]["message"],
        "control-plane idempotency key is required"
    );

    let created = client
        .idempotent_post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({
            "name": "team-crud",
            "allowed_models": ["gpt-5.4"],
            "request_budget": 2,
            "rpm_limit": 10,
            "tpm_limit": 1000
        }))
        .send()
        .expect("create key request should be sent");
    assert_eq!(created.status().as_u16(), 201);
    let created: serde_json::Value = created.json().expect("create key response should be json");
    let first_token = created["token"]
        .as_str()
        .expect("generated token should be returned once")
        .to_string();
    assert_eq!(created["key"]["source"], "admin");
    assert_eq!(created["key"]["name"], "team-crud");
    let virtual_key_id = created["key"]["virtual_key_id"]
        .as_str()
        .expect("virtual key id should be returned");
    let parsed_virtual_key_id = virtual_key_id
        .parse::<prodex_domain::VirtualKeyId>()
        .expect("virtual key id should be canonical uuid");
    assert_eq!(parsed_virtual_key_id.to_string(), virtual_key_id);

    let first = client
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth(&first_token)
        .json(&serde_json::json!({"model": "gpt-5.4", "input": "first"}))
        .send()
        .expect("request with generated key should be sent");
    assert_eq!(first.status().as_u16(), 200);
    let _ = upstream
        .body_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("upstream should receive first request");

    let disabled = client
        .idempotent_patch(format!(
            "http://{}/v1/prodex/gateway/keys/team-crud",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({"disabled": true}))
        .send()
        .expect("disable key request should be sent");
    assert_eq!(disabled.status().as_u16(), 200);
    assert_eq!(
        disabled.json::<serde_json::Value>().unwrap()["key"]["disabled"],
        true
    );

    let rejected = client
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth(&first_token)
        .json(&serde_json::json!({"model": "gpt-5.4", "input": "blocked"}))
        .send()
        .expect("request with disabled key should be sent");
    assert_eq!(rejected.status().as_u16(), 401);

    let rotated = client
        .idempotent_patch(format!(
            "http://{}/v1/prodex/gateway/keys/team-crud",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({"disabled": false, "rotate": true}))
        .send()
        .expect("rotate key request should be sent");
    assert_eq!(rotated.status().as_u16(), 200);
    let rotated: serde_json::Value = rotated.json().expect("rotate response should be json");
    let rotated_token = rotated["token"]
        .as_str()
        .expect("rotated token should be returned once");
    assert_ne!(rotated_token, first_token);

    let old_rejected = client
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth(&first_token)
        .json(&serde_json::json!({"model": "gpt-5.4", "input": "old"}))
        .send()
        .expect("request with old rotated key should be sent");
    assert_eq!(old_rejected.status().as_u16(), 401);

    let second = client
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth(rotated_token)
        .json(&serde_json::json!({"model": "gpt-5.4", "input": "second"}))
        .send()
        .expect("request with rotated key should be sent");
    assert_eq!(second.status().as_u16(), 200);
    let _ = upstream
        .body_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("upstream should receive second request");

    let deleted = client
        .idempotent_delete(format!(
            "http://{}/v1/prodex/gateway/keys/team-crud",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .send()
        .expect("delete key request should be sent");
    assert_eq!(deleted.status().as_u16(), 200);

    let missing = client
        .get(format!(
            "http://{}/v1/prodex/gateway/keys/team-crud",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .send()
        .expect("get deleted key request should be sent");
    assert_eq!(missing.status().as_u16(), 404);
    let store = wait_for_json_file(&root.join("gateway-virtual-keys.json"));
    assert_eq!(store["keys"].as_array().unwrap().len(), 0);
    let audit_log = fs::read_to_string(audit_dir.join("prodex-audit.log"))
        .expect("gateway admin audit log should be written");
    assert!(audit_log.contains(r#""component":"gateway_admin""#));
    assert!(audit_log.contains(r#""action":"create_key""#));
    assert!(audit_log.contains(r#""action":"update_key""#));
    assert!(audit_log.contains(r#""action":"rotate_key""#));
    assert!(audit_log.contains(r#""action":"delete_key""#));
    assert!(audit_log.contains(r#""action":"auth_failed""#));
    assert!(audit_log.contains(r#""reason":"admin_authentication_required""#));
    assert!(audit_log.contains(r#""key_name":"team-crud""#));
    assert!(!audit_log.contains(admin_token));
    assert!(!audit_log.contains(&first_token));
    assert!(!audit_log.contains(rotated_token));
}

#[test]
fn gateway_admin_mutation_idempotency_key_rejects_duplicate_replay() {
    let root = temp_root("gateway-admin-idempotency");
    let audit_dir = root.join("audit");
    fs::create_dir_all(&audit_dir).expect("audit dir should be created");
    let _audit_env = TestEnvVarGuard::set("PRODEX_AUDIT_LOG_DIR", audit_dir.to_str().unwrap());
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(0);
    let admin_token = "admin-idempotency-token";
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
        gateway_admin_tokens: vec![RuntimeGatewayAdminToken {
            name: "admin".to_string(),
            token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(admin_token),
            role: RuntimeGatewayAdminRole::Admin,
            allowed_key_prefixes: Vec::new(),
            tenant_id: None,
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
        }],
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
    let create_url = format!("http://{}/v1/prodex/gateway/keys", proxy.listen_addr);
    let first = client
        .post(&create_url)
        .bearer_auth(admin_token)
        .header("Idempotency-Key", "idem-create-key-1")
        .json(&serde_json::json!({"name": "team-idem-a"}))
        .send()
        .expect("first idempotent create should be sent");
    assert_eq!(first.status().as_u16(), 201);

    let duplicate = client
        .post(&create_url)
        .bearer_auth(admin_token)
        .header("Idempotency-Key", "idem-create-key-1")
        .json(&serde_json::json!({"name": "team-idem-b"}))
        .send()
        .expect("duplicate idempotent create should be sent");
    assert_eq!(duplicate.status().as_u16(), 409);
    let duplicate: serde_json::Value = duplicate.json().expect("duplicate response should be json");
    assert_eq!(duplicate["error"]["code"], "duplicate_idempotency_key");

    let keys = client
        .get(&create_url)
        .bearer_auth(admin_token)
        .send()
        .expect("keys list should be sent");
    assert_eq!(keys.status().as_u16(), 200);
    let keys: serde_json::Value = keys.json().expect("keys list should be json");
    let names = keys["keys"]
        .as_array()
        .expect("keys should be array")
        .iter()
        .filter_map(|key| key["name"].as_str())
        .collect::<Vec<_>>();
    assert!(names.contains(&"team-idem-a"));
    assert!(!names.contains(&"team-idem-b"));

    let audit_log = fs::read_to_string(audit_dir.join("prodex-audit.log"))
        .expect("idempotency denial should be audited");
    assert!(audit_log.contains(r#""component":"gateway_admin""#));
    assert!(audit_log.contains(r#""action":"request_denied""#));
    assert!(audit_log.contains(r#""reason":"duplicate_idempotency_key""#));
    assert!(audit_log.contains(r#""actor":"admin""#));
    assert!(audit_log.contains(r#""path":"/v1/prodex/gateway/keys""#));
    assert!(!audit_log.contains(admin_token));
    assert!(!audit_log.contains("idem-create-key-1"));
}

#[test]
fn gateway_admin_idempotency_key_is_scoped_by_admin_principal() {
    let root = temp_root("gateway-admin-idempotency-scope");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(0);
    let tenant_a_token = "tenant-a-admin-idem-token";
    let tenant_b_token = "tenant-b-admin-idem-token";
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
    let create_url = format!("http://{}/v1/prodex/gateway/keys", proxy.listen_addr);
    let tenant_a = client
        .post(&create_url)
        .bearer_auth(tenant_a_token)
        .header("Idempotency-Key", "same-client-key")
        .json(&serde_json::json!({"name": "tenant-a-key"}))
        .send()
        .expect("tenant-a idempotent create should be sent");
    assert_eq!(tenant_a.status().as_u16(), 201);

    let tenant_b = client
        .post(&create_url)
        .bearer_auth(tenant_b_token)
        .header("Idempotency-Key", "same-client-key")
        .json(&serde_json::json!({"name": "tenant-b-key"}))
        .send()
        .expect("tenant-b idempotent create should be sent");
    assert_eq!(tenant_b.status().as_u16(), 201);
}

#[test]
fn gateway_admin_key_mutations_honor_if_match_etag() {
    let root = temp_root("gateway-admin-etag");
    let audit_dir = root.join("audit");
    fs::create_dir_all(&audit_dir).expect("audit dir should be created");
    let _audit_env = TestEnvVarGuard::set("PRODEX_AUDIT_LOG_DIR", audit_dir.to_str().unwrap());
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(0);
    let admin_token = "admin-etag-token";
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
        gateway_admin_tokens: vec![RuntimeGatewayAdminToken {
            name: "admin".to_string(),
            token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(admin_token),
            role: RuntimeGatewayAdminRole::Admin,
            allowed_key_prefixes: Vec::new(),
            tenant_id: None,
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
        }],
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
    let keys_url = format!("http://{}/v1/prodex/gateway/keys", proxy.listen_addr);
    let key_url = format!("{keys_url}/team-etag");
    let created = client
        .idempotent_post(&keys_url)
        .bearer_auth(admin_token)
        .json(&serde_json::json!({"name": "team-etag"}))
        .send()
        .expect("create key should be sent");
    assert_eq!(created.status().as_u16(), 201);

    let get_key = client
        .get(&key_url)
        .bearer_auth(admin_token)
        .send()
        .expect("get key should be sent");
    assert_eq!(get_key.status().as_u16(), 200);
    assert_eq!(
        get_key
            .headers()
            .get("cache-control")
            .and_then(|value| value.to_str().ok()),
        Some("no-store")
    );
    assert_eq!(
        get_key
            .headers()
            .get("x-content-type-options")
            .and_then(|value| value.to_str().ok()),
        Some("nosniff")
    );
    let etag = get_key
        .headers()
        .get("etag")
        .and_then(|value| value.to_str().ok())
        .expect("GET key should return ETag")
        .to_string();
    assert!(etag.starts_with("\"gateway-key-"));

    let patched = client
        .idempotent_patch(&key_url)
        .bearer_auth(admin_token)
        .header("If-Match", &etag)
        .json(&serde_json::json!({"disabled": true}))
        .send()
        .expect("patch key should be sent");
    assert_eq!(patched.status().as_u16(), 200);

    let stale_delete = client
        .idempotent_delete(&key_url)
        .bearer_auth(admin_token)
        .header("If-Match", "\"gateway-key-0\"")
        .send()
        .expect("stale delete should be sent");
    assert_eq!(stale_delete.status().as_u16(), 412);
    let stale_delete: serde_json::Value = stale_delete
        .json()
        .expect("stale delete response should be json");
    assert_eq!(stale_delete["error"]["code"], "precondition_failed");

    let audit_log = fs::read_to_string(audit_dir.join("prodex-audit.log"))
        .expect("precondition denial should be audited");
    assert!(audit_log.contains(r#""component":"gateway_admin""#));
    assert!(audit_log.contains(r#""action":"request_denied""#));
    assert!(audit_log.contains(r#""reason":"precondition_failed""#));
    assert!(audit_log.contains(r#""actor":"admin""#));
    assert!(audit_log.contains(r#""path":"/v1/prodex/gateway/keys/team-etag""#));
    assert!(!audit_log.contains(admin_token));
    assert!(!audit_log.contains("gateway-key-0"));

    let still_present = client
        .get(&key_url)
        .bearer_auth(admin_token)
        .send()
        .expect("get key after stale delete should be sent");
    assert_eq!(still_present.status().as_u16(), 200);
}

#[test]
fn gateway_admin_policy_backed_key_mutation_denials_are_audited() {
    let root = temp_root("gateway-admin-read-only-audit");
    let audit_dir = root.join("audit");
    fs::create_dir_all(&audit_dir).expect("audit dir should be created");
    let _audit_env = TestEnvVarGuard::set("PRODEX_AUDIT_LOG_DIR", audit_dir.to_str().unwrap());
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(0);
    let admin_token = "admin-read-only-token";
    let policy_token = "policy-read-only-token";
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
        gateway_admin_tokens: vec![RuntimeGatewayAdminToken {
            name: "admin".to_string(),
            token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(admin_token),
            role: RuntimeGatewayAdminRole::Admin,
            allowed_key_prefixes: Vec::new(),
            tenant_id: None,
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
        }],
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: vec![runtime_proxy_crate::RuntimeGatewayVirtualKey {
            name: "policy-key".to_string(),
            tenant_id: Some("tenant-a".to_string()),
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(policy_token),
            allowed_models: Vec::new(),
            budget_microusd: None,
            request_budget: None,
            rpm_limit: None,
            tpm_limit: None,
        }],
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("gateway proxy should start");

    let client = reqwest::blocking::Client::new();
    let key_url = format!(
        "http://{}/v1/prodex/gateway/keys/policy-key",
        proxy.listen_addr
    );

    let patch = client
        .idempotent_patch(&key_url)
        .bearer_auth(admin_token)
        .json(&serde_json::json!({"disabled": true}))
        .send()
        .expect("policy-backed key patch should be sent");
    assert_eq!(patch.status().as_u16(), 403);
    let patch: serde_json::Value = patch.json().expect("patch response should be json");
    assert_eq!(patch["error"]["code"], "gateway_key_read_only");

    let delete = client
        .idempotent_delete(&key_url)
        .bearer_auth(admin_token)
        .send()
        .expect("policy-backed key delete should be sent");
    assert_eq!(delete.status().as_u16(), 403);
    let delete: serde_json::Value = delete.json().expect("delete response should be json");
    assert_eq!(delete["error"]["code"], "gateway_key_read_only");

    let audit_log = fs::read_to_string(audit_dir.join("prodex-audit.log"))
        .expect("read-only mutation denials should be audited");
    assert!(audit_log.contains(r#""component":"gateway_admin""#));
    assert!(audit_log.contains(r#""action":"authorization_denied""#));
    assert!(audit_log.contains(r#""reason":"gateway_key_read_only""#));
    assert!(audit_log.contains(r#""resource":"gateway_key""#));
    assert!(audit_log.contains(r#""action":"update_key""#));
    assert!(audit_log.contains(r#""action":"delete_key""#));
    assert!(audit_log.contains(r#""resource_name":"policy-key""#));
    assert!(audit_log.contains(r#""actor":"admin""#));
    assert!(!audit_log.contains(admin_token));
    assert!(!audit_log.contains(policy_token));
}

#[test]
fn gateway_admin_invalid_json_uses_stable_error_without_parser_details() {
    let root = temp_root("gateway-admin-invalid-json");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(0);
    let admin_token = "admin-invalid-json-token";
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
        gateway_admin_tokens: vec![RuntimeGatewayAdminToken {
            name: "admin".to_string(),
            token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(admin_token),
            role: RuntimeGatewayAdminRole::Admin,
            tenant_id: None,
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            allowed_key_prefixes: Vec::new(),
        }],
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
    })
    .expect("gateway proxy should start");

    let response = reqwest::blocking::Client::new()
        .idempotent_post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .header("content-type", "application/json")
        .body(r#"{"name":"broken""#)
        .send()
        .expect("invalid json request should be sent");
    assert_eq!(response.status().as_u16(), 400);
    let body: serde_json::Value = response.json().expect("error response should be json");
    assert_eq!(body["error"]["code"], "invalid_json");
    assert_eq!(body["error"]["message"], "request body is not valid JSON");
    let rendered = body.to_string();
    assert!(!rendered.contains("line"));
    assert!(!rendered.contains("column"));
    assert!(!rendered.contains("EOF"));
    assert!(!rendered.contains(admin_token));
}

#[test]
fn gateway_admin_corrupt_key_store_uses_stable_error_without_parser_details() {
    let root = temp_root("gateway-admin-corrupt-key-store");
    let paths = app_paths_for_root(root.clone());
    let upstream = TestUpstream::start_n(0);
    let admin_token = "admin-corrupt-store-token";
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
        gateway_admin_tokens: vec![RuntimeGatewayAdminToken {
            name: "admin".to_string(),
            token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(admin_token),
            role: RuntimeGatewayAdminRole::Admin,
            tenant_id: None,
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            allowed_key_prefixes: Vec::new(),
        }],
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
    })
    .expect("gateway proxy should start");

    std::fs::write(
        root.join("gateway-virtual-keys.json"),
        b"{ not valid json at line 1 }",
    )
    .expect("corrupt key store should be written");
    let response = reqwest::blocking::Client::new()
        .idempotent_post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({
            "name": "new-key",
            "tenant_id": "tenant-a"
        }))
        .send()
        .expect("create request should be sent");
    assert_eq!(response.status().as_u16(), 500);
    let body: serde_json::Value = response.json().expect("error response should be json");
    assert_eq!(body["error"]["code"], "gateway_key_store_invalid");
    assert_eq!(body["error"]["message"], "gateway key store is invalid");
    let rendered = body.to_string();
    assert!(!rendered.contains("line"));
    assert!(!rendered.contains("column"));
    assert!(!rendered.contains("expected"));
    assert!(!rendered.contains("not valid json"));
    assert!(!rendered.contains(admin_token));
}

#[test]
fn gateway_admin_ledger_load_errors_use_stable_response_without_path_details() {
    let root = temp_root("gateway-admin-ledger-load-error");
    let paths = app_paths_for_root(root.clone());
    fs::create_dir_all(root.join("gateway-billing-ledger.jsonl"))
        .expect("ledger path directory should be created");
    let upstream = TestUpstream::start_n(0);
    let admin_token = "admin-ledger-load-error-token";
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
        gateway_admin_tokens: vec![RuntimeGatewayAdminToken {
            name: "admin".to_string(),
            token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(admin_token),
            role: RuntimeGatewayAdminRole::Admin,
            tenant_id: None,
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            allowed_key_prefixes: Vec::new(),
        }],
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
    for (path, code, message) in [
        (
            "ledger",
            "gateway_billing_ledger_load_failed",
            "gateway billing ledger could not be loaded",
        ),
        (
            "ledger.csv",
            "gateway_billing_ledger_load_failed",
            "gateway billing ledger could not be loaded",
        ),
        (
            "ledger/summary",
            "gateway_billing_summary_load_failed",
            "gateway billing summary could not be loaded",
        ),
        (
            "ledger/summary.csv",
            "gateway_billing_summary_load_failed",
            "gateway billing summary could not be loaded",
        ),
    ] {
        let response = client
            .get(format!(
                "http://{}/v1/prodex/gateway/{path}",
                proxy.listen_addr
            ))
            .bearer_auth(admin_token)
            .send()
            .expect("admin ledger request should be sent");
        assert_eq!(response.status().as_u16(), 500);
        let body: serde_json::Value = response.json().expect("error response should be json");
        assert_eq!(body["error"]["code"], code);
        assert_eq!(body["error"]["message"], message);
        let rendered = body.to_string();
        assert!(!rendered.contains("Is a directory"));
        assert!(!rendered.contains("gateway-billing-ledger.jsonl"));
        assert!(!rendered.contains(&root.display().to_string()));
        assert!(!rendered.contains(admin_token));
    }
}

#[test]
fn gateway_admin_key_store_lock_errors_use_stable_response_without_path_details() {
    let root = temp_root("gateway-admin-key-store-lock-error");
    let paths = app_paths_for_root(root.clone());
    fs::create_dir_all(root.join("gateway-virtual-keys.json.lock"))
        .expect("lock path directory should be created");
    let upstream = TestUpstream::start_n(0);
    let admin_token = "admin-key-store-lock-error-token";
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
        gateway_admin_tokens: vec![RuntimeGatewayAdminToken {
            name: "admin".to_string(),
            token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(admin_token),
            role: RuntimeGatewayAdminRole::Admin,
            tenant_id: None,
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            allowed_key_prefixes: Vec::new(),
        }],
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

    let response = reqwest::blocking::Client::new()
        .idempotent_post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({
            "name": "new-key",
            "tenant_id": "tenant-a"
        }))
        .send()
        .expect("create request should be sent");
    assert_eq!(response.status().as_u16(), 500);
    let body: serde_json::Value = response.json().expect("error response should be json");
    assert_eq!(body["error"]["code"], "gateway_key_store_lock_failed");
    assert_eq!(
        body["error"]["message"],
        "gateway key store lock could not be acquired"
    );
    let rendered = body.to_string();
    assert!(!rendered.contains("Is a directory"));
    assert!(!rendered.contains("gateway-virtual-keys.json.lock"));
    assert!(!rendered.contains(&root.display().to_string()));
    assert!(!rendered.contains(admin_token));
}
