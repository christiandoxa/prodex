use super::support::*;
use crate::AppState;
use crate::runtime_launch::proxy_startup::local_rewrite::{
    RuntimeGatewayGuardrailWebhookConfig, RuntimeGatewayObservabilityConfig,
    RuntimeGatewaySsoConfig, RuntimeGatewayStateStore, RuntimeGatewayVirtualKeyUsageDelta,
    RuntimeLocalRewriteProviderOptions, RuntimeLocalRewriteProxyStartOptions,
    runtime_gateway_virtual_key_usage_apply_deltas, start_runtime_local_rewrite_proxy,
};
use crate::runtime_launch::proxy_startup::local_rewrite_gateway_backend_connection::{
    runtime_gateway_postgres_migrate_compatibility_state,
    runtime_gateway_postgres_migrate_enterprise_state,
    runtime_gateway_sqlite_create_current_schema_for_tests,
};
use postgres::NoTls;
use prodex_provider_core::{calculate_cost_microusd, estimate_request_input_tokens};
use std::fs;
use std::sync::{Arc, Barrier};
use std::time::Duration;

pub(super) fn runtime_gateway_postgres_create_current_schema_for_tests(url: &str) {
    let tls = prodex_storage_postgres_runtime::PostgresTlsConfig::explicit_disable();
    runtime_gateway_postgres_migrate_enterprise_state(url, &tls)
        .expect("postgres enterprise migrations should apply");
    runtime_gateway_postgres_migrate_compatibility_state(url, &tls)
        .expect("postgres compatibility migrations should apply");
}

#[test]
fn gateway_invalid_virtual_key_store_fails_closed_on_startup() {
    let root = temp_root("gateway-invalid-key-store");
    let paths = app_paths_for_root(root.clone());
    fs::write(root.join("gateway-virtual-keys.json"), "{")
        .expect("invalid gateway key store should be written");

    let result = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths: &paths,
        state: &AppState::default(),
        upstream_base_url: "http://127.0.0.1:1/v1".to_string(),
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
    });
    let err = match result {
        Ok(proxy) => {
            drop(proxy);
            panic!("invalid gateway key store should fail closed at startup");
        }
        Err(err) => err,
    };
    let message = format!("{err:#}");
    assert!(
        message.contains("failed to load gateway virtual key store"),
        "unexpected error: {message}"
    );
}

#[test]
fn gateway_invalid_stored_virtual_key_hash_fails_closed_on_startup() {
    let root = temp_root("gateway-invalid-key-hash");
    let paths = app_paths_for_root(root.clone());
    fs::write(
        root.join("gateway-virtual-keys.json"),
        serde_json::json!({
            "version": 1,
            "keys": [{
                "name": "team-a",
                "token_hash_base64": "not-base64",
                "created_at_epoch": 1,
                "updated_at_epoch": 1
            }]
        })
        .to_string(),
    )
    .expect("invalid gateway key hash store should be written");

    let result = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths: &paths,
        state: &AppState::default(),
        upstream_base_url: "http://127.0.0.1:1/v1".to_string(),
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
    });
    let err = match result {
        Ok(proxy) => {
            drop(proxy);
            panic!("invalid stored gateway key hash should fail closed at startup");
        }
        Err(err) => err,
    };
    let message = format!("{err:#}");
    assert!(
        message.contains("invalid token hash"),
        "unexpected error: {message}"
    );
}

#[test]
fn gateway_invalid_usage_store_fails_closed_on_startup() {
    let root = temp_root("gateway-invalid-usage-store");
    let paths = app_paths_for_root(root.clone());
    fs::write(root.join("gateway-virtual-key-usage.json"), "{")
        .expect("invalid gateway usage store should be written");

    let result = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths: &paths,
        state: &AppState::default(),
        upstream_base_url: "http://127.0.0.1:1/v1".to_string(),
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
        gateway_virtual_keys: vec![runtime_proxy_crate::RuntimeGatewayVirtualKey {
            name: "team-a".to_string(),
            tenant_id: None,
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token("team-token"),
            allowed_models: Vec::new(),
            budget_microusd: Some(1),
            request_budget: Some(1),
            rpm_limit: Some(1),
            tpm_limit: Some(1),
        }],
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    });
    let err = match result {
        Ok(proxy) => {
            drop(proxy);
            panic!("invalid gateway usage store should fail closed at startup");
        }
        Err(err) => err,
    };
    let message = format!("{err:#}");
    assert!(
        message.contains("failed to load gateway virtual key usage"),
        "unexpected error: {message}"
    );
}

#[test]
fn gateway_usage_delta_store_merges_batches_without_losing_counts() {
    let root = temp_root("gateway-usage-delta-merge");
    let path = root.join("gateway-virtual-key-usage.json");
    let ledger_path = root.join("gateway-billing-ledger.jsonl");
    let state_store = RuntimeGatewayStateStore::File {
        key_store_path: root.join("gateway-virtual-keys.json"),
        usage_path: path.clone(),
        ledger_path: ledger_path.clone(),
    };
    let call_id_1 = format!("prodex-{}", prodex_domain::CallId::new());
    let call_id_2 = format!("prodex-{}", prodex_domain::CallId::new());

    runtime_gateway_virtual_key_usage_apply_deltas(
        &state_store,
        &[RuntimeGatewayVirtualKeyUsageDelta {
            request_id: 1,
            typed_request_id: format!("prodex-{}", prodex_domain::RequestId::new()),
            call_id: call_id_1.clone(),
            key_name: "team-a".to_string(),
            tenant_id: None,
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            model: "gpt-5.4".to_string(),
            minute_epoch: 100,
            input_tokens: 7,
            reserved_tokens: 7,
            estimated_cost_microusd: Some(11),
            created_at_epoch: 1_700_000_000,
        }],
    )
    .expect("first delta batch should save");
    runtime_gateway_virtual_key_usage_apply_deltas(
        &state_store,
        &[RuntimeGatewayVirtualKeyUsageDelta {
            request_id: 2,
            typed_request_id: format!("prodex-{}", prodex_domain::RequestId::new()),
            call_id: call_id_2,
            key_name: "team-a".to_string(),
            tenant_id: None,
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            model: "gpt-5.4".to_string(),
            minute_epoch: 100,
            input_tokens: 13,
            reserved_tokens: 13,
            estimated_cost_microusd: Some(17),
            created_at_epoch: 1_700_000_001,
        }],
    )
    .expect("second delta batch should merge");
    runtime_gateway_virtual_key_usage_apply_deltas(
        &state_store,
        &[RuntimeGatewayVirtualKeyUsageDelta {
            request_id: 2,
            typed_request_id: format!("prodex-{}", prodex_domain::RequestId::new()),
            call_id: format!("prodex-{}", prodex_domain::CallId::new()),
            key_name: "team-a".to_string(),
            tenant_id: None,
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            model: "gpt-5.4".to_string(),
            minute_epoch: 100,
            input_tokens: 13,
            reserved_tokens: 13,
            estimated_cost_microusd: Some(17),
            created_at_epoch: 1_700_000_001,
        }],
    )
    .expect("duplicate delta should be idempotent");

    let usage = wait_for_json_file(&path);
    assert_eq!(usage["team-a"]["requests_total"], 2);
    assert_eq!(usage["team-a"]["requests_this_minute"], 2);
    assert_eq!(usage["team-a"]["tokens_this_minute"], 20);
    assert_eq!(usage["team-a"]["spend_microusd"], 28);
    let ledger = fs::read_to_string(&ledger_path).expect("ledger should be written");
    assert_eq!(ledger.lines().count(), 2);
    assert!(ledger.contains(&format!("\"call_id\":\"{call_id_1}\"")));
    assert!(ledger.contains("\"estimated_cost_microusd\":17"));
}

#[test]
fn gateway_sqlite_usage_deltas_are_idempotent_by_request_id() {
    let root = temp_root("gateway-sqlite-usage-idempotent");
    let db_path = root.join("gateway-state.sqlite");
    runtime_gateway_sqlite_create_current_schema_for_tests(&db_path)
        .expect("sqlite schema fixture should be created");
    let state_store = RuntimeGatewayStateStore::sqlite(db_path.clone());
    let delta = RuntimeGatewayVirtualKeyUsageDelta {
        request_id: 7,
        typed_request_id: format!("prodex-{}", prodex_domain::RequestId::new()),
        call_id: format!("prodex-{}", prodex_domain::CallId::new()),
        key_name: "team-a".to_string(),
        tenant_id: None,
        team_id: None,
        project_id: None,
        user_id: None,
        budget_id: None,
        model: "gpt-5.4".to_string(),
        minute_epoch: 100,
        input_tokens: 13,
        reserved_tokens: 13,
        estimated_cost_microusd: Some(17),
        created_at_epoch: 1_700_000_001,
    };

    runtime_gateway_virtual_key_usage_apply_deltas(&state_store, &[delta.clone(), delta])
        .expect("duplicate sqlite deltas should save idempotently");

    wait_for_sqlite_usage_total(&db_path, "team-a", 1);
    let conn = rusqlite::Connection::open(&db_path).expect("sqlite database should open");
    let rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM prodex_gateway_billing_ledger WHERE request_id = 7",
            [],
            |row| row.get(0),
        )
        .expect("ledger count should load");
    assert_eq!(rows, 1);
}

#[test]
fn gateway_sqlite_state_store_persists_admin_keys_and_usage() {
    let root = temp_root("gateway-sqlite-state");
    let paths = app_paths_for_root(root.clone());
    let db_path = root.join("gateway-state.sqlite");
    runtime_gateway_sqlite_create_current_schema_for_tests(&db_path)
        .expect("sqlite schema fixture should be created");
    let state_store = RuntimeGatewayStateStore::sqlite(db_path.clone());
    let upstream = TestUpstream::start();
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
        gateway_state_store: state_store.clone(),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: vec![runtime_proxy_crate::RuntimeGatewayRouteAlias {
            alias: "prodex-costly".to_string(),
            models: vec!["gpt-costly".to_string()],
            strategy: runtime_proxy_crate::RuntimeGatewayRouteStrategy::Fallback,
            model_metrics: std::collections::BTreeMap::from([(
                "gpt-costly".to_string(),
                runtime_proxy_crate::RuntimeGatewayRouteModelMetrics {
                    input_cost_per_million_microusd: Some(1_000_000),
                    output_cost_per_million_microusd: Some(2_000_000),
                    latency_ms: None,
                    rpm_limit: None,
                    tpm_limit: None,
                },
            )]),
        }],
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("sqlite gateway proxy should start");
    let client = reqwest::blocking::Client::new();
    let created = client
        .idempotent_post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({
            "name": "team-sqlite",
            "tenant_id": prodex_domain::TenantId::new().to_string()
        }))
        .send()
        .expect("sqlite create key request should be sent");
    assert_eq!(created.status().as_u16(), 201);
    let created: serde_json::Value = created.json().expect("create response should be json");
    let token = created["token"]
        .as_str()
        .expect("generated sqlite token should be returned")
        .to_string();
    let virtual_key_id = created["key"]["virtual_key_id"]
        .as_str()
        .expect("sqlite key should include virtual key id")
        .to_string();

    let request_body = serde_json::json!({
        "model": "prodex-costly",
        "input": "sqlite",
        "max_output_tokens": 20
    });
    let response = client
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth(&token)
        .json(&request_body)
        .send()
        .expect("sqlite virtual key request should be sent");
    assert_eq!(response.status().as_u16(), 200);
    let _ = upstream
        .body_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("upstream should receive sqlite request");
    wait_for_sqlite_usage_total(&db_path, "team-sqlite", 1);
    wait_for_sqlite_ledger_key_response_status(&db_path, "team-sqlite", 200);
    let conn = rusqlite::Connection::open(&db_path).expect("sqlite database should open");
    let reservation_rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM prodex_reservations WHERE virtual_key_id = ?1",
            [virtual_key_id.clone()],
            |row| row.get(0),
        )
        .expect("reservation count should load");
    assert_eq!(reservation_rows, 1);
    let committed_rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM prodex_reservations WHERE virtual_key_id = ?1 AND committed_at_unix_ms IS NOT NULL",
            [virtual_key_id.clone()],
            |row| row.get(0),
        )
        .expect("committed reservation count should load");
    let committed_rows = if committed_rows == 0 {
        let start = std::time::Instant::now();
        loop {
            let committed_rows: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM prodex_reservations WHERE virtual_key_id = ?1 AND committed_at_unix_ms IS NOT NULL",
                    [virtual_key_id.clone()],
                    |row| row.get(0),
                )
                .expect("committed reservation count should load");
            if committed_rows > 0 || start.elapsed() >= Duration::from_secs(2) {
                break committed_rows;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    } else {
        committed_rows
    };
    assert_eq!(committed_rows, 1);
    let (reserved_tokens, committed_tokens, reserved_cost_micros, committed_cost_micros): (
        i64,
        i64,
        i64,
        i64,
    ) = conn
        .query_row(
            "SELECT reserved_tokens, committed_tokens, reserved_cost_micros, committed_cost_micros FROM prodex_budget_counters WHERE virtual_key_id = ?1",
            [virtual_key_id.clone()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("budget counters should load");
    let request_body_bytes = serde_json::to_vec(&request_body).expect("request body should encode");
    let estimated_input_tokens = estimate_request_input_tokens(&request_body_bytes);
    let estimated_reserved_cost_micros = calculate_cost_microusd(
        Some(estimated_input_tokens),
        Some(20),
        prodex_provider_core::ProviderModelCost {
            input_cost_per_million_microusd: Some(1_000_000),
            output_cost_per_million_microusd: Some(2_000_000),
        },
    )
    .expect("reserved cost estimate should be known");
    assert_eq!(reserved_tokens, 0);
    assert_eq!(committed_tokens, 18);
    assert_eq!(reserved_cost_micros, 0);
    assert_eq!(committed_cost_micros, 29);
    let released_rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM prodex_usage_ledger WHERE reservation_id IN (SELECT reservation_id FROM prodex_reservations WHERE virtual_key_id = ?1) AND event_kind = 'released'",
            [virtual_key_id.clone()],
            |row| row.get(0),
        )
        .expect("released ledger count should load");
    assert_eq!(released_rows, 1);
    let released_cost_micros: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(cost_micros), 0) FROM prodex_usage_ledger WHERE reservation_id IN (SELECT reservation_id FROM prodex_reservations WHERE virtual_key_id = ?1) AND event_kind = 'released'",
            [virtual_key_id.clone()],
            |row| row.get(0),
        )
        .expect("released ledger cost should load");
    assert_eq!(
        released_cost_micros,
        i64::try_from(estimated_reserved_cost_micros.saturating_sub(29))
            .expect("released cost should fit i64")
    );
    drop(proxy);

    let restarted = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
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
        gateway_state_store: state_store,
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("sqlite gateway proxy should restart");
    let keys = client
        .get(format!(
            "http://{}/v1/prodex/gateway/keys",
            restarted.listen_addr
        ))
        .bearer_auth(admin_token)
        .send()
        .expect("sqlite key list request should be sent");
    assert_eq!(keys.status().as_u16(), 200);
    let keys: serde_json::Value = keys.json().expect("key list response should be json");
    assert_eq!(keys["state_backend"], "sqlite");
    assert_eq!(keys["keys"][0]["name"], "team-sqlite");
    assert_eq!(keys["keys"][0]["usage"]["requests_total"], 1);
    let ledger = client
        .get(format!(
            "http://{}/v1/prodex/gateway/ledger",
            restarted.listen_addr
        ))
        .bearer_auth(admin_token)
        .send()
        .expect("sqlite ledger request should be sent");
    assert_eq!(ledger.status().as_u16(), 200);
    let ledger: serde_json::Value = ledger.json().expect("ledger response should be json");
    assert_eq!(ledger["state_backend"], "sqlite");
    assert_eq!(ledger["records"][0]["key_name"], "team-sqlite");
    let call_id = ledger["records"][0]["call_id"]
        .as_str()
        .expect("ledger call ID should be a string");
    let call_id_uuid = call_id
        .strip_prefix("prodex-")
        .expect("ledger call ID should keep prodex prefix")
        .parse::<prodex_domain::CallId>()
        .expect("ledger call ID should use typed UUIDv7 value");
    assert_eq!(
        call_id_uuid.as_uuid().get_version_num(),
        7,
        "ledger call ID should not use process-local request sequence: {call_id}"
    );
    assert_eq!(ledger["records"][0]["response_status"], 200);
    assert_eq!(ledger["records"][0]["output_tokens"], 11);
    assert!(db_path.exists());
    assert!(!root.join("gateway-virtual-keys.json").exists());
}

#[test]
fn gateway_sqlite_state_store_rotates_admin_keys() {
    let root = temp_root("gateway-sqlite-rotate-key");
    let paths = app_paths_for_root(root.clone());
    let db_path = root.join("gateway-state.sqlite");
    runtime_gateway_sqlite_create_current_schema_for_tests(&db_path)
        .expect("sqlite schema fixture should be created");
    let state_store = RuntimeGatewayStateStore::sqlite(db_path);
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
        gateway_state_store: state_store,
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("sqlite gateway proxy should start");
    let client = reqwest::blocking::Client::new();

    let created = client
        .idempotent_post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({
            "name": "team-sqlite-rotate",
            "tenant_id": prodex_domain::TenantId::new().to_string()
        }))
        .send()
        .expect("sqlite create key request should be sent");
    assert_eq!(created.status().as_u16(), 201);
    let created: serde_json::Value = created.json().expect("create response should be json");
    let first_token = created["token"]
        .as_str()
        .expect("generated sqlite token should be returned")
        .to_string();

    let first = client
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth(&first_token)
        .json(&serde_json::json!({"model": "gpt-5.4", "input": "before-rotate"}))
        .send()
        .expect("sqlite request before rotate should be sent");
    assert_eq!(first.status().as_u16(), 200);
    let _ = upstream
        .body_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("upstream should receive sqlite request before rotate");

    let rotated = client
        .idempotent_patch(format!(
            "http://{}/v1/prodex/gateway/keys/team-sqlite-rotate",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({"rotate": true}))
        .send()
        .expect("sqlite rotate key request should be sent");
    assert_eq!(rotated.status().as_u16(), 200);
    let rotated: serde_json::Value = rotated.json().expect("rotate response should be json");
    let rotated_token = rotated["token"]
        .as_str()
        .expect("rotated sqlite token should be returned")
        .to_string();
    assert_ne!(rotated_token, first_token);

    let old_rejected = client
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth(&first_token)
        .json(&serde_json::json!({"model": "gpt-5.4", "input": "old-token"}))
        .send()
        .expect("sqlite request with old token should be sent");
    assert_eq!(old_rejected.status().as_u16(), 401);

    let second = client
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth(&rotated_token)
        .json(&serde_json::json!({"model": "gpt-5.4", "input": "after-rotate"}))
        .send()
        .expect("sqlite request with rotated token should be sent");
    assert_eq!(second.status().as_u16(), 200);
    let _ = upstream
        .body_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("upstream should receive sqlite request after rotate");
}

#[test]
fn gateway_sqlite_shared_backend_allows_only_one_budget_limited_reservation_across_proxies() {
    let root = temp_root("gateway-sqlite-shared-backend");
    let paths = app_paths_for_root(root.clone());
    let db_path = root.join("gateway-state.sqlite");
    runtime_gateway_sqlite_create_current_schema_for_tests(&db_path)
        .expect("sqlite schema fixture should be created");
    let state_store = RuntimeGatewayStateStore::sqlite(db_path.clone());
    let upstream = TestUpstream::start_n(1);
    let admin_token = "admin-token";
    let route_aliases = vec![runtime_proxy_crate::RuntimeGatewayRouteAlias {
        alias: "prodex-costly".to_string(),
        models: vec!["gpt-costly".to_string()],
        strategy: runtime_proxy_crate::RuntimeGatewayRouteStrategy::Fallback,
        model_metrics: std::collections::BTreeMap::from([(
            "gpt-costly".to_string(),
            runtime_proxy_crate::RuntimeGatewayRouteModelMetrics {
                input_cost_per_million_microusd: Some(1_000_000),
                output_cost_per_million_microusd: Some(2_000_000),
                latency_ms: None,
                rpm_limit: None,
                tpm_limit: None,
            },
        )]),
    }];

    let proxy_a = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
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
        gateway_state_store: state_store.clone(),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: route_aliases.clone(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("first sqlite gateway proxy should start");
    let client = reqwest::blocking::Client::new();
    let created = client
        .idempotent_post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy_a.listen_addr
        ))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({
            "name": "team-shared",
            "tenant_id": prodex_domain::TenantId::new().to_string(),
            "budget_microusd": 42_u64
        }))
        .send()
        .expect("shared backend create key request should be sent");
    assert_eq!(created.status().as_u16(), 201);
    let created: serde_json::Value = created.json().expect("create response should be json");
    let token = created["token"]
        .as_str()
        .expect("generated shared token should be returned")
        .to_string();
    let virtual_key_id = created["key"]["virtual_key_id"]
        .as_str()
        .expect("shared key should include virtual key id")
        .to_string();

    let proxy_b = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
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
        gateway_state_store: state_store.clone(),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: route_aliases,
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("second sqlite gateway proxy should start");

    let barrier = Arc::new(Barrier::new(2));
    let request_body = serde_json::json!({
        "model": "prodex-costly",
        "input": "sqlite",
        "max_output_tokens": 20
    });
    let send = |listen_addr: std::net::SocketAddr,
                barrier: Arc<Barrier>,
                token: String,
                request_body: serde_json::Value| {
        std::thread::spawn(move || {
            let client = reqwest::blocking::Client::new();
            barrier.wait();
            client
                .post(format!("http://{}/v1/responses", listen_addr))
                .bearer_auth(token)
                .json(&request_body)
                .send()
                .expect("shared backend request should be sent")
                .status()
                .as_u16()
        })
    };
    let first = send(
        proxy_a.listen_addr,
        Arc::clone(&barrier),
        token.clone(),
        request_body.clone(),
    );
    let second = send(proxy_b.listen_addr, barrier, token, request_body.clone());
    let mut statuses = vec![
        first.join().expect("first request thread should finish"),
        second.join().expect("second request thread should finish"),
    ];
    statuses.sort_unstable();
    assert_eq!(statuses, vec![200, 403]);
    let denied: serde_json::Value = client
        .post(format!("http://{}/v1/responses", proxy_b.listen_addr))
        .bearer_auth(created["token"].as_str().unwrap())
        .json(&request_body)
        .send()
        .expect("follow-up denied request should be sent")
        .json()
        .expect("denied response should be json");
    assert_eq!(denied["error"]["code"], "budget_exceeded");

    let _ = upstream
        .body_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("exactly one upstream request should be admitted");
    wait_for_sqlite_usage_total(&db_path, "team-shared", 1);
    let conn = rusqlite::Connection::open(&db_path).expect("sqlite database should open");
    let reservation_rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM prodex_reservations WHERE virtual_key_id = ?1",
            [virtual_key_id.clone()],
            |row| row.get(0),
        )
        .expect("reservation count should load");
    assert_eq!(reservation_rows, 1);
    let committed_rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM prodex_reservations WHERE virtual_key_id = ?1 AND committed_at_unix_ms IS NOT NULL",
            [virtual_key_id.clone()],
            |row| row.get(0),
        )
        .expect("committed reservation count should load");
    let committed_rows = if committed_rows == 0 {
        let start = std::time::Instant::now();
        loop {
            let committed_rows: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM prodex_reservations WHERE virtual_key_id = ?1 AND committed_at_unix_ms IS NOT NULL",
                    [virtual_key_id.clone()],
                    |row| row.get(0),
                )
                .expect("committed reservation count should load");
            if committed_rows > 0 || start.elapsed() >= Duration::from_secs(2) {
                break committed_rows;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    } else {
        committed_rows
    };
    assert_eq!(committed_rows, 1);
    let (reserved_tokens, reserved_cost_micros, committed_tokens, committed_cost_micros): (
        i64,
        i64,
        i64,
        i64,
    ) = conn
        .query_row(
            "SELECT reserved_tokens, reserved_cost_micros, committed_tokens, committed_cost_micros FROM prodex_budget_counters WHERE virtual_key_id = ?1",
            [virtual_key_id.clone()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("budget counters should load");
    assert_eq!(reserved_tokens, 0);
    assert_eq!(reserved_cost_micros, 0);
    assert_eq!(committed_tokens, 18);
    assert_eq!(committed_cost_micros, 29);
    let committed_ledger_rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM prodex_usage_ledger WHERE reservation_id IN (SELECT reservation_id FROM prodex_reservations WHERE virtual_key_id = ?1) AND event_kind = 'committed'",
            [virtual_key_id.clone()],
            |row| row.get(0),
        )
        .expect("committed ledger count should load");
    let released_ledger_rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM prodex_usage_ledger WHERE reservation_id IN (SELECT reservation_id FROM prodex_reservations WHERE virtual_key_id = ?1) AND event_kind = 'released'",
            [virtual_key_id],
            |row| row.get(0),
        )
        .expect("released ledger count should load");
    assert_eq!(committed_ledger_rows, 1);
    assert_eq!(released_ledger_rows, 1);
}

#[test]
fn gateway_postgres_shared_backend_allows_only_one_budget_limited_reservation_across_proxies() {
    let Some(url) = std::env::var("PRODEX_TEST_POSTGRES_URL").ok() else {
        eprintln!("skipping: PRODEX_TEST_POSTGRES_URL is not set");
        return;
    };
    let coordination_redis_url = std::env::var("PRODEX_TEST_REDIS_URL").ok();

    let root = temp_root("gateway-postgres-shared-backend");
    let paths = app_paths_for_root(root.clone());
    runtime_gateway_postgres_create_current_schema_for_tests(&url);
    let state_store = RuntimeGatewayStateStore::postgres_with_coordination(
        "PRODEX_TEST_POSTGRES_URL".to_string(),
        url.clone(),
        coordination_redis_url.clone(),
    );
    let upstream = TestUpstream::start_n(1);
    let admin_token = "admin-token";
    let route_aliases = vec![runtime_proxy_crate::RuntimeGatewayRouteAlias {
        alias: "prodex-costly".to_string(),
        models: vec!["gpt-costly".to_string()],
        strategy: runtime_proxy_crate::RuntimeGatewayRouteStrategy::Fallback,
        model_metrics: std::collections::BTreeMap::from([(
            "gpt-costly".to_string(),
            runtime_proxy_crate::RuntimeGatewayRouteModelMetrics {
                input_cost_per_million_microusd: Some(1_000_000),
                output_cost_per_million_microusd: Some(2_000_000),
                latency_ms: None,
                rpm_limit: None,
                tpm_limit: None,
            },
        )]),
    }];

    let proxy_a = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
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
        gateway_state_store: state_store.clone(),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: route_aliases.clone(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("first postgres gateway proxy should start");
    let client = reqwest::blocking::Client::new();
    let key_name = format!(
        "team-shared-postgres-{}",
        prodex_domain::VirtualKeyId::new()
    );
    let mut create_payload = serde_json::json!({
        "name": key_name,
        "tenant_id": prodex_domain::TenantId::new().to_string(),
        "budget_microusd": 42_u64
    });
    if coordination_redis_url.is_some() {
        create_payload["rpm_limit"] = serde_json::json!(1_u64);
    }
    let created = client
        .idempotent_post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy_a.listen_addr
        ))
        .bearer_auth(admin_token)
        .json(&create_payload)
        .send()
        .expect("shared postgres create key request should be sent");
    let created_status = created.status().as_u16();
    let created_body = created
        .text()
        .expect("shared postgres create response should be readable");
    assert_eq!(
        created_status, 201,
        "shared postgres create key failed: {created_body}"
    );
    let created: serde_json::Value =
        serde_json::from_str(&created_body).expect("create response should be json");
    let token = created["token"]
        .as_str()
        .expect("generated shared postgres token should be returned")
        .to_string();
    let virtual_key_id = created["key"]["virtual_key_id"]
        .as_str()
        .expect("shared postgres key should include virtual key id")
        .parse::<prodex_domain::VirtualKeyId>()
        .expect("shared postgres key should use typed virtual key id");

    let proxy_b = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
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
        gateway_state_store: state_store.clone(),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: route_aliases,
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("second postgres gateway proxy should start");

    let barrier = Arc::new(Barrier::new(2));
    let request_body = serde_json::json!({
        "model": "prodex-costly",
        "input": "postgres",
        "max_output_tokens": 20
    });
    let send = |listen_addr: std::net::SocketAddr,
                barrier: Arc<Barrier>,
                token: String,
                request_body: serde_json::Value| {
        std::thread::spawn(move || {
            let client = reqwest::blocking::Client::new();
            barrier.wait();
            client
                .post(format!("http://{}/v1/responses", listen_addr))
                .bearer_auth(token)
                .json(&request_body)
                .send()
                .expect("shared postgres request should be sent")
                .status()
                .as_u16()
        })
    };
    let first = send(
        proxy_a.listen_addr,
        Arc::clone(&barrier),
        token.clone(),
        request_body.clone(),
    );
    let second = send(proxy_b.listen_addr, barrier, token, request_body.clone());
    let mut statuses = vec![
        first.join().expect("first request thread should finish"),
        second.join().expect("second request thread should finish"),
    ];
    statuses.sort_unstable();
    assert_eq!(
        statuses,
        if coordination_redis_url.is_some() {
            vec![200, 429]
        } else {
            vec![200, 403]
        }
    );
    let denied: serde_json::Value = client
        .post(format!("http://{}/v1/responses", proxy_b.listen_addr))
        .bearer_auth(created["token"].as_str().unwrap())
        .json(&request_body)
        .send()
        .expect("follow-up denied postgres request should be sent")
        .json()
        .expect("denied response should be json");
    assert_eq!(
        denied["error"]["code"],
        if coordination_redis_url.is_some() {
            "rpm_limit_exceeded"
        } else {
            "budget_exceeded"
        }
    );

    let _ = upstream
        .body_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("exactly one upstream postgres request should be admitted");

    let mut db = postgres::Client::connect(&url, NoTls).expect("postgres database should open");
    let requests_total = {
        let start = std::time::Instant::now();
        loop {
            let requests_total = db
                .query_opt(
                    "SELECT requests_total FROM prodex_gateway_virtual_key_usage WHERE key_name = $1",
                    &[&key_name],
                )
                .expect("usage row should load")
                .map(|row| row.get::<_, i64>(0))
                .unwrap_or_default();
            if requests_total > 0 || start.elapsed() >= Duration::from_secs(2) {
                break requests_total;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    };
    assert_eq!(requests_total, 1);
    let reservation_rows: i64 = db
        .query_one(
            "SELECT COUNT(*) FROM prodex_reservations WHERE virtual_key_id = $1",
            &[&virtual_key_id.as_uuid()],
        )
        .expect("reservation count should load")
        .get(0);
    assert_eq!(reservation_rows, 1);
    let committed_rows: i64 = db
        .query_one(
            "SELECT COUNT(*) FROM prodex_reservations WHERE virtual_key_id = $1 AND committed_at_unix_ms IS NOT NULL",
            &[&virtual_key_id.as_uuid()],
        )
        .expect("committed reservation count should load")
        .get(0);
    let committed_rows = if committed_rows == 0 {
        let start = std::time::Instant::now();
        loop {
            let committed_rows: i64 = db
                .query_one(
                    "SELECT COUNT(*) FROM prodex_reservations WHERE virtual_key_id = $1 AND committed_at_unix_ms IS NOT NULL",
                    &[&virtual_key_id.as_uuid()],
                )
                .expect("committed reservation count should load")
                .get(0);
            if committed_rows > 0 || start.elapsed() >= Duration::from_secs(2) {
                break committed_rows;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    } else {
        committed_rows
    };
    assert_eq!(committed_rows, 1);
    let budget_row = db
        .query_one(
            "SELECT reserved_tokens, reserved_cost_micros, committed_tokens, committed_cost_micros FROM prodex_budget_counters WHERE virtual_key_id = $1",
            &[&virtual_key_id.as_uuid()],
        )
        .expect("budget counters should load");
    let reserved_tokens: i64 = budget_row.get(0);
    let reserved_cost_micros: i64 = budget_row.get(1);
    let committed_tokens: i64 = budget_row.get(2);
    let committed_cost_micros: i64 = budget_row.get(3);
    assert_eq!(reserved_tokens, 0);
    assert_eq!(reserved_cost_micros, 0);
    assert_eq!(committed_tokens, 18);
    assert_eq!(committed_cost_micros, 29);
    let committed_ledger_rows: i64 = db
        .query_one(
            "SELECT COUNT(*) FROM prodex_usage_ledger WHERE reservation_id IN (SELECT reservation_id FROM prodex_reservations WHERE virtual_key_id = $1) AND event_kind = 'committed'",
            &[&virtual_key_id.as_uuid()],
        )
        .expect("committed ledger count should load")
        .get(0);
    let released_ledger_rows: i64 = db
        .query_one(
            "SELECT COUNT(*) FROM prodex_usage_ledger WHERE reservation_id IN (SELECT reservation_id FROM prodex_reservations WHERE virtual_key_id = $1) AND event_kind = 'released'",
            &[&virtual_key_id.as_uuid()],
        )
        .expect("released ledger count should load")
        .get(0);
    assert_eq!(committed_ledger_rows, 1);
    assert_eq!(released_ledger_rows, 1);
}

#[test]
fn gateway_postgres_state_store_rotates_admin_keys() {
    let Some(url) = std::env::var("PRODEX_TEST_POSTGRES_URL").ok() else {
        eprintln!("skipping: PRODEX_TEST_POSTGRES_URL is not set");
        return;
    };

    let root = temp_root("gateway-postgres-rotate-key");
    let paths = app_paths_for_root(root);
    runtime_gateway_postgres_create_current_schema_for_tests(&url);
    let state_store =
        RuntimeGatewayStateStore::postgres("PRODEX_TEST_POSTGRES_URL".to_string(), url);
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
        gateway_state_store: state_store,
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("postgres gateway proxy should start");
    let client = reqwest::blocking::Client::new();
    let key_name = format!(
        "team-postgres-rotate-{}",
        prodex_domain::VirtualKeyId::new()
    );

    let created = client
        .idempotent_post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({"name": key_name}))
        .send()
        .expect("postgres create key request should be sent");
    let created_status = created.status().as_u16();
    let created_body = created
        .text()
        .expect("postgres create response should be readable");
    assert_eq!(
        created_status, 201,
        "postgres create key failed: {created_body}"
    );
    let created: serde_json::Value =
        serde_json::from_str(&created_body).expect("create response should be json");
    let first_token = created["token"]
        .as_str()
        .expect("generated postgres token should be returned")
        .to_string();

    let first = client
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth(&first_token)
        .json(&serde_json::json!({"model": "gpt-5.4", "input": "before-rotate"}))
        .send()
        .expect("postgres request before rotate should be sent");
    assert_eq!(first.status().as_u16(), 200);
    let _ = upstream
        .body_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("upstream should receive postgres request before rotate");

    let rotated = client
        .idempotent_patch(format!(
            "http://{}/v1/prodex/gateway/keys/{}",
            proxy.listen_addr, key_name
        ))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({"rotate": true}))
        .send()
        .expect("postgres rotate key request should be sent");
    let rotated_status = rotated.status().as_u16();
    let rotated_body = rotated
        .text()
        .expect("postgres rotate response should be readable");
    assert_eq!(
        rotated_status, 200,
        "postgres rotate key failed: {rotated_body}"
    );
    let rotated: serde_json::Value =
        serde_json::from_str(&rotated_body).expect("rotate response should be json");
    let rotated_token = rotated["token"]
        .as_str()
        .expect("rotated postgres token should be returned")
        .to_string();
    assert_ne!(rotated_token, first_token);

    let old_rejected = client
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth(&first_token)
        .json(&serde_json::json!({"model": "gpt-5.4", "input": "old-token"}))
        .send()
        .expect("postgres request with old token should be sent");
    assert_eq!(old_rejected.status().as_u16(), 401);

    let second = client
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth(&rotated_token)
        .json(&serde_json::json!({"model": "gpt-5.4", "input": "after-rotate"}))
        .send()
        .expect("postgres request with rotated token should be sent");
    assert_eq!(second.status().as_u16(), 200);
    let _ = upstream
        .body_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("upstream should receive postgres request after rotate");
}
