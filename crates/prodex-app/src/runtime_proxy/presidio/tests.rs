use super::engine::{
    InspectionExecutionOutcome, RuntimePresidioFailClosedPolicy, runtime_local_inspection_required,
    runtime_presidio_redact_body,
};
use super::findings::{
    PresidioAnalyzerResult, runtime_presidio_findings, runtime_presidio_inspection_plan,
    runtime_presidio_inspection_source,
};
use super::json_body::PresidioJsonString;
use super::local::RuntimeTenantDetectorPatterns;
use super::registry::{
    MAX_RUNTIME_PRESIDIO_REGISTRY_ENTRIES, RuntimePresidioRedactionState,
    register_runtime_presidio_redaction_proxy_state,
    unregister_runtime_presidio_redaction_proxy_state, validate_runtime_presidio_registry_insert,
};
use super::telemetry::{
    runtime_inspection_error_outcome, runtime_inspection_failure_type,
    runtime_inspection_metric_message,
};
use crate::presidio_runtime::PresidioLanguageMode;
use crate::{
    AppPaths, AppState, RuntimeConfig, RuntimeContinuationStatuses, RuntimePresidioRedactionConfig,
    RuntimeProxyLaneAdmission, RuntimeProxyLaneLimits, RuntimeProxyRequest,
    RuntimeRotationProxyShared, RuntimeRotationState,
};
use prodex_domain::TenantId;
use prodex_observability::{
    InspectionCoverageClass, InspectionFindingCategory, InspectionMaskingAction, InspectionOutcome,
    InspectionStage, plan_inspection_metric,
};
use prodex_runtime_policy::RuntimePolicyInspectionPattern;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;
use tiny_http::{Header as TinyHeader, Response as TinyResponse, Server as TinyServer};
use tokio::runtime::Builder as TokioRuntimeBuilder;

#[path = "presidio_noop_anonymizer.rs"]
mod noop_anonymizer;

fn start_presidio_fixture(
    response_body: &'static str,
    expected_path: &'static str,
    expected_snippet: &'static str,
) -> (String, thread::JoinHandle<()>) {
    start_presidio_fixture_with_delay(
        response_body,
        expected_path,
        expected_snippet,
        Duration::ZERO,
    )
}

fn start_presidio_fixture_with_delay(
    response_body: &'static str,
    expected_path: &'static str,
    expected_snippet: &'static str,
    delay: Duration,
) -> (String, thread::JoinHandle<()>) {
    let server = TinyServer::http("127.0.0.1:0").unwrap();
    let addr = server.server_addr().to_ip().unwrap();
    let handle = thread::spawn(move || {
        let mut request = server.recv().unwrap();
        assert_eq!(request.url(), expected_path);
        let mut body = String::new();
        request.as_reader().read_to_string(&mut body).unwrap();
        assert!(body.contains(expected_snippet), "{body}");
        thread::sleep(delay);
        let response = TinyResponse::from_string(response_body)
            .with_header(TinyHeader::from_bytes("Content-Type", "application/json").unwrap());
        let _ = request.respond(response);
    });
    (format!("http://{addr}"), handle)
}

fn start_unavailable_presidio_fixture() -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        drop(stream);
    });
    (format!("http://{addr}"), handle)
}

#[test]
fn runtime_presidio_redact_body_anonymizes_request_payload() {
    let (analyzer_url, analyzer_handle) = start_presidio_fixture(
        r#"[{"start":8,"end":24,"score":0.99,"entity_type":"EMAIL_ADDRESS"}]"#,
        "/analyze",
        "user@example.com",
    );
    let (anonymizer_url, anonymizer_handle) = start_presidio_fixture(
        r#"{"text":"contact <EMAIL_ADDRESS>"}"#,
        "/anonymize",
        "EMAIL_ADDRESS",
    );
    let config = RuntimePresidioRedactionConfig {
        analyzer_url,
        anonymizer_url,
        languages: vec!["en".to_string()],
        language_mode: PresidioLanguageMode::Fixed,
        fail_closed: true,
        trusted_hosts: Vec::new(),
        timeout_ms: 10_000,
        max_response_bytes: 4 * 1024 * 1024,
        max_concurrency: 8,
    };
    let state = Arc::new(RuntimePresidioRedactionState::new(config).unwrap());
    let runtime = TokioRuntimeBuilder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let attempt = runtime
        .block_on(runtime_presidio_redact_body(
            br#"{"type":"response.create","input":"contact user@example.com"}"#.to_vec(),
            state,
        ))
        .unwrap();
    let InspectionExecutionOutcome::Redacted(redacted) = attempt else {
        panic!("Presidio fixture should succeed");
    };
    assert_eq!(redacted.source.findings.len(), 1);
    assert_eq!(
        redacted.source.masked_findings,
        vec![prodex_domain::FindingKind::EmailAddress]
    );
    let text = String::from_utf8(redacted.body).unwrap();
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert!(!text.contains("user@example.com"));
    assert_eq!(json["type"], "response.create");
    assert_eq!(json["input"], "contact <EMAIL_ADDRESS>");
    analyzer_handle.join().unwrap();
    anonymizer_handle.join().unwrap();
}

#[test]
fn runtime_presidio_detector_failure_matrix_is_bounded_and_content_preserving() {
    enum FailureCase {
        Timeout,
        Unavailable,
        Malformed,
        NonUtf8,
    }

    for case in [
        FailureCase::Timeout,
        FailureCase::Unavailable,
        FailureCase::Malformed,
        FailureCase::NonUtf8,
    ] {
        let (
            body,
            analyzer_url,
            timeout_ms,
            handle,
            expected_outcome,
            expected_error,
            expected_failure_type,
        ) = match case {
            FailureCase::Timeout => {
                let (url, handle) = start_presidio_fixture_with_delay(
                    "[]",
                    "/analyze",
                    "synthetic-timeout",
                    Duration::from_millis(100),
                );
                (
                    br#"{"input":"synthetic-timeout"}"#.to_vec(),
                    url,
                    10,
                    Some(handle),
                    InspectionOutcome::Timeout,
                    "failed to call Presidio Analyzer",
                    "timeout",
                )
            }
            FailureCase::Unavailable => {
                let listener = TcpListener::bind("127.0.0.1:0").unwrap();
                let url = format!("http://{}", listener.local_addr().unwrap());
                let handle = thread::spawn(move || {
                    let (stream, _) = listener.accept().unwrap();
                    drop(stream);
                });
                (
                    br#"{"input":"synthetic-unavailable"}"#.to_vec(),
                    url,
                    1_000,
                    Some(handle),
                    InspectionOutcome::Error,
                    "failed to call Presidio Analyzer",
                    "unavailable",
                )
            }
            FailureCase::Malformed => {
                let (url, handle) =
                    start_presidio_fixture("not-json", "/analyze", "synthetic-malformed");
                (
                    br#"{"input":"synthetic-malformed"}"#.to_vec(),
                    url,
                    1_000,
                    Some(handle),
                    InspectionOutcome::Error,
                    "failed to parse Presidio Analyzer response",
                    "malformed_response",
                )
            }
            FailureCase::NonUtf8 => (
                vec![0xff, 0xfe],
                "http://127.0.0.1:1".to_string(),
                1_000,
                None,
                InspectionOutcome::Error,
                "request body is not UTF-8",
                "execution_failure",
            ),
        };
        let original = body.clone();
        let state = Arc::new(
            RuntimePresidioRedactionState::new(RuntimePresidioRedactionConfig {
                analyzer_url: analyzer_url.clone(),
                anonymizer_url: analyzer_url,
                languages: vec!["en".to_string()],
                language_mode: PresidioLanguageMode::Fixed,
                fail_closed: false,
                trusted_hosts: Vec::new(),
                timeout_ms,
                max_response_bytes: 1024,
                max_concurrency: 1,
            })
            .unwrap(),
        );
        let runtime = TokioRuntimeBuilder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let attempt = runtime
            .block_on(runtime_presidio_redact_body(body, state))
            .unwrap();
        let InspectionExecutionOutcome::Failed(failure) = attempt else {
            panic!("detector failure fixture must not succeed");
        };
        assert_eq!(failure.body, original);
        assert_eq!(
            runtime_inspection_error_outcome(&failure.error),
            expected_outcome
        );
        assert!(failure.error.to_string().contains(expected_error));
        assert_eq!(
            runtime_inspection_failure_type(&failure.error),
            expected_failure_type
        );
        if let Some(handle) = handle {
            handle.join().unwrap();
        }
    }
}

#[test]
fn runtime_presidio_rejects_unbounded_remote_finding_count_without_losing_content() {
    let results = (0..=prodex_domain::MAX_INSPECTION_FINDINGS)
        .map(|_| {
            serde_json::json!({
                "start": 0,
                "end": 1,
                "score": 0.9,
                "entity_type": "PERSON"
            })
        })
        .collect::<Vec<_>>();
    let response_body: &'static str =
        Box::leak(serde_json::to_string(&results).unwrap().into_boxed_str());
    let (analyzer_url, handle) =
        start_presidio_fixture(response_body, "/analyze", "bounded-findings");
    let original = b"bounded-findings".to_vec();
    let state = Arc::new(
        RuntimePresidioRedactionState::new(RuntimePresidioRedactionConfig {
            analyzer_url: analyzer_url.clone(),
            anonymizer_url: analyzer_url,
            languages: vec!["en".to_string()],
            language_mode: PresidioLanguageMode::Fixed,
            fail_closed: false,
            trusted_hosts: Vec::new(),
            timeout_ms: 1_000,
            max_response_bytes: 1024 * 1024,
            max_concurrency: 1,
        })
        .unwrap(),
    );
    let runtime = TokioRuntimeBuilder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let attempt = runtime
        .block_on(runtime_presidio_redact_body(original.clone(), state))
        .unwrap();
    let InspectionExecutionOutcome::Failed(failure) = attempt else {
        panic!("an over-limit finding response must fail");
    };
    assert_eq!(failure.body, original);
    assert!(
        failure
            .error
            .to_string()
            .contains("finding count exceeded safe limit")
    );
    handle.join().unwrap();
}

#[test]
fn presidio_unicode_scalar_offsets_become_field_byte_offsets() {
    let findings = runtime_presidio_findings(
        &[PresidioJsonString {
            path: "$.tools[0].arguments.*".to_string(),
            text: "é user@example.com".to_string(),
            sensitive_kind: None,
        }],
        "",
        &[PresidioAnalyzerResult {
            start: 2,
            end: 18,
            score: 0.99,
            entity_type: "EMAIL_ADDRESS".to_string(),
            language: "en".to_string(),
        }],
    )
    .unwrap();

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].location().byte_range(), 3..19);
    assert_eq!(
        findings[0].location().field_path(),
        "$.tools[0].arguments.*"
    );
    let detected_only = runtime_presidio_inspection_source(
        prodex_domain::InspectionCoverage::Full,
        findings,
        false,
    )
    .unwrap();
    assert!(detected_only.masked_findings.is_empty());
}

#[test]
fn disabled_personal_inspection_preserves_compatibility() {
    assert!(!runtime_local_inspection_required(
        prodex_config::GovernanceRolloutMode::Off,
        prodex_config::GovernanceMode::Personal,
        false,
        false,
    ));
    assert!(runtime_local_inspection_required(
        prodex_config::GovernanceRolloutMode::Off,
        prodex_config::GovernanceMode::Personal,
        true,
        false,
    ));
    assert!(runtime_local_inspection_required(
        prodex_config::GovernanceRolloutMode::Observe,
        prodex_config::GovernanceMode::Personal,
        false,
        false,
    ));
    assert!(runtime_local_inspection_required(
        prodex_config::GovernanceRolloutMode::Off,
        prodex_config::GovernanceMode::EnterpriseEnforce,
        false,
        false,
    ));
    assert!(
        !RuntimePresidioFailClosedPolicy::derive(
            prodex_config::GovernanceRolloutMode::Off,
            prodex_config::GovernanceMode::Personal,
            false,
            false,
            Some(false),
        )
        .is_closed()
    );
    assert!(
        RuntimePresidioFailClosedPolicy::derive(
            prodex_config::GovernanceRolloutMode::Enforce,
            prodex_config::GovernanceMode::Personal,
            false,
            false,
            Some(false),
        )
        .is_closed()
    );
}

#[test]
fn inspection_plan_pins_selected_detector_revision() {
    let revision = prodex_domain::DetectorRevisionId::new("tenant-rules-42").unwrap();

    let plan = runtime_presidio_inspection_plan(
        Vec::new(),
        prodex_domain::DataClassification::Internal,
        &revision,
    )
    .unwrap();

    assert_eq!(plan.result.detector_revision().as_str(), "tenant-rules-42");
}

#[test]
fn presidio_registry_rejects_unbounded_unique_paths_but_allows_replacement() {
    assert!(
        validate_runtime_presidio_registry_insert(MAX_RUNTIME_PRESIDIO_REGISTRY_ENTRIES, false)
            .is_err()
    );
    validate_runtime_presidio_registry_insert(MAX_RUNTIME_PRESIDIO_REGISTRY_ENTRIES, true).unwrap();
}

#[test]
fn inspection_metric_log_has_only_bounded_content_free_dimensions() {
    let metric = plan_inspection_metric(
        InspectionStage::External,
        InspectionCoverageClass::Partial,
        InspectionFindingCategory::Multiple,
        InspectionMaskingAction::Masked,
        InspectionOutcome::Timeout,
        u64::MAX,
    )
    .unwrap();
    let message = runtime_inspection_metric_message(&metric).unwrap();

    assert!(message.contains("event_metric_name=prodex_inspection_events_total"));
    assert!(message.contains("inspection_stage=external"));
    assert!(message.contains("inspection_coverage=partial"));
    assert!(message.contains("inspection_finding_category=multiple"));
    assert!(message.contains("inspection_masking_action=masked"));
    assert!(message.contains("inspection_outcome=timeout"));
    assert!(message.contains("duration_micros=120000000"));
    for forbidden in [
        "payload-secret-sentinel",
        "tenant_id",
        "request_id",
        "field_path",
        "detector_id",
    ] {
        assert!(!message.contains(forbidden), "{message}");
    }
}

fn test_governance(
    mode: prodex_config::GovernanceMode,
    inspection: prodex_config::GovernanceRolloutMode,
) -> prodex_config::GovernanceConfig {
    prodex_config::GovernanceConfig {
        mode,
        inspection,
        ..prodex_config::GovernanceConfig::personal_compatible()
    }
}

fn presidio_test_shared(
    name: &str,
    governance: prodex_config::GovernanceConfig,
) -> RuntimeRotationProxyShared {
    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "prodex-presidio-test-{name}-{}-{id}",
        std::process::id()
    ));
    let paths = AppPaths {
        state_file: root.join("state.json"),
        managed_profiles_root: root.join("profiles"),
        shared_codex_root: root.join("shared-codex"),
        legacy_shared_codex_root: root.join("shared"),
        root,
    };
    let log_path = std::env::temp_dir().join(format!(
        "prodex-presidio-test-{name}-{}-{id}.log",
        std::process::id()
    ));
    crate::runtime_core_shared::prepare_runtime_proxy_test_log_path(&log_path);
    let mut runtime_config = RuntimeConfig::compatibility_current();
    runtime_config.governance = governance;
    runtime_config.tenant_detector_patterns = Default::default();

    RuntimeRotationProxyShared {
        smart_context_engine: Arc::new(crate::RuntimeSmartContextEngine::default()),
        runtime_config: Arc::new(runtime_config),
        upstream_no_proxy: false,
        auto_redeem_enabled: false,
        async_client: reqwest::Client::new(),
        compact_client: reqwest::Client::new(),
        // `await_runtime_proxy_async_task` synchronously receives work spawned on this
        // runtime, so the fixture needs its own worker just like the production runtime.
        async_runtime: Arc::new(
            TokioRuntimeBuilder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
                .unwrap(),
        ),
        runtime: Arc::new(Mutex::new(RuntimeRotationState {
            paths,
            state: AppState::default(),
            upstream_base_url: "http://127.0.0.1".to_string(),
            include_code_review: false,
            current_profile: "test".to_string(),
            profile_usage_auth: Default::default(),
            turn_state_bindings: Default::default(),
            session_id_bindings: Default::default(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: Default::default(),
            profile_usage_snapshots: Default::default(),
            profile_retry_backoff_until: Default::default(),
            profile_transport_backoff_until: Default::default(),
            profile_route_circuit_open_until: Default::default(),
            profile_backoff_updated_at: Default::default(),
            profile_health: Default::default(),
        })),
        log_path,
        request_sequence: Arc::new(AtomicU64::new(1)),
        state_save_revision: Arc::new(AtomicU64::new(0)),
        local_overload_backoff_until: Arc::new(AtomicU64::new(0)),
        active_request_count: Arc::new(AtomicUsize::new(0)),
        active_request_limit: 8,
        runtime_state_lock_wait_counters:
            RuntimeRotationProxyShared::new_runtime_state_lock_wait_counters(),
        lane_admission: RuntimeProxyLaneAdmission::new(RuntimeProxyLaneLimits {
            responses: 8,
            compact: 8,
            websocket: 8,
            standard: 8,
        }),
    }
}

struct PresidioTestRegistration {
    log_path: PathBuf,
}

impl Drop for PresidioTestRegistration {
    fn drop(&mut self) {
        unregister_runtime_presidio_redaction_proxy_state(&self.log_path);
    }
}

fn register_test_presidio(
    shared: &RuntimeRotationProxyShared,
    analyzer_url: String,
    fail_closed: bool,
    timeout_ms: u64,
    max_concurrency: usize,
) -> PresidioTestRegistration {
    register_runtime_presidio_redaction_proxy_state(
        &shared.log_path,
        Some(RuntimePresidioRedactionConfig {
            analyzer_url: analyzer_url.clone(),
            anonymizer_url: analyzer_url,
            languages: vec!["en".to_string()],
            language_mode: PresidioLanguageMode::Fixed,
            fail_closed,
            trusted_hosts: Vec::new(),
            timeout_ms,
            max_response_bytes: 1024 * 1024,
            max_concurrency,
        }),
    )
    .unwrap();
    PresidioTestRegistration {
        log_path: shared.log_path.clone(),
    }
}

fn test_request(value: &str) -> RuntimeProxyRequest {
    RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/responses".to_string(),
        headers: Vec::new(),
        body: serde_json::json!({"input": value}).to_string().into_bytes(),
    }
}

fn test_detector_revision() -> prodex_domain::DetectorRevisionId {
    prodex_domain::DetectorRevisionId::new("runtime-inspection-v1").unwrap()
}

fn assert_denied_external_log(
    log_path: &std::path::Path,
    outcome: &str,
    coverage: &str,
    failure_type: &str,
) {
    let log = crate::runtime_core_shared::read_runtime_proxy_test_log(log_path);
    assert!(log.contains("inspection_stage=external"), "{log}");
    assert!(
        log.contains(&format!("inspection_coverage={coverage}")),
        "{log}"
    );
    assert!(log.contains("inspection_masking_action=denied"), "{log}");
    assert!(
        log.contains(&format!("inspection_outcome={outcome}")),
        "{log}"
    );
    assert!(
        log.contains("inspection_stage=request_enforcement"),
        "{log}"
    );
    assert!(
        log.contains(&format!("failure_type={failure_type}")),
        "{log}"
    );
}

#[test]
fn effective_fail_closed_policy_combines_all_enforcement_sources() {
    use prodex_config::{GovernanceMode, GovernanceRolloutMode};

    let open = RuntimePresidioFailClosedPolicy::derive(
        GovernanceRolloutMode::Observe,
        GovernanceMode::Personal,
        false,
        false,
        Some(false),
    );
    assert_eq!(open, RuntimePresidioFailClosedPolicy::Open);
    for (rollout, mode, legacy, tenant, explicit) in [
        (
            GovernanceRolloutMode::Enforce,
            GovernanceMode::Personal,
            false,
            false,
            Some(false),
        ),
        (
            GovernanceRolloutMode::Off,
            GovernanceMode::EnterpriseEnforce,
            false,
            false,
            Some(false),
        ),
        (
            GovernanceRolloutMode::Off,
            GovernanceMode::BankEnforce,
            false,
            false,
            Some(false),
        ),
        (
            GovernanceRolloutMode::Off,
            GovernanceMode::Personal,
            true,
            false,
            Some(false),
        ),
        (
            GovernanceRolloutMode::Off,
            GovernanceMode::Personal,
            false,
            true,
            Some(false),
        ),
        (
            GovernanceRolloutMode::Off,
            GovernanceMode::Personal,
            false,
            false,
            Some(true),
        ),
    ] {
        assert_eq!(
            RuntimePresidioFailClosedPolicy::derive(rollout, mode, legacy, tenant, explicit),
            RuntimePresidioFailClosedPolicy::Closed
        );
    }
}

#[test]
fn http_external_inspection_fails_closed_for_governance_and_detector_failures() {
    use prodex_config::{GovernanceMode, GovernanceRolloutMode};

    for (name, governance, explicit) in [
        (
            "governance-enforce",
            test_governance(GovernanceMode::Personal, GovernanceRolloutMode::Enforce),
            false,
        ),
        (
            "enterprise-enforce",
            test_governance(
                GovernanceMode::EnterpriseEnforce,
                GovernanceRolloutMode::Off,
            ),
            false,
        ),
        (
            "bank-enforce",
            test_governance(GovernanceMode::BankEnforce, GovernanceRolloutMode::Off),
            false,
        ),
        (
            "explicit-true",
            test_governance(GovernanceMode::Personal, GovernanceRolloutMode::Off),
            true,
        ),
    ] {
        let (analyzer_url, handle) = start_unavailable_presidio_fixture();
        let shared = presidio_test_shared(name, governance);
        let registration = register_test_presidio(&shared, analyzer_url, explicit, 1_000, 1);
        let original = test_request(name).body;
        let mut request = RuntimeProxyRequest {
            body: original.clone(),
            ..test_request(name)
        };
        let result = super::http::apply_runtime_presidio_redaction_to_request_with_rules(
            1,
            &mut request,
            &shared,
            false,
            None,
            &governance,
            &Default::default(),
            &test_detector_revision(),
        );
        assert!(result.is_err(), "{name} should not reach upstream");
        assert_eq!(request.body, original);
        assert_denied_external_log(
            &registration.log_path,
            "error",
            "unsupported",
            "unavailable",
        );
        handle.join().unwrap();
    }
}

#[test]
fn local_enforcement_does_not_require_an_unconfigured_external_service() {
    let governance = test_governance(
        prodex_config::GovernanceMode::Personal,
        prodex_config::GovernanceRolloutMode::Off,
    );
    let shared = presidio_test_shared("missing-presidio-legacy", governance);
    let mut request = test_request("legacy-local");
    let result = super::http::apply_runtime_presidio_redaction_to_request_with_rules(
        1,
        &mut request,
        &shared,
        true,
        None,
        &governance,
        &Default::default(),
        &test_detector_revision(),
    );
    assert!(result.is_ok());

    let tenant_id = TenantId::new();
    let patterns = RuntimeTenantDetectorPatterns::compile(&[RuntimePolicyInspectionPattern {
        tenant_id,
        id: "tenant-secret".to_string(),
        pattern: "tenant-secret".to_string(),
    }])
    .unwrap();
    let shared = presidio_test_shared("missing-presidio-tenant", governance);
    let mut request = test_request("tenant-secret");
    let result = super::http::apply_runtime_presidio_redaction_to_request_with_rules(
        1,
        &mut request,
        &shared,
        false,
        Some(tenant_id),
        &governance,
        &patterns,
        &test_detector_revision(),
    );
    assert!(result.is_ok());
    assert!(
        !String::from_utf8(request.body)
            .unwrap()
            .contains("tenant-secret")
    );
}

#[test]
fn http_external_timeout_malformed_and_concurrency_fail_closed() {
    let cases = [
        ("timeout", "[]", 10, 1, Duration::from_millis(100)),
        ("malformed", "not-json", 1_000, 1, Duration::ZERO),
    ];
    for (name, response, timeout_ms, max_concurrency, delay) in cases {
        let (analyzer_url, handle) =
            start_presidio_fixture_with_delay(response, "/analyze", name, delay);
        let governance = test_governance(
            prodex_config::GovernanceMode::Personal,
            prodex_config::GovernanceRolloutMode::Enforce,
        );
        let shared = presidio_test_shared(name, governance);
        let registration =
            register_test_presidio(&shared, analyzer_url, false, timeout_ms, max_concurrency);
        let original = test_request(name).body;
        let mut request = RuntimeProxyRequest {
            body: original.clone(),
            ..test_request(name)
        };
        let result = super::http::apply_runtime_presidio_redaction_to_request_with_rules(
            1,
            &mut request,
            &shared,
            false,
            None,
            &governance,
            &Default::default(),
            &test_detector_revision(),
        );
        assert!(result.is_err(), "{name} should fail closed");
        assert_eq!(request.body, original);
        let expected_outcome = if name == "timeout" {
            "timeout"
        } else {
            "error"
        };
        let failure_type = if name == "timeout" {
            "timeout"
        } else {
            "malformed_response"
        };
        assert_denied_external_log(
            &registration.log_path,
            expected_outcome,
            "unsupported",
            failure_type,
        );
        handle.join().unwrap();
    }

    let governance = test_governance(
        prodex_config::GovernanceMode::Personal,
        prodex_config::GovernanceRolloutMode::Enforce,
    );
    let shared = presidio_test_shared("concurrency", governance);
    let registration =
        register_test_presidio(&shared, "http://127.0.0.1:1".to_string(), false, 1_000, 0);
    let original = test_request("concurrency").body;
    let mut request = RuntimeProxyRequest {
        body: original.clone(),
        ..test_request("concurrency")
    };
    let result = super::http::apply_runtime_presidio_redaction_to_request_with_rules(
        1,
        &mut request,
        &shared,
        false,
        None,
        &governance,
        &Default::default(),
        &test_detector_revision(),
    );
    assert!(result.is_err());
    assert_eq!(request.body, original);
    assert_denied_external_log(
        &registration.log_path,
        "error",
        "unsupported",
        "concurrency_exhaustion",
    );
}

#[test]
fn http_external_partial_coverage_fails_closed_without_upstream() {
    let (analyzer_url, handle) = start_presidio_fixture("[]", "/analyze", "partial-coverage");
    let governance = test_governance(
        prodex_config::GovernanceMode::Personal,
        prodex_config::GovernanceRolloutMode::Enforce,
    );
    let shared = presidio_test_shared("partial-http", governance);
    let registration = register_test_presidio(&shared, analyzer_url, false, 1_000, 1);
    let mut request = RuntimeProxyRequest {
        body: serde_json::json!({
            "input": "partial-coverage",
            "input_image": {"url": "https://example.com/synthetic.png"}
        })
        .to_string()
        .into_bytes(),
        ..test_request("partial-coverage")
    };
    let result = super::http::apply_runtime_presidio_redaction_to_request_with_rules(
        1,
        &mut request,
        &shared,
        false,
        None,
        &governance,
        &Default::default(),
        &test_detector_revision(),
    );
    assert!(result.is_err());
    assert_denied_external_log(
        &registration.log_path,
        "denied",
        "partial",
        "unsupported_coverage",
    );
    handle.join().unwrap();
}

#[test]
fn http_and_websocket_observe_fail_open_with_explicit_false() {
    let governance = test_governance(
        prodex_config::GovernanceMode::EnterpriseObserve,
        prodex_config::GovernanceRolloutMode::Observe,
    );
    let shared = presidio_test_shared("observe-http", governance);
    let registration =
        register_test_presidio(&shared, "http://127.0.0.1:1".to_string(), false, 1_000, 1);
    let original = test_request("observe").body;
    let mut request = RuntimeProxyRequest {
        body: original.clone(),
        ..test_request("observe")
    };
    let result = super::http::apply_runtime_presidio_redaction_to_request_with_rules(
        1,
        &mut request,
        &shared,
        false,
        None,
        &governance,
        &Default::default(),
        &test_detector_revision(),
    )
    .unwrap();
    assert_eq!(request.body, original);
    assert_eq!(
        result.result.coverage(),
        prodex_domain::InspectionCoverage::Partial
    );
    let log = crate::runtime_core_shared::read_runtime_proxy_test_log(&registration.log_path);
    assert!(log.contains("inspection_masking_action=none"), "{log}");
    assert!(
        !log.contains("inspection_stage=request_enforcement"),
        "{log}"
    );

    let shared = presidio_test_shared("observe-websocket", governance);
    let registration =
        register_test_presidio(&shared, "http://127.0.0.1:1".to_string(), false, 1_000, 1);
    let text = test_request("observe-websocket");
    let inspected =
        super::websocket::apply_runtime_presidio_redaction_to_websocket_text_with_rules(
            1,
            std::str::from_utf8(&text.body).unwrap(),
            &shared,
            false,
            None,
            &governance,
            &Default::default(),
            &test_detector_revision(),
        )
        .unwrap();
    assert_eq!(
        inspected.text.as_ref(),
        std::str::from_utf8(&text.body).unwrap()
    );
    let log = crate::runtime_core_shared::read_runtime_proxy_test_log(&registration.log_path);
    assert!(log.contains("inspection_masking_action=none"), "{log}");
    assert!(
        !log.contains("inspection_stage=request_enforcement"),
        "{log}"
    );
}

#[test]
fn websocket_external_failures_and_partial_coverage_fail_closed() {
    use prodex_config::{GovernanceMode, GovernanceRolloutMode};

    for (name, governance, explicit) in [
        (
            "websocket-governance-enforce",
            test_governance(GovernanceMode::Personal, GovernanceRolloutMode::Enforce),
            false,
        ),
        (
            "websocket-enterprise-enforce",
            test_governance(
                GovernanceMode::EnterpriseEnforce,
                GovernanceRolloutMode::Off,
            ),
            false,
        ),
        (
            "websocket-bank-enforce",
            test_governance(GovernanceMode::BankEnforce, GovernanceRolloutMode::Off),
            false,
        ),
        (
            "websocket-explicit-true",
            test_governance(GovernanceMode::Personal, GovernanceRolloutMode::Off),
            true,
        ),
    ] {
        let (analyzer_url, handle) = start_unavailable_presidio_fixture();
        let shared = presidio_test_shared(name, governance);
        let registration = register_test_presidio(&shared, analyzer_url, explicit, 1_000, 1);
        let text = test_request(name);
        let body = std::str::from_utf8(&text.body).unwrap();
        let result =
            super::websocket::apply_runtime_presidio_redaction_to_websocket_text_with_rules(
                1,
                body,
                &shared,
                false,
                None,
                &governance,
                &Default::default(),
                &test_detector_revision(),
            );
        assert!(result.is_err(), "{name} should not reach upstream");
        assert_denied_external_log(
            &registration.log_path,
            "error",
            "unsupported",
            "unavailable",
        );
        handle.join().unwrap();
    }

    let (analyzer_url, handle) = start_presidio_fixture_with_delay(
        "not-json",
        "/analyze",
        "websocket-malformed",
        Duration::ZERO,
    );
    let governance = test_governance(GovernanceMode::Personal, GovernanceRolloutMode::Enforce);
    let shared = presidio_test_shared("websocket-malformed", governance);
    let registration = register_test_presidio(&shared, analyzer_url, false, 1_000, 1);
    let text = test_request("websocket-malformed");
    let result = super::websocket::apply_runtime_presidio_redaction_to_websocket_text_with_rules(
        1,
        std::str::from_utf8(&text.body).unwrap(),
        &shared,
        false,
        None,
        &governance,
        &Default::default(),
        &test_detector_revision(),
    );
    assert!(result.is_err());
    assert_denied_external_log(
        &registration.log_path,
        "error",
        "unsupported",
        "malformed_response",
    );
    handle.join().unwrap();

    let shared = presidio_test_shared("websocket-concurrency", governance);
    let registration =
        register_test_presidio(&shared, "http://127.0.0.1:1".to_string(), false, 1_000, 0);
    let text = test_request("websocket-concurrency");
    let result = super::websocket::apply_runtime_presidio_redaction_to_websocket_text_with_rules(
        1,
        std::str::from_utf8(&text.body).unwrap(),
        &shared,
        false,
        None,
        &governance,
        &Default::default(),
        &test_detector_revision(),
    );
    assert!(result.is_err());
    assert_denied_external_log(
        &registration.log_path,
        "error",
        "unsupported",
        "concurrency_exhaustion",
    );

    let (analyzer_url, handle) = start_presidio_fixture("[]", "/analyze", "websocket-partial");
    let governance = test_governance(GovernanceMode::Personal, GovernanceRolloutMode::Enforce);
    let shared = presidio_test_shared("websocket-partial", governance);
    let registration = register_test_presidio(&shared, analyzer_url, false, 1_000, 1);
    let text = serde_json::json!({
        "input": "websocket-partial",
        "input_image": {"url": "https://example.com/synthetic.png"}
    })
    .to_string();
    let result = super::websocket::apply_runtime_presidio_redaction_to_websocket_text_with_rules(
        1,
        &text,
        &shared,
        false,
        None,
        &governance,
        &Default::default(),
        &test_detector_revision(),
    );
    assert!(result.is_err());
    assert_denied_external_log(
        &registration.log_path,
        "denied",
        "partial",
        "unsupported_coverage",
    );
    handle.join().unwrap();
}
