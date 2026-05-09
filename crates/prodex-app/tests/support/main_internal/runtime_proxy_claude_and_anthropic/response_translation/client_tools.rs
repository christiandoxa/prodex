use super::*;

#[test]
fn runtime_anthropic_response_from_json_value_preserves_versioned_client_tools_as_generic_tool_use()
{
    let response = runtime_anthropic_response_from_json_value(
        &serde_json::json!({
            "usage": {
                "input_tokens": 11,
                "output_tokens": 5
            },
            "output": [
                {
                    "type": "function_call",
                    "call_id": "call_bash",
                    "name": "bash_20250124",
                    "arguments": "{\"command\":\"ls -la\"}"
                },
                {
                    "type": "function_call",
                    "call_id": "call_text_editor",
                    "name": "text_editor_20250124",
                    "arguments": "{\"path\":\"/tmp/prodex/notes.txt\"}"
                },
                {
                    "type": "function_call",
                    "call_id": "call_computer",
                    "name": "computer_20250124",
                    "arguments": "{\"display_width_px\":1440}"
                }
            ]
        }),
        "claude-sonnet-4-6",
        false,
    );

    let content = response
        .get("content")
        .and_then(serde_json::Value::as_array)
        .expect("content should be an array");

    assert_eq!(
        content
            .iter()
            .map(|block| block.get("type").and_then(serde_json::Value::as_str))
            .collect::<Vec<_>>(),
        vec![Some("tool_use"), Some("tool_use"), Some("tool_use")]
    );
    assert_eq!(
        content
            .iter()
            .map(|block| block.get("name").and_then(serde_json::Value::as_str))
            .collect::<Vec<_>>(),
        vec![
            Some("bash_20250124"),
            Some("text_editor_20250124"),
            Some("computer_20250124"),
        ]
    );
}

#[test]
fn runtime_anthropic_response_from_json_value_maps_shell_call_to_bash_tool_use() {
    let response = runtime_anthropic_response_from_json_value(
        &serde_json::json!({
            "usage": {
                "input_tokens": 11,
                "output_tokens": 5
            },
            "output": [
                {
                    "type": "shell_call",
                    "call_id": "call_shell",
                    "action": {
                        "commands": ["ls -la"],
                        "timeout_ms": 1200,
                        "max_output_length": 4096
                    }
                }
            ]
        }),
        "claude-sonnet-4-6",
        false,
    );

    let content = response
        .get("content")
        .and_then(serde_json::Value::as_array)
        .expect("content should be an array");
    assert_eq!(
        content[0].get("type").and_then(serde_json::Value::as_str),
        Some("tool_use")
    );
    assert_eq!(
        content[0].get("name").and_then(serde_json::Value::as_str),
        Some("bash")
    );
    assert_eq!(
        content[0]
            .get("input")
            .and_then(|input| input.get("command"))
            .and_then(serde_json::Value::as_str),
        Some("ls -la")
    );
    assert_eq!(
        content[0]
            .get("input")
            .and_then(|input| input.get("timeout_ms"))
            .and_then(serde_json::Value::as_u64),
        Some(1200)
    );
    assert_eq!(
        content[0]
            .get("input")
            .and_then(|input| input.get("max_output_length"))
            .and_then(serde_json::Value::as_u64),
        Some(4096)
    );
    assert_eq!(
        response
            .get("stop_reason")
            .and_then(serde_json::Value::as_str),
        Some("tool_use")
    );
}

#[test]
fn runtime_anthropic_response_from_json_value_joins_shell_call_commands_for_bash_tool_use() {
    let response = runtime_anthropic_response_from_json_value(
        &serde_json::json!({
            "usage": {
                "input_tokens": 9,
                "output_tokens": 4
            },
            "output": [
                {
                    "type": "shell_call",
                    "call_id": "call_shell_multi",
                    "action": {
                        "commands": ["pwd", "ls -la"]
                    }
                }
            ]
        }),
        "claude-sonnet-4-6",
        false,
    );

    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("input"))
            .and_then(|input| input.get("command"))
            .and_then(serde_json::Value::as_str),
        Some("pwd\nls -la")
    );
}

#[test]
fn runtime_anthropic_response_from_json_value_maps_computer_call_to_computer_tool_use() {
    let response = runtime_anthropic_response_from_json_value(
        &serde_json::json!({
            "usage": {
                "input_tokens": 10,
                "output_tokens": 5
            },
            "output": [
                {
                    "type": "computer_call",
                    "call_id": "call_computer",
                    "actions": [
                        {
                            "type": "click",
                            "button": "left",
                            "x": 321,
                            "y": 654
                        }
                    ]
                }
            ]
        }),
        "claude-opus-4-6",
        false,
    );

    let content = response
        .get("content")
        .and_then(serde_json::Value::as_array)
        .expect("content should be an array");
    assert_eq!(
        content[0].get("type").and_then(serde_json::Value::as_str),
        Some("tool_use")
    );
    assert_eq!(
        content[0].get("name").and_then(serde_json::Value::as_str),
        Some("computer")
    );
    assert_eq!(
        content[0]
            .get("input")
            .and_then(|input| input.get("action"))
            .and_then(serde_json::Value::as_str),
        Some("left_click")
    );
    assert_eq!(
        content[0]
            .get("input")
            .and_then(|input| input.get("coordinate"))
            .and_then(serde_json::Value::as_array)
            .map(|coordinates| {
                coordinates
                    .iter()
                    .filter_map(serde_json::Value::as_i64)
                    .collect::<Vec<_>>()
            }),
        Some(vec![321, 654])
    );
}

#[test]
fn runtime_anthropic_response_from_json_value_preserves_raw_computer_actions_when_not_lossless() {
    let response = runtime_anthropic_response_from_json_value(
        &serde_json::json!({
            "usage": {
                "input_tokens": 8,
                "output_tokens": 4
            },
            "output": [
                {
                    "type": "computer_call",
                    "call_id": "call_computer_raw",
                    "actions": [
                        {
                            "type": "screenshot"
                        },
                        {
                            "type": "click",
                            "button": "left",
                            "x": 10,
                            "y": 20
                        }
                    ]
                }
            ]
        }),
        "claude-opus-4-6",
        false,
    );

    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("input"))
            .and_then(|input| input.get("actions"))
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
}

#[test]
fn runtime_anthropic_response_from_json_value_preserves_memory_tool_use() {
    let response = runtime_anthropic_response_from_json_value(
        &serde_json::json!({
            "usage": {
                "input_tokens": 7,
                "output_tokens": 3
            },
            "output": [
                {
                    "type": "function_call",
                    "call_id": "call_memory",
                    "name": "memory",
                    "arguments": "{\"command\":\"view\",\"path\":\"/memories\"}"
                }
            ]
        }),
        "claude-sonnet-4-6",
        false,
    );

    let content = response
        .get("content")
        .and_then(serde_json::Value::as_array)
        .expect("content should be an array");
    assert_eq!(
        content[0].get("type").and_then(serde_json::Value::as_str),
        Some("tool_use")
    );
    assert_eq!(
        content[0].get("name").and_then(serde_json::Value::as_str),
        Some("memory")
    );
    assert_eq!(
        content[0]
            .get("input")
            .and_then(|input| input.get("command"))
            .and_then(serde_json::Value::as_str),
        Some("view")
    );
}

#[test]
fn runtime_anthropic_response_from_json_value_preserves_tool_use_and_usage() {
    let response = runtime_anthropic_response_from_json_value(
        &serde_json::json!({
            "usage": {
                "input_tokens": 12,
                "output_tokens": 7,
                "input_tokens_details": {
                    "cached_tokens": 3
                }
            },
            "output": [
                {
                    "type": "reasoning",
                    "summary": [
                        {
                            "type": "summary_text",
                            "text": "Plan first",
                        }
                    ]
                },
                {
                    "type": "message",
                    "content": [
                        {
                            "type": "output_text",
                            "text": "Hello from prodex",
                        }
                    ]
                },
                {
                    "type": "function_call",
                    "call_id": "call_123",
                    "name": "shell",
                    "arguments": "{\"cmd\":\"ls\"}"
                }
            ]
        }),
        "claude-sonnet-4-6",
        true,
    );

    assert_eq!(
        response.get("type").and_then(serde_json::Value::as_str),
        Some("message")
    );
    assert_eq!(
        response.get("model").and_then(serde_json::Value::as_str),
        Some("claude-sonnet-4-6")
    );
    assert_eq!(
        response
            .get("stop_reason")
            .and_then(serde_json::Value::as_str),
        Some("tool_use")
    );
    let content = response
        .get("content")
        .and_then(serde_json::Value::as_array)
        .expect("content should be an array");
    assert_eq!(
        content[0].get("type").and_then(serde_json::Value::as_str),
        Some("thinking")
    );
    assert_eq!(
        content[1].get("text").and_then(serde_json::Value::as_str),
        Some("Hello from prodex")
    );
    assert_eq!(
        content[2].get("type").and_then(serde_json::Value::as_str),
        Some("tool_use")
    );
    assert_eq!(
        content[2].get("id").and_then(serde_json::Value::as_str),
        Some("call_123")
    );
    assert_eq!(
        response
            .get("usage")
            .and_then(|usage| usage.get("cache_read_input_tokens"))
            .and_then(serde_json::Value::as_u64),
        Some(3)
    );
    assert_eq!(
        response
            .get("usage")
            .and_then(|usage| usage.get("server_tool_use"))
            .and_then(|server_tool_use| server_tool_use.get("web_search_requests"))
            .and_then(serde_json::Value::as_u64),
        Some(0)
    );
}

#[test]
fn runtime_anthropic_response_from_json_value_counts_web_search_tool_use() {
    let response = runtime_anthropic_response_from_json_value(
        &serde_json::json!({
            "usage": {
                "input_tokens": 12,
                "output_tokens": 7
            },
            "output": [
                {
                    "type": "function_call",
                    "call_id": "call_123",
                    "name": "WebSearch",
                    "arguments": "{\"query\":\"OpenAI latest news today\"}"
                }
            ]
        }),
        "claude-sonnet-4-6",
        false,
    );

    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("tool_use")
    );
    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("WebSearch")
    );
    assert_eq!(
        response
            .get("usage")
            .and_then(|usage| usage.get("server_tool_use"))
            .and_then(|server_tool_use| server_tool_use.get("web_search_requests"))
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
}

#[test]
fn runtime_anthropic_response_from_json_value_counts_web_fetch_tool_use() {
    let response = runtime_anthropic_response_from_json_value(
        &serde_json::json!({
            "usage": {
                "input_tokens": 12,
                "output_tokens": 7
            },
            "output": [
                {
                    "type": "function_call",
                    "call_id": "call_456",
                    "name": "web_fetch",
                    "arguments": "{\"url\":\"https://example.com/article\"}"
                }
            ]
        }),
        "claude-sonnet-4-6",
        false,
    );

    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("server_tool_use")
    );
    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("web_fetch")
    );
    assert_eq!(
        response
            .get("usage")
            .and_then(|usage| usage.get("server_tool_use"))
            .and_then(|server_tool_use| server_tool_use.get("web_fetch_requests"))
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
}

#[test]
fn runtime_anthropic_response_from_json_value_counts_tool_search_tool_use() {
    let response = runtime_anthropic_response_from_json_value(
        &serde_json::json!({
            "usage": {
                "input_tokens": 12,
                "output_tokens": 7
            },
            "output": [
                {
                    "type": "function_call",
                    "call_id": "call_search",
                    "name": "tool_search_tool_regex",
                    "arguments": "{\"query\":\"browser_fetch\"}"
                }
            ]
        }),
        "claude-sonnet-4-6",
        false,
    );

    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("server_tool_use")
    );
    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("tool_search_tool_regex")
    );
    assert_eq!(
        response
            .get("usage")
            .and_then(|usage| usage.get("server_tool_use"))
            .and_then(|server_tool_use| server_tool_use.get("tool_search_requests"))
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
}

#[test]
fn runtime_anthropic_response_from_json_value_uses_registered_server_tool_aliases() {
    let mut server_tools = RuntimeAnthropicServerTools::default();
    server_tools.register("browser_fetch", "web_fetch");
    let response = runtime_anthropic_response_from_json_value_with_carried_usage(
        &serde_json::json!({
            "usage": {
                "input_tokens": 12,
                "output_tokens": 7
            },
            "output": [
                {
                    "type": "function_call",
                    "call_id": "call_alias",
                    "name": "browser_fetch",
                    "arguments": "{\"url\":\"https://example.com/article\"}"
                }
            ]
        }),
        "claude-sonnet-4-6",
        false,
        0,
        0,
        0,
        0,
        Some(&server_tools),
    );

    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("server_tool_use")
    );
    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("web_fetch")
    );
    assert_eq!(
        response
            .get("usage")
            .and_then(|usage| usage.get("server_tool_use"))
            .and_then(|server_tool_use| server_tool_use.get("web_fetch_requests"))
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
}
