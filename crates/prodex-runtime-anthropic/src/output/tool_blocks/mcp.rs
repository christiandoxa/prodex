use super::*;

pub fn runtime_anthropic_mcp_call_blocks_from_output_item(
    item: &serde_json::Value,
) -> Vec<serde_json::Value> {
    let call_id = item
        .get("id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("mcp_call")
        .to_string();
    let name = item
        .get("name")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("mcp_tool")
        .to_string();
    let server_name = item
        .get("server_label")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("mcp")
        .to_string();
    let input = runtime_anthropic_tool_input_from_arguments(
        item.get("arguments")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("{}"),
    );

    let mut content = vec![serde_json::json!({
        "type": "mcp_tool_use",
        "id": call_id,
        "name": name,
        "server_name": server_name,
        "input": input,
    })];

    let mut result_content = Vec::new();
    if let Some(output) = item
        .get("output")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        result_content.push(serde_json::json!({
            "type": "text",
            "text": output,
        }));
    }
    let is_error = item
        .get("error")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|error| {
            result_content.push(serde_json::json!({
                "type": "text",
                "text": error,
            }));
            true
        })
        .unwrap_or(false);
    if !result_content.is_empty() {
        content.push(serde_json::json!({
            "type": "mcp_tool_result",
            "tool_use_id": content
                .first()
                .and_then(|block| block.get("id"))
                .cloned()
                .unwrap_or_else(|| serde_json::Value::String("mcp_call".to_string())),
            "is_error": is_error,
            "content": result_content,
        }));
    }

    content
}

pub fn runtime_anthropic_mcp_approval_request_block_from_output_item(
    item: &serde_json::Value,
) -> serde_json::Value {
    let id = item
        .get("id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("mcp_approval_request");
    let name = item
        .get("name")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("mcp_tool");
    let server_name = item
        .get("server_label")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("mcp");
    let arguments = item
        .get("arguments")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .unwrap_or("{}");
    let input = runtime_anthropic_tool_input_from_arguments(arguments);
    serde_json::json!({
        "type": "mcp_approval_request",
        "id": id,
        "name": name,
        "server_name": server_name,
        "server_label": server_name,
        "arguments": arguments,
        "input": input,
    })
}

pub fn runtime_anthropic_mcp_list_tools_block_from_output_item(
    item: &serde_json::Value,
) -> serde_json::Value {
    let id = item
        .get("id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("mcp_list_tools");
    let server_name = item
        .get("server_label")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("mcp");
    let mut block = serde_json::Map::new();
    block.insert(
        "type".to_string(),
        serde_json::Value::String("mcp_list_tools".to_string()),
    );
    block.insert("id".to_string(), serde_json::Value::String(id.to_string()));
    block.insert(
        "server_name".to_string(),
        serde_json::Value::String(server_name.to_string()),
    );
    block.insert(
        "server_label".to_string(),
        serde_json::Value::String(server_name.to_string()),
    );
    if let Some(tools) = item.get("tools").filter(|value| value.is_array()).cloned() {
        block.insert("tools".to_string(), tools);
    }
    if let Some(error) = item
        .get("error")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        block.insert(
            "error".to_string(),
            serde_json::Value::String(error.to_string()),
        );
    }
    serde_json::Value::Object(block)
}
