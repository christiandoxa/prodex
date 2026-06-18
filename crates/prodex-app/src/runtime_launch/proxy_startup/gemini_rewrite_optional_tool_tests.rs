use super::gemini_rewrite_test_support::conversation_store;
use super::runtime_gemini_generate_request_body;

#[test]
fn gemini_maps_mcp_optional_tools_to_function_declarations() {
    let body = serde_json::json!({
        "model": "gemini-2.5-pro",
        "stream": true,
        "input": "compress and inspect the workspace",
        "tools": [
            {
                "type": "mcp_tool",
                "name": "mcp__prodex_sqz__sqz_read_file",
                "description": "Read a file through SQZ.",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"}
                    },
                    "required": ["path"]
                }
            },
            {
                "type": "mcp_tool",
                "name": "mcp__prodex_token_savior__find_symbol",
                "description": "Find a symbol through token-savior.",
                "schema": {
                    "type": "object",
                    "properties": {
                        "name": {"type": "string"}
                    }
                }
            },
            {
                "type": "mcp_toolset",
                "mcp_server_name": "prodex-sqz",
                "description": "SQZ optimizer MCP server.",
                "default_config": {"enabled": false},
                "configs": {
                    "compress": {"enabled": true},
                    "sqz_read_file": {"enabled": true}
                }
            }
        ],
        "tool_choice": {
            "type": "mcp_tool",
            "name": "mcp__prodex_sqz__sqz_read_file"
        }
    });

    let translated = runtime_gemini_generate_request_body(
        &serde_json::to_vec(&body).unwrap(),
        &conversation_store(),
        false,
        None,
        None,
    )
    .expect("request should translate");
    let value: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
    let declarations = value["tools"][0]["functionDeclarations"]
        .as_array()
        .expect("function declarations should be present");
    let names = declarations
        .iter()
        .filter_map(|declaration| declaration["name"].as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        names,
        vec![
            "mcp__prodex_sqz__sqz_read_file",
            "mcp__prodex_token_savior__find_symbol",
            "mcp__prodex_sqz__compress",
        ]
    );
    assert_eq!(declarations[0]["parameters"]["required"][0], "path");
    assert_eq!(
        declarations[1]["parameters"]["properties"]["name"]["type"],
        "string"
    );
    assert!(
        declarations[2]["parameters"]
            .get("additionalProperties")
            .is_none()
    );
    assert_eq!(
        value["toolConfig"]["functionCallingConfig"]["allowedFunctionNames"][0],
        "mcp__prodex_sqz__sqz_read_file"
    );
}

#[test]
fn gemini_sanitizes_optional_tool_schemas_for_function_declarations() {
    let body = serde_json::json!({
        "model": "gemini-2.5-pro",
        "input": "read and compress",
        "tools": [{
            "type": "mcp_tool",
            "name": "mcp__prodex_sqz__sqz_read_file",
            "description": "Read a file through SQZ.",
            "parametersJsonSchema": {
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "path": {
                        "anyOf": [
                            {"type": "string", "description": "File path"},
                            {"type": "null"}
                        ]
                    },
                    "detail": {
                        "type": ["string", "null"],
                        "enum": ["high", "original", 1],
                        "default": "high"
                    }
                },
                "required": ["path"]
            }
        }]
    });

    let translated = runtime_gemini_generate_request_body(
        &serde_json::to_vec(&body).unwrap(),
        &conversation_store(),
        false,
        None,
        None,
    )
    .expect("request should translate");
    let value: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
    let parameters = &value["tools"][0]["functionDeclarations"][0]["parameters"];

    assert_eq!(parameters["type"], "object");
    assert!(parameters.get("$schema").is_none());
    assert!(parameters.get("additionalProperties").is_none());
    assert!(parameters["properties"]["path"].get("anyOf").is_none());
    assert_eq!(parameters["properties"]["path"]["type"], "string");
    assert_eq!(parameters["properties"]["path"]["nullable"], true);
    assert_eq!(parameters["properties"]["detail"]["type"], "string");
    assert_eq!(parameters["properties"]["detail"]["nullable"], true);
    assert_eq!(
        parameters["properties"]["detail"]["enum"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn gemini_preserves_composed_tool_schemas_for_function_declarations() {
    let body = serde_json::json!({
        "model": "gemini-2.5-pro",
        "input": "use richer schema",
        "tools": [{
            "type": "mcp_tool",
            "name": "mcp__workspace__rich_tool",
            "description": "Use a composed JSON schema.",
            "parametersJsonSchema": {
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "type": "object",
                "properties": {
                    "target": {
                        "oneOf": [
                            {"type": "string", "description": "Named target"},
                            {"type": "integer", "description": "Numeric target"}
                        ]
                    },
                    "metadata": {
                        "allOf": [
                            {
                                "type": "object",
                                "properties": {
                                    "source": {"type": "string"}
                                }
                            },
                            {
                                "type": "object",
                                "properties": {
                                    "confidence": {"type": "number"}
                                }
                            }
                        ]
                    }
                },
                "required": ["target"]
            }
        }]
    });

    let translated = runtime_gemini_generate_request_body(
        &serde_json::to_vec(&body).unwrap(),
        &conversation_store(),
        false,
        None,
        None,
    )
    .expect("request should translate");
    let value: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
    let parameters = &value["tools"][0]["functionDeclarations"][0]["parameters"];

    assert_eq!(parameters["type"], "object");
    assert_eq!(
        parameters["properties"]["target"]["oneOf"][0]["type"],
        "string"
    );
    assert_eq!(
        parameters["properties"]["target"]["oneOf"][1]["type"],
        "integer"
    );
    assert_eq!(
        parameters["properties"]["metadata"]["allOf"][0]["properties"]["source"]["type"],
        "string"
    );
    assert_eq!(
        parameters["properties"]["metadata"]["allOf"][1]["properties"]["confidence"]["type"],
        "number"
    );
}

#[test]
fn gemini_maps_required_and_none_tool_choice_modes() {
    for (choice, mode) in [("required", "ANY"), ("none", "NONE")] {
        let body = serde_json::json!({
            "model": "gemini-2.5-pro",
            "input": "maybe use a tool",
            "tools": [{
                "type": "function",
                "name": "shell",
                "parameters": {"type": "object"}
            }],
            "tool_choice": choice
        });

        let translated = runtime_gemini_generate_request_body(
            &serde_json::to_vec(&body).unwrap(),
            &conversation_store(),
            false,
            None,
            None,
        )
        .expect("request should translate");
        let value: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

        assert_eq!(value["toolConfig"]["functionCallingConfig"]["mode"], mode);
        assert!(
            value["toolConfig"]["functionCallingConfig"]["allowedFunctionNames"].is_null(),
            "{choice} should not restrict the allowed function names"
        );
    }
}
