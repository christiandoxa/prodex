use super::{TestUpstream, app_paths_for_root, runtime_gateway_test_admin_token, temp_root};
use crate::AppState;
use crate::runtime_launch::proxy_startup::local_rewrite::{
    RuntimeGatewayGuardrailWebhookConfig, RuntimeGatewayObservabilityConfig,
    RuntimeGatewaySsoConfig, RuntimeGatewayStateStore, RuntimeLocalRewriteProviderOptions,
    RuntimeLocalRewriteProxyStartOptions, start_runtime_local_rewrite_proxy,
};
use std::fs;
use std::path::Path;

#[test]
fn local_rewrite_rejects_url_secret_before_log_or_listener_setup() {
    let root = temp_root("local-rewrite-url-boundary");
    let paths = app_paths_for_root(root.clone());
    let log_dir = root.join("runtime-logs");
    let _log_dir = crate::TestEnvVarGuard::set("PRODEX_RUNTIME_LOG_DIR", log_dir.to_str().unwrap());
    let sentinel = "local-rewrite-url-secret-sentinel";

    let error = match start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths: &paths,
        state: &AppState::default(),
        upstream_base_url: format!("https://user:{sentinel}@example.test/v1"),
        provider: RuntimeLocalRewriteProviderOptions::OpenAiResponses {
            api_keys: Vec::new(),
        },
        upstream_no_proxy: false,
        smart_context_enabled: false,
        presidio_redaction_enabled: false,
        model_context_window_tokens: None,
        preferred_listen_addr: Some("127.0.0.1:0"),
        gateway_auth_token_hash: None,
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: None,
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    }) {
        Ok(_) => panic!("credential-bearing URL should fail before local rewrite startup"),
        Err(error) => error.to_string(),
    };

    assert!(
        error.contains("no credentials, query, or fragment"),
        "{error}"
    );
    assert!(!error.contains(sentinel), "{error}");
    assert!(
        !log_dir.exists(),
        "invalid URL must fail before the runtime log directory is created"
    );
    let _ = fs::remove_dir_all(root);
}

fn assert_files_exclude_canaries(root: &Path, canaries: &[&str]) {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
        Err(error) => panic!("failed to scan {}: {error}", root.display()),
    };
    for entry in entries {
        let path = match entry {
            Ok(entry) => entry.path(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => panic!("failed to scan {}: {error}", root.display()),
        };
        if path.is_dir() {
            assert_files_exclude_canaries(&path, canaries);
            continue;
        }
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => panic!("failed to scan {}: {error}", path.display()),
        };
        for canary in canaries {
            assert!(
                !bytes
                    .windows(canary.len())
                    .any(|window| window == canary.as_bytes()),
                "secret canary escaped into {}",
                path.display()
            );
        }
    }
}

#[test]
fn gateway_release_secret_canaries_only_reach_the_authorized_upstream() {
    const RESPONSE_CANARY: &str = "response-content-canary-release-test";
    let root = temp_root("gateway-release-secret-canary");
    let paths = app_paths_for_root(root.clone());
    let runtime_logs = root.join("runtime-logs");
    let audit_logs = root.join("audit-logs");
    let _runtime_log =
        crate::TestEnvVarGuard::set("PRODEX_RUNTIME_LOG_DIR", runtime_logs.to_str().unwrap());
    let _audit_log =
        crate::TestEnvVarGuard::set("PRODEX_AUDIT_LOG_DIR", audit_logs.to_str().unwrap());
    let nonce = prodex_domain::TenantId::new().to_string();
    let provider_canary = format!("provider-secret-canary-{nonce}");
    let virtual_key_canary = format!("virtual-key-canary-{nonce}");
    let admin_canary = format!("admin-secret-canary-{nonce}");
    let prompt_canary = format!("prompt-canary-{nonce}");
    let upstream = TestUpstream::start_with_response_body(
        r#"{"id":"resp_canary","output_text":"response-content-canary-release-test"}"#,
    );
    let proxy = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths: &paths,
        state: &AppState::default(),
        upstream_base_url: format!("http://{}/v1", upstream.addr),
        provider: RuntimeLocalRewriteProviderOptions::OpenAiResponses {
            api_keys: vec![provider_canary.clone()],
        },
        upstream_no_proxy: false,
        smart_context_enabled: false,
        presidio_redaction_enabled: false,
        model_context_window_tokens: None,
        preferred_listen_addr: Some("127.0.0.1:0"),
        gateway_auth_token_hash: None,
        gateway_admin_tokens: vec![runtime_gateway_test_admin_token(&admin_canary)],
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: vec![runtime_proxy_crate::RuntimeGatewayVirtualKey {
            name: "canary-key".to_string(),
            tenant_id: Some("tenant-canary".to_string()),
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
                &virtual_key_canary,
            ),
            allowed_models: vec!["gpt-5.4".to_string()],
            budget_microusd: Some(1_000_000),
            request_budget: Some(10),
            rpm_limit: Some(10),
            tpm_limit: Some(10_000),
        }],
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: None,
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .unwrap();

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth(&virtual_key_canary)
        .json(&serde_json::json!({"model": "gpt-5.4", "input": &prompt_canary}))
        .send()
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
    let response_body = response.text().unwrap();
    for canary in [
        provider_canary.as_str(),
        virtual_key_canary.as_str(),
        admin_canary.as_str(),
        prompt_canary.as_str(),
    ] {
        assert!(!response_body.contains(canary));
    }
    assert!(response_body.contains(RESPONSE_CANARY));

    let upstream_headers = upstream.headers_rx.recv().unwrap();
    assert!(upstream_headers.iter().any(|(name, value)| {
        name == "authorization" && value == &format!("Bearer {provider_canary}")
    }));
    let upstream_body = String::from_utf8(upstream.body_rx.recv().unwrap()).unwrap();
    assert!(upstream_body.contains(&prompt_canary));

    drop(proxy);
    assert_files_exclude_canaries(
        &root,
        &[
            &provider_canary,
            &virtual_key_canary,
            &admin_canary,
            &prompt_canary,
            RESPONSE_CANARY,
        ],
    );
}
