use super::{
    deepseek_provider_core_apply_reasoning_from_responses_request,
    deepseek_provider_core_thinking_enabled, deepseek_provider_core_validate_reasoning_shape,
};

#[test]
fn deepseek_provider_core_reasoning_preserves_wire_mapping_and_errors() {
    let cases = [
        (
            serde_json::json!({"reasoning": {"effort": "\u{2003}XHIGH\u{2003}"}}),
            false,
            serde_json::json!({"reasoning_effort": "max", "thinking": {"type": "enabled"}}),
            true,
        ),
        (
            serde_json::json!({"reasoning_effort": "medium"}),
            false,
            serde_json::json!({"reasoning_effort": "high", "thinking": {"type": "enabled"}}),
            true,
        ),
        (
            serde_json::json!({"reasoning_effort": "low"}),
            false,
            serde_json::json!({"reasoning_effort": "high", "thinking": {"type": "enabled"}}),
            true,
        ),
        (
            serde_json::json!({"reasoning_effort": "minimal"}),
            false,
            serde_json::json!({"thinking": {"type": "disabled"}}),
            false,
        ),
        (
            serde_json::json!({"reasoning_effort": "none"}),
            false,
            serde_json::json!({"thinking": {"type": "disabled"}}),
            false,
        ),
        (
            serde_json::json!({"reasoning": {}, "reasoning_effort": "max"}),
            false,
            serde_json::json!({"reasoning_effort": "max", "thinking": {"type": "enabled"}}),
            true,
        ),
        (
            serde_json::json!({"reasoning_effort": "xhigh"}),
            true,
            serde_json::json!({"reasoning_effort": "high"}),
            true,
        ),
        (
            serde_json::json!({"reasoning_effort": "medium"}),
            true,
            serde_json::json!({"reasoning_effort": "medium"}),
            true,
        ),
        (
            serde_json::json!({"reasoning_effort": "low"}),
            true,
            serde_json::json!({"reasoning_effort": "low"}),
            true,
        ),
        (
            serde_json::json!({"reasoning_effort": "minimal"}),
            true,
            serde_json::json!({"reasoning_effort": "minimal"}),
            false,
        ),
        (
            serde_json::json!({"reasoning_effort": "none"}),
            true,
            serde_json::json!({"reasoning_effort": "none"}),
            false,
        ),
        (serde_json::json!({}), false, serde_json::json!({}), false),
    ];

    for (value, gemini_compat, expected, expected_thinking) in cases {
        let mut request = serde_json::Map::new();
        deepseek_provider_core_apply_reasoning_from_responses_request(
            &value,
            &mut request,
            "Test provider",
            gemini_compat,
        )
        .unwrap();

        assert_eq!(serde_json::Value::Object(request), expected, "{value}");
        assert_eq!(
            deepseek_provider_core_thinking_enabled(&value),
            expected_thinking,
            "{value}"
        );
    }

    for (value, expected) in [
        (
            serde_json::json!({"reasoning": null}),
            "Test provider reasoning must be an object",
        ),
        (
            serde_json::json!({"reasoning": []}),
            "Test provider reasoning must be an object",
        ),
        (
            serde_json::json!({"reasoning": {"effort": null}}),
            "Test provider reasoning.effort must be a string",
        ),
        (
            serde_json::json!({"reasoning_effort": 1}),
            "Test provider reasoning_effort must be a string",
        ),
        (
            serde_json::json!({"reasoning": {"summary": "auto"}}),
            "Test provider reasoning.summary is not supported by this Responses adapter",
        ),
    ] {
        assert_eq!(
            deepseek_provider_core_validate_reasoning_shape(&value, "Test provider").unwrap_err(),
            expected
        );
        let mut request = serde_json::Map::new();
        assert_eq!(
            deepseek_provider_core_apply_reasoning_from_responses_request(
                &value,
                &mut request,
                "Test provider",
                false,
            )
            .unwrap_err(),
            expected
        );
        assert!(request.is_empty());
        assert!(!deepseek_provider_core_thinking_enabled(&value));
    }
}

#[test]
fn deepseek_provider_core_reasoning_preserves_nonobject_and_bounds_input() {
    for value in [
        serde_json::Value::Null,
        serde_json::json!([]),
        serde_json::json!("request"),
        serde_json::json!(1),
    ] {
        let mut request = serde_json::Map::new();
        deepseek_provider_core_apply_reasoning_from_responses_request(
            &value,
            &mut request,
            "Test provider",
            false,
        )
        .unwrap();
        assert!(request.is_empty());
        assert_eq!(
            deepseek_provider_core_validate_reasoning_shape(&value, "Test provider"),
            Ok(())
        );
        assert!(!deepseek_provider_core_thinking_enabled(&value));
    }

    let large_unrelated_input = serde_json::json!({
        "reasoning_effort": "high",
        "padding": "雪".repeat(prodex_mojo_core::rich::DEEPSEEK_KERNEL_MAX_BYTES / 3 + 1)
    });
    assert_eq!(
        deepseek_provider_core_validate_reasoning_shape(&large_unrelated_input, "Test provider"),
        Ok(())
    );
    assert!(deepseek_provider_core_thinking_enabled(
        &large_unrelated_input
    ));
    let mut request = serde_json::Map::new();
    deepseek_provider_core_apply_reasoning_from_responses_request(
        &large_unrelated_input,
        &mut request,
        "Test provider",
        false,
    )
    .unwrap();
    assert_eq!(
        serde_json::Value::Object(request),
        serde_json::json!({"reasoning_effort": "high", "thinking": {"type": "enabled"}})
    );

    let oversized = serde_json::json!({
        "reasoning_effort": "x".repeat(prodex_mojo_core::rich::DEEPSEEK_KERNEL_MAX_BYTES)
    });
    let expected = format!(
        "Test provider request policy input exceeds {} bytes",
        prodex_mojo_core::rich::DEEPSEEK_KERNEL_MAX_BYTES
    );
    assert_eq!(
        deepseek_provider_core_validate_reasoning_shape(&oversized, "Test provider").unwrap_err(),
        expected
    );
    assert_eq!(
        deepseek_provider_core_apply_reasoning_from_responses_request(
            &oversized,
            &mut serde_json::Map::new(),
            "Test provider",
            false,
        )
        .unwrap_err(),
        expected
    );
    assert!(!deepseek_provider_core_thinking_enabled(&oversized));

    for (label, gemini_compat) in [("Test provider", false), ("Test Gemini", true)] {
        let value = serde_json::json!({"reasoning_effort": "unsupported"});
        assert_eq!(
            deepseek_provider_core_apply_reasoning_from_responses_request(
                &value,
                &mut serde_json::Map::new(),
                label,
                gemini_compat,
            )
            .unwrap_err(),
            format!("{label} reasoning effort is not supported")
        );
    }
}
