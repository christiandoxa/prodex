use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use aws_lc_rs::signature::{Ed25519KeyPair, KeyPair};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use prodex_domain::{DataClassification, PrincipalId, SecretRef, TenantId};
use prodex_provider_core::{ProviderId, provider_adapter};
use prodex_runtime_policy::{
    RuntimeGovernanceDataClassification, RuntimeGovernanceMode, RuntimeGovernancePolicyChannel,
    RuntimeGovernancePolicyEffect, RuntimeGovernancePolicyFailureMode, RuntimeGovernancePolicyRule,
    RuntimeGovernancePolicyRuleCondition, RuntimeGovernanceProviderTrustTier,
    RuntimeGovernanceRolloutMode, RuntimeGovernanceUnknownClassificationBehavior,
    RuntimePolicyGovernanceArtifactVerifier, RuntimePolicyGovernanceProviderSettings,
    RuntimePolicyGovernanceSessionSettings, RuntimePolicyGovernanceSettings,
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

#[path = "gateway_policy_readiness/artifacts.rs"]
mod artifacts;
use artifacts::{bank_artifacts, pointer_table, seed_authority, seed_mismatched_active_revision};

struct BankGatewayFixture {
    proxy: RuntimeRotationProxy,
    database_path: PathBuf,
    tenant_id: TenantId,
    data_token: &'static str,
    upstream: TestUpstream,
}

const VALID_FAR_FUTURE_UNIX_MS: u64 = 4_102_444_800_000;

#[test]
fn enterprise_sse_postcommit_audit_failure_degrades_without_retry_and_recovers() {
    let mut first_chunk =
        b"data: {\"type\":\"response.output_text.delta\",\"delta\":\"ssss\"}\n\n".to_vec();
    first_chunk.extend(std::iter::repeat_n(b's', 4_096));
    let upstream = TestUpstream::start_with_delayed_chunks(
        2,
        "text/event-stream; charset=utf-8",
        vec![
            first_chunk,
            b"data: {\"type\":\"response.output_text.delta\",\"delta\":\"blocked-postcommit-marker\"}\n\n"
                .to_vec(),
        ],
    );
    let fixture = start_enterprise_gateway_with_options(
        "gateway-enterprise-sse-postcommit-audit",
        VALID_FAR_FUTURE_UNIX_MS,
        None,
        upstream,
        runtime_proxy_crate::RuntimeGatewayGuardrailConfig {
            blocked_output_keywords: vec!["blocked-postcommit-marker".to_string()],
            ..runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default()
        },
    );
    install_postcommit_audit_failure(&fixture.database_path);

    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(2))
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap();
    let mut response = client
        .post(format!("http://{}/v1/responses", fixture.proxy.listen_addr))
        .bearer_auth(fixture.data_token)
        .header(reqwest::header::CONNECTION, "close")
        .json(&serde_json::json!({"model": "gpt-5", "input": "hello", "stream": true}))
        .send()
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
    let mut body = Vec::new();
    loop {
        let mut chunk = [0_u8; 4 * 1024];
        let read = std::io::Read::read(&mut response, &mut chunk).unwrap();
        assert!(read > 0, "safe SSE chunk was not committed");
        body.extend_from_slice(&chunk[..read]);
        if String::from_utf8_lossy(&body).contains("ssss")
            && body.windows(2).any(|window| window == b"\n\n")
        {
            break;
        }
    }
    let mut tail = Vec::new();
    assert!(std::io::Read::read_to_end(&mut response, &mut tail).is_err());
    body.extend(tail);
    let body = String::from_utf8_lossy(&body);
    assert!(body.contains("ssss"));
    assert!(!body.contains("blocked-postcommit-marker"));
    assert_eq!(
        fixture
            .upstream
            .path_rx
            .recv_timeout(Duration::from_secs(2))
            .unwrap(),
        "/v1/responses"
    );
    assert!(
        fixture
            .upstream
            .path_rx
            .recv_timeout(Duration::from_millis(300))
            .is_err(),
        "committed SSE response must not retry upstream"
    );

    let readiness = wait_for_readiness(&fixture.proxy, false, Duration::from_secs(2));
    assert_eq!(readiness["status"], "governance_audit_unavailable");
    assert_eq!(readiness["governance_audit_available"], false);
    assert_audit_request_is_unavailable(&fixture);

    remove_postcommit_audit_failure(&fixture.database_path);
    let readiness = wait_for_readiness(&fixture.proxy, true, Duration::from_secs(5));
    assert_eq!(readiness["governance_audit_available"], true);
    let connection = Connection::open(&fixture.database_path).unwrap();
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM prodex_audit_log WHERE action = 'gateway.governance.response_postcommit_block'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        1
    );
    let (tenant_id, principal_id) = connection
        .query_row(
            "SELECT tenant_id, principal_id FROM prodex_audit_log
             WHERE action = 'gateway.governance.response_postcommit_block'",
            [],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .unwrap();
    assert_eq!(tenant_id, fixture.tenant_id.to_string());
    assert!(!principal_id.is_empty());
}

#[test]
fn bank_readyz_and_data_plane_fail_closed_when_policy_is_expired() {
    let fixture = start_bank_gateway("gateway-bank-policy-expiry", now_unix_ms());
    let body = wait_for_readiness(&fixture.proxy, false, Duration::from_secs(2));
    assert_eq!(body["status"], "governance_policy_unavailable");
    assert_eq!(body["governance_policy_available"], false);
    assert_liveness_stays_up(&fixture.proxy);
    assert_policy_request_is_unavailable(&fixture);
}

#[test]
fn bank_readyz_and_data_plane_fail_closed_when_authority_invalidates_policy() {
    let fixture = start_bank_gateway("gateway-bank-policy-invalidation", VALID_FAR_FUTURE_UNIX_MS);
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

#[test]
fn bank_readyz_fails_when_classification_rules_are_invalidated() {
    assert_bank_readyz_fails_when_artifact_is_invalidated(
        "gateway-bank-classification-invalidation",
        GovernanceArtifactKind::ClassificationRules,
    );
}

#[test]
fn bank_readyz_fails_when_provider_registry_is_invalidated() {
    assert_bank_readyz_fails_when_artifact_is_invalidated(
        "gateway-bank-provider-invalidation",
        GovernanceArtifactKind::ProviderRegistry,
    );
}

#[test]
fn bank_readyz_fails_when_routing_scores_are_invalidated() {
    assert_bank_readyz_fails_when_artifact_is_invalidated(
        "gateway-bank-routing-invalidation",
        GovernanceArtifactKind::RoutingScores,
    );
}

#[test]
fn bank_bootstrap_uses_compatible_lkg_when_active_revision_mismatches_bundle() {
    for (name, kind) in [
        (
            "gateway-bank-policy-incompatible",
            GovernanceArtifactKind::Policy,
        ),
        (
            "gateway-bank-classification-mismatch",
            GovernanceArtifactKind::ClassificationRules,
        ),
        (
            "gateway-bank-provider-mismatch",
            GovernanceArtifactKind::ProviderRegistry,
        ),
        (
            "gateway-bank-routing-mismatch",
            GovernanceArtifactKind::RoutingScores,
        ),
    ] {
        let fixture =
            start_bank_gateway_with_mismatched_active(name, VALID_FAR_FUTURE_UNIX_MS, Some(kind));
        assert_ready(&fixture.proxy, true, "ok");
    }
}

fn assert_bank_readyz_fails_when_artifact_is_invalidated(name: &str, kind: GovernanceArtifactKind) {
    let fixture = start_bank_gateway(name, VALID_FAR_FUTURE_UNIX_MS);
    assert_ready(&fixture.proxy, true, "ok");

    Connection::open(&fixture.database_path)
        .unwrap()
        .execute(
            &format!(
                "UPDATE {} SET active_revision_id = 'invalidated', \
                 last_known_good_revision_id = 'invalidated' WHERE tenant_id = ?1",
                pointer_table(kind),
            ),
            [fixture.tenant_id.to_string()],
        )
        .unwrap();

    let body = wait_for_readiness(&fixture.proxy, false, Duration::from_secs(8));
    assert_eq!(body["status"], "governance_policy_unavailable");
    assert_eq!(body["governance_policy_available"], false);
    assert_liveness_stays_up(&fixture.proxy);
}

fn start_bank_gateway(name: &str, policy_valid_until_unix_ms: u64) -> BankGatewayFixture {
    start_bank_gateway_with_mismatched_active(name, policy_valid_until_unix_ms, None)
}

fn start_bank_gateway_with_mismatched_active(
    name: &str,
    policy_valid_until_unix_ms: u64,
    mismatched_active: Option<GovernanceArtifactKind>,
) -> BankGatewayFixture {
    start_bank_gateway_with_options(
        name,
        policy_valid_until_unix_ms,
        mismatched_active,
        TestUpstream::start_n(0),
        runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
    )
}

fn start_bank_gateway_with_options(
    name: &str,
    policy_valid_until_unix_ms: u64,
    mismatched_active: Option<GovernanceArtifactKind>,
    upstream: TestUpstream,
    gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig,
) -> BankGatewayFixture {
    start_gateway_with_options(
        name,
        policy_valid_until_unix_ms,
        mismatched_active,
        upstream,
        gateway_guardrails,
        (
            RuntimeGovernanceMode::BankEnforce,
            RuntimeGovernanceDataClassification::Public,
            vec![RuntimeGovernancePolicyRule {
                id: "test.allow-api".to_string(),
                condition: RuntimeGovernancePolicyRuleCondition {
                    channel: Some(RuntimeGovernancePolicyChannel::Api),
                    ..Default::default()
                },
                effect: RuntimeGovernancePolicyEffect::Allow,
                obligations: Vec::new(),
                reason_code: "policy.test_allow".to_string(),
            }],
        ),
    )
}

fn start_enterprise_gateway_with_options(
    name: &str,
    policy_valid_until_unix_ms: u64,
    mismatched_active: Option<GovernanceArtifactKind>,
    upstream: TestUpstream,
    gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig,
) -> BankGatewayFixture {
    start_gateway_with_options(
        name,
        policy_valid_until_unix_ms,
        mismatched_active,
        upstream,
        gateway_guardrails,
        (
            RuntimeGovernanceMode::EnterpriseEnforce,
            RuntimeGovernanceDataClassification::Public,
            vec![RuntimeGovernancePolicyRule {
                id: "test.allow-api".to_string(),
                condition: RuntimeGovernancePolicyRuleCondition {
                    channel: Some(RuntimeGovernancePolicyChannel::Api),
                    ..Default::default()
                },
                effect: RuntimeGovernancePolicyEffect::Allow,
                obligations: Vec::new(),
                reason_code: "policy.test_allow".to_string(),
            }],
        ),
    )
}

fn start_gateway_with_options(
    name: &str,
    policy_valid_until_unix_ms: u64,
    mismatched_active: Option<GovernanceArtifactKind>,
    upstream: TestUpstream,
    gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig,
    governance: (
        RuntimeGovernanceMode,
        RuntimeGovernanceDataClassification,
        Vec<RuntimeGovernancePolicyRule>,
    ),
) -> BankGatewayFixture {
    let (mode, classification_default, policy_rules) = governance;
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
        mode,
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
        classification_default,
        classification_unknown: RuntimeGovernanceUnknownClassificationBehavior::Deny,
        policy_failure_mode: RuntimeGovernancePolicyFailureMode::Closed,
        active_policy_revision: Some(policy_revision),
        policy_rules,
        session: RuntimePolicyGovernanceSessionSettings {
            absolute_timeout_seconds: Some(3_600),
            idle_timeout_seconds: Some(900),
            max_concurrent: Some(10),
        },
        ..RuntimePolicyGovernanceSettings::default()
    };
    let artifacts = bank_artifacts(&settings);
    seed_authority(&database_path, tenant_id, &signing_key, &artifacts);
    if let Some(kind) = mismatched_active {
        seed_mismatched_active_revision(&database_path, tenant_id, &signing_key, &artifacts, kind);
    }

    let mut runtime_config = RuntimeConfig::offline_default(&paths).unwrap();
    runtime_config.governance = crate::runtime_governance::runtime_governance_config(&settings);
    runtime_config.governance_policy = settings;
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
            gateway_guardrails,
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

fn assert_audit_request_is_unavailable(fixture: &BankGatewayFixture) {
    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses", fixture.proxy.listen_addr))
        .bearer_auth(fixture.data_token)
        .json(&serde_json::json!({"model": "gpt-5", "input": "hello"}))
        .send()
        .unwrap();
    assert_eq!(response.status().as_u16(), 503);
    assert_eq!(
        response.json::<serde_json::Value>().unwrap()["error"]["code"],
        "governance_audit_unavailable"
    );
    assert!(
        fixture
            .upstream
            .path_rx
            .recv_timeout(Duration::from_millis(100))
            .is_err()
    );
}

fn install_postcommit_audit_failure(database_path: &Path) {
    Connection::open(database_path)
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER fail_postcommit_audit BEFORE INSERT ON prodex_audit_log
             WHEN NEW.action = 'gateway.governance.response_postcommit_block'
             BEGIN SELECT RAISE(ABORT, 'postcommit audit unavailable'); END;",
        )
        .unwrap();
}

fn remove_postcommit_audit_failure(database_path: &Path) {
    Connection::open(database_path)
        .unwrap()
        .execute_batch("DROP TRIGGER fail_postcommit_audit")
        .unwrap();
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}
