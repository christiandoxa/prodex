use super::*;

#[test]
fn translate_runtime_anthropic_messages_request_keeps_versioned_builtin_client_tools() {
    let _env_guard = TestEnvVarGuard::unset("PRODEX_CLAUDE_NATIVE_CLIENT_TOOLS");
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "tool_choice": {
                "type": "tool",
                "name": "computer_20250124"
            },
            "tools": [
                {
                    "type": "bash_20250124",
                    "description": "Run shell commands"
                },
                {
                    "type": "text_editor_20250124",
                    "description": "Edit local files"
                },
                {
                    "type": "computer_20250124",
                    "display_width_px": 1440,
                    "display_height_px": 900,
                    "display_number": 1
                },
                {
                    "type": "memory_20250818",
                    "description": "Persist project memory"
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Open the browser and inspect the page"
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

    assert_eq!(tools.len(), 4);
    assert_eq!(
        tools
            .iter()
            .map(|tool| tool.get("type").and_then(serde_json::Value::as_str))
            .collect::<Vec<_>>(),
        vec![
            Some("function"),
            Some("function"),
            Some("function"),
            Some("function"),
        ]
    );
    assert_eq!(
        tools
            .iter()
            .map(|tool| tool.get("name").and_then(serde_json::Value::as_str))
            .collect::<Vec<_>>(),
        vec![
            Some("bash"),
            Some("str_replace_based_edit_tool"),
            Some("computer"),
            Some("memory"),
        ]
    );
    assert_eq!(
        tools[0]
            .get("description")
            .and_then(serde_json::Value::as_str),
        Some("Run shell commands")
    );
    assert_eq!(
        tools[1]
            .get("description")
            .and_then(serde_json::Value::as_str),
        Some("Edit local files")
    );
    assert_eq!(
        tools[2]
            .get("description")
            .and_then(serde_json::Value::as_str),
        Some(
            "Interact with the graphical computer display. Display resolution: 1440x900 pixels. Display number: 1."
        )
    );
    assert_eq!(
        tools[3]
            .get("description")
            .and_then(serde_json::Value::as_str),
        Some("Persist project memory")
    );
    assert_eq!(
        tools[0]
            .get("parameters")
            .and_then(|parameters| parameters.get("properties"))
            .and_then(|properties| properties.get("command"))
            .and_then(|property| property.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("string")
    );
    assert_eq!(
        tools[1]
            .get("parameters")
            .and_then(|parameters| parameters.get("properties"))
            .and_then(|properties| properties.get("command"))
            .and_then(|property| property.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("string")
    );
    assert_eq!(
        tools[1]
            .get("parameters")
            .and_then(|parameters| parameters.get("properties"))
            .and_then(|properties| properties.get("old_str"))
            .and_then(|property| property.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("string")
    );
    assert!(
        tools[1]
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
        tools[1]
            .get("parameters")
            .and_then(|parameters| parameters.get("properties"))
            .and_then(|properties| properties.get("insert_text"))
            .and_then(|property| property.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("string")
    );
    assert_eq!(
        tools[2]
            .get("parameters")
            .and_then(|parameters| parameters.get("properties"))
            .and_then(|properties| properties.get("action"))
            .and_then(|property| property.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("string")
    );
    assert_eq!(
        tools[2]
            .get("parameters")
            .and_then(|parameters| parameters.get("properties"))
            .and_then(|properties| properties.get("coordinate"))
            .and_then(|property| property.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("array")
    );
    assert_eq!(
        tools[3]
            .get("parameters")
            .and_then(|parameters| parameters.get("properties"))
            .and_then(|properties| properties.get("command"))
            .and_then(|property| property.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("string")
    );
    assert!(
        tools[3]
            .get("parameters")
            .and_then(|parameters| parameters.get("properties"))
            .and_then(|properties| properties.get("command"))
            .and_then(|property| property.get("enum"))
            .and_then(serde_json::Value::as_array)
            .is_some_and(|values| values.iter().any(|value| value.as_str() == Some("rename")))
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

