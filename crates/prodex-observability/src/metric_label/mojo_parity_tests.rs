use super::{TelemetryAttribute, validate_telemetry_metric_label_rust};

fn assert_mojo_matches_rust(key: &str, value: &str) {
    let expected = validate_telemetry_metric_label_rust(key, value);
    let actual = TelemetryAttribute::metric_label(key, value)
        .as_metric_label()
        .map(|_| ());
    assert_eq!(actual, expected, "key={key:?}, value={value:?}");
}

#[test]
fn metric_label_mojo_policy_matches_rust_boundaries() {
    let keys = [
        "provider",
        "Trace.Id",
        "status-class",
        "tenant_id",
        "Tenant-ID",
        "prefix.request-id.suffix",
        "user_id_hint",
        "promptSize",
        "virtual-key_count",
        "api.key_type",
        "",
        "contains space",
        "contains\nnewline",
        "non-ascii-é",
    ];
    let values = [
        "openai",
        "low_cardinality",
        "~",
        "",
        "contains space",
        "contains\nnewline",
        "018f0000-0000-7000-8000-000000000001",
        "018f0000000070008000000000000001",
        "018f0000-0000-7000_8000-000000000001",
        "abcdef0123456789abcdef0123456789",
        "abcdef0123456789abcdef012345678g",
        "non-ascii-é",
    ];
    for key in keys {
        for value in values {
            assert_mojo_matches_rust(key, value);
        }
    }

    for length in 0..=130 {
        let text = "a".repeat(length);
        assert_mojo_matches_rust(&text, "safe");
        assert_mojo_matches_rust("provider", &text);
    }

    for byte in 0..=u8::MAX {
        let text = char::from(byte).to_string();
        assert_mojo_matches_rust(&text, &text);
    }
}
