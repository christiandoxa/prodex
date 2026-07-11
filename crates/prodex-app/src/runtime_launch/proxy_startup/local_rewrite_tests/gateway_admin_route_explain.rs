use super::{
    AppState, RuntimeGatewayAdminRole, RuntimeGatewayAdminToken,
    RuntimeGatewayGuardrailWebhookConfig, RuntimeGatewayObservabilityConfig,
    RuntimeGatewaySsoConfig, RuntimeGatewayStateStore, RuntimeLocalRewriteProviderOptions,
    RuntimeLocalRewriteProxyStartOptions, TestUpstream, app_paths_for_root,
    runtime_gateway_test_admin_token, runtime_gateway_test_secret,
    start_runtime_local_rewrite_proxy, temp_root,
};
use crate::TestEnvVarGuard;
use crate::runtime_launch::proxy_startup::local_rewrite_gateway_admin_route_explain::RUNTIME_GATEWAY_ROUTE_EXPLAIN_MAX_BODY_BYTES;
use std::collections::BTreeMap;
use std::fs;
use std::sync::atomic::Ordering;

fn route_explain_admin_token(
    name: &str,
    token: &str,
    role: RuntimeGatewayAdminRole,
    tenant_id: Option<String>,
) -> RuntimeGatewayAdminToken {
    RuntimeGatewayAdminToken {
        name: name.to_string(),
        token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(token),
        role,
        tenant_id,
        team_id: None,
        project_id: None,
        user_id: None,
        budget_id: None,
        allowed_key_prefixes: Vec::new(),
    }
}

fn route_explain_alias() -> runtime_proxy_crate::RuntimeGatewayRouteAlias {
    runtime_proxy_crate::RuntimeGatewayRouteAlias {
        alias: "prodex-fast".to_string(),
        models: vec!["gpt-5.4".to_string(), "gpt-5.4-mini".to_string()],
        strategy: runtime_proxy_crate::RuntimeGatewayRouteStrategy::Fallback,
        model_metrics: BTreeMap::new(),
    }
}

fn start_route_explain_gateway(
    paths: &crate::AppPaths,
    upstream: &TestUpstream,
    admin_tokens: Vec<RuntimeGatewayAdminToken>,
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
        gateway_admin_tokens: admin_tokens,
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(paths),
        gateway_virtual_keys: vec![runtime_proxy_crate::RuntimeGatewayVirtualKey {
            name: "data-key".to_string(),
            tenant_id: None,
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token("data-token"),
            allowed_models: Vec::new(),
            budget_microusd: None,
            request_budget: None,
            rpm_limit: None,
            tpm_limit: None,
        }],
        gateway_route_aliases: vec![route_explain_alias()],
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig {
            url: Some(format!("http://{}/guardrail", upstream.addr)),
            phases: vec!["request".to_string()],
            bearer_token: Some(runtime_gateway_test_secret("route-explain-webhook-secret")),
            fail_closed: true,
        },
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("route explain gateway should start")
}

fn explain_url(proxy: &crate::RuntimeRotationProxy) -> String {
    format!(
        "http://{}/v1/prodex/gateway/routes/explain",
        proxy.listen_addr
    )
}

fn explain_request(
    client: &reqwest::blocking::Client,
    proxy: &crate::RuntimeRotationProxy,
    token: &str,
    body: serde_json::Value,
) -> reqwest::blocking::Response {
    client
        .post(explain_url(proxy))
        .bearer_auth(token)
        .json(&body)
        .send()
        .expect("route explain request should be sent")
}

fn assert_explain_response_matches_openapi_shape(
    client: &reqwest::blocking::Client,
    proxy: &crate::RuntimeRotationProxy,
    token: &str,
    response: &serde_json::Value,
) {
    let spec: serde_json::Value = client
        .get(format!(
            "http://{}/v1/prodex/gateway/openapi.json",
            proxy.listen_addr
        ))
        .bearer_auth(token)
        .send()
        .unwrap()
        .json()
        .unwrap();
    let schema = &spec["components"]["schemas"]["GatewayRouteExplainResponse"];
    let properties = schema["properties"].as_object().unwrap();
    for required in schema["required"].as_array().unwrap() {
        assert!(
            response.get(required.as_str().unwrap()).is_some(),
            "missing required response field {required}"
        );
    }
    for field in response.as_object().unwrap().keys() {
        assert!(
            properties.contains_key(field),
            "undocumented response field {field}"
        );
    }
    assert_eq!(schema["additionalProperties"], false);
}

fn strict_policy() -> serde_json::Value {
    serde_json::json!({
        "enabled": true,
        "unknown_context": "reject",
        "safe_window_tokens": 128000,
        "oversized_output": "reject"
    })
}

fn lane_snapshot(proxy: &crate::RuntimeRotationProxy) -> Vec<(usize, u64, u64)> {
    [
        runtime_proxy_crate::RuntimeRouteKind::Responses,
        runtime_proxy_crate::RuntimeRouteKind::Compact,
        runtime_proxy_crate::RuntimeRouteKind::Websocket,
        runtime_proxy_crate::RuntimeRouteKind::Standard,
    ]
    .into_iter()
    .map(|lane| {
        (
            proxy
                .lane_admission
                .active_counter(lane)
                .load(Ordering::Relaxed),
            proxy
                .lane_admission
                .admissions_total_counter(lane)
                .load(Ordering::Relaxed),
            proxy
                .lane_admission
                .releases_total_counter(lane)
                .load(Ordering::Relaxed),
        )
    })
    .collect()
}

#[test]
fn gateway_route_explain_requires_admin_auth_and_allows_scoped_viewer_and_admin() {
    let root = temp_root("gateway-route-explain-auth");
    let audit_dir = root.join("audit");
    let _runtime_logs = TestEnvVarGuard::set(
        "PRODEX_RUNTIME_LOG_DIR",
        root.join("runtime-logs").to_str().unwrap(),
    );
    let _audit_logs = TestEnvVarGuard::set("PRODEX_AUDIT_LOG_DIR", audit_dir.to_str().unwrap());
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(0);
    let tenant_id = prodex_domain::TenantId::new().to_string();
    let proxy = start_route_explain_gateway(
        &paths,
        &upstream,
        vec![
            route_explain_admin_token(
                "viewer",
                "viewer-token",
                RuntimeGatewayAdminRole::Viewer,
                Some(tenant_id.clone()),
            ),
            route_explain_admin_token("admin", "admin-token", RuntimeGatewayAdminRole::Admin, None),
        ],
    );
    let client = reqwest::blocking::Client::new();
    let body = serde_json::json!({
        "endpoint": "responses",
        "requested_model": "prodex-fast",
        "request": {},
        "policy": strict_policy()
    });

    assert_eq!(
        client
            .post(explain_url(&proxy))
            .json(&body)
            .send()
            .unwrap()
            .status()
            .as_u16(),
        401
    );
    assert_eq!(
        explain_request(&client, &proxy, "data-token", body.clone())
            .status()
            .as_u16(),
        401
    );
    for token in ["viewer-token", "admin-token"] {
        let response = explain_request(&client, &proxy, token, body.clone());
        assert_eq!(response.status().as_u16(), 200);
        let response: serde_json::Value = response.json().unwrap();
        assert_eq!(response["object"], "gateway.route_explanation");
        assert_eq!(response["requested_model"], "prodex-fast");
        assert_eq!(response["trace"]["commit_state"], "pre_commit");
        assert_explain_response_matches_openapi_shape(&client, &proxy, token, &response);
    }
    let mut current_state_body = body.clone();
    current_state_body["include_current_state"] = serde_json::Value::Bool(true);
    let scoped = explain_request(&client, &proxy, "viewer-token", current_state_body.clone());
    assert_eq!(scoped.status().as_u16(), 403);
    assert_eq!(
        scoped.json::<serde_json::Value>().unwrap()["error"]["code"],
        "gateway_route_state_scope_forbidden"
    );
    let unscoped = explain_request(&client, &proxy, "admin-token", current_state_body);
    assert_eq!(unscoped.status().as_u16(), 200);
    assert_eq!(
        unscoped.json::<serde_json::Value>().unwrap()["current_load_included"],
        true
    );
    let audit = fs::read_to_string(audit_dir.join("prodex-audit.log")).unwrap();
    assert!(audit.contains(&format!(r#""tenant_id":"{tenant_id}""#)));
    assert!(audit.contains(r#""control_plane_action":"control_plane.route.explain""#));
}

#[test]
fn gateway_route_explain_is_bounded_validated_and_uses_json_errors() {
    let root = temp_root("gateway-route-explain-errors");
    let _runtime_logs = TestEnvVarGuard::set(
        "PRODEX_RUNTIME_LOG_DIR",
        root.join("runtime-logs").to_str().unwrap(),
    );
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(0);
    let proxy = start_route_explain_gateway(
        &paths,
        &upstream,
        vec![runtime_gateway_test_admin_token("admin-token")],
    );
    let client = reqwest::blocking::Client::new();

    let malformed = client
        .post(explain_url(&proxy))
        .bearer_auth("admin-token")
        .header("content-type", "application/json")
        .body("{")
        .send()
        .unwrap();
    assert_eq!(malformed.status().as_u16(), 400);
    assert_eq!(
        malformed.json::<serde_json::Value>().unwrap()["error"]["code"],
        "invalid_json"
    );

    let wrong_method = client
        .get(explain_url(&proxy))
        .bearer_auth("admin-token")
        .send()
        .unwrap();
    assert_eq!(wrong_method.status().as_u16(), 405);
    assert_eq!(
        wrong_method.json::<serde_json::Value>().unwrap()["error"]["code"],
        "control_plane_method_not_allowed"
    );

    let invalid = explain_request(
        &client,
        &proxy,
        "admin-token",
        serde_json::json!({"endpoint": "responses", "requested_model": ""}),
    );
    assert_eq!(invalid.status().as_u16(), 422);
    assert_eq!(
        invalid.json::<serde_json::Value>().unwrap()["error"]["code"],
        "invalid_requested_model"
    );

    let invalid_owner = explain_request(
        &client,
        &proxy,
        "admin-token",
        serde_json::json!({
            "endpoint": "responses",
            "requested_model": "gpt-5.4",
            "owner_model": "bad owner"
        }),
    );
    assert_eq!(invalid_owner.status().as_u16(), 422);
    assert_eq!(
        invalid_owner.json::<serde_json::Value>().unwrap()["error"]["code"],
        "invalid_owner_model"
    );

    let oversized = client
        .post(explain_url(&proxy))
        .bearer_auth("admin-token")
        .header("content-type", "application/json")
        .body(vec![b'x'; RUNTIME_GATEWAY_ROUTE_EXPLAIN_MAX_BODY_BYTES + 1])
        .send()
        .unwrap();
    assert_eq!(oversized.status().as_u16(), 413);
    assert_eq!(
        oversized.json::<serde_json::Value>().unwrap()["error"]["code"],
        "request_body_too_large"
    );
}

#[test]
fn gateway_route_explain_reports_no_route_and_unknown_output_limit() {
    let root = temp_root("gateway-route-explain-decisions");
    let _runtime_logs = TestEnvVarGuard::set(
        "PRODEX_RUNTIME_LOG_DIR",
        root.join("runtime-logs").to_str().unwrap(),
    );
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(0);
    let proxy = start_route_explain_gateway(
        &paths,
        &upstream,
        vec![runtime_gateway_test_admin_token("admin-token")],
    );
    let client = reqwest::blocking::Client::new();

    let missing_owner = explain_request(
        &client,
        &proxy,
        "admin-token",
        serde_json::json!({
            "endpoint": "responses",
            "requested_model": "prodex-fast",
            "request": {},
            "hard_affinity_required": true,
            "policy": strict_policy()
        }),
    );
    assert_eq!(missing_owner.status().as_u16(), 200);
    let missing_owner: serde_json::Value = missing_owner.json().unwrap();
    assert!(missing_owner["selected_candidate"].is_null());
    assert_eq!(
        missing_owner["final_no_route_reason"],
        "affinity_owner_unavailable"
    );
    assert_eq!(missing_owner["hard_affinity_applied"], true);
    assert_eq!(missing_owner["trace"]["affinity"]["outcome"], "exhausted");

    let unknown_owner = explain_request(
        &client,
        &proxy,
        "admin-token",
        serde_json::json!({
            "endpoint": "responses",
            "requested_model": "prodex-fast",
            "request": {},
            "owner_model": "gpt-unknown-owner",
            "policy": strict_policy()
        }),
    );
    assert_eq!(unknown_owner.status().as_u16(), 200);
    let unknown_owner: serde_json::Value = unknown_owner.json().unwrap();
    assert!(unknown_owner["selected_candidate"].is_null());
    assert_eq!(unknown_owner["hard_affinity_required"], false);
    assert_eq!(unknown_owner["hard_affinity_applied"], true);
    assert_eq!(
        unknown_owner["trace"]["terminal_outcome"],
        "affinity_exhausted"
    );

    let known_owner = explain_request(
        &client,
        &proxy,
        "admin-token",
        serde_json::json!({
            "endpoint": "responses",
            "requested_model": "prodex-fast",
            "request": {},
            "owner_model": "gpt-5.4",
            "policy": strict_policy()
        }),
    );
    assert_eq!(known_owner.status().as_u16(), 200);
    let known_owner: serde_json::Value = known_owner.json().unwrap();
    assert!(known_owner["selected_candidate"].is_string());
    assert_eq!(known_owner["hard_affinity_required"], false);
    assert_eq!(known_owner["hard_affinity_applied"], true);
    assert_eq!(known_owner["trace"]["affinity"]["kind"], "strict");

    let no_route = explain_request(
        &client,
        &proxy,
        "admin-token",
        serde_json::json!({
            "endpoint": "responses",
            "requested_model": "unknown-model",
            "request": {},
            "policy": strict_policy()
        }),
    );
    assert_eq!(no_route.status().as_u16(), 200);
    let no_route: serde_json::Value = no_route.json().unwrap();
    assert!(no_route["selected_candidate"].is_null());
    assert_eq!(
        no_route["final_no_route_reason"],
        "catalog_entry_unavailable"
    );
    assert_eq!(no_route["trace"]["terminal_outcome"], "no_candidate");

    let output_unknown = explain_request(
        &client,
        &proxy,
        "admin-token",
        serde_json::json!({
            "endpoint": "responses",
            "requested_model": "gpt-5.4",
            "request": {"max_output_tokens": 1000},
            "policy": strict_policy()
        }),
    );
    assert_eq!(output_unknown.status().as_u16(), 200);
    let output_unknown: serde_json::Value = output_unknown.json().unwrap();
    assert!(
        output_unknown["warnings"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("output_limit_unknown"))
    );
}

#[test]
fn gateway_route_explain_does_not_mutate_counters_state_or_emit_sensitive_data() {
    let root = temp_root("gateway-route-explain-side-effects");
    let runtime_dir = root.join("runtime-logs");
    let audit_dir = root.join("audit");
    let _runtime_logs =
        TestEnvVarGuard::set("PRODEX_RUNTIME_LOG_DIR", runtime_dir.to_str().unwrap());
    let _audit_logs = TestEnvVarGuard::set("PRODEX_AUDIT_LOG_DIR", audit_dir.to_str().unwrap());
    let paths = app_paths_for_root(root.clone());
    let upstream = TestUpstream::start_n(0);
    let proxy = start_route_explain_gateway(
        &paths,
        &upstream,
        vec![runtime_gateway_test_admin_token("admin-token")],
    );
    let client = reqwest::blocking::Client::new();
    proxy
        .gateway_route_load
        .as_ref()
        .unwrap()
        .lock()
        .unwrap()
        .insert(
            "gpt-5.4".to_string(),
            runtime_proxy_crate::RuntimeGatewayRouteModelState {
                in_flight: 3,
                ..runtime_proxy_crate::RuntimeGatewayRouteModelState::default()
            },
        );
    let active_before = proxy.active_request_count.load(Ordering::Relaxed);
    let sequence_before = proxy.request_sequence.load(Ordering::Relaxed);
    let lanes_before = lane_snapshot(&proxy);
    let load_before = proxy
        .gateway_route_load
        .as_ref()
        .unwrap()
        .lock()
        .unwrap()
        .clone();
    let usage_before = proxy
        .gateway_usage
        .as_ref()
        .unwrap()
        .lock()
        .unwrap()
        .clone();
    let internal_before = proxy
        .gateway_side_effect_snapshot
        .as_ref()
        .map(|snapshot| snapshot())
        .unwrap();
    let state_before = fs::read(&paths.state_file).ok();
    let usage_file = root.join("gateway-virtual-key-usage.json");
    let ledger_file = root.join("gateway-billing-ledger.jsonl");
    let usage_file_before = fs::read(&usage_file).ok();
    let ledger_file_before = fs::read(&ledger_file).ok();
    let sensitive = "sensitive-prompt-token-123";
    let sensitive_tool_argument = "sensitive-tool-argument-456";
    let prompt_cache_key = "route-explain-no-side-effect-cache-key";
    assert!(crate::runtime_prompt_cache_bound_profile(Some(prompt_cache_key)).is_none());

    let response = explain_request(
        &client,
        &proxy,
        "admin-token",
        serde_json::json!({
            "endpoint": "responses",
            "requested_model": "prodex-fast",
            "request": {
                "input": [
                    {"role": "user", "content": [{"type": "input_text", "text": sensitive}]},
                    {"type": "function_call", "call_id": "call-test", "name": "lookup", "arguments": sensitive_tool_argument}
                ],
                "tools": [{"type": "function", "name": "lookup", "parameters": {}}],
                "prompt_cache_key": prompt_cache_key,
                "session_id": "route-explain-no-side-effect-session"
            },
            "include_current_state": true,
            "diagnostic_seed": 7,
            "policy": strict_policy()
        }),
    );
    assert_eq!(response.status().as_u16(), 200);
    let response_text = response.text().unwrap();
    assert!(!response_text.contains(sensitive));
    assert!(!response_text.contains(sensitive_tool_argument));
    let response: serde_json::Value = serde_json::from_str(&response_text).unwrap();
    assert_eq!(response["current_load_included"], true);
    assert_eq!(response["health_quota_included"], false);
    assert_eq!(response["diagnostic_seed"], 7);
    assert_eq!(response["hard_affinity_required"], false);
    assert_eq!(response["hard_affinity_applied"], false);
    assert!(response["owner_model"].is_null());
    assert_eq!(response["omitted_candidates"], 0);
    assert_eq!(response["trace"]["commit_state"], "pre_commit");
    assert_eq!(
        response["candidate_matrix"]
            .as_array()
            .unwrap()
            .iter()
            .find(|candidate| candidate["model"] == "gpt-5.4")
            .unwrap()["inflight_count"],
        3
    );

    assert_eq!(
        proxy.active_request_count.load(Ordering::Relaxed),
        active_before
    );
    assert_eq!(
        proxy.request_sequence.load(Ordering::Relaxed),
        sequence_before
    );
    assert_eq!(lane_snapshot(&proxy), lanes_before);
    assert_eq!(
        *proxy.gateway_route_load.as_ref().unwrap().lock().unwrap(),
        load_before
    );
    assert_eq!(
        *proxy.gateway_usage.as_ref().unwrap().lock().unwrap(),
        usage_before
    );
    assert_eq!(
        proxy
            .gateway_side_effect_snapshot
            .as_ref()
            .map(|snapshot| snapshot())
            .unwrap(),
        internal_before
    );
    assert!(crate::runtime_prompt_cache_bound_profile(Some(prompt_cache_key)).is_none());
    assert_eq!(fs::read(&paths.state_file).ok(), state_before);
    assert_eq!(fs::read(&usage_file).ok(), usage_file_before);
    assert_eq!(fs::read(&ledger_file).ok(), ledger_file_before);

    let audit = fs::read_to_string(audit_dir.join("prodex-audit.log")).unwrap();
    assert!(audit.contains(r#""action":"route_explain""#));
    assert!(audit.contains(r#""control_plane_action":"control_plane.route.explain""#));
    assert!(audit.contains(r#""selected_route_id":"candidate-0001""#));
    assert!(audit.contains(r#""diagnostic_seed":7"#));
    assert!(audit.contains(r#""current_load_included":true"#));
    assert!(audit.contains(r#""health_quota_included":false"#));
    assert!(audit.contains(r#""hard_affinity_required":false"#));
    assert!(audit.contains(r#""hard_affinity_applied":false"#));
    assert!(!audit.contains(sensitive));
    assert!(!audit.contains(sensitive_tool_argument));
    assert!(!audit.contains("admin-token"));
    assert!(!audit.contains("route-explain-webhook-secret"));
    let runtime_log = fs::read_to_string(&proxy.log_path).unwrap_or_default();
    assert!(!runtime_log.contains(sensitive));
    assert!(!runtime_log.contains(sensitive_tool_argument));
    assert!(!runtime_log.contains("admin-token"));
    assert!(!runtime_log.contains("route-explain-webhook-secret"));
}
