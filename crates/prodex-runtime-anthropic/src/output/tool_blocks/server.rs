use super::*;

pub fn runtime_anthropic_server_tool_use_block(
    call_id: &str,
    tool_name: &str,
    input: serde_json::Value,
    server_tools: Option<&RuntimeAnthropicServerTools>,
) -> Option<serde_json::Value> {
    runtime_anthropic_server_tool_registration_for_call(tool_name, server_tools).map(
        |(response_name, block_type)| {
            if block_type == "mcp_tool_use" {
                let mut input = input;
                let server_name = input
                    .get("server_name")
                    .and_then(serde_json::Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string);
                if let Some(object) = input.as_object_mut() {
                    object.remove("server_name");
                }
                let mut block = serde_json::Map::new();
                block.insert(
                    "type".to_string(),
                    serde_json::Value::String("mcp_tool_use".to_string()),
                );
                block.insert(
                    "id".to_string(),
                    serde_json::Value::String(call_id.to_string()),
                );
                block.insert("name".to_string(), serde_json::Value::String(response_name));
                block.insert("input".to_string(), input);
                if let Some(server_name) = server_name {
                    block.insert(
                        "server_name".to_string(),
                        serde_json::Value::String(server_name),
                    );
                }
                serde_json::Value::Object(block)
            } else {
                serde_json::json!({
                    "type": "server_tool_use",
                    "id": call_id,
                    "name": response_name,
                    "input": input,
                })
            }
        },
    )
}
