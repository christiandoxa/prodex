use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use aws_lc_rs::signature::{Ed25519KeyPair, KeyPair};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use prodex_domain::{DataClassification, PrincipalId, SecretRef, TenantId};
use prodex_provider_core::{ProviderId, provider_adapter};
use prodex_runtime_policy::{
    RuntimeGovernanceDataClassification, RuntimeGovernanceMode, RuntimeGovernancePolicyFailureMode,
    RuntimeGovernanceProviderTrustTier, RuntimeGovernanceRolloutMode,
    RuntimeGovernanceUnknownClassificationBehavior, RuntimePolicyGovernanceArtifactVerifier,
    RuntimePolicyGovernanceProviderSettings, RuntimePolicyGovernanceSessionSettings,
    RuntimePolicyGovernanceSettings,
};
use prodex_storage::{GovernanceArtifactKind, governance_support};
use rusqlite::{Connection, params};
use sha2::{Digest, Sha256};

use super::{
    AppState, RuntimeGatewayGuardrailWebhookConfig, RuntimeGatewayObservabilityConfig,
    RuntimeGatewaySsoConfig, RuntimeGatewayStateStore, RuntimeLocalRewriteProviderOptions,
    RuntimeLocalRewriteProxyStartOptions, TestUpstream, app_paths_for_root, temp_root,
};
use crate::runtime_launch::proxy_startup::local_rewrite::start_runtime_local_rewrite_proxy_with_file_access;
use crate::runtime_launch::proxy_startup::local_rewrite_gateway_backend_connection::runtime_gateway_sqlite_create_current_schema_for_tests;
use crate::{RuntimeConfig, RuntimeRotationProxy};

struct BankGatewayFixture {
    proxy: RuntimeRotationProxy,
    database_path: PathBuf,
    tenant_id: TenantId,
    data_token: &'static str,
    upstream: TestUpstream,
}

#[test]
fn bank_readyz_and_data_plane_fail_closed_when_policy_expires() {
    let expires_at = now_unix_ms().saturating_add(2_000);
    let fixture = start_bank_gateway("gateway-bank-policy-expiry", expires_at);
    assert_ready(&fixture.proxy, true, "ok");

    let body = wait_for_readiness(&fixture.proxy, false, Duration::from_secs(5));
    assert_eq!(body["status"], "governance_policy_unavailable");
    assert_eq!(body["governance_policy_available"], false);
    assert_liveness_stays_up(&fixture.proxy);
    assert_policy_request_is_unavailable(&fixture);
}

#[test]
fn bank_readyz_and_data_plane_fail_closed_when_authority_invalidates_policy() {
    let fixture = start_bank_gateway("gateway-bank-policy-invalidation", u64::MAX);
    assert_ready(&fixture.proxy, true, "ok");

    Connection::open(&fixture.database_path)
        .unwrap()
        .execute(
            "UPDATE prodex_policy_pointers
             SET active_revision_id = 'invalidated', last_known_good_revision_id = 'invalidated'
             WHERE tenant_id = ?1",
            [fixture.tenant_id.to_string()],
        )
        .unwrap();

    let body = wait_for_readiness(&fixture.proxy, false, Duration::from_secs(8));
    assert_eq!(body["status"], "governance_policy_unavailable");
    assert_eq!(body["governance_policy_available"], false);
    assert_liveness_stays_up(&fixture.proxy);
    assert_policy_request_is_unavailable(&fixture);
}

fn start_bank_gateway(name: &str, policy_valid_until_unix_ms: u64) -> BankGatewayFixture {
    let root = temp_root(name);
    let paths = app_paths_for_root(root.clone());
    let database_path = root.join("gateway.sqlite");
    runtime_gateway_sqlite_create_current_schema_for_tests(&database_path).unwrap();
    let tenant_id = TenantId::new();
    let signing_key = Ed25519KeyPair::from_seed_unchecked(&[29_u8; 32]).unwrap();
    let policy_revision = prodex_domain::PolicyRevisionId::new();
    let settings = RuntimePolicyGovernanceSettings {
        authority_tenants: vec![tenant_id],
        artifact_verifiers: vec![RuntimePolicyGovernanceArtifactVerifier {
            key_id: "test-authority".to_string(),
            ed25519_public_key_base64: STANDARD.encode(signing_key.public_key().as_ref()),
        }],
        mode: RuntimeGovernanceMode::BankEnforce,
        inspection: RuntimeGovernanceRolloutMode::Enforce,
        classification: RuntimeGovernanceRolloutMode::Enforce,
        policy: RuntimeGovernanceRolloutMode::Enforce,
        routing: RuntimeGovernanceRolloutMode::Enforce,
        mandatory_audit: true,
        anonymous_data_plane: false,
        raw_secret_sources: false,
        policy_revision: Some(policy_revision),
        policy_valid_until_unix_ms: Some(policy_valid_until_unix_ms),
        classification_revision: Some("classification-v1".to_string()),
        classification_checksum: Some("classification-v1".to_string()),
        provider_registry_revision: Some(1),
        routing_score_revision: Some(1),
        provider: Some(RuntimePolicyGovernanceProviderSettings {
            descriptor_revision: 1,
            enabled: true,
            revoked: false,
            trust_tier: RuntimeGovernanceProviderTrustTier::RestrictedApproved,
            local_execution: true,
            maximum_classification: RuntimeGovernanceDataClassification::Restricted,
            regions: vec!["test-region".to_string()],
            retention_seconds: 0,
            training_use: false,
        }),
        classification_default: RuntimeGovernanceDataClassification::Restricted,
        classification_unknown: RuntimeGovernanceUnknownClassificationBehavior::Deny,
        policy_failure_mode: RuntimeGovernancePolicyFailureMode::Closed,
        active_policy_revision: Some(policy_revision),
        session: RuntimePolicyGovernanceSessionSettings {
            absolute_timeout_seconds: Some(3_600),
            idle_timeout_seconds: Some(900),
            max_concurrent: Some(10),
        },
        ..RuntimePolicyGovernanceSettings::default()
    };
    let artifacts = bank_artifacts(&settings);
    seed_authority(&database_path, tenant_id, &signing_key, &artifacts);

    let mut runtime_config = RuntimeConfig::offline_default(&paths).unwrap();
    runtime_config.governance = crate::runtime_governance::runtime_governance_config(&settings);
    runtime_config.governance_policy = settings;
    let upstream = TestUpstream::start_n(0);
    let data_token = "bank-data-token";
    let proxy = start_runtime_local_rewrite_proxy_with_file_access(
        RuntimeLocalRewriteProxyStartOptions {
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
            gateway_state_store: RuntimeGatewayStateStore::sqlite(database_path.clone()),
            gateway_virtual_keys: vec![runtime_proxy_crate::RuntimeGatewayVirtualKey {
                name: "bank-data".to_string(),
                tenant_id: Some(tenant_id.to_string()),
                team_id: None,
                project_id: None,
                user_id: None,
                budget_id: None,
                token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(data_token),
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
        },
        Arc::new(runtime_config),
        false,
        None,
        Default::default(),
        prodex_provider_core::resolve_harness_mode(None, None),
    )
    .unwrap();

    BankGatewayFixture {
        proxy,
        database_path,
        tenant_id,
        data_token,
        upstream,
    }
}

fn bank_artifacts(
    settings: &RuntimePolicyGovernanceSettings,
) -> Vec<(GovernanceArtifactKind, String, Vec<u8>)> {
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
    let classification = serde_json::to_vec(&serde_json::json!({
        "schema_version": 1,
        "detector_revision": "detector-v1",
        "patterns": [],
        "classification_revision": "classification-v1",
        "classification_checksum": classification_checksum,
        "unsupported_coverage_floor": "restricted",
        "classification_rules": []
    }))
    .unwrap();
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
    let provider_registry = serde_json::to_vec(&serde_json::json!({
        "schema_version": 2,
        "revision": 1,
        "pricing_revision": 1,
        "descriptors": [{
            "revision": 1,
            "pricing_revision": 1,
            "provider": "openai",
            "credential_ref": SecretRef::new("runtime-provider", "openai", None::<String>),
            "enabled": true,
            "revoked": false,
            "executable": true,
            "endpoints": endpoints,
            "capabilities": crate::runtime_launch::proxy_startup::local_rewrite_application_data_plane::runtime_gateway_provider_executable_capabilities(ProviderId::OpenAi),
            "regions": ["*"],
            "local_execution": true,
            "trust_tier": "restricted_approved",
            "maximum_classification": "restricted",
            "retention_seconds": 0,
            "training_use": false,
            "model_costs": {"*": {
                "input_cost_per_million_microusd": 1_000_000,
                "output_cost_per_million_microusd": 2_000_000
            }},
            "cost": 2_000,
            "latency": 3_000,
            "risk": 1_000,
            "priority": 8_000
        }]
    }))
    .unwrap();
    let routing = serde_json::to_vec(&serde_json::json!({
        "schema_version": 1,
        "revision": 1,
        "weights": {
            "health": 2_000,
            "load": 1_000,
            "cost": 3_000,
            "latency": 1_000,
            "risk": 1_000,
            "priority": 1_000,
            "affinity": 1_000
        }
    }))
    .unwrap();

    vec![
        (
            GovernanceArtifactKind::Policy,
            settings.policy_revision.unwrap().to_string(),
            serde_json::to_vec(settings).unwrap(),
        ),
        (
            GovernanceArtifactKind::ClassificationRules,
            "classification-v1".to_string(),
            classification,
        ),
        (
            GovernanceArtifactKind::ProviderRegistry,
            "1".to_string(),
            provider_registry,
        ),
        (
            GovernanceArtifactKind::RoutingScores,
            "1".to_string(),
            routing,
        ),
    ]
}

fn seed_authority(
    database_path: &Path,
    tenant_id: TenantId,
    signing_key: &Ed25519KeyPair,
    artifacts: &[(GovernanceArtifactKind, String, Vec<u8>)],
) {
    let connection = Connection::open(database_path).unwrap();
    connection
        .execute(
            "INSERT INTO prodex_tenants
             (tenant_id, display_name, created_at_unix_ms, updated_at_unix_ms)
             VALUES (?1, 'test tenant', 1, 1)",
            [tenant_id.to_string()],
        )
        .unwrap();
    for (kind, revision, artifact) in artifacts {
        let signature = signing_key.sign(&governance_support::artifact_signature_message(
            tenant_id, *kind, revision, artifact,
        ));
        connection
            .execute(
                "INSERT INTO prodex_governance_revision_artifacts
                 (tenant_id, artifact_kind, revision_id, artifact_checksum, compiled_artifact,
                  created_by, created_at_unix_ms, signature_key_id, artifact_signature)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, 'test-authority', ?7)",
                params![
                    tenant_id.to_string(),
                    artifact_kind_label(*kind),
                    revision,
                    governance_support::artifact_checksum(artifact),
                    artifact,
                    PrincipalId::new().to_string(),
                    STANDARD.encode(signature.as_ref()),
                ],
            )
            .unwrap();
        connection
            .execute(
                &format!(
                    "INSERT INTO {} (tenant_id, active_revision_id, last_known_good_revision_id,
                     etag, updated_at_unix_ms) VALUES (?1, ?2, ?2, 'etag-1', 1)",
                    pointer_table(*kind),
                ),
                params![tenant_id.to_string(), revision],
            )
            .unwrap();
    }
}

fn artifact_kind_label(kind: GovernanceArtifactKind) -> &'static str {
    match kind {
        GovernanceArtifactKind::Policy => "policy",
        GovernanceArtifactKind::ClassificationRules => "classification_rules",
        GovernanceArtifactKind::ProviderRegistry => "provider_registry",
        GovernanceArtifactKind::RoutingScores => "routing_scores",
    }
}

fn pointer_table(kind: GovernanceArtifactKind) -> &'static str {
    match kind {
        GovernanceArtifactKind::Policy => "prodex_policy_pointers",
        GovernanceArtifactKind::ClassificationRules => "prodex_classification_rule_pointers",
        GovernanceArtifactKind::ProviderRegistry => "prodex_provider_registry_pointers",
        GovernanceArtifactKind::RoutingScores => "prodex_routing_score_pointers",
    }
}

fn assert_ready(proxy: &RuntimeRotationProxy, ready: bool, status: &str) {
    let response = reqwest::blocking::Client::new()
        .get(format!("http://{}/readyz", proxy.listen_addr))
        .send()
        .unwrap();
    assert_eq!(response.status().as_u16(), if ready { 200 } else { 503 });
    let body: serde_json::Value = response.json().unwrap();
    assert_eq!(body["ready"], ready);
    assert_eq!(body["status"], status);
    assert_eq!(body["governance_policy_available"], ready);
}

fn wait_for_readiness(
    proxy: &RuntimeRotationProxy,
    ready: bool,
    timeout: Duration,
) -> serde_json::Value {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        let response = reqwest::blocking::Client::new()
            .get(format!("http://{}/readyz", proxy.listen_addr))
            .send()
            .unwrap();
        let status = response.status().as_u16();
        let body: serde_json::Value = response.json().unwrap();
        if body["ready"] == ready {
            assert_eq!(status, if ready { 200 } else { 503 });
            return body;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "readiness did not change: {body}"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn assert_liveness_stays_up(proxy: &RuntimeRotationProxy) {
    for path in ["/livez", "/startupz"] {
        let response = reqwest::blocking::Client::new()
            .get(format!("http://{}{}", proxy.listen_addr, path))
            .send()
            .unwrap();
        assert_eq!(response.status().as_u16(), 200);
        assert_eq!(response.json::<serde_json::Value>().unwrap()["ready"], true);
    }
}

fn assert_policy_request_is_unavailable(fixture: &BankGatewayFixture) {
    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses", fixture.proxy.listen_addr))
        .bearer_auth(fixture.data_token)
        .json(&serde_json::json!({"model": "gpt-5", "input": "hello"}))
        .send()
        .unwrap();
    let status = response.status().as_u16();
    let body = response.text().unwrap();
    assert_eq!(status, 503, "{body}");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&body).unwrap()["error"]["code"],
        "gateway_policy_unavailable"
    );
    assert!(
        fixture
            .upstream
            .path_rx
            .recv_timeout(Duration::from_millis(100))
            .is_err()
    );
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}
