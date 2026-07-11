use super::*;
use crate::{RuntimeProjectedProviderCredential, TestEnvVarGuard};
use prodex_domain::SecretRef;
use secret_store::ProjectedSecretProvider;
use std::os::unix::fs::{PermissionsExt as _, symlink};

#[test]
fn rotation_is_observed_without_environment_reload() {
    let root = temp_root("projected-provider-rotation");
    let paths = app_paths_for_root(root);
    let projected_root = paths.root.join("projected");
    write_generation(&projected_root, "..generation-1", "first-projected-key");
    symlink("..generation-1", projected_root.join("..data")).unwrap();
    let credential = RuntimeProjectedProviderCredential::new(
        SecretRef::new("external", "provider-key", None::<String>),
        ProjectedSecretProvider::new(&projected_root, "external").unwrap(),
    );
    let upstream = TestUpstream::start_n(2);
    let proxy = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths: &paths,
        state: &AppState::default(),
        upstream_base_url: format!("http://{}/v1", upstream.addr),
        provider: RuntimeLocalRewriteProviderOptions::OpenAiResponses {
            api_keys: Vec::new(),
        }
        .with_projected_credential(credential),
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
    })
    .expect("gateway proxy should start");
    let client = reqwest::blocking::Client::new();

    assert_eq!(send(&client, proxy.listen_addr).status().as_u16(), 200);
    assert_authorization(&upstream, "Bearer first-projected-key");

    write_generation(&projected_root, "..generation-2", "second-projected-key");
    symlink("..generation-2", projected_root.join("..data.next")).unwrap();
    fs::rename(
        projected_root.join("..data.next"),
        projected_root.join("..data"),
    )
    .unwrap();
    let _env = TestEnvVarGuard::set("OPENAI_API_KEY", "environment-key-must-not-win");

    assert_eq!(send(&client, proxy.listen_addr).status().as_u16(), 200);
    assert_authorization(&upstream, "Bearer second-projected-key");
    let log = fs::read_to_string(&proxy.log_path).unwrap();
    for secret in [
        "first-projected-key",
        "second-projected-key",
        "environment-key-must-not-win",
    ] {
        assert!(!log.contains(secret));
    }
}

fn write_generation(root: &Path, generation: &str, value: &str) {
    let generation = root.join(generation);
    fs::create_dir_all(&generation).unwrap();
    fs::set_permissions(root, fs::Permissions::from_mode(0o700)).unwrap();
    fs::set_permissions(&generation, fs::Permissions::from_mode(0o700)).unwrap();
    let key = generation.join("provider-key");
    fs::write(&key, value).unwrap();
    fs::set_permissions(key, fs::Permissions::from_mode(0o600)).unwrap();
}

fn send(
    client: &reqwest::blocking::Client,
    addr: std::net::SocketAddr,
) -> reqwest::blocking::Response {
    client
        .post(format!("http://{addr}/v1/responses"))
        .json(&serde_json::json!({"model": "gpt-5.4", "input": "hello"}))
        .send()
        .expect("gateway request should be sent")
}

fn assert_authorization(upstream: &TestUpstream, expected: &str) {
    let headers = upstream
        .headers_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("projected request should reach upstream");
    assert!(
        headers.iter().any(|(name, value)| {
            name.eq_ignore_ascii_case("authorization") && value == expected
        })
    );
    assert!(
        headers
            .iter()
            .all(|(_, value)| { !value.contains("environment-key-must-not-win") })
    );
}
