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

    for key in [
        "Tenant-ID",
        "prefix.request-id.suffix",
        "user_id_hint",
        "promptSize",
        "virtual-key_count",
        "api.key_type",
    ] {
        assert_eq!(
            TelemetryAttribute::metric_label(key, "safe").as_metric_label(),
            Err(TelemetryAttributeError::InvalidKey),
            "normalized sensitive key should be rejected: {key}"
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
fn metric_label_limits_and_ascii_rules_have_exact_boundaries() {
    let key = "a".repeat(128);
    assert_eq!(
        TelemetryAttribute::metric_label(&key, "safe").as_metric_label(),
        Ok((key.as_str(), "safe"))
    );
    let key = "a".repeat(129);
    assert_eq!(
        TelemetryAttribute::metric_label(&key, "safe").as_metric_label(),
        Err(TelemetryAttributeError::InvalidKey)
    );

    let value = "a".repeat(128);
    assert_eq!(
        TelemetryAttribute::metric_label("provider", &value).as_metric_label(),
        Ok(("provider", value.as_str()))
    );
    let value = "a".repeat(129);
    assert_eq!(
        TelemetryAttribute::metric_label("provider", &value).as_metric_label(),
        Err(TelemetryAttributeError::InvalidValue)
    );

    for key in ["", "contains space", "contains\nnewline", "non-ascii-é"] {
        assert_eq!(
            TelemetryAttribute::metric_label(key, "safe").as_metric_label(),
            Err(TelemetryAttributeError::InvalidKey),
            "invalid key should be rejected: {key:?}"
        );
    }
    for value in ["contains space", "contains\nnewline", "non-ascii-é"] {
        assert_eq!(
            TelemetryAttribute::metric_label("provider", value).as_metric_label(),
            Err(TelemetryAttributeError::InvalidValue),
            "invalid value should be rejected: {value:?}"
        );
    }
}

#[test]
fn label_validation_keeps_precedence_and_identifier_near_misses() {
    assert_eq!(
        TelemetryAttribute::metric_label("request_id", "").as_metric_label(),
        Err(TelemetryAttributeError::InvalidKey)
    );
    assert_eq!(
        TelemetryAttribute::metric_label("Trace.Id", "~").as_metric_label(),
        Ok(("Trace.Id", "~"))
    );
    assert_eq!(
        TelemetryAttribute::metric_label("provider", "018f0000-0000-7000_8000-000000000001")
            .as_metric_label(),
        Ok(("provider", "018f0000-0000-7000_8000-000000000001"))
    );
    assert_eq!(
        TelemetryAttribute::metric_label("provider", "abcdef0123456789abcdef012345678g")
            .as_metric_label(),
        Ok(("provider", "abcdef0123456789abcdef012345678g"))
    );
}

#[test]
fn debug_output_redacts_metric_value() {
    let attribute = TelemetryAttribute::metric_label("provider", "openai");
    let rendered = format!("{attribute:?}");
    assert!(rendered.contains("provider"));
    assert!(!rendered.contains("openai"));
    assert!(rendered.contains("<redacted>"));
}
