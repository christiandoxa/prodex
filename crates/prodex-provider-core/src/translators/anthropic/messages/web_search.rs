#[cfg(feature = "mojo")]
use super::super::anthropic_mojo_value;
#[cfg(feature = "mojo")]
use super::json_fragment;
#[cfg(feature = "mojo")]
use prodex_mojo_core::rich::{AnthropicRequestKernelInput, AnthropicRequestKernelOperation};
use serde_json::{Value, json};

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
pub(super) fn merge_anthropic_web_search_result(output: &mut [Value], block: &Value) {
    let Some(tool_use_id) = block.get("tool_use_id").and_then(Value::as_str) else {
        return;
    };
    let sources = anthropic_web_search_sources(block);
    let Some(call) = output.iter_mut().rev().find(|item| {
        item.get("type").and_then(Value::as_str) == Some("web_search_call")
            && item.get("id").and_then(Value::as_str) == Some(tool_use_id)
    }) else {
        return;
    };
    call["action"]["sources"] = Value::Array(sources);
}

#[cfg(feature = "mojo")]
fn anthropic_web_search_sources(block: &Value) -> Vec<Value> {
    block
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|result| {
            let url = result.get("url").and_then(Value::as_str)?;
            let mut source = json!({"type": "url", "url": url});
            if let Some(title) = result.get("title").and_then(Value::as_str) {
                source["title"] = Value::String(title.to_string());
            }
            Some(source)
        })
        .collect()
}
