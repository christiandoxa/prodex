use super::*;
use crate::runtime_launch::proxy_startup::local_rewrite_gateway_backend_connection::runtime_gateway_sqlite_create_current_schema_for_tests;

#[derive(Clone, Copy)]
enum TestBackend {
    File,
    Sqlite,
}

#[test]
fn gateway_file_admin_mutations_reject_foreign_records_replaced_by_second_proxy() {
    assert_foreign_key_replacement_is_rejected(TestBackend::File);
}

#[test]
fn gateway_sqlite_admin_mutations_reject_foreign_records_replaced_by_second_proxy() {
    assert_foreign_key_replacement_is_rejected(TestBackend::Sqlite);
}

fn assert_foreign_key_replacement_is_rejected(backend: TestBackend) {
    let suffix = match backend {
        TestBackend::File => "file",
        TestBackend::Sqlite => "sqlite",
    };
    let root = temp_root(&format!("gateway-admin-toctou-{suffix}"));
    let paths = app_paths_for_root(root.clone());
    let state_store = match backend {
        TestBackend::File => RuntimeGatewayStateStore::file(&paths),
        TestBackend::Sqlite => {
            let path = root.join("gateway-state.sqlite");
            runtime_gateway_sqlite_create_current_schema_for_tests(&path)
                .expect("sqlite schema fixture should be created");
            RuntimeGatewayStateStore::sqlite(path)
        }
    };
    let upstream = TestUpstream::start_n(0);
    let root_token = "root-admin-token";
    let tenant_token = "tenant-admin-token";
    let tenant_a = prodex_domain::TenantId::new().to_string();
    let tenant_b = prodex_domain::TenantId::new().to_string();
    let proxy_a = start_toctou_proxy(
        &paths,
        state_store.clone(),
        upstream.addr,
        root_token,
        tenant_token,
        &tenant_a,
    );
    let client = reqwest::blocking::Client::new();
    let key_url_a = format!(
        "http://{}/v1/prodex/gateway/keys/shared-key",
        proxy_a.listen_addr
    );
    let created = client
        .idempotent_post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy_a.listen_addr
        ))
        .bearer_auth(tenant_token)
        .json(&serde_json::json!({"name": "shared-key"}))
        .send()
        .expect("tenant key create request should be sent");
    assert_eq!(created.status().as_u16(), 201);
    let created_user = client
        .idempotent_post(format!(
            "http://{}/v1/prodex/gateway/scim/v2/Users",
            proxy_a.listen_addr
        ))
        .bearer_auth(tenant_token)
        .json(&serde_json::json!({
            "userName": "shared-user@example.com",
            "displayName": "Tenant A User"
        }))
        .send()
        .expect("tenant SCIM create request should be sent");
    assert_eq!(created_user.status().as_u16(), 201);
    let created_user = created_user
        .json::<serde_json::Value>()
        .expect("created SCIM user should be json");
    let user_id = created_user["id"]
        .as_str()
        .expect("created SCIM user should have an id")
        .to_string();

    let proxy_b = start_toctou_proxy(
        &paths,
        state_store,
        upstream.addr,
        root_token,
        tenant_token,
        &tenant_a,
    );
    let key_url_b = format!(
        "http://{}/v1/prodex/gateway/keys/shared-key",
        proxy_b.listen_addr
    );
    let scim_url_a = format!(
        "http://{}/v1/prodex/gateway/scim/v2/Users/{user_id}",
        proxy_a.listen_addr
    );
    let scim_url_b = format!(
        "http://{}/v1/prodex/gateway/scim/v2/Users/{user_id}",
        proxy_b.listen_addr
    );
    let replaced = client
        .idempotent_patch(&key_url_b)
        .bearer_auth(root_token)
        .header("If-Match", "*")
        .json(&serde_json::json!({
            "tenant_id": tenant_b,
            "disabled": false
        }))
        .send()
        .expect("root key replacement request should be sent");
    assert_eq!(replaced.status().as_u16(), 200);
    let replaced_user = client
        .idempotent_patch(&scim_url_b)
        .bearer_auth(root_token)
        .json(&serde_json::json!({
            "tenant_id": tenant_b,
            "displayName": "Tenant B User"
        }))
        .send()
        .expect("root SCIM replacement request should be sent");
    assert_eq!(replaced_user.status().as_u16(), 200);

    let stale_update = client
        .idempotent_patch(&key_url_a)
        .bearer_auth(tenant_token)
        .header("If-Match", "*")
        .json(&serde_json::json!({"disabled": true}))
        .send()
        .expect("stale tenant key update request should be sent");
    assert_scope_forbidden(stale_update);

    let after_update = get_key(&client, &key_url_b, root_token);
    assert_eq!(after_update["tenant_id"], tenant_b);
    assert_eq!(after_update["disabled"], false);

    let stale_scim_update = client
        .idempotent_patch(&scim_url_a)
        .bearer_auth(tenant_token)
        .json(&serde_json::json!({"displayName": "Unauthorized Update"}))
        .send()
        .expect("stale tenant SCIM update request should be sent");
    assert_scope_forbidden(stale_scim_update);

    let after_scim_update = get_scim_user(&client, &scim_url_b, root_token);
    assert_eq!(after_scim_update["tenant_id"], tenant_b);
    assert_eq!(after_scim_update["displayName"], "Tenant B User");

    let stale_delete = client
        .idempotent_delete(&key_url_a)
        .bearer_auth(tenant_token)
        .header("If-Match", "*")
        .send()
        .expect("stale tenant key delete request should be sent");
    assert_scope_forbidden(stale_delete);

    let after_delete = get_key(&client, &key_url_b, root_token);
    assert_eq!(after_delete["tenant_id"], tenant_b);
    assert_eq!(after_delete["disabled"], false);
}

fn start_toctou_proxy(
    paths: &crate::AppPaths,
    state_store: RuntimeGatewayStateStore,
    upstream_addr: std::net::SocketAddr,
    root_token: &str,
    tenant_token: &str,
    tenant_id: &str,
) -> crate::RuntimeRotationProxy {
    start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths,
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
        gateway_admin_tokens: vec![
            runtime_gateway_test_admin_token(root_token),
            RuntimeGatewayAdminToken {
                name: "tenant-admin".to_string(),
                token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
                    tenant_token,
                ),
                role: RuntimeGatewayAdminRole::Admin,
                tenant_id: Some(tenant_id.to_string()),
                team_id: None,
                project_id: None,
                user_id: None,
                budget_id: None,
                allowed_key_prefixes: Vec::new(),
            },
        ],
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: state_store,
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("TOCTOU gateway proxy should start")
}

fn get_key(client: &reqwest::blocking::Client, url: &str, token: &str) -> serde_json::Value {
    let response = client
        .get(url)
        .bearer_auth(token)
        .send()
        .expect("root key get request should be sent");
    assert_eq!(response.status().as_u16(), 200);
    response
        .json::<serde_json::Value>()
        .expect("key response should be json")["key"]
        .clone()
}

fn get_scim_user(client: &reqwest::blocking::Client, url: &str, token: &str) -> serde_json::Value {
    let response = client
        .get(url)
        .bearer_auth(token)
        .send()
        .expect("root SCIM get request should be sent");
    assert_eq!(response.status().as_u16(), 200);
    response
        .json::<serde_json::Value>()
        .expect("SCIM response should be json")
}

fn assert_scope_forbidden(response: reqwest::blocking::Response) {
    assert_eq!(response.status().as_u16(), 403);
    let body = response
        .json::<serde_json::Value>()
        .expect("scope denial should be json");
    assert_eq!(body["error"]["code"], "gateway_admin_key_scope_forbidden");
}
