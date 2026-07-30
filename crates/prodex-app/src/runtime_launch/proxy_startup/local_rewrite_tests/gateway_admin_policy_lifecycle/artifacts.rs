use super::*;

#[test]
fn gateway_governance_artifacts_use_generic_maker_checker_lifecycle() {
    let root = temp_root("gateway-routing-scores-lifecycle");
    let paths = app_paths_for_root(root.clone());
    let database_path = root.join("gateway.sqlite");
    runtime_gateway_sqlite_create_current_schema_for_tests(&database_path).unwrap();
    let tenant = TenantId::new();
    let connection = Connection::open(&database_path).unwrap();
    connection
        .execute(
            "INSERT INTO prodex_tenants (tenant_id, display_name, created_at_unix_ms, updated_at_unix_ms)
             VALUES (?1, 'test tenant', 1, 1)",
            [tenant.to_string()],
        )
        .unwrap();
    drop(connection);

    let admin = |name: &str, token: &str| RuntimeGatewayAdminToken {
        name: name.to_string(),
        token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(token),
        role: RuntimeGatewayAdminRole::Admin,
        tenant_id: Some(tenant.to_string()),
        team_id: None,
        project_id: None,
        user_id: None,
        budget_id: None,
        allowed_key_prefixes: Vec::new(),
    };
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
        gateway_admin_tokens: vec![
            admin("maker", "routing-maker-token"),
            admin("checker", "routing-checker-token"),
        ],
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::sqlite(database_path),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: None,
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .unwrap();
    let client = reqwest::blocking::Client::new();
    let base = format!(
        "http://{}/v1/prodex/gateway/routing-scores",
        proxy.listen_addr
    );
    let artifact = serde_json::json!({
        "schema_version": 1,
        "revision": 7,
        "weights": {
            "health": 2_000,
            "load": 1_000,
            "cost": 3_000,
            "latency": 1_000,
            "risk": 1_000,
            "priority": 1_000,
            "affinity": 1_000
        }
    });
    assert_eq!(
        client
            .post(format!("{base}/validate"))
            .bearer_auth("routing-maker-token")
            .json(&serde_json::json!({"artifact": artifact.clone()}))
            .send()
            .unwrap()
            .status()
            .as_u16(),
        200
    );
    assert_eq!(
        client
            .post(format!("{base}/validate"))
            .bearer_auth("routing-maker-token")
            .json(&serde_json::json!({"artifact": {
                "schema_version": 1,
                "revision": 8,
                "weights": {
                    "health": 0,
                    "load": 0,
                    "cost": 10_001,
                    "latency": 0,
                    "risk": 0,
                    "priority": 0,
                    "affinity": 0
                }
            }}))
            .send()
            .unwrap()
            .status()
            .as_u16(),
        400
    );

    #[derive(serde::Serialize)]
    struct ClassificationChecksumInput {
        unsupported_coverage_floor: DataClassification,
        rules: Vec<serde_json::Value>,
    }
    let classification_checksum = Sha256::digest(
        serde_json::to_vec(&ClassificationChecksumInput {
            unsupported_coverage_floor: DataClassification::Restricted,
            rules: Vec::new(),
        })
        .unwrap(),
    )
    .iter()
    .map(|byte| format!("{byte:02x}"))
    .collect::<String>();
    let classification_artifact = serde_json::json!({
        "schema_version": 1,
        "detector_revision": "detector-v1",
        "patterns": [],
        "classification_revision": "classification-v1",
        "classification_checksum": classification_checksum,
        "unsupported_coverage_floor": "restricted",
        "classification_rules": []
    });
    let classification_base = format!(
        "http://{}/v1/prodex/gateway/classification-rules",
        proxy.listen_addr
    );
    let classification_valid = client
        .post(&classification_base)
        .bearer_auth("routing-maker-token")
        .header("Idempotency-Key", "classification-create-v1")
        .json(&serde_json::json!({
            "revision_id": "classification-v1",
            "artifact": classification_artifact.clone()
        }))
        .send()
        .unwrap();
    assert!(matches!(classification_valid.status().as_u16(), 200 | 201));
    assert_eq!(
        client
            .post(&classification_base)
            .bearer_auth("routing-maker-token")
            .header("Idempotency-Key", "classification-create-mismatched")
            .json(&serde_json::json!({
                "revision_id": "classification-v2",
                "artifact": classification_artifact
            }))
            .send()
            .unwrap()
            .status()
            .as_u16(),
        400
    );

    let adapter = provider_adapter(ProviderId::OpenAi);
    let endpoints = adapter
        .supported_endpoints()
        .iter()
        .copied()
        .filter(|endpoint| {
            crate::runtime_launch::proxy_startup::local_rewrite_application_data_plane::runtime_gateway_provider_capability_is_executable(
                adapter.capability_status(*endpoint),
            )
        })
        .collect::<Vec<_>>();
    let provider_artifact = serde_json::json!({
        "schema_version": 2,
        "revision": 7,
        "pricing_revision": 4,
        "descriptors": [{
            "revision": 9,
            "pricing_revision": 4,
            "provider": "openai",
            "credential_ref": SecretRef::new("runtime-provider", "openai", None::<String>),
            "enabled": true,
            "revoked": false,
            "executable": true,
            "endpoints": endpoints,
            "capabilities": crate::runtime_launch::proxy_startup::local_rewrite_application_data_plane::runtime_gateway_provider_executable_capabilities(ProviderId::OpenAi),
            "regions": ["*"],
            "local_execution": false,
            "trust_tier": "enterprise",
            "maximum_classification": "confidential",
            "retention_seconds": 0,
            "training_use": false,
            "model_costs": {
                "*": {
                    "input_cost_per_million_microusd": 1_000_000,
                    "output_cost_per_million_microusd": 2_000_000
                }
            },
            "cost": 2_000,
            "latency": 3_000,
            "risk": 1_000,
            "priority": 8_000
        }]
    });
    let provider_base = format!(
        "http://{}/v1/prodex/gateway/provider-registries",
        proxy.listen_addr
    );
    let provider_valid = client
        .post(&provider_base)
        .bearer_auth("routing-maker-token")
        .header("Idempotency-Key", "provider-registry-create-v7")
        .json(&serde_json::json!({
            "revision_id": "7",
            "artifact": provider_artifact.clone()
        }))
        .send()
        .unwrap();
    assert!(matches!(provider_valid.status().as_u16(), 200 | 201));
    assert_eq!(
        client
            .post(&provider_base)
            .bearer_auth("routing-maker-token")
            .header("Idempotency-Key", "provider-registry-create-mismatched")
            .json(&serde_json::json!({
                "revision_id": "8",
                "artifact": provider_artifact
            }))
            .send()
            .unwrap()
            .status()
            .as_u16(),
        400
    );

    let mismatched = client
        .post(&base)
        .bearer_auth("routing-maker-token")
        .header("Idempotency-Key", "routing-create-mismatched")
        .json(&serde_json::json!({"revision_id": "8", "artifact": artifact.clone()}))
        .send()
        .unwrap();
    assert_eq!(mismatched.status().as_u16(), 400);

    let revision = "7".to_string();
    let created = client
        .post(&base)
        .bearer_auth("routing-maker-token")
        .header("Idempotency-Key", "routing-create-v7")
        .json(&serde_json::json!({"revision_id": &revision, "artifact": artifact}))
        .send()
        .unwrap();
    assert!(matches!(created.status().as_u16(), 200 | 201));
    let approval = submit(
        &client,
        &base,
        "routing-maker-token",
        &revision,
        "routing-approval-v7",
    );
    assert_eq!(
        vote(
            &client,
            &base,
            "routing-checker-token",
            &revision,
            &approval,
            1,
            "approve",
            "routing-approve-v7",
        )
        .status()
        .as_u16(),
        200
    );
    let activated = activate(
        &client,
        &base,
        "routing-checker-token",
        &revision,
        &approval,
        "*",
        "routing-activate-v7",
        "activate",
    );
    assert_eq!(activated.status().as_u16(), 200);
    let etag = activated.headers()["etag"].to_str().unwrap().to_string();
    let status: serde_json::Value = client
        .get(format!("{base}/status"))
        .bearer_auth("routing-checker-token")
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(status["active_revision_id"], revision);
    assert_eq!(status["object"], "governance.routing_scores_status");

    let revoked = activate(
        &client,
        &base,
        "routing-checker-token",
        &revision,
        &approval,
        &etag,
        "routing-revoke-v7",
        "revoke",
    );
    assert_eq!(revoked.status().as_u16(), 200);
    let revoked_etag = revoked.headers()["etag"].to_str().unwrap().to_string();
    let revoked: serde_json::Value = revoked.json().unwrap();
    assert_eq!(revoked["active_revision_id"], serde_json::Value::Null);
    assert_eq!(
        revoked["last_known_good_revision_id"],
        serde_json::Value::Null
    );
    assert_eq!(
        activate(
            &client,
            &base,
            "routing-checker-token",
            &revision,
            &approval,
            &revoked_etag,
            "routing-reactivate-revoked-v7",
            "activate",
        )
        .status()
        .as_u16(),
        409
    );
}

pub(super) fn create_revision(
    client: &reqwest::blocking::Client,
    base: &str,
    token: &str,
    revision: &str,
    effect: &str,
    key: &str,
) -> serde_json::Value {
    let failure_mode = if effect == "allow" { "open" } else { "closed" };
    let response = client
        .post(base)
        .bearer_auth(token)
        .header("Idempotency-Key", key)
        .json(&serde_json::json!({
            "revision_id": revision,
            "artifact": {
                "policy_revision": revision,
                "policy_failure_mode": failure_mode
            }
        }))
        .send()
        .unwrap();
    let status = response.status().as_u16();
    let body = response.text().unwrap();
    assert!(matches!(status, 200 | 201), "status={status} body={body}");
    serde_json::from_str(&body).unwrap()
}

pub(super) fn submit(
    client: &reqwest::blocking::Client,
    base: &str,
    token: &str,
    revision: &str,
    approval: &str,
) -> String {
    let response = client
        .post(format!("{base}/{revision}/submit"))
        .bearer_auth(token)
        .header("Idempotency-Key", format!("submit-{approval}"))
        .json(&serde_json::json!({"approval_id": approval, "required_quorum": 1}))
        .send()
        .unwrap();
    assert!(matches!(response.status().as_u16(), 200 | 201));
    response.json::<serde_json::Value>().unwrap()["approval_id"]
        .as_str()
        .unwrap()
        .to_string()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn vote(
    client: &reqwest::blocking::Client,
    base: &str,
    token: &str,
    revision: &str,
    approval: &str,
    version: u64,
    decision: &str,
    key: &str,
) -> reqwest::blocking::Response {
    client
        .post(format!("{base}/{revision}/approvals/{approval}/votes"))
        .bearer_auth(token)
        .header("Idempotency-Key", key)
        .json(&serde_json::json!({"decision": decision, "expected_version": version}))
        .send()
        .unwrap()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn activate(
    client: &reqwest::blocking::Client,
    base: &str,
    token: &str,
    revision: &str,
    approval: &str,
    etag: &str,
    key: &str,
    action: &str,
) -> reqwest::blocking::Response {
    client
        .post(format!("{base}/{revision}/{action}"))
        .bearer_auth(token)
        .header("Idempotency-Key", key)
        .header("If-Match", etag)
        .json(&serde_json::json!({"approval_id": approval}))
        .send()
        .unwrap()
}
