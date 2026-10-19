use super::*;

fn function(name: &str, parameters: serde_json::Value) -> serde_json::Value {
    serde_json::json!({"type":"function","function":{"name":name,"parameters":parameters}})
}

#[test]
fn compiled_function_tools_preserve_replacement_position_passthrough_and_error_precedence() {
    let generic = function(
        "lookup",
        serde_json::json!({"type":"object","additionalProperties":true}),
    );
    let specific = function(
        "lookup",
        serde_json::json!({"type":"object","properties":{"q":{"type":"string"}}}),
    );
    let other = function("other", serde_json::json!({}));
    let opaque = serde_json::json!({"unknown": "kept"});
    assert_eq!(
        deepseek_provider_core_dedup_and_validate_function_tools(
            vec![
                generic.clone(),
                other.clone(),
                opaque.clone(),
                generic.clone(),
                specific.clone(),
                generic.clone()
            ],
            false,
            "Kiro"
        )
        .unwrap(),
        vec![other, opaque, specific.clone()]
    );
    let different = function(
        "lookup",
        serde_json::json!({"properties":{"q":{"type":"number"}}}),
    );
    assert_eq!(
        deepseek_provider_core_dedup_and_validate_function_tools(
            vec![
                specific.clone(),
                different,
                function("bad.name", serde_json::json!([]))
            ],
            false,
            "Gemini"
        )
        .unwrap_err(),
        "Gemini function tool name `lookup` is duplicated after translation"
    );
    assert_eq!(
        deepseek_provider_core_dedup_and_validate_function_tools(
            vec![function("bad.name", serde_json::json!([])), specific],
            false,
            "Gemini"
        )
        .unwrap_err(),
        "Gemini function tool names must use only letters, numbers, underscores, or dashes and be at most 64 bytes"
    );
}

#[test]
fn compiled_function_tools_compare_normalized_schema_and_canonical_numeric_values() {
    let missing = serde_json::json!({"function":{"name":"lookup"}});
    let explicit = serde_json::json!({"function":{"name":"lookup","strict":false,"parameters":{"type":"object","properties":{},"required":[],"additionalProperties":true}}});
    let normalized = deepseek_provider_core_dedup_and_validate_function_tools(
        vec![missing, explicit],
        true,
        "DeepSeek",
    )
    .unwrap();
    assert_eq!(
        normalized,
        vec![
            serde_json::json!({"function":{"name":"lookup","strict":true,"parameters":{"type":"object","properties":{},"required":[],"additionalProperties":false}}})
        ]
    );
    for strict in [false, true] {
        let a = function("zero", serde_json::json!({"type":"number","enum":[-0.0]}));
        let b = function("zero", serde_json::json!({"type":"number","enum":[0.0]}));
        assert_eq!(
            deepseek_provider_core_dedup_and_validate_function_tools(
                vec![a, b],
                strict,
                "DeepSeek"
            )
            .unwrap()
            .len(),
            1
        );
        let a = function("zero", serde_json::json!({"type":"number","enum":[0]}));
        let b = function("zero", serde_json::json!({"type":"number","enum":[0.0]}));
        assert!(
            deepseek_provider_core_dedup_and_validate_function_tools(
                vec![a, b],
                strict,
                "DeepSeek"
            )
            .unwrap_err()
            .contains("duplicated after translation")
        );
    }
}

#[test]
fn compiled_function_tool_name_limits_and_object_name_precedence_reach_public_callers() {
    for (limit, length, accepted) in [
        (0, 0, false),
        (64, 64, true),
        (64, 65, false),
        (128, 128, true),
        (128, 129, false),
        (usize::MAX, 129, true),
    ] {
        let name = "a".repeat(length);
        assert_eq!(
            deepseek_provider_core_validate_function_name_with_max_bytes(&name, "Gemini", limit)
                .is_ok(),
            accepted
        );
    }
    for name in ["bad.name", "héllo", "with space", "", "\0"] {
        assert!(deepseek_provider_core_validate_function_name(name, "DeepSeek").is_err());
    }
    for (value, expected) in [
        (
            serde_json::json!({"name":" ","function":{"name":"nested"}}),
            None,
        ),
        (
            serde_json::json!({"name":7,"function":{"name":"nested"}}),
            Some("nested"),
        ),
        (
            serde_json::json!({"name":"direct","function":{"name":"nested"}}),
            Some("direct"),
        ),
    ] {
        assert_eq!(
            deepseek_provider_core_tool_name_from_tool_object(value.as_object().unwrap())
                .as_deref(),
            expected
        );
    }
}

#[test]
fn compiled_function_tools_preserve_generic_scalar_schema_and_large_stable_order() {
    let generic = function(
        "lookup",
        serde_json::json!({"type":"string","additionalProperties":true}),
    );
    let specific = function("lookup", serde_json::json!({"type":"string","enum":["a"]}));
    for strict in [false, true] {
        let output = deepseek_provider_core_dedup_and_validate_function_tools(
            vec![generic.clone(), specific.clone(), generic.clone()],
            strict,
            "DeepSeek",
        )
        .unwrap();
        assert_eq!(output.len(), 1);
        assert_eq!(
            output[0]["function"]["parameters"]["enum"],
            serde_json::json!(["a"])
        );
    }
    let tools = (0..2048)
        .map(|index| function(&format!("tool_{index:04}"), serde_json::json!({})))
        .collect::<Vec<_>>();
    assert_eq!(
        deepseek_provider_core_dedup_and_validate_function_tools(tools.clone(), false, "DeepSeek")
            .unwrap(),
        tools
    );
}

#[test]
fn compiled_function_tool_choice_and_escaped_diagnostics_preserve_public_contract() {
    let provider = "Example \"provider\"";
    let name = "lookup\nname";
    let tool = function(name, serde_json::json!([]));
    assert_eq!(
        deepseek_provider_core_validate_function_parameters(&tool, name, provider).unwrap_err(),
        format!("{provider} function tool `{name}` parameters must be an object")
    );
    let choice = serde_json::json!({"function":{"name":"lookup"}});
    let names = std::collections::BTreeSet::from(["other".to_string()]);
    assert_eq!(
        deepseek_provider_core_validate_tool_choice_target(&choice, &names, "Kiro").unwrap_err(),
        "Kiro named tool_choice `lookup` does not match any translated function tool"
    );
    deepseek_provider_core_validate_tool_choice_target(
        &choice,
        &std::collections::BTreeSet::from(["lookup".to_string()]),
        "Kiro",
    )
    .unwrap();
}

#[test]
fn compiled_strict_schema_preserves_the_existing_per_schema_byte_limit() {
    let mut schema = serde_json::json!({"type":"object","description":""});
    let size = serde_json::to_vec(&schema).unwrap().len();
    schema["description"] =
        serde_json::json!("x".repeat(prodex_mojo_core::rich::DEEPSEEK_KERNEL_MAX_BYTES - size));
    let mut tool = function("lookup", schema.clone());
    deepseek_provider_core_apply_strict_function_schema(&mut tool, "DeepSeek").unwrap();
    let mut description = schema["description"].as_str().unwrap().to_string();
    description.push('x');
    schema["description"] = serde_json::json!(description);
    let mut too_large = function("lookup", schema);
    assert_eq!(
        deepseek_provider_core_apply_strict_function_schema(&mut too_large, "DeepSeek")
            .unwrap_err(),
        "DeepSeek strict tool schema `lookup` could not be normalized"
    );
}
