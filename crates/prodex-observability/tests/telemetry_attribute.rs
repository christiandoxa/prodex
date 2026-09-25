use prodex_observability::{TelemetryAttribute, TelemetryAttributeError};

#[test]
fn safe_low_cardinality_metric_label_round_trips() {
    let attribute = TelemetryAttribute::metric_label("provider", "openai");
    assert_eq!(attribute.as_metric_label(), Ok(("provider", "openai")));
}

#[test]
fn sensitive_identifier_keys_are_rejected() {
    for key in [
        "tenant_id",
        "user_id",
        "principal_id",
        "request_id",
        "call_id",
        "virtual_key",
        "api_key",
        "prompt",
    ] {
        let attribute = TelemetryAttribute::metric_label(key, "safe");
        assert_eq!(
            attribute.as_metric_label(),
            Err(TelemetryAttributeError::InvalidKey),
            "sensitive key should be rejected: {key}"
        );
    }
}

#[test]
fn identifier_shaped_and_non_graphic_values_are_rejected() {
    for value in [
        "018f0000-0000-7000-8000-000000000001",
        "abcdef0123456789abcdef0123456789",
        "contains space",
        "",
    ] {
        let attribute = TelemetryAttribute::metric_label("provider", value);
        assert_eq!(
            attribute.as_metric_label(),
            Err(TelemetryAttributeError::InvalidValue),
            "high-cardinality or unsafe value should be rejected: {value:?}"
        );
    }
}

#[test]
fn debug_output_redacts_metric_value() {
    let attribute = TelemetryAttribute::metric_label("provider", "openai");
    let rendered = format!("{attribute:?}");
    assert!(rendered.contains("provider"));
    assert!(!rendered.contains("openai"));
    assert!(rendered.contains("<redacted>"));
}

#[cfg(feature = "mojo")]
#[test]
fn telemetry_metric_label_validation_uses_compiled_mojo() {
    assert_eq!(
        TelemetryAttribute::metric_label("provider", "openai").as_metric_label(),
        Ok(("provider", "openai"))
    );
}
