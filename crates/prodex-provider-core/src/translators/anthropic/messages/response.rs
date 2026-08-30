use super::*;

#[cfg(feature = "mojo")]
use prodex_mojo_core::rich::{
    AnthropicRequestKernelInput, AnthropicRequestKernelOperation, AnthropicResponseBlock,
    AnthropicResponseBlockKind, AnthropicResponsePlanKind, plan_anthropic_response_blocks,
};

#[derive(Clone, Copy)]
enum ResponseBlockKind {
    Text,
    ToolUse,
    WebSearchCall,
    WebSearchResult,
    Thinking,
}

struct ResponseBlockInput {
    kind: ResponseBlockKind,
    has_text: bool,
    value: Option<Value>,
}

#[derive(Clone, Copy)]
enum ResponsePlanKind {
    Message,
    ToolUse,
    WebSearchCall,
    WebSearchResult,
    Reasoning,
}

#[derive(Clone, Copy)]
struct ResponsePlanItem {
    kind: ResponsePlanKind,
    start: usize,
    count: usize,
    input_index: usize,
}

fn anthropic_response_block_input(block: &Value) -> Result<ResponseBlockInput, String> {
    let kind = match block.get("type").and_then(Value::as_str) {
        Some("text") => {
            if block.get("text").and_then(Value::as_str).is_none() {
                return Err("Anthropic text block must contain text".to_string());
            }
            ResponseBlockKind::Text
        }
        Some("tool_use") => ResponseBlockKind::ToolUse,
        Some("server_tool_use") => ResponseBlockKind::WebSearchCall,
        Some("web_search_tool_result") => ResponseBlockKind::WebSearchResult,
        Some("thinking") => ResponseBlockKind::Thinking,
        Some(kind) => {
            return Err(format!(
                "unsupported Anthropic Messages content block `{kind}`"
            ));
        }
        None => return Err("Anthropic Messages content block requires type".to_string()),
    };
    let value = match kind {
        ResponseBlockKind::ToolUse => Some(anthropic_tool_use_item(block)?),
        ResponseBlockKind::WebSearchCall => Some(anthropic_web_search_call(block)?),
        _ => None,
    };
    let has_text = match kind {
        ResponseBlockKind::Text => true,
        ResponseBlockKind::Thinking => block.get("thinking").and_then(Value::as_str).is_some(),
        _ => false,
    };
    Ok(ResponseBlockInput {
        kind,
        has_text,
        value,
    })
}

#[cfg(feature = "mojo")]
fn plan_with_mojo(inputs: &[ResponseBlockInput]) -> Result<Vec<ResponsePlanItem>, String> {
    let mojo_inputs = inputs
        .iter()
        .map(|input| AnthropicResponseBlock {
            kind: match input.kind {
                ResponseBlockKind::Text => AnthropicResponseBlockKind::Text,
                ResponseBlockKind::ToolUse => AnthropicResponseBlockKind::ToolUse,
                ResponseBlockKind::WebSearchCall => AnthropicResponseBlockKind::WebSearchCall,
                ResponseBlockKind::WebSearchResult => AnthropicResponseBlockKind::WebSearchResult,
                ResponseBlockKind::Thinking => AnthropicResponseBlockKind::Thinking,
            },
            has_text: input.has_text,
        })
        .collect::<Vec<_>>();
    plan_anthropic_response_blocks(&mojo_inputs)
        .map_err(|error| format!("Anthropic Messages response plan failed: {error:?}"))
        .map(|plan| {
            plan.into_iter()
                .map(|item| ResponsePlanItem {
                    kind: match item.kind {
                        AnthropicResponsePlanKind::Message => ResponsePlanKind::Message,
                        AnthropicResponsePlanKind::ToolUse => ResponsePlanKind::ToolUse,
                        AnthropicResponsePlanKind::WebSearchCall => ResponsePlanKind::WebSearchCall,
                        AnthropicResponsePlanKind::WebSearchResult => {
                            ResponsePlanKind::WebSearchResult
                        }
                        AnthropicResponsePlanKind::Reasoning => ResponsePlanKind::Reasoning,
                    },
                    start: item.start,
                    count: item.count,
                    input_index: item.input_index,
                })
                .collect()
        })
}

#[cfg(not(feature = "mojo"))]
fn plan_with_rust(inputs: &[ResponseBlockInput]) -> Vec<ResponsePlanItem> {
    let mut plan = Vec::new();
    let mut text_start = None;
    for (input_index, input) in inputs.iter().enumerate() {
        if matches!(input.kind, ResponseBlockKind::Text) {
            text_start.get_or_insert(input_index);
            continue;
        }
        if let Some(start) = text_start.take() {
            plan.push(ResponsePlanItem {
                kind: ResponsePlanKind::Message,
                start,
                count: input_index - start,
                input_index: 0,
            });
        }
        let kind = match input.kind {
            ResponseBlockKind::ToolUse => Some(ResponsePlanKind::ToolUse),
            ResponseBlockKind::WebSearchCall => Some(ResponsePlanKind::WebSearchCall),
            ResponseBlockKind::WebSearchResult => Some(ResponsePlanKind::WebSearchResult),
            ResponseBlockKind::Thinking if input.has_text => Some(ResponsePlanKind::Reasoning),
            ResponseBlockKind::Text | ResponseBlockKind::Thinking => None,
        };
        if let Some(kind) = kind {
            plan.push(ResponsePlanItem {
                kind,
                start: 0,
                count: 0,
                input_index,
            });
        }
    }
    if let Some(start) = text_start {
        plan.push(ResponsePlanItem {
            kind: ResponsePlanKind::Message,
            start,
            count: inputs.len() - start,
            input_index: 0,
        });
    }
    plan
}

#[cfg(feature = "mojo")]
fn render_response_message(blocks: &[Value]) -> Result<Value, String> {
    let blocks = super::json_fragment(&Value::Array(blocks.to_vec()))?;
    let mut input =
        AnthropicRequestKernelInput::new(AnthropicRequestKernelOperation::ResponseMessage);
    input.blocks = Some(&blocks);
    super::anthropic_mojo_value(input)
}

#[cfg(not(feature = "mojo"))]
fn render_response_message(blocks: &[Value]) -> Result<Value, String> {
    let content = blocks
        .iter()
        .map(|block| {
            let text = block
                .get("text")
                .and_then(Value::as_str)
                .ok_or_else(|| "Anthropic text block must contain text".to_string())?;
            Ok(json!({"type": "output_text", "text": text}))
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(json!({
        "type": "message",
        "role": "assistant",
        "content": content,
    }))
}

#[cfg(feature = "mojo")]
fn render_response_reasoning(block: &Value) -> Result<Value, String> {
    let block = super::json_fragment(block)?;
    let mut input =
        AnthropicRequestKernelInput::new(AnthropicRequestKernelOperation::ResponseReasoning);
    input.content = Some(&block);
    super::anthropic_mojo_value(input)
}

#[cfg(not(feature = "mojo"))]
fn render_response_reasoning(block: &Value) -> Result<Value, String> {
    let thinking = block
        .get("thinking")
        .and_then(Value::as_str)
        .ok_or_else(|| "Anthropic response plan referenced invalid reasoning".to_string())?;
    Ok(json!({
        "type": "reasoning",
        "summary": [{"type": "summary_text", "text": thinking}],
    }))
}

pub(super) fn anthropic_response_output(content: &[Value]) -> Result<Vec<Value>, String> {
    let inputs = content
        .iter()
        .map(anthropic_response_block_input)
        .collect::<Result<Vec<_>, _>>()?;
    #[cfg(feature = "mojo")]
    let plan = plan_with_mojo(&inputs)?;
    #[cfg(not(feature = "mojo"))]
    let plan = plan_with_rust(&inputs);

    let mut output = Vec::new();
    for item in plan {
        match item.kind {
            ResponsePlanKind::Message => {
                let end = item
                    .start
                    .checked_add(item.count)
                    .ok_or_else(|| "Anthropic response plan range overflowed".to_string())?;
                let blocks = content
                    .get(item.start..end)
                    .ok_or_else(|| "Anthropic response plan referenced invalid text".to_string())?;
                output.push(render_response_message(blocks)?);
            }
            ResponsePlanKind::ToolUse | ResponsePlanKind::WebSearchCall => {
                let value = inputs
                    .get(item.input_index)
                    .and_then(|input| input.value.clone())
                    .ok_or_else(|| "Anthropic response plan referenced invalid item".to_string())?;
                output.push(value);
            }
            ResponsePlanKind::WebSearchResult => {
                let block = content.get(item.input_index).ok_or_else(|| {
                    "Anthropic response plan referenced invalid result".to_string()
                })?;
                merge_anthropic_web_search_result(&mut output, block);
            }
            ResponsePlanKind::Reasoning => {
                let block = content.get(item.input_index).ok_or_else(|| {
                    "Anthropic response plan referenced invalid reasoning".to_string()
                })?;
                output.push(render_response_reasoning(block)?);
            }
        }
    }
    Ok(output)
}
