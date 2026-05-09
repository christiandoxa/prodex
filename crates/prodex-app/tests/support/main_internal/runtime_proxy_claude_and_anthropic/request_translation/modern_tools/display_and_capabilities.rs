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

