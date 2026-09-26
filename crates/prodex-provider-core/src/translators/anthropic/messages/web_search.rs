use super::super::anthropic_mojo_value;
use super::json_fragment;
use prodex_mojo_core::rich::{AnthropicRequestKernelInput, AnthropicRequestKernelOperation};
use serde_json::Value;

#[cfg(feature = "mojo")]
pub(super) fn anthropic_web_search_call(block: &Value) -> Result<Value, String> {
    let Some(id) = block.get("id").and_then(Value::as_str) else {
        return Err("Anthropic server_tool_use block must contain id".to_string());
    };
    if block.get("name").and_then(Value::as_str) != Some("web_search") {
        return Err("unsupported Anthropic server tool".to_string());
    }
    let block = json_fragment(block)?;
    let id = json_fragment(&Value::String(id.to_string()))?;
    let mut input =
        AnthropicRequestKernelInput::new(AnthropicRequestKernelOperation::WebSearchCall);
    input.id = Some(&id);
    input.content = Some(&block);
    input.choice_kind = 0;
    anthropic_mojo_value(input)
}

#[cfg(feature = "mojo")]
pub(super) fn merge_anthropic_web_search_result(
    output: &mut [Value],
    block: &Value,
) -> Result<(), String> {
    let blocks = json_fragment(&Value::Array(output.to_vec()))?;
    let content = json_fragment(block)?;
    let mut input =
        AnthropicRequestKernelInput::new(AnthropicRequestKernelOperation::WebSearchResult);
    input.choice_kind = 1;
    input.blocks = Some(&blocks);
    input.content = Some(&content);
    let merged = anthropic_mojo_value(input)?;
    let merged = merged
        .as_array()
        .ok_or_else(|| "Anthropic web-search result kernel returned a non-array".to_string())?;
    if merged.len() != output.len() {
        return Err("Anthropic web-search result kernel changed output count".to_string());
    }
    output.clone_from_slice(merged);
    Ok(())
}

pub(super) fn anthropic_web_search_result_sources(block: &Value) -> Result<Vec<Value>, String> {
    let content = json_fragment(block)?;
    let mut input =
        AnthropicRequestKernelInput::new(AnthropicRequestKernelOperation::WebSearchResult);
    input.content = Some(&content);
    let sources = anthropic_mojo_value(input)?;
    sources
        .as_array()
        .cloned()
        .ok_or_else(|| "Anthropic web-search source kernel returned a non-array".to_string())
}

pub(super) fn anthropic_web_search_stream_item(
    id: &str,
    input_json: &str,
    sources: &[Value],
    in_progress: bool,
) -> Result<Value, String> {
    let id = json_fragment(&Value::String(id.to_string()))?;
    let sources = json_fragment(&Value::Array(sources.to_vec()))?;
    let mut input =
        AnthropicRequestKernelInput::new(AnthropicRequestKernelOperation::WebSearchCall);
    input.stream = true;
    input.choice_kind = if in_progress { 1 } else { 0 };
    input.id = Some(&id);
    input.input = Some(input_json);
    input.blocks = Some(&sources);
    anthropic_mojo_value(input)
}
