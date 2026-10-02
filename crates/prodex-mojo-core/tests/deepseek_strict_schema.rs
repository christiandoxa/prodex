#![cfg(feature = "mojo-rich")]

use prodex_mojo_core::rich::{DeepSeekKernelInput, DeepSeekKernelOperation, deepseek_kernel};

fn normalize(schema: &str) -> Result<Vec<u8>, prodex_mojo_core::MojoError> {
    let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::StrictFunctionSchema);
    input.input = Some(schema);
    deepseek_kernel(input)
}

#[test]
fn strict_schema_operation_matches_rust_validator_acceptance_cases() {
    let cases = [
        (
            "unsupported pattern keyword",
            r#"{"type":"object","properties":{"query":{"type":"string","pattern":"x"}}}"#,
            false,
        ),
        ("non-array enum", r#"{"enum":"x"}"#, false),
        ("non-string type", r#"{"type":1}"#, false),
        ("non-array anyOf", r#"{"anyOf":{}}"#, false),
        (
            "anyOf still rejects unsupported siblings",
            r#"{"anyOf":[],"pattern":"x"}"#,
            false,
        ),
        ("unsupported type", r#"{"type":"null"}"#, false),
        ("array without items", r#"{"type":"array"}"#, false),
        ("non-object root", "[]", false),
        (
            "non-object nested schema",
            r#"{"type":"array","items":null}"#,
            false,
        ),
        (
            "non-object property schema",
            r#"{"type":"object","properties":{"query":"string"}}"#,
            false,
        ),
        (
            "non-object properties",
            r#"{"type":"object","properties":[]}"#,
            false,
        ),
        ("non-object anyOf item", r#"{"anyOf":["string"]}"#, false),
        (
            "nested array and anyOf",
            r#"{"type":"object","properties":{"lookup":{"anyOf":[{"type":"array","items":{"type":"object","properties":{"count":{"type":"integer"}}}},{"type":"string"}]}}}"#,
            true,
        ),
        (
            "type defaults to object",
            r#"{"properties":{"query":{"type":"string"}}}"#,
            true,
        ),
        ("empty anyOf", r#"{"anyOf":[]}"#, true),
        (
            "anyOf returns before sibling value checks",
            r#"{"anyOf":[{"type":"string"}],"enum":"ignored","type":42,"properties":[],"items":null}"#,
            true,
        ),
        (
            "primitive ignores properties and items contents",
            r#"{"type":"string","properties":[],"items":null}"#,
            true,
        ),
    ];

    for (name, schema, expected) in cases {
        let result = normalize(schema);
        assert_eq!(result.is_ok(), expected, "{name}: {result:?}");
    }
}
