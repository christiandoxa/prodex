use super::*;

use prodex_mojo_core::rich::{
    AnthropicRequestKernelInput, AnthropicRequestKernelOperation,
    AnthropicResponseBlockClassificationError, AnthropicResponseBlockClassificationInput,
    AnthropicResponseBlockKind, AnthropicResponsePlanKind, plan_anthropic_response_blocks,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResponseBlockKind {
    Text,
    ToolUse,
    WebSearchCall,
    WebSearchResult,
    Thinking,
}

#[derive(Debug, PartialEq)]
struct ResponseBlockInput {
    kind: ResponseBlockKind,
    has_text: bool,
    value: Option<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResponsePlanKind {
    Message,
    ToolUse,
    WebSearchCall,
    WebSearchResult,
    Reasoning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ResponsePlanItem {
    kind: ResponsePlanKind,
    start: usize,
    count: usize,
    input_index: usize,
}

fn response_block_input(
    block: &Value,
    kind: ResponseBlockKind,
    has_text: bool,
) -> Result<ResponseBlockInput, String> {
    let value = match kind {
        ResponseBlockKind::ToolUse => Some(anthropic_tool_use_item(block)?),
        ResponseBlockKind::WebSearchCall => Some(anthropic_web_search_call(block)?),
        _ => None,
    };
    Ok(ResponseBlockInput {
        kind,
        has_text,
        value,
    })
}

fn response_plan_with_mojo(
    content: &[Value],
) -> Result<(Vec<ResponseBlockInput>, Vec<ResponsePlanItem>), String> {
    let classification_input = content
        .iter()
        .map(|block| AnthropicResponseBlockClassificationInput {
            type_name: block.get("type").and_then(Value::as_str),
            has_text_field: block.get("text").and_then(Value::as_str).is_some(),
            has_thinking_field: block.get("thinking").and_then(Value::as_str).is_some(),
            has_id_field: block.get("id").and_then(Value::as_str).is_some(),
            has_name_field: block.get("name").and_then(Value::as_str).is_some(),
            name_is_web_search: block.get("name").and_then(Value::as_str) == Some("web_search"),
        })
        .collect::<Vec<_>>();
    let plan = plan_anthropic_response_blocks(&classification_input)
        .map_err(|error| format!("Anthropic Messages response plan failed: {error:?}"))?;

    let inputs = content
        .iter()
        .zip(plan.blocks)
        .map(|(block, classified)| {
            let kind = match classified.kind {
                AnthropicResponseBlockKind::Text => ResponseBlockKind::Text,
                AnthropicResponseBlockKind::ToolUse => ResponseBlockKind::ToolUse,
                AnthropicResponseBlockKind::WebSearchCall => ResponseBlockKind::WebSearchCall,
                AnthropicResponseBlockKind::WebSearchResult => ResponseBlockKind::WebSearchResult,
                AnthropicResponseBlockKind::Thinking => ResponseBlockKind::Thinking,
            };
            response_block_input(block, kind, classified.has_text)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if let Some(error) = plan.issue {
        return Err(match error {
            AnthropicResponseBlockClassificationError::MissingType { .. } => {
                "Anthropic Messages content block requires type".to_string()
            }
            AnthropicResponseBlockClassificationError::UnsupportedType { index } => content
                .get(index)
                .and_then(|block| block.get("type"))
                .and_then(Value::as_str)
                .map(|kind| format!("unsupported Anthropic Messages content block `{kind}`"))
                .unwrap_or_else(|| {
                    "Anthropic Messages response classifier returned an invalid block index"
                        .to_string()
                }),
            AnthropicResponseBlockClassificationError::MissingText { .. } => {
                "Anthropic text block must contain text".to_string()
            }
            AnthropicResponseBlockClassificationError::MissingToolUseId { .. } => {
                "Anthropic tool_use block must contain id".to_string()
            }
            AnthropicResponseBlockClassificationError::MissingToolUseName { .. } => {
                "Anthropic tool_use block must contain name".to_string()
            }
            AnthropicResponseBlockClassificationError::MissingServerToolId { .. } => {
                "Anthropic server_tool_use block must contain id".to_string()
            }
            AnthropicResponseBlockClassificationError::UnsupportedServerTool { .. } => {
                "unsupported Anthropic server tool".to_string()
            }
        });
    }
    let items = plan
        .items
        .into_iter()
        .map(|item| ResponsePlanItem {
            kind: match item.kind {
                AnthropicResponsePlanKind::Message => ResponsePlanKind::Message,
                AnthropicResponsePlanKind::ToolUse => ResponsePlanKind::ToolUse,
                AnthropicResponsePlanKind::WebSearchCall => ResponsePlanKind::WebSearchCall,
                AnthropicResponsePlanKind::WebSearchResult => ResponsePlanKind::WebSearchResult,
                AnthropicResponsePlanKind::Reasoning => ResponsePlanKind::Reasoning,
            },
            start: item.start,
            count: item.count,
            input_index: item.input_index,
        })
        .collect();
    Ok((inputs, items))
}

fn render_response_message(blocks: &[Value]) -> Result<Value, String> {
    let blocks = super::json_fragment(&Value::Array(blocks.to_vec()))?;
    let mut input =
        AnthropicRequestKernelInput::new(AnthropicRequestKernelOperation::ResponseMessage);
    input.blocks = Some(&blocks);
    super::anthropic_mojo_value(input)
}

fn render_response_reasoning(block: &Value) -> Result<Value, String> {
    let block = super::json_fragment(block)?;
    let mut input =
        AnthropicRequestKernelInput::new(AnthropicRequestKernelOperation::ResponseReasoning);
    input.content = Some(&block);
    super::anthropic_mojo_value(input)
}

pub(super) fn anthropic_response_output(content: &[Value]) -> Result<Vec<Value>, String> {
    let (inputs, plan) = response_plan_with_mojo(content)?;

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
                merge_anthropic_web_search_result(&mut output, block)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mojo_response_block_classification_and_plan_match_expected_values() {
        let valid = vec![
            json!({"type": "text", "text": ""}),
            json!({"type": "tool_use", "id": "call_test", "name": "read_file", "input": {}}),
            json!({"type": "server_tool_use", "id": "search_test", "name": "web_search", "input": {"query": "release"}}),
            json!({"type": "web_search_tool_result", "tool_use_id": "search_test", "content": []}),
            json!({"type": "thinking", "thinking": ""}),
            json!({"type": "thinking", "thinking": false}),
            json!({"type": "thinking"}),
        ];
        let (actual_inputs, actual_plan) = response_plan_with_mojo(&valid).unwrap();
        assert_eq!(
            actual_inputs
                .iter()
                .map(|input| (input.kind, input.has_text))
                .collect::<Vec<_>>(),
            vec![
                (ResponseBlockKind::Text, true),
                (ResponseBlockKind::ToolUse, false),
                (ResponseBlockKind::WebSearchCall, false),
                (ResponseBlockKind::WebSearchResult, false),
                (ResponseBlockKind::Thinking, true),
                (ResponseBlockKind::Thinking, false),
                (ResponseBlockKind::Thinking, false),
            ]
        );
        assert_eq!(
            actual_plan,
            vec![
                ResponsePlanItem {
                    kind: ResponsePlanKind::Message,
                    start: 0,
                    count: 1,
                    input_index: 0
                },
                ResponsePlanItem {
                    kind: ResponsePlanKind::ToolUse,
                    start: 0,
                    count: 0,
                    input_index: 1
                },
                ResponsePlanItem {
                    kind: ResponsePlanKind::WebSearchCall,
                    start: 0,
                    count: 0,
                    input_index: 2
                },
                ResponsePlanItem {
                    kind: ResponsePlanKind::WebSearchResult,
                    start: 0,
                    count: 0,
                    input_index: 3
                },
                ResponsePlanItem {
                    kind: ResponsePlanKind::Reasoning,
                    start: 0,
                    count: 0,
                    input_index: 4
                },
            ]
        );

        for (content, expected) in [
            (
                vec![json!({})],
                "Anthropic Messages content block requires type",
            ),
            (
                vec![json!({"type": 1})],
                "Anthropic Messages content block requires type",
            ),
            (
                vec![json!({"type": "future_block"})],
                "unsupported Anthropic Messages content block `future_block`",
            ),
            (
                vec![json!({"type": "text", "text": false})],
                "Anthropic text block must contain text",
            ),
            (
                vec![json!({"type": "tool_use"})],
                "Anthropic tool_use block must contain id",
            ),
            (
                vec![json!({"type": "tool_use", "id": "call_test"})],
                "Anthropic tool_use block must contain name",
            ),
            (
                vec![json!({"type": "server_tool_use"})],
                "Anthropic server_tool_use block must contain id",
            ),
            (
                vec![json!({"type": "server_tool_use", "id": "search_test", "name": "other"})],
                "unsupported Anthropic server tool",
            ),
            (
                vec![json!({"type": "tool_use"}), json!({"type": "future_block"})],
                "Anthropic tool_use block must contain id",
            ),
        ] {
            assert_eq!(
                response_plan_with_mojo(&content).unwrap_err(),
                expected,
                "{content:?}",
            );
        }

        let oversized_tool = json!({
            "type": "tool_use",
            "id": "call_test",
            "name": "read_file",
            "input": {"payload": "x".repeat(4 * 1024 * 1024)},
        });
        let content = vec![oversized_tool, json!({"type": "future_block"})];
        assert!(
            response_plan_with_mojo(&content)
                .unwrap_err()
                .starts_with("Anthropic request kernel failed:"),
            "materialization failure must precede a later classification issue"
        );
    }
}
