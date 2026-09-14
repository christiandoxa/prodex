use super::tool_shapes::{anthropic_tool_choice, anthropic_tools};
use super::{AnthropicChatRequest, anthropic_web_search_tool, json_fragment};
use prodex_mojo_core::rich::{AnthropicRequestKernelInput, AnthropicRequestKernelOperation};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

pub(super) fn build_anthropic_chat_request(
    system: &[String],
    messages: Vec<Value>,
    chat: &Map<String, Value>,
) -> Result<AnthropicChatRequest, String> {
    let chat_json = json_fragment(&Value::Object(chat.clone()))?;
    let messages = json_fragment(&Value::Array(messages))?;
    let system_text = system.join("\n\n");
    let system = (!system_text.is_empty())
        .then(|| json_fragment(&Value::String(system_text)))
        .transpose()?;
    let mut degradation_details = BTreeMap::new();
    let mut tools = match chat.get("tools") {
        Some(tools) => anthropic_tools(tools)?,
        None => Vec::new(),
    };
    if let Some(options) = chat.get("web_search_options") {
        let (tool, ignored_context_size) = anthropic_web_search_tool(options)?;
        tools.push(tool);
        if let Some(context_size) = ignored_context_size {
            degradation_details.insert(
                "web_search_options.search_context_size".to_string(),
                json!({"from": context_size, "to": "provider_default"}),
            );
        }
    }
    let tool_choice = if let Some(choice) = chat.get("tool_choice") {
        match anthropic_tool_choice(choice)? {
            Some(choice) => Some(json_fragment(&choice)?),
            None => {
                tools.clear();
                degradation_details.clear();
                None
            }
        }
    } else {
        None
    };
    let tools = (!tools.is_empty())
        .then(|| json_fragment(&Value::Array(tools)))
        .transpose()?;

    let mut input = AnthropicRequestKernelInput::new(AnthropicRequestKernelOperation::RequestBody);
    input.content = Some(&chat_json);
    input.messages = Some(&messages);
    input.system = system.as_deref();
    input.tools = tools.as_deref();
    input.tool_choice = tool_choice.as_deref();
    let output = super::super::anthropic_mojo_body(input)?;
    let Some((&kind, body)) = output.split_first() else {
        return Err("Anthropic request kernel returned an empty result".to_string());
    };
    if kind == 2 {
        return String::from_utf8(body.to_vec()).map(Err).map_err(|error| {
            format!("Anthropic request kernel returned invalid UTF-8: {error}")
        })?;
    }
    if kind != 1 {
        return Err("Anthropic request kernel returned an invalid result".to_string());
    }
    let request: Value = serde_json::from_slice(body)
        .map_err(|error| format!("Anthropic request kernel returned invalid JSON: {error}"))?;
    let request = request
        .as_object()
        .cloned()
        .ok_or_else(|| "Anthropic request kernel returned a non-object".to_string())?;
    Ok((request, degradation_details))
}
