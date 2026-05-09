use super::*;

#[test]
fn translate_runtime_anthropic_messages_request_appends_computer_display_context_to_existing_description()
 {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "tools": [
                {
                    "type": "computer_20250124",
                    "description": "Inspect the browser UI.",
                    "display_width_px": 1280,
                    "display_height_px": 720,
                    "display_number": 2
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Inspect the page"
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
    let description = body
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .and_then(|tools| tools.first())
        .and_then(|tool| tool.get("description"))
        .and_then(serde_json::Value::as_str);

    assert_eq!(
        description,
        Some(
            "Inspect the browser UI.\n\nInteract with the graphical computer display. Display resolution: 1280x720 pixels. Display number: 2."
        )
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_preserves_modern_builtin_tool_capabilities() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "tools": [
                {
                    "type": "text_editor_20250728",
                    "description": "Edit project files.",
                    "max_characters": 8192
                },
                {
                    "type": "computer_20251124",
                    "description": "Inspect the browser UI.",
                    "display_width_px": 1600,
                    "display_height_px": 900,
                    "display_number": 1,
                    "enable_zoom": true
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Inspect the project state"
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

    assert_eq!(
        tools[0]
            .get("description")
            .and_then(serde_json::Value::as_str),
        Some(
            "Edit project files.\n\nEdit local text files with string replacement operations. View results may be truncated to 8192 characters."
        )
    );
    assert_eq!(
        tools[0]
            .get("parameters")
            .and_then(|parameters| parameters.get("properties"))
            .and_then(|properties| properties.get("command"))
            .and_then(|property| property.get("enum"))
            .and_then(serde_json::Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .collect::<Vec<_>>()
            }),
        Some(vec!["view", "create", "str_replace", "insert"])
    );
    assert_eq!(
        tools[0]
            .get("parameters")
            .and_then(|parameters| parameters.get("properties"))
            .and_then(|properties| properties.get("insert_text"))
            .and_then(|property| property.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("string")
    );
    assert_eq!(
        tools[1]
            .get("description")
            .and_then(serde_json::Value::as_str),
        Some(
            "Inspect the browser UI.\n\nInteract with the graphical computer display. Display resolution: 1600x900 pixels. Display number: 1. Zoom action enabled."
        )
    );
    assert!(
        tools[1]
            .get("parameters")
            .and_then(|parameters| parameters.get("properties"))
            .and_then(|properties| properties.get("action"))
            .and_then(|property| property.get("enum"))
            .and_then(serde_json::Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .collect::<Vec<_>>()
            })
            .is_some_and(|values| values.contains(&"zoom"))
    );
    assert_eq!(
        tools[1]
            .get("parameters")
            .and_then(|parameters| parameters.get("properties"))
            .and_then(|properties| properties.get("region"))
            .and_then(|property| property.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("array")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_preserves_modern_builtin_client_tool_capabilities()
{
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-opus-4-6",
            "tools": [
                {
                    "type": "text_editor_20250728",
                    "name": "str_replace_based_edit_tool",
                    "max_characters": 10000
                },
                {
                    "type": "computer_20251124",
                    "display_width_px": 1600,
                    "display_height_px": 900,
                    "display_number": 1,
                    "enable_zoom": true
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Inspect and edit the project files."
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

    assert_eq!(
        tools[0]
            .get("description")
            .and_then(serde_json::Value::as_str),
        Some(
            "Edit local text files with string replacement operations. View results may be truncated to 10000 characters."
        )
    );
    assert!(
        !tools[0]
            .get("parameters")
            .and_then(|parameters| parameters.get("properties"))
            .and_then(|properties| properties.get("command"))
            .and_then(|property| property.get("enum"))
            .and_then(serde_json::Value::as_array)
            .is_some_and(|values| values
                .iter()
                .any(|value| value.as_str() == Some("undo_edit")))
    );
    assert_eq!(
        tools[0]
            .get("parameters")
            .and_then(|parameters| parameters.get("properties"))
            .and_then(|properties| properties.get("insert_text"))
            .and_then(|property| property.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("string")
    );
    assert_eq!(
        tools[1]
            .get("description")
            .and_then(serde_json::Value::as_str),
        Some(
            "Interact with the graphical computer display. Display resolution: 1600x900 pixels. Display number: 1. Zoom action enabled."
        )
    );
    assert!(
        tools[1]
            .get("parameters")
            .and_then(|parameters| parameters.get("properties"))
            .and_then(|properties| properties.get("action"))
            .and_then(|property| property.get("enum"))
            .and_then(serde_json::Value::as_array)
            .is_some_and(|values| values.iter().any(|value| value.as_str() == Some("zoom")))
    );
    assert_eq!(
        tools[1]
            .get("parameters")
            .and_then(|parameters| parameters.get("properties"))
            .and_then(|properties| properties.get("region"))
            .and_then(|property| property.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("array")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_maps_bash_tool_to_native_shell_when_enabled() {
    let _guard = TestEnvVarGuard::set("PRODEX_CLAUDE_NATIVE_CLIENT_TOOLS", "shell");
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "tool_choice": {
                "type": "tool",
                "name": "bash_20250124"
            },
            "tools": [
                {
                    "type": "bash_20250124",
                    "description": "Run shell commands"
                }
            ],
            "messages": [
                {
                    "role": "assistant",
                    "content": [
                        {
                            "type": "text",
                            "text": "Running ls."
                        },
                        {
                            "type": "tool_use",
                            "id": "toolu_bash",
                            "name": "bash",
                            "input": {
                                "command": "ls -la",
                                "timeout_ms": 1200,
                                "max_output_length": 4096
                            }
                        }
                    ]
                },
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "tool_result",
                            "tool_use_id": "toolu_bash",
                            "content": "file1\nfile2"
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
        Some("shell")
    );
    assert_eq!(
        body.get("tool_choice").and_then(serde_json::Value::as_str),
        Some("required")
    );

    let input = body
        .get("input")
        .and_then(serde_json::Value::as_array)
        .expect("input array should exist");
    assert_eq!(
        input[0].get("role").and_then(serde_json::Value::as_str),
        Some("assistant")
    );
    assert_eq!(
        input[1].get("type").and_then(serde_json::Value::as_str),
        Some("shell_call")
    );
    assert_eq!(
        input[1].get("call_id").and_then(serde_json::Value::as_str),
        Some("toolu_bash")
    );
    assert_eq!(
        input[1]
            .get("action")
            .and_then(|action| action.get("commands"))
            .and_then(serde_json::Value::as_array)
            .and_then(|commands| commands.first())
            .and_then(serde_json::Value::as_str),
        Some("ls -la")
    );
    assert_eq!(
        input[2].get("type").and_then(serde_json::Value::as_str),
        Some("shell_call_output")
    );
    assert_eq!(
        input[2]
            .get("max_output_length")
            .and_then(serde_json::Value::as_u64),
        Some(4096)
    );
    assert_eq!(
        input[2]
            .get("output")
            .and_then(serde_json::Value::as_array)
            .and_then(|output| output.first())
            .and_then(|entry| entry.get("stdout"))
            .and_then(serde_json::Value::as_str),
        Some("file1\nfile2")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_maps_computer_tool_to_native_computer_when_enabled()
{
    let _guard = TestEnvVarGuard::set("PRODEX_CLAUDE_NATIVE_CLIENT_TOOLS", "computer");
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-opus-4-6",
            "tool_choice": {
                "type": "tool",
                "name": "computer_20250124"
            },
            "tools": [
                {
                    "type": "computer_20250124",
                    "display_width_px": 1440,
                    "display_height_px": 900,
                    "display_number": 1
                }
            ],
            "messages": [
                {
                    "role": "assistant",
                    "content": [
                        {
                            "type": "text",
                            "text": "Capturing the current screen."
                        },
                        {
                            "type": "tool_use",
                            "id": "toolu_computer",
                            "name": "computer",
                            "input": {
                                "action": "screenshot"
                            }
                        }
                    ]
                },
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "tool_result",
                            "tool_use_id": "toolu_computer",
                            "content": [
                                {
                                    "type": "image",
                                    "source": {
                                        "type": "base64",
                                        "media_type": "image/png",
                                        "data": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO7+o2kAAAAASUVORK5CYII="
                                    }
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

    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("computer")
    );
    assert_eq!(
        body.get("tool_choice").and_then(serde_json::Value::as_str),
        Some("required")
    );

    let input = body
        .get("input")
        .and_then(serde_json::Value::as_array)
        .expect("input array should exist");
    assert_eq!(
        input[1].get("type").and_then(serde_json::Value::as_str),
        Some("computer_call")
    );
    assert_eq!(
        input[1]
            .get("actions")
            .and_then(serde_json::Value::as_array)
            .and_then(|actions| actions.first())
            .and_then(|action| action.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("screenshot")
    );
    assert_eq!(
        input[2].get("type").and_then(serde_json::Value::as_str),
        Some("computer_call_output")
    );
    assert_eq!(
        input[2]
            .get("output")
            .and_then(|output| output.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("computer_screenshot")
    );
    assert!(
        input[2]
            .get("output")
            .and_then(|output| output.get("image_url"))
            .and_then(serde_json::Value::as_str)
            .is_some_and(|value| value.starts_with("data:image/png;base64,"))
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_falls_back_for_ambiguous_native_computer_tool_choice()
 {
    let _guard = TestEnvVarGuard::set("PRODEX_CLAUDE_NATIVE_CLIENT_TOOLS", "computer");
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-opus-4-6",
            "tool_choice": {
                "type": "tool",
                "name": "computer_20250124"
            },
            "tools": [
                {
                    "type": "computer_20250124",
                    "display_width_px": 1440,
                    "display_height_px": 900
                },
                {
                    "name": "lookup_logs",
                    "input_schema": {
                        "type": "object"
                    }
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Inspect the UI"
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

    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .map(|tools| {
                tools
                    .iter()
                    .map(|tool| tool.get("type").and_then(serde_json::Value::as_str))
                    .collect::<Vec<_>>()
            }),
        Some(vec![Some("function"), Some("function")])
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
        Some("computer")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_falls_back_for_ambiguous_native_bash_tool_choice() {
    let _guard = TestEnvVarGuard::set("PRODEX_CLAUDE_NATIVE_CLIENT_TOOLS", "shell");
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "tool_choice": {
                "type": "tool",
                "name": "bash_20250124"
            },
            "tools": [
                {
                    "type": "bash_20250124",
                    "description": "Run shell commands"
                },
                {
                    "name": "lookup_logs",
                    "description": "Read application logs",
                    "input_schema": {
                        "type": "object"
                    }
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Inspect the logs"
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
    assert_eq!(
        tools
            .iter()
            .map(|tool| tool.get("type").and_then(serde_json::Value::as_str))
            .collect::<Vec<_>>(),
        vec![Some("function"), Some("function")]
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
        Some("bash")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_normalizes_versioned_client_tool_choice_aliases() {
    let cases = [
        ("bash_20250124", "bash"),
        ("text_editor_20250124", "str_replace_based_edit_tool"),
        ("computer_20250124", "computer"),
        ("memory_20250818", "memory"),
    ];

    for (versioned_tool_type, expected_name) in cases {
        let request = RuntimeProxyRequest {
            method: "POST".to_string(),
            path_and_query: "/v1/messages?beta=true".to_string(),
            headers: vec![],
            body: serde_json::json!({
                "model": "claude-sonnet-4-6",
                "tool_choice": {
                    "type": "tool",
                    "name": versioned_tool_type
                },
                "tools": [
                    {
                        "type": versioned_tool_type,
                        "description": "Run a versioned client tool"
                    }
                ],
                "messages": [
                    {
                        "role": "user",
                        "content": "Use the requested tool"
                    }
                ]
            })
            .to_string()
            .into_bytes(),
        };

        let translated = translate_runtime_anthropic_messages_request(&request)
            .expect("translation should succeed");
        let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
            .expect("translated body should parse");

        assert_eq!(
            body.get("tools")
                .and_then(serde_json::Value::as_array)
                .and_then(|tools| tools.first())
                .and_then(|tool| tool.get("type"))
                .and_then(serde_json::Value::as_str),
            Some("function"),
            "tool type should remain a generic function for {versioned_tool_type}"
        );
        assert_eq!(
            body.get("tools")
                .and_then(serde_json::Value::as_array)
                .and_then(|tools| tools.first())
                .and_then(|tool| tool.get("name"))
                .and_then(serde_json::Value::as_str),
            Some(expected_name),
            "tool name should normalize for {versioned_tool_type}"
        );
        assert_eq!(
            body.get("tool_choice")
                .and_then(|tool_choice| tool_choice.get("type"))
                .and_then(serde_json::Value::as_str),
            Some("function"),
            "tool_choice type should normalize for {versioned_tool_type}"
        );
        assert_eq!(
            body.get("tool_choice")
                .and_then(|tool_choice| tool_choice.get("name"))
                .and_then(serde_json::Value::as_str),
            Some(expected_name),
            "tool_choice name should normalize for {versioned_tool_type}"
        );
    }
}

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

#[test]
fn translate_runtime_anthropic_messages_request_maps_versioned_code_execution_server_tool() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "tool_choice": {
                "type": "tool",
                "name": "code_execution"
            },
            "tools": [
                {
                    "type": "code_execution_20250825"
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Analyze the attached data"
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
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("function")
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("code_execution")
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
        Some("code_execution")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_maps_versioned_tool_search_server_tool() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "tool_choice": {
                "type": "tool",
                "name": "tool_search_tool_regex"
            },
            "tools": [
                {
                    "type": "tool_search_tool_regex_20251119"
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Find tools related to browser fetch"
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
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("function")
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("tool_search_tool_regex")
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("parameters"))
            .and_then(|parameters| parameters.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("object")
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
        Some("tool_search_tool_regex")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_maps_versioned_web_fetch_server_tool() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "tool_choice": {
                "type": "tool",
                "name": "web_fetch"
            },
            "tools": [
                {
                    "type": "web_fetch_20260409",
                    "description": "Fetch the contents of a web page.",
                    "input_schema": {
                        "type": "object"
                    }
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Fetch https://example.com"
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
        Some("web_fetch")
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("function")
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("web_fetch")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_compacts_verbose_web_search_tool_result_text() {
    let raw_output = concat!(
        "Web search results for query: \"berita reksadana terbaru Indonesia April 2026\"\n\n",
        "Links: [{\"url\":\"https://example.com/a\"},{\"url\":\"https://example.com/b\"}]\n\n",
        "Links: [{\"url\":\"https://example.com/b\"},{\"url\":\"https://example.com/c\"}]\n\n",
        "No links found.\n\n",
        "Saya sudah melakukan web search untuk kueri itu. Hasil paling relevan yang saya temukan:\n",
        "1. Contoh hasil pertama\n",
        "Link: https://example.com/a\n\n",
        "Kalau mau, saya bisa lanjutkan dengan rangkuman detail.\n\n",
        "REMINDER: You MUST include the sources above in your response to the user using markdown hyperlinks."
    );
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "messages": [
                {
                    "role": "assistant",
                    "content": [
                        {
                            "type": "tool_use",
                            "id": "call_ws_1",
                            "name": "WebSearch",
                            "input": {
                                "query": "berita reksadana terbaru Indonesia April 2026"
                            }
                        }
                    ]
                },
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "tool_result",
                            "tool_use_id": "call_ws_1",
                            "content": raw_output
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
    let input = body
        .get("input")
        .and_then(serde_json::Value::as_array)
        .expect("input array should exist");

    assert_eq!(input.len(), 2);
    assert_eq!(
        input[1].get("type").and_then(serde_json::Value::as_str),
        Some("function_call_output")
    );

    let output = input[1]
        .get("output")
        .and_then(serde_json::Value::as_str)
        .expect("tool result output should be a string");
    assert!(
        output.len() < raw_output.len(),
        "normalized output should be smaller than the raw verbose payload"
    );
    let output: serde_json::Value =
        serde_json::from_str(output).expect("normalized output should be valid JSON");
    assert_eq!(
        output.get("query").and_then(serde_json::Value::as_str),
        Some("berita reksadana terbaru Indonesia April 2026")
    );
    assert_eq!(
        output
            .get("content_blocks")
            .and_then(serde_json::Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.get("url").and_then(serde_json::Value::as_str))
                    .collect::<Vec<_>>()
            }),
        Some(vec![
            "https://example.com/a",
            "https://example.com/b",
            "https://example.com/c"
        ])
    );
    assert_eq!(
        output.get("text").and_then(serde_json::Value::as_str),
        Some(
            "Saya sudah melakukan web search untuk kueri itu. Hasil paling relevan yang saya temukan:\n1. Contoh hasil pertama"
        )
    );
}

#[test]
fn runtime_proxy_anthropic_reasoning_effort_normalizes_output_config_levels() {
    let _effort_guard = TestEnvVarGuard::unset("PRODEX_CLAUDE_REASONING_EFFORT");
    let cases = [
        ("gpt-5.4", "low", Some("low")),
        ("gpt-5.4", "medium", Some("medium")),
        ("gpt-5.4", "high", Some("high")),
        ("gpt-5.4", "max", Some("xhigh")),
        ("gpt-5", "max", Some("high")),
        ("gpt-5.4", "HIGH", Some("high")),
        ("gpt-5.4", "unknown", None),
    ];

    for (target_model, input_effort, expected) in cases {
        let value = serde_json::json!({
            "output_config": {
                "effort": input_effort,
            }
        });
        assert_eq!(
            runtime_proxy_anthropic_reasoning_effort(&value, target_model).as_deref(),
            expected,
            "input effort {input_effort:?} normalized incorrectly for target model {target_model}"
        );
    }
}

#[test]
fn translate_runtime_anthropic_messages_request_maps_max_effort_to_xhigh_for_supported_model() {
    let _effort_guard = TestEnvVarGuard::unset("PRODEX_CLAUDE_REASONING_EFFORT");
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "gpt-5.4",
            "thinking": {
                "type": "adaptive"
            },
            "output_config": {
                "effort": "max",
            },
            "messages": [
                {
                    "role": "user",
                    "content": "hello"
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
    assert_eq!(
        body.get("reasoning")
            .and_then(|reasoning| reasoning.get("effort"))
            .and_then(serde_json::Value::as_str),
        Some("xhigh")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_keeps_max_effort_at_high_for_legacy_model() {
    let _effort_guard = TestEnvVarGuard::unset("PRODEX_CLAUDE_REASONING_EFFORT");
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "gpt-5",
            "thinking": {
                "type": "adaptive"
            },
            "output_config": {
                "effort": "max",
            },
            "messages": [
                {
                    "role": "user",
                    "content": "hello"
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
    assert_eq!(
        body.get("reasoning")
            .and_then(|reasoning| reasoning.get("effort"))
            .and_then(serde_json::Value::as_str),
        Some("high")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_honors_reasoning_override_env() {
    let _effort_guard = TestEnvVarGuard::set("PRODEX_CLAUDE_REASONING_EFFORT", "xhigh");
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "gpt-5.2",
            "thinking": {
                "type": "adaptive"
            },
            "output_config": {
                "effort": "low",
            },
            "messages": [
                {
                    "role": "user",
                    "content": "hello"
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
    assert_eq!(
        body.get("reasoning")
            .and_then(|reasoning| reasoning.get("effort"))
            .and_then(serde_json::Value::as_str),
        Some("xhigh")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_maps_web_search_server_tool() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "tool_choice": {
                "type": "tool",
                "name": "web_search"
            },
            "tools": [
                {
                    "type": "web_search_20260209",
                    "name": "web_search",
                    "allowed_domains": ["example.com"],
                    "user_location": {
                        "type": "approximate",
                        "country": "ID",
                        "city": "Jakarta",
                        "region": "Jakarta",
                    }
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Cari berita terbaru"
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

    assert_eq!(
        body.get("tool_choice").and_then(serde_json::Value::as_str),
        Some("required")
    );
    assert_eq!(
        body.get("include")
            .and_then(serde_json::Value::as_array)
            .and_then(|include| include.first())
            .and_then(serde_json::Value::as_str),
        Some("web_search_call.action.sources")
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("web_search")
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("filters"))
            .and_then(|filters| filters.get("allowed_domains"))
            .and_then(serde_json::Value::as_array)
            .and_then(|domains| domains.first())
            .and_then(serde_json::Value::as_str),
        Some("example.com")
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("user_location"))
            .and_then(|location| location.get("country"))
            .and_then(serde_json::Value::as_str),
        Some("ID")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_preserves_claude_web_search_tool_name() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "tool_choice": {
                "type": "tool",
                "name": "WebSearch"
            },
            "tools": [
                {
                    "name": "WebSearch",
                    "description": "Search the web for current information.",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "query": {
                                "type": "string"
                            },
                            "allowed_domains": {
                                "type": "array",
                                "items": {
                                    "type": "string"
                                }
                            },
                            "blocked_domains": {
                                "type": "array",
                                "items": {
                                    "type": "string"
                                }
                            }
                        },
                        "required": ["query"]
                    }
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Cari berita terbaru"
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
        Some("WebSearch")
    );
    assert!(
        body.get("include").is_none(),
        "generic Claude WebSearch tools should not enable Responses web_search include filters"
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("function")
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("WebSearch")
    );
    assert!(
        !translated.server_tools.needs_buffered_translation(),
        "generic Claude WebSearch tools must stay on the direct streaming/function path"
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_preserves_claude_web_fetch_tool_name() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "tool_choice": {
                "type": "tool",
                "name": "WebFetch"
            },
            "tools": [
                {
                    "name": "WebFetch",
                    "description": "Fetch a web page.",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "url": {
                                "type": "string"
                            },
                            "prompt": {
                                "type": "string"
                            }
                        },
                        "required": ["url"]
                    }
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Lihat isi https://example.com"
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
        Some("WebFetch")
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("function")
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("WebFetch")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_forces_implicit_web_search_tool_choice() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "tools": [
                {
                    "type": "web_search_20250305",
                    "name": "web_search",
                    "allowed_domains": []
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Perform a web search for the latest OpenAI news"
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

    assert_eq!(
        body.get("tool_choice").and_then(serde_json::Value::as_str),
        Some("required")
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("web_search")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_preserves_mcp_approval_response_block() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "mcp_approval_response",
                            "approval_request_id": "mcpapr_1",
                            "approve": true,
                            "reason": "Looks good"
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
    let input = body
        .get("input")
        .and_then(serde_json::Value::as_array)
        .expect("input array should exist");

    assert_eq!(input.len(), 1);
    assert_eq!(
        input[0].get("type").and_then(serde_json::Value::as_str),
        Some("mcp_approval_response")
    );
    assert_eq!(
        input[0]
            .get("approval_request_id")
            .and_then(serde_json::Value::as_str),
        Some("mcpapr_1")
    );
    assert_eq!(
        input[0].get("approve").and_then(serde_json::Value::as_bool),
        Some(true)
    );
}
