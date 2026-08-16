use super::{
    InspectionExecutionOutcome, RuntimePresidioRedactionConfig, RuntimePresidioRedactionState,
    runtime_presidio_redact_body, start_presidio_fixture,
};
use crate::presidio_runtime::PresidioLanguageMode;
use std::sync::Arc;
use tokio::runtime::Builder as TokioRuntimeBuilder;

#[test]
fn runtime_presidio_redact_body_rejects_noop_anonymizer_with_findings() {
    let (analyzer_url, analyzer_handle) = start_presidio_fixture(
        r#"[{"start":8,"end":24,"score":0.99,"entity_type":"EMAIL_ADDRESS"}]"#,
        "/analyze",
        "user@example.com",
    );
    let (anonymizer_url, anonymizer_handle) = start_presidio_fixture(
        r#"{"text":"contact user@example.com"}"#,
        "/anonymize",
        "user@example.com",
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
    let original = br#"{"type":"response.create","input":"contact user@example.com"}"#;
    let runtime = TokioRuntimeBuilder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let attempt = runtime
        .block_on(runtime_presidio_redact_body(original.to_vec(), state))
        .unwrap();
    let InspectionExecutionOutcome::Failed(failure) = attempt else {
        panic!("a no-op anonymizer response must fail inspection");
    };
    assert_eq!(failure.body, original);
    assert!(failure.error.to_string().contains("unchanged text"));
    analyzer_handle.join().unwrap();
    anonymizer_handle.join().unwrap();
}
