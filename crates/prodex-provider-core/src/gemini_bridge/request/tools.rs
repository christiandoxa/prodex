//! Gemini request tool-shape bridge helpers.

use crate::translators::{
    gemini_builtin_tool_indices, gemini_builtin_tools_from_request,
    gemini_function_declaration_from_openai_tool, gemini_sanitize_function_schema,
    gemini_validate_openai_tools, gemini_validate_openai_tools_with_nested_function_paths,
};

use crate::gemini_bridge::gemini_provider_core_apply_gemini3_tool_declaration_overrides;

pub fn gemini_provider_core_sanitize_function_schema(
    schema: &serde_json::Value,
) -> serde_json::Value {
    gemini_sanitize_function_schema(schema)
}

pub fn gemini_provider_core_tool_config_from_request(
    value: &serde_json::Value,
) -> Result<Option<serde_json::Value>, String> {
    super::request_contents::gemini_bridge_request_tool_config(value)
}

pub fn gemini_provider_core_function_tools_from_chat(
    chat: &serde_json::Value,
    model: &str,
    mut filter_declarations: impl FnMut(&mut Vec<serde_json::Value>),
) -> Result<Option<serde_json::Value>, String> {
    gemini_provider_core_validate_request_tools(chat)?;
    let Some(tools) = chat.get("tools").and_then(serde_json::Value::as_array) else {
        return Ok(None);
    };
    let mut declarations = Vec::new();
    let builtin_indexes = gemini_builtin_tool_indices(tools);
    for (index, tool) in tools.iter().enumerate() {
        if builtin_indexes.binary_search(&index).is_ok() {
            continue;
        }
        let declaration = gemini_function_declaration_from_openai_tool(tool)
            .ok_or_else(|| {
                format!(
                    "invalid_tool_declaration: Gemini request field `tools[{index}]` could not be translated"
                )
            })?;
        declarations.push(declaration);
    }
    gemini_provider_core_apply_gemini3_tool_declaration_overrides(model, &mut declarations);
    filter_declarations(&mut declarations);
    Ok((!declarations.is_empty()).then(|| {
        serde_json::json!([{
            "functionDeclarations": declarations,
        }])
    }))
}

pub fn gemini_provider_core_function_tools_from_chat_checked(
    chat: &serde_json::Value,
    model: &str,
    filter_declarations: impl FnMut(&mut Vec<serde_json::Value>),
) -> Result<Option<serde_json::Value>, String> {
    gemini_provider_core_function_tools_from_chat(chat, model, filter_declarations)
}

pub fn gemini_provider_core_tools_from_requests(
    original: &serde_json::Value,
    chat: &serde_json::Value,
    model: &str,
    filter_declarations: impl FnMut(&mut Vec<serde_json::Value>),
) -> Result<Option<serde_json::Value>, String> {
    gemini_provider_core_validate_request_tools(original)?;
    gemini_provider_core_validate_request_tools(chat)?;
    let mut tools = original
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .map(|tools| gemini_builtin_tools_from_request(tools))
        .unwrap_or_default();
    if let Some(serde_json::Value::Array(function_tools)) =
        gemini_provider_core_function_tools_from_chat(chat, model, filter_declarations)?
    {
        tools.extend(function_tools);
    }
    Ok((!tools.is_empty()).then_some(serde_json::Value::Array(tools)))
}

pub fn gemini_provider_core_tools_from_requests_checked(
    original: &serde_json::Value,
    chat: &serde_json::Value,
    model: &str,
    filter_declarations: impl FnMut(&mut Vec<serde_json::Value>),
) -> Result<Option<serde_json::Value>, String> {
    gemini_provider_core_tools_from_requests(original, chat, model, filter_declarations)
}

pub fn gemini_provider_core_validate_request_tools(
    value: &serde_json::Value,
) -> Result<(), String> {
    match value.get("tools") {
        Some(tools) => gemini_validate_openai_tools(tools),
        None => Ok(()),
    }
}

pub fn gemini_provider_core_validate_request_tools_with_nested_function_paths(
    value: &serde_json::Value,
) -> Result<(), String> {
    match value.get("tools") {
        Some(tools) => gemini_validate_openai_tools_with_nested_function_paths(tools),
        None => Ok(()),
    }
}

pub fn gemini_provider_core_request_body_without_tool(
    body: &[u8],
    tool_name: &str,
) -> Option<Vec<u8>> {
    let value = serde_json::from_slice::<serde_json::Value>(body).ok()?;
    // Normalize escaped property names and duplicate keys before Mojo scans the bytes.
    let body = serde_json::to_vec(&value).ok()?;
    super::request_contents::gemini_bridge_request_without_tool(&body, tool_name)
}

pub fn gemini_provider_core_unsupported_tool_fallback_body(
    body: &[u8],
    error_body: &[u8],
) -> Option<(&'static str, Vec<u8>)> {
    // Fixed ABI labels only. Mojo determines retry priority and exhaustion.
    const TOOL_NAMES: [&str; 4] = ["computerUse", "codeExecution", "urlContext", "googleSearch"];
    let rejected = TOOL_NAMES
        .iter()
        .enumerate()
        .fold(0_u8, |mask, (index, tool)| {
            mask | (u8::from(crate::provider_error_rejects_request_member(
                error_body, tool,
            )) << index)
        });
    let mut attempted = 0_u8;
    while let Some(index) =
        prodex_mojo_core::gemini_tooling_policy::gemini_unsupported_tool_retry_choice(
            rejected, attempted,
        )
        .expect("Mojo Gemini unsupported-tool retry choice returned invalid output")
    {
        attempted |= 1 << index;
        let tool_name = TOOL_NAMES[index];
        if let Some(rewritten) = gemini_provider_core_request_body_without_tool(body, tool_name) {
            return Some((tool_name, rewritten));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn flattened_function_tool_mojo_source_does_not_override_nested_presence() {
        let request = json!({
            "tools": [
                {"type": "function", "name": "flat-東京", "description": "flat", "parameters": {"type": "object"}},
                {"type": "function", "function": {"name": "nested", "parameters": {"type": "object"}},
                    "name": "ignored-flat"},
            ]
        });
        let result =
            gemini_provider_core_function_tools_from_chat(&request, "gemini-2.5-flash", |_| {})
                .unwrap()
                .unwrap();
        assert_eq!(result[0]["functionDeclarations"][0]["name"], "flat-東京");
        assert_eq!(result[0]["functionDeclarations"][1]["name"], "nested");
        assert_eq!(result[0]["functionDeclarations"][0]["description"], "flat");
        assert_eq!(
            result[0]["functionDeclarations"][1].get("description"),
            None
        );

        let invalid_nested = json!({
            "tools": [{"type": "function", "function": null, "name": "flat-ignored"}]
        });
        assert!(
            gemini_provider_core_function_tools_from_chat(
                &invalid_nested,
                "gemini-2.5-flash",
                |_| {},
            )
            .is_err()
        );
    }

    #[test]
    fn function_declaration_uses_the_mojo_union_contract() {
        let result = gemini_provider_core_function_tools_from_chat(
            &json!({
                "tools": [
                    {
                        "type": "function",
                        "function": {
                            "name": "lookup-雪",
                            "description": "find ☃",
                            "parameters": {
                                "anyOf": [],
                                "oneOf": [{"type": "string"}]
                            }
                        }
                    },
                    {
                        "type": "function",
                        "function": {
                            "name": "second",
                            "parameters": {"type": "object"}
                        }
                    }
                ]
            }),
            "gemini-2.5-flash",
            |_| {},
        )
        .unwrap();

        assert_eq!(
            result,
            Some(json!([{
                "functionDeclarations": [{
                    "name": "lookup-雪",
                    "description": "find ☃",
                    "parameters": {"type": "string"}
                }, {
                    "name": "second",
                    "parameters": {"type": "object"}
                }]
            }]))
        );
    }

    #[test]
    fn compiled_gemini_builtin_batch_keeps_mixed_builtin_groups_and_functions_in_order() {
        let request = json!({
            "tools": [
                {"type":"url_context"},
                {"type":"function", "function":{"name":"lookup_東京", "parameters":{"type":"object"}}},
                {"type":"code_interpreter"},
                {"type":"function", "function":{"name":"search_one", "parameters":{"type":"object"}}},
                {"type":"web_search_preview_2099"},
                {"type":"function", "function":{"name":"search_two", "parameters":{"type":"object"}}},
                {"type":"computer_use", "computerUse":{"environment":"ENVIRONMENT_BROWSER"}},
                {"type":"function", "function":{"name":"lookup_🧭", "parameters":{"type":"object"}}},
            ]
        });
        let tools = request["tools"].as_array().unwrap();
        assert_eq!(gemini_builtin_tool_indices(tools), [0, 2, 4, 6]);

        let expected_declarations = json!([{
            "functionDeclarations": [
                {"name":"lookup_東京", "parameters":{"type":"object"}},
                {"name":"search_one", "parameters":{"type":"object"}},
                {"name":"search_two", "parameters":{"type":"object"}},
                {"name":"lookup_🧭", "parameters":{"type":"object"}}
            ]
        }]);
        assert_eq!(
            gemini_provider_core_function_tools_from_chat(&request, "gemini-2.5-flash", |_| {}),
            Ok(Some(expected_declarations.clone())),
        );
        let tools_value = gemini_provider_core_tools_from_requests(
            &request,
            &request,
            "gemini-2.5-flash",
            |_| {},
        )
        .unwrap()
        .unwrap();
        let normalized = tools_value.as_array().unwrap();
        assert_eq!(normalized.len(), 5);
        assert_eq!(
            normalized[0],
            json!({"computerUse":{"environment":"ENVIRONMENT_BROWSER"}})
        );
        assert_eq!(normalized[1], json!({"codeExecution":{}}));
        assert_eq!(normalized[2], json!({"googleSearch":{}}));
        assert_eq!(normalized[3], json!({"urlContext":{}}));
        assert_eq!(normalized[4], expected_declarations[0]);
    }

    #[test]
    fn builtin_tools_use_mojo_grouping_and_canonical_order() {
        let result = gemini_provider_core_tools_from_requests(
            &json!({
                "tools": [
                    {"type": "url_context"},
                    {"type": "code_interpreter"},
                    {"type": "web_search_preview_2099"},
                    {"type": "computer_use", "computerUse": {
                        "environment": 7,
                        "excludedPredefinedFunctions": ["click", "type"]
                    }},
                    {"type": "code_execution"}
                ]
            }),
            &json!({}),
            "gemini-2.5-flash",
            |_| {},
        )
        .unwrap();

        assert_eq!(
            result,
            Some(json!([
                {"computerUse": {
                    "environment": "ENVIRONMENT_BROWSER",
                    "excludedPredefinedFunctions": ["click", "type"]
                }},
                {"codeExecution": {}},
                {"googleSearch": {}},
                {"urlContext": {}}
            ]))
        );
    }

    #[test]
    fn wrong_type_computer_use_does_not_fall_back_to_outer_environment() {
        let result = gemini_provider_core_tools_from_requests(
            &json!({
                "tools": [{
                    "type": "computerUse",
                    "computerUse": 7,
                    "environment": "ENVIRONMENT_APP"
                }]
            }),
            &json!({}),
            "gemini-2.5-flash",
            |_| {},
        )
        .unwrap();

        assert_eq!(
            result,
            Some(json!([{"computerUse":{"environment":"ENVIRONMENT_BROWSER"}}]))
        );
    }

    #[test]
    fn request_body_without_tool_preserves_remaining_order() {
        let body = br#"{"tail":true,"request":{"tools":[{"keep":"a"},{"computer\u0055se":{}},{"keep":"b"}]}}"#;
        assert_eq!(
            gemini_provider_core_request_body_without_tool(body, "computerUse"),
            Some(br#"{"request":{"tools":[{"keep":"a"},{"keep":"b"}]},"tail":true}"#.to_vec())
        );
        assert_eq!(
            gemini_provider_core_request_body_without_tool(
                br#"{"request":{"tools":[{"keep":true}]}}"#,
                "computerUse"
            ),
            None
        );
        assert_eq!(
            gemini_provider_core_request_body_without_tool(b"{\"request\":", "computerUse"),
            None
        );
    }

    #[test]
    fn unsupported_tool_fallback_drops_the_explicitly_rejected_builtin() {
        let request =
            br#"{"tail":true,"request":{"tools":[{"keep":"a"},{"googleSearch":{}},{"keep":"b"}]}}"#;
        let error =
            br#"{"error":{"status":"INVALID_ARGUMENT","message":["Unknown name googleSearch"]}}"#;

        assert_eq!(
            gemini_provider_core_unsupported_tool_fallback_body(request, error),
            Some((
                "googleSearch",
                br#"{"request":{"tools":[{"keep":"a"},{"keep":"b"}]},"tail":true}"#.to_vec(),
            ))
        );
        assert_eq!(
            gemini_provider_core_unsupported_tool_fallback_body(
                request,
                br#"{"error":{"message":"request failed for googleSearch"}}"#,
            ),
            None
        );
    }

    #[test]
    fn unsupported_tool_retry_chooses_next_rejected_available_builtin_via_mojo() {
        let request = br#"{"request":{"tools":[{"googleSearch":{}},{"keep":true}]}}"#;
        let both_rejected = br#"{"error":{"status":"INVALID_ARGUMENT","message":["Unknown name computerUse","Unknown name googleSearch"]}}"#;
        assert_eq!(
            gemini_provider_core_unsupported_tool_fallback_body(request, both_rejected),
            Some((
                "googleSearch",
                br#"{"request":{"tools":[{"keep":true}]}}"#.to_vec()
            )),
        );

        let with_both = br#"{"request":{"tools":[{"computerUse":{}},{"googleSearch":{}}]}}"#;
        assert_eq!(
            gemini_provider_core_unsupported_tool_fallback_body(with_both, both_rejected),
            Some((
                "computerUse",
                br#"{"request":{"tools":[{"googleSearch":{}}]}}"#.to_vec()
            )),
        );
        let no_rejection = br#"{"error":{"message":"unsupported tool error"}}"#;
        assert_eq!(
            gemini_provider_core_unsupported_tool_fallback_body(with_both, no_rejection),
            None,
        );
    }

    #[test]
    fn function_tool_validation_keeps_missing_and_wrong_type_precedence() {
        for (tool, expected) in [
            (
                json!({"function": null, "name": "flat", "parameters": {}}),
                "tools[0].function` must be an object",
            ),
            (
                json!({"function": {"name": 7, "description": 3}}),
                "tools[0].function.name` must be a non-empty string",
            ),
            (
                json!({"function": {"name": "lookup", "description": 3}}),
                "tools[0].function.parameters` is required",
            ),
            (
                json!({
                    "function": {"name": "lookup", "parameters": false, "description": 3}
                }),
                "tools[0].function.parameters` must be an object",
            ),
            (
                json!({
                    "function": {"name": "lookup", "parameters": {}, "description": 3}
                }),
                "tools[0].function.description` must be a string",
            ),
        ] {
            let error =
                gemini_provider_core_validate_request_tools(&json!({"tools": [tool]})).unwrap_err();
            assert!(error.contains(expected), "{error}");
        }
    }

    #[test]
    fn runtime_function_tool_validation_preserves_nested_paths_for_flat_tools() {
        let error =
            gemini_provider_core_validate_request_tools_with_nested_function_paths(&json!({
                "tools": [
                    {"type": "function", "name": "lookup", "parameters": {}},
                    {"type": "function", "name": "missing_schema"}
                ]
            }))
            .unwrap_err();

        assert!(error.contains("tools[1].function.parameters"), "{error}");
    }
}
