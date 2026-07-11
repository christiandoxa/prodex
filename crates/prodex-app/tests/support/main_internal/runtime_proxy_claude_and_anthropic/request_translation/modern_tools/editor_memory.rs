use super::*;

#[test]
fn translate_runtime_anthropic_messages_request_roundtrips_versioned_text_editor_tool_use_and_result()
 {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "tool_choice": {
                "type": "tool",
                "name": "text_editor_20250124"
            },
            "tools": [
                {
                    "type": "text_editor_20250124",
                    "description": "Edit local files"
                }
            ],
            "messages": [
                {
                    "role": "assistant",
                    "content": [
                        {
                            "type": "tool_use",
                            "id": "toolu_text_editor",
                            "name": "text_editor_20250124",
                            "input": {
                                "path": "/tmp/prodex/notes.txt",
                                "old_string": "foo",
                                "new_string": "bar"
                            }
                        }
                    ]
                },
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "tool_result",
                            "tool_use_id": "toolu_text_editor",
                            "content": [
                                {
                                    "type": "text",
                                    "text": "Replaced 1 occurrence."
                                }
                            ]
                        }
                    ]
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");

    let tools = body
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .expect("translated tools should exist");
    assert_eq!(tools.len(), 1);
    assert_eq!(
        tools[0].get("type").and_then(serde_json::Value::as_str),
        Some("function")
    );
    assert_eq!(
        tools[0].get("name").and_then(serde_json::Value::as_str),
        Some("str_replace_based_edit_tool")
    );
    assert_eq!(
        body.get("tool_choice")
            .and_then(|tool_choice| tool_choice.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("function")
    );
    assert_eq!(
        body.get("tool_choice")
            .and_then(|tool_choice| tool_choice.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("str_replace_based_edit_tool")
    );

    let input = body
        .get("input")
        .and_then(serde_json::Value::as_array)
        .expect("input array should exist");
    assert_eq!(
        input[0].get("type").and_then(serde_json::Value::as_str),
        Some("function_call")
    );
    assert_eq!(
        input[0].get("name").and_then(serde_json::Value::as_str),
        Some("text_editor_20250124")
    );
    assert_eq!(
        input[1].get("type").and_then(serde_json::Value::as_str),
        Some("function_call_output")
    );
    assert_eq!(
        input[1].get("output").and_then(serde_json::Value::as_str),
        Some("Replaced 1 occurrence.")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_maps_memory_tool_definition_to_builtin_function() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "tool_choice": {
                "type": "tool",
                "name": "memory_20250818"
            },
            "tools": [
                {
                    "type": "memory_20250818"
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Remember that I prefer concise answers"
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");
    let tools = body
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .expect("tools array should exist");

    assert_eq!(tools.len(), 1);
    assert_eq!(
        tools[0].get("type").and_then(serde_json::Value::as_str),
        Some("function")
    );
    assert_eq!(
        tools[0].get("name").and_then(serde_json::Value::as_str),
        Some("memory")
    );
    assert_eq!(
        tools[0]
            .get("description")
            .and_then(serde_json::Value::as_str),
        Some("Store and retrieve information across conversations using persistent memory files.")
    );
    assert_eq!(
        tools[0]
            .get("parameters")
            .and_then(|parameters| parameters.get("properties"))
            .and_then(|properties| properties.get("command"))
            .and_then(|command| command.get("enum"))
            .and_then(serde_json::Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .collect::<Vec<_>>()
            }),
        Some(vec![
            "view",
            "create",
            "str_replace",
            "insert",
            "delete",
            "rename",
        ])
    );
    assert_eq!(
        body.get("tool_choice")
            .and_then(|tool_choice| tool_choice.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("function")
    );
    assert_eq!(
        body.get("tool_choice")
            .and_then(|tool_choice| tool_choice.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("memory")
    );
    assert!(
        body.get("instructions")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|instructions| instructions.contains("/memories")),
        "memory tool should append memory guidance to instructions"
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_appends_memory_tool_guidance_to_system_instructions()
 {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "system": "System instructions",
            "tools": [
                {
                    "type": "memory_20250818",
                    "name": "memory"
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Remember that I prefer concise answers"
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");
    let instructions = body
        .get("instructions")
        .and_then(serde_json::Value::as_str)
        .expect("instructions should exist");

    assert!(instructions.contains("System instructions"));
    assert!(instructions.contains("/memories"));
    assert!(instructions.contains("memory"));
}

