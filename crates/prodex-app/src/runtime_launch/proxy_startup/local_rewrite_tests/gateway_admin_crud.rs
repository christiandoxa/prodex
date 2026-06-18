use super::*;
use std::ffi::OsString;
use std::fs;
use std::sync::{Mutex, MutexGuard, OnceLock};

fn env_lock() -> &'static Mutex<()> {
    static TEST_ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    TEST_ENV_LOCK.get_or_init(|| Mutex::new(()))
}

struct TestEnvVarGuard {
    key: &'static str,
    previous: Option<OsString>,
    _guard: MutexGuard<'static, ()>,
}

impl TestEnvVarGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let guard = env_lock().lock().unwrap();
        let previous = std::env::var_os(key);
        // SAFETY: the shared test env lock serializes mutation and restoration.
        unsafe { std::env::set_var(key, value) };
        Self {
            key,
            previous,
            _guard: guard,
        }
    }
}

impl Drop for TestEnvVarGuard {
    fn drop(&mut self) {
        if let Some(previous) = self.previous.as_ref() {
            // SAFETY: the shared test env lock serializes mutation and restoration.
            unsafe { std::env::set_var(self.key, previous) };
        } else {
            // SAFETY: the shared test env lock serializes mutation and restoration.
            unsafe { std::env::remove_var(self.key) };
        }
    }
}

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
        gateway_auth_token_hash: Some(runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
            admin_token,
        )),
        gateway_admin_tokens: Vec::new(),
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
    assert_eq!(providers["providers"].as_array().unwrap().len(), 6);
    assert_eq!(providers["providers"][0]["provider"], "openai");
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

    let dashboard = client
        .get(format!(
            "http://{}/v1/prodex/gateway/admin",
            proxy.listen_addr
        ))
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

    let created = client
        .post(format!(
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
        .patch(format!(
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
        .patch(format!(
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
        .delete(format!(
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
    assert!(audit_log.contains(r#""key_name":"team-crud""#));
    assert!(!audit_log.contains(&first_token));
    assert!(!audit_log.contains(rotated_token));
}
