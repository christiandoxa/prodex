use super::gateway_state::runtime_gateway_postgres_create_current_schema_for_tests;
use super::{
    AppState, RuntimeGatewayGuardrailWebhookConfig, RuntimeGatewayObservabilityConfig,
    RuntimeGatewaySsoConfig, RuntimeGatewayStateStore, RuntimeLocalRewriteProviderOptions,
    RuntimeLocalRewriteProxyStartOptions, TestUpstream, app_paths_for_root,
    runtime_gateway_test_admin_token, start_runtime_local_rewrite_proxy, temp_root,
};
use postgres::NoTls;
use std::sync::{Arc, Barrier};
use std::time::Duration;

fn start_proxy(
    paths: &crate::AppPaths,
    state_store: RuntimeGatewayStateStore,
    upstream: &TestUpstream,
    admin_token: &str,
) -> crate::RuntimeRotationProxy {
    start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths,
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
        gateway_state_store: state_store,
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("postgres gateway proxy should start")
}

fn create_grouped_key(
    client: &reqwest::blocking::Client,
    listen_addr: std::net::SocketAddr,
    admin_token: &str,
    name: &str,
    tenant_id: &str,
    budget_id: &str,
) -> String {
    let response = client
        .post(format!("http://{listen_addr}/v1/prodex/gateway/keys"))
        .bearer_auth(admin_token)
        .header("Idempotency-Key", format!("create-{tenant_id}-{name}"))
        .json(&serde_json::json!({
            "name": name,
            "tenant_id": tenant_id,
            "budget_id": budget_id,
            "request_budget": 1_u64,
        }))
        .send()
        .expect("grouped key create request should be sent");
    let status = response.status();
    let body = response
        .text()
        .expect("grouped key response should be readable");
    assert_eq!(status.as_u16(), 201, "grouped key create failed: {body}");
    serde_json::from_str::<serde_json::Value>(&body)
        .expect("grouped key response should be json")["token"]
        .as_str()
        .expect("grouped key token should be returned")
        .to_string()
}

#[test]
fn gateway_postgres_grouped_request_budget_is_atomic_across_two_proxies_and_keys() {
    let Some(url) = std::env::var("PRODEX_TEST_POSTGRES_URL").ok() else {
        eprintln!("skipping: PRODEX_TEST_POSTGRES_URL is not set");
        return;
    };
    let root = temp_root("gateway-postgres-grouped-request-budget");
    let paths = app_paths_for_root(root);
    runtime_gateway_postgres_create_current_schema_for_tests(&url);
    let state_store =
        RuntimeGatewayStateStore::postgres("PRODEX_TEST_POSTGRES_URL".to_string(), url.clone());
    let upstream = TestUpstream::start_n(1);
    let admin_token = "grouped-budget-admin";
    let proxy_a = start_proxy(&paths, state_store.clone(), &upstream, admin_token);
    let client = reqwest::blocking::Client::new();
    let tenant_id = prodex_domain::TenantId::new().to_string();
    let budget_id = format!("shared-{}", prodex_domain::RequestId::new());
    let key_suffix = prodex_domain::VirtualKeyId::new();
    let alpha_name = format!("grouped-alpha-{key_suffix}");
    let beta_name = format!("grouped-beta-{key_suffix}");
    let alpha = create_grouped_key(
        &client,
        proxy_a.listen_addr,
        admin_token,
        &alpha_name,
        &tenant_id,
        &budget_id,
    );
    let beta = create_grouped_key(
        &client,
        proxy_a.listen_addr,
        admin_token,
        &beta_name,
        &tenant_id,
        &budget_id,
    );
    let proxy_b = start_proxy(&paths, state_store, &upstream, admin_token);

    let barrier = Arc::new(Barrier::new(2));
    let send = |listen_addr, token: String, barrier: Arc<Barrier>| {
        std::thread::spawn(move || {
            let client = reqwest::blocking::Client::new();
            barrier.wait();
            let response = client
                .post(format!("http://{listen_addr}/v1/responses"))
                .bearer_auth(token)
                .json(&serde_json::json!({"model": "gpt-5.4", "input": "grouped"}))
                .send()
                .expect("grouped request should be sent");
            let status = response.status().as_u16();
            let body = response
                .text()
                .expect("grouped response should be readable");
            (status, body)
        })
    };
    let first = send(proxy_a.listen_addr, alpha, Arc::clone(&barrier));
    let second = send(proxy_b.listen_addr, beta, barrier);
    let mut responses = [first.join().unwrap(), second.join().unwrap()];
    responses.sort_by_key(|(status, _)| *status);
    assert_eq!(
        responses
            .iter()
            .map(|(status, _)| *status)
            .collect::<Vec<_>>(),
        vec![200, 403]
    );
    let denied: serde_json::Value = serde_json::from_str(&responses[1].1).unwrap();
    assert_eq!(denied["error"]["code"], "request_budget_exceeded");
    let _ = upstream
        .body_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("exactly one grouped request should reach upstream");

    let mut db = postgres::Client::connect(&url, NoTls).expect("postgres should connect");
    let row = db
        .query_one(
            "SELECT COALESCE(SUM(request_count), 0)::BIGINT, \
                    (SELECT COUNT(*) FROM prodex_reservations WHERE tenant_id = $1), \
                    (SELECT COUNT(*) FROM prodex_usage_ledger WHERE tenant_id = $1) \
             FROM prodex_budget_counters WHERE tenant_id = $1",
            &[&tenant_id.parse::<uuid::Uuid>().unwrap()],
        )
        .expect("grouped accounting rows should load");
    assert_eq!(row.get::<_, i64>(0), 1);
    assert_eq!(row.get::<_, i64>(1), 1);
    assert!(row.get::<_, i64>(2) >= 1);
}
