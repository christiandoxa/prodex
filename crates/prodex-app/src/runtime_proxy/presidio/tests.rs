use super::engine::{
    InspectionExecutionOutcome, runtime_local_inspection_required, runtime_presidio_redact_body,
};
use super::findings::{
    PresidioAnalyzerResult, runtime_presidio_findings, runtime_presidio_inspection_plan,
    runtime_presidio_inspection_source,
};
use super::json_body::PresidioJsonString;
use super::registry::{
    MAX_RUNTIME_PRESIDIO_REGISTRY_ENTRIES, RuntimePresidioRedactionState,
    validate_runtime_presidio_registry_insert,
};
use super::telemetry::{runtime_inspection_error_outcome, runtime_inspection_metric_message};
use crate::RuntimePresidioRedactionConfig;
use crate::presidio_runtime::PresidioLanguageMode;
use prodex_observability::{
    InspectionCoverageClass, InspectionFindingCategory, InspectionMaskingAction, InspectionOutcome,
    InspectionStage, plan_inspection_metric,
};
use std::net::TcpListener;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tiny_http::{Header as TinyHeader, Response as TinyResponse, Server as TinyServer};
use tokio::runtime::Builder as TokioRuntimeBuilder;

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
        let (body, analyzer_url, timeout_ms, handle, expected_outcome, expected_error) = match case
        {
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
                )
            }
            FailureCase::Unavailable => {
                let listener = TcpListener::bind("127.0.0.1:0").unwrap();
                let url = format!("http://{}", listener.local_addr().unwrap());
                drop(listener);
                (
                    br#"{"input":"synthetic-unavailable"}"#.to_vec(),
                    url,
                    1_000,
                    None,
                    InspectionOutcome::Error,
                    "failed to call Presidio Analyzer",
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
                )
            }
            FailureCase::NonUtf8 => (
                vec![0xff, 0xfe],
                "http://127.0.0.1:1".to_string(),
                1_000,
                None,
                InspectionOutcome::Error,
                "request body is not UTF-8",
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
fn presidio_rejects_malformed_finding_metadata() {
    let error = runtime_presidio_findings(
        &[PresidioJsonString {
            path: "$.input".to_string(),
            text: "safe".to_string(),
            sensitive_kind: None,
        }],
        "",
        &[PresidioAnalyzerResult {
            start: 0,
            end: 4,
            score: f64::NAN,
            entity_type: "PERSON".to_string(),
            language: "en".to_string(),
        }],
    )
    .unwrap_err();

    assert!(error.to_string().contains("invalid confidence score"));
}

#[test]
fn disabled_personal_inspection_preserves_compatibility() {
    assert!(!runtime_local_inspection_required(
        prodex_config::GovernanceRolloutMode::Off,
        false,
        false,
    ));
    assert!(runtime_local_inspection_required(
        prodex_config::GovernanceRolloutMode::Off,
        true,
        false,
    ));
    assert!(runtime_local_inspection_required(
        prodex_config::GovernanceRolloutMode::Observe,
        false,
        false,
    ));
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
