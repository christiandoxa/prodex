use super::deepseek_provider_core_apply_strict_function_schema;
use serde_json::json;

#[test]
fn normalizes_nested_any_of_and_array_schemas() {
    let mut tool = json!({
        "function": {
            "name": "lookup",
            "parameters": {
                "anyOf": [
                    {"type": "object", "properties": {"query": {"type": "string"}}},
                    {"type": "array", "items": {"properties": {"count": {"type": "integer"}}}}
                ]
            }
        }
    });
    deepseek_provider_core_apply_strict_function_schema(&mut tool, "DeepSeek").unwrap();
    assert_eq!(tool["function"]["strict"], true);
    assert_eq!(
        tool["function"]["parameters"],
        json!({
            "anyOf": [
                {
                    "type": "object",
                    "properties": {"query": {"type": "string"}},
                    "required": ["query"],
                    "additionalProperties": false
                },
                {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {"count": {"type": "integer"}},
                        "required": ["count"],
                        "additionalProperties": false
                    }
                }
            ]
        })
    );

    let mut empty = json!({"function": {"name": "lookup", "parameters": {"anyOf": []}}});
    deepseek_provider_core_apply_strict_function_schema(&mut empty, "DeepSeek").unwrap();
    assert_eq!(empty["function"]["parameters"], json!({"anyOf": []}));
}

#[test]
fn preserves_nested_validation_error_path() {
    let mut tool = json!({
        "function": {
            "name": "lookup",
            "parameters": {"type": "array", "items": {"type": "string", "pattern": "x"}}
        }
    });
    assert_eq!(
        deepseek_provider_core_apply_strict_function_schema(&mut tool, "DeepSeek").unwrap_err(),
        "DeepSeek strict tool schema `lookup.items` uses unsupported keyword `pattern`"
    );
}
