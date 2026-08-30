use super::tool_shapes::{anthropic_tool_choice, anthropic_tools};
use super::{AnthropicChatRequest, DEFAULT_MAX_TOKENS, anthropic_web_search_tool, json_fragment};
use prodex_mojo_core::rich::{AnthropicRequestKernelInput, AnthropicRequestKernelOperation};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

pub(super) fn build_anthropic_chat_request(
    system: &[String],
    messages: Vec<Value>,
    chat: &Map<String, Value>,
) -> Result<AnthropicChatRequest, String> {
    let model_value = chat
        .get("model")
        .cloned()
        .unwrap_or_else(|| Value::String("auto".to_string()));
    let max_tokens_value = chat
        .get("max_tokens")
        .cloned()
        .unwrap_or_else(|| Value::from(DEFAULT_MAX_TOKENS));
    let model = json_fragment(&model_value)?;
    let messages = json_fragment(&Value::Array(messages))?;
    let max_tokens = json_fragment(&max_tokens_value)?;
    let system_text = system.join("\n\n");
    let system = (!system_text.is_empty())
        .then(|| json_fragment(&Value::String(system_text)))
        .transpose()?;
    let temperature = chat.get("temperature").map(json_fragment).transpose()?;
    let top_p = chat.get("top_p").map(json_fragment).transpose()?;
    let stop_sequences = if let Some(stop) = chat.get("stop") {
        let stop = match stop {
            Value::String(_) => Value::Array(vec![stop.clone()]),
            Value::Array(_) => stop.clone(),
            _ => return Err("Responses `stop` must be a string or array".to_string()),
        };
        Some(json_fragment(&stop)?)
    } else {
        None
    };

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
    input.stream = chat.get("stream").and_then(Value::as_bool).unwrap_or(false);
    input.model = Some(&model);
    input.messages = Some(&messages);
    input.max_tokens = Some(&max_tokens);
    input.system = system.as_deref();
    input.temperature = temperature.as_deref();
    input.top_p = top_p.as_deref();
    input.stop_sequences = stop_sequences.as_deref();
    input.tools = tools.as_deref();
    input.tool_choice = tool_choice.as_deref();
    let request = super::anthropic_mojo_value(input)?;
    let request = request
        .as_object()
        .cloned()
        .ok_or_else(|| "Anthropic request kernel returned a non-object".to_string())?;
    Ok((request, degradation_details))
}
