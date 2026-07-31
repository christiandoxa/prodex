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
    )
}

fn start_gateway_with_options(
    name: &str,
    policy_valid_until_unix_ms: u64,
    mismatched_active: Option<GovernanceArtifactKind>,
    upstream: TestUpstream,
    gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig,
    mode: RuntimeGovernanceMode,
    classification_default: RuntimeGovernanceDataClassification,
    policy_rules: Vec<RuntimeGovernancePolicyRule>,
) -> BankGatewayFixture {
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
    let mut policy_settings = settings.clone();
    policy_settings.classification_checksum = Some(classification_checksum.clone());
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
            serde_json::to_vec(&policy_settings).unwrap(),
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
        seed_revision(
            &connection,
            tenant_id,
            signing_key,
            *kind,
            revision,
            artifact,
        );
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

fn seed_mismatched_active_revision(
    database_path: &Path,
    tenant_id: TenantId,
    signing_key: &Ed25519KeyPair,
    artifacts: &[(GovernanceArtifactKind, String, Vec<u8>)],
    kind: GovernanceArtifactKind,
) {
    let (_, lkg_revision, artifact) = artifacts
        .iter()
        .find(|(candidate, _, _)| *candidate == kind)
        .unwrap();
    let active_revision = match kind {
        GovernanceArtifactKind::Policy => prodex_domain::PolicyRevisionId::new().to_string(),
        GovernanceArtifactKind::ClassificationRules => "classification-v2".to_string(),
        GovernanceArtifactKind::ProviderRegistry | GovernanceArtifactKind::RoutingScores => {
            "2".to_string()
        }
    };
    let mismatched_artifact = if kind == GovernanceArtifactKind::Policy {
        let mut settings =
            serde_json::from_slice::<RuntimePolicyGovernanceSettings>(artifact).unwrap();
        let revision = active_revision.parse().unwrap();
        settings.policy_revision = Some(revision);
        settings.active_policy_revision = Some(revision);
        settings.classification_revision = Some("classification-v2".to_string());
        settings.classification_checksum = Some("classification-v2".to_string());
        settings.provider_registry_revision = Some(2);
        settings.routing_score_revision = Some(2);
        serde_json::to_vec(&settings).unwrap()
    } else {
        let mut artifact = artifact.clone();
        artifact.push(b' ');
        artifact
    };
    let connection = Connection::open(database_path).unwrap();
    connection
        .execute(
            &format!(
                "UPDATE {} SET lifecycle_state = 'superseded' \
                 WHERE tenant_id = ?1 AND revision_id = ?2",
                revision_table(kind),
            ),
            params![tenant_id.to_string(), lkg_revision],
        )
        .unwrap();
    seed_revision(
        &connection,
        tenant_id,
        signing_key,
        kind,
        &active_revision,
        &mismatched_artifact,
    );
    connection
        .execute(
            &format!(
                "UPDATE {} SET active_revision_id = ?2, last_known_good_revision_id = ?3, \
                 etag = 'etag-mismatch', updated_at_unix_ms = 2 WHERE tenant_id = ?1",
                pointer_table(kind),
            ),
            params![tenant_id.to_string(), active_revision, lkg_revision],
        )
        .unwrap();
}

fn seed_revision(
    connection: &Connection,
    tenant_id: TenantId,
    signing_key: &Ed25519KeyPair,
    kind: GovernanceArtifactKind,
    revision: &str,
    artifact: &[u8],
) {
    let checksum = governance_support::artifact_checksum(artifact);
    let created_by = PrincipalId::new().to_string();
    let signature = signing_key.sign(&governance_support::artifact_signature_message(
        tenant_id, kind, revision, artifact,
    ));
    connection
        .execute(
            "INSERT INTO prodex_governance_revision_artifacts
             (tenant_id, artifact_kind, revision_id, artifact_checksum, compiled_artifact,
              created_by, created_at_unix_ms, signature_key_id, artifact_signature)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, 'test-authority', ?7)",
            params![
                tenant_id.to_string(),
                artifact_kind_label(kind),
                revision,
                checksum,
                artifact,
                created_by,
                STANDARD.encode(signature.as_ref()),
            ],
        )
        .unwrap();
    match kind {
        GovernanceArtifactKind::Policy => connection.execute(
            "INSERT INTO prodex_policy_revisions (
                tenant_id, revision_id, artifact_checksum, compiled_metadata,
                lifecycle_state, created_by, created_at_unix_ms
             ) VALUES (?1, ?2, ?3, '{}', 'active', ?4, 1)",
            params![tenant_id.to_string(), revision, checksum, created_by],
        ),
        GovernanceArtifactKind::ClassificationRules => connection.execute(
            "INSERT INTO prodex_classification_rule_revisions (
                tenant_id, revision_id, artifact_checksum, compiled_metadata,
                lifecycle_state, created_at_unix_ms
             ) VALUES (?1, ?2, ?3, '{}', 'active', 1)",
            params![tenant_id.to_string(), revision, checksum],
        ),
        GovernanceArtifactKind::ProviderRegistry => connection.execute(
            "INSERT INTO prodex_provider_registry_revisions (
                tenant_id, revision_id, artifact_checksum, lifecycle_state, created_at_unix_ms
             ) VALUES (?1, ?2, ?3, 'active', 1)",
            params![tenant_id.to_string(), revision, checksum],
        ),
        GovernanceArtifactKind::RoutingScores => connection.execute(
            "INSERT INTO prodex_routing_score_revisions (
                tenant_id, revision_id, artifact_checksum, fixed_point_weights,
                lifecycle_state, created_at_unix_ms
             ) VALUES (?1, ?2, ?3, '{}', 'active', 1)",
            params![tenant_id.to_string(), revision, checksum],
        ),
    }
    .unwrap();
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

fn revision_table(kind: GovernanceArtifactKind) -> &'static str {
    match kind {
        GovernanceArtifactKind::Policy => "prodex_policy_revisions",
        GovernanceArtifactKind::ClassificationRules => "prodex_classification_rule_revisions",
        GovernanceArtifactKind::ProviderRegistry => "prodex_provider_registry_revisions",
        GovernanceArtifactKind::RoutingScores => "prodex_routing_score_revisions",
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
