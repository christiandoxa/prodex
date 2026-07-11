use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use tiny_http::{Header as TinyHeader, Response as TinyResponse, Server as TinyServer};

use super::{
    AppState, RuntimeGatewayGuardrailWebhookConfig, RuntimeGatewayObservabilityConfig,
    RuntimeGatewaySsoConfig, RuntimeGatewayStateStore, RuntimeLocalRewriteProviderOptions,
    RuntimeLocalRewriteProxyStartOptions, TestUpstream, app_paths_for_root,
    start_runtime_local_rewrite_proxy, temp_root,
};

#[test]
fn gateway_operational_health_endpoints_are_public_and_machine_readable() {
    let root = temp_root("gateway-operational-health");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(0);
    let gateway_token = "gateway-token";
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
            gateway_token,
        )),
        gateway_admin_tokens: Vec::new(),
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
    for (path, probe) in [
        ("/livez", "livez"),
        ("/readyz", "readyz"),
        ("/startupz", "startupz"),
    ] {
        let response = client
            .get(format!("http://{}{}", proxy.listen_addr, path))
            .send()
            .expect("health request should be sent");
        assert_eq!(response.status().as_u16(), 200);
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_string();
        assert!(content_type.contains("application/json"));
        let body: serde_json::Value = response.json().expect("health response should be json");
        assert_eq!(body["object"], "gateway.health");
        assert_eq!(body["probe"], probe);
        assert_eq!(body["status"], "ok");
        assert_eq!(body["ready"], true);
        assert_eq!(body["local_overload"], false);
        assert_eq!(body["draining"], false);
        assert!(body["active_request_limit"].as_u64().is_some());

        let head = client
            .head(format!("http://{}{}", proxy.listen_addr, path))
            .send()
            .expect("health HEAD request should be sent");
        assert_eq!(head.status().as_u16(), 200);
        assert!(
            head.bytes()
                .expect("health HEAD response body should be readable")
                .is_empty()
        );

        let rejected_method = client
            .post(format!("http://{}{}", proxy.listen_addr, path))
            .send()
            .expect("health method rejection should be sent");
        assert_eq!(rejected_method.status().as_u16(), 405);
        assert_eq!(
            rejected_method
                .headers()
                .get("allow")
                .and_then(|value| value.to_str().ok()),
            Some("GET, HEAD")
        );
        let rejected_method: serde_json::Value = rejected_method
            .json()
            .expect("health method rejection should be json");
        assert_eq!(rejected_method["object"], "gateway.health");
        assert_eq!(rejected_method["probe"], probe);
        assert_eq!(rejected_method["status"], "method_not_allowed");
    }
}

#[test]
fn gateway_rejects_unknown_and_ambiguous_targets_before_upstream_work() {
    let root = temp_root("gateway-canonical-target-denial");
    let paths = app_paths_for_root(root);
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

    let unknown = client
        .get(format!("http://{}/v1/not-supported", proxy.listen_addr))
        .send()
        .expect("unknown request should be sent");
    assert_eq!(unknown.status().as_u16(), 404);
    let unknown: serde_json::Value = unknown.json().expect("unknown response should be json");
    assert_eq!(unknown["error"]["code"], "route_not_available");

    for path in ["/v1//responses", "/v1/%2e%2e/admin", "/v1/%2Fadmin"] {
        let mut stream = TcpStream::connect(proxy.listen_addr)
            .expect("raw ambiguous request should connect to gateway");
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("raw gateway request should set a read timeout");
        write!(
            stream,
            "GET {path} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
            proxy.listen_addr,
        )
        .expect("raw ambiguous request should be written exactly");
        let mut raw = Vec::new();
        stream
            .read_to_end(&mut raw)
            .expect("raw gateway response should be readable");
        let split = raw
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .expect("raw gateway response should contain headers");
        let headers = std::str::from_utf8(&raw[..split])
            .expect("raw gateway response headers should be UTF-8");
        assert!(headers.starts_with("HTTP/1.1 400"), "{path}: {headers}");
        let response: serde_json::Value =
            serde_json::from_slice(&raw[split + 4..]).expect("invalid response should be JSON");
        assert_eq!(response["error"]["code"], "invalid_request_target");
    }
}

#[test]
fn gateway_operational_health_exposes_active_policy_version() {
    prodex_runtime_policy::clear_runtime_policy_cache();
    let root = temp_root("gateway-health-policy-version");
    std::fs::write(root.join("policy.toml"), "version = 1\n").expect("policy should be written");
    let _home = crate::TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let paths = app_paths_for_root(root);
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
        gateway_admin_tokens: Vec::new(),
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

    let body: serde_json::Value = reqwest::blocking::Client::new()
        .get(format!("http://{}/readyz", proxy.listen_addr))
        .send()
        .expect("health request should be sent")
        .json()
        .expect("health response should be json");
    assert_eq!(body["policy_version"], 1);
    prodex_runtime_policy::clear_runtime_policy_cache();
}

#[test]
fn gateway_readyz_fails_while_draining_without_failing_livez_or_startupz() {
    let _workers = crate::TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_WORKER_COUNT", "4");
    let root = temp_root("gateway-health-draining");
    let paths = app_paths_for_root(root);
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
        gateway_admin_tokens: Vec::new(),
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

    proxy
        .shutdown
        .store(true, std::sync::atomic::Ordering::SeqCst);

    let client = reqwest::blocking::Client::new();
    let readyz = client
        .get(format!("http://{}/readyz", proxy.listen_addr))
        .send()
        .expect("readyz should be sent");
    assert_eq!(readyz.status().as_u16(), 503);
    let readyz: serde_json::Value = readyz.json().expect("readyz response should be json");
    assert_eq!(readyz["status"], "draining");
    assert_eq!(readyz["ready"], false);
    assert_eq!(readyz["local_overload"], false);
    assert_eq!(readyz["draining"], true);

    let readyz_head = client
        .head(format!("http://{}/readyz", proxy.listen_addr))
        .send()
        .expect("readyz HEAD should be sent");
    assert_eq!(readyz_head.status().as_u16(), 503);
    assert!(
        readyz_head
            .bytes()
            .expect("readyz HEAD response body should be readable")
            .is_empty()
    );

    for path in ["/livez", "/startupz"] {
        let response = client
            .get(format!("http://{}{}", proxy.listen_addr, path))
            .send()
            .expect("liveness/startup probe should be sent");
        assert_eq!(response.status().as_u16(), 200);
        let body: serde_json::Value = response.json().expect("probe response should be json");
        assert_eq!(body["status"], "ok");
        assert_eq!(body["ready"], true);
        assert_eq!(body["draining"], true);
    }
}

#[test]
fn gateway_readyz_fails_during_local_overload_while_livez_and_startupz_stay_up() {
    let _limit_guard =
        crate::TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_ACTIVE_REQUEST_LIMIT", "4");
    let root = temp_root("gateway-health-overload");
    let paths = app_paths_for_root(root);
    let server = TinyServer::http("127.0.0.1:0").expect("held upstream should bind");
    let upstream_addr = server
        .server_addr()
        .to_ip()
        .expect("held upstream should expose TCP addr");
    let (seen_tx, seen_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let upstream = thread::spawn(move || {
        let mut requests = Vec::new();
        for _ in 0..4 {
            let mut request = server.recv().expect("held upstream should receive request");
            let mut body = Vec::new();
            request
                .as_reader()
                .read_to_end(&mut body)
                .expect("held upstream should read request body");
            seen_tx
                .send(())
                .expect("held upstream should signal receipt");
            requests.push(request);
        }
        release_rx.recv().expect("held upstream should be released");
        for request in requests {
            let mut response = TinyResponse::from_string(
                r#"{"id":"resp_test","usage":{"input_tokens":7,"output_tokens":11,"total_tokens":18}}"#,
            )
            .with_status_code(200);
            response
                .add_header(TinyHeader::from_bytes("content-type", "application/json").unwrap());
            let _ = request.respond(response);
        }
    });
    let proxy = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths: &paths,
        state: &AppState::default(),
        upstream_base_url: format!("http://{upstream_addr}/v1"),
        provider: RuntimeLocalRewriteProviderOptions::OpenAiResponses {
            api_keys: vec!["upstream-key".to_string()],
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
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("gateway proxy should start");

    let first_addr = proxy.listen_addr;
    let held_requests = (0..4)
        .map(|index| {
            thread::spawn(move || {
                reqwest::blocking::Client::new()
                    .post(format!("http://{first_addr}/v1/responses"))
                    .json(
                        &serde_json::json!({"model": "gpt-5.4", "input": format!("hold-{index}")}),
                    )
                    .send()
                    .expect("held request should be sent")
                    .status()
                    .as_u16()
            })
        })
        .collect::<Vec<_>>();
    for _ in 0..4 {
        seen_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("held upstream should receive held request");
    }

    let client = reqwest::blocking::Client::new();
    let overloaded = client
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .json(&serde_json::json!({"model": "gpt-5.4", "input": "overload"}))
        .send()
        .expect("overload request should be sent");
    assert_eq!(overloaded.status().as_u16(), 503);

    let readyz = client
        .get(format!("http://{}/readyz", proxy.listen_addr))
        .send()
        .expect("readyz should be sent");
    assert_eq!(readyz.status().as_u16(), 503);
    let readyz: serde_json::Value = readyz.json().expect("readyz response should be json");
    assert_eq!(readyz["status"], "overloaded");
    assert_eq!(readyz["ready"], false);
    assert_eq!(readyz["local_overload"], true);
    assert_eq!(readyz["draining"], false);

    let readyz_head = client
        .head(format!("http://{}/readyz", proxy.listen_addr))
        .send()
        .expect("readyz HEAD should be sent");
    assert_eq!(readyz_head.status().as_u16(), 503);
    assert!(
        readyz_head
            .bytes()
            .expect("readyz HEAD response body should be readable")
            .is_empty()
    );

    for path in ["/livez", "/startupz"] {
        let response = client
            .get(format!("http://{}{}", proxy.listen_addr, path))
            .send()
            .expect("liveness/startup probe should be sent");
        assert_eq!(response.status().as_u16(), 200);
        let body: serde_json::Value = response.json().expect("probe response should be json");
        assert_eq!(body["ready"], true);
        assert_eq!(body["draining"], false);
    }

    release_tx.send(()).expect("held upstream should release");
    for request in held_requests {
        assert_eq!(request.join().expect("held request should finish"), 200);
    }
    upstream.join().expect("held upstream should finish");
}
