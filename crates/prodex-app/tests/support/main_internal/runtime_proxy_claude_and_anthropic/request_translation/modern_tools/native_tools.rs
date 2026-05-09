use super::*;

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

