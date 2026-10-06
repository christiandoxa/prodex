use super::*;

use prodex_mojo_core::rich::{
    AnthropicRequestKernelInput, AnthropicRequestKernelOperation, AnthropicResponseBlock,
    AnthropicResponseBlockClassificationError, AnthropicResponseBlockClassificationInput,
    AnthropicResponseBlockKind, AnthropicResponsePlanItem, AnthropicResponsePlanKind,
    plan_anthropic_response_blocks,
};

fn response_plan_with_mojo(
    content: &[Value],
) -> Result<
    (
        Vec<AnthropicResponseBlock>,
        Vec<Option<Value>>,
        Vec<AnthropicResponsePlanItem>,
    ),
    String,
> {
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
        .zip(&plan.blocks)
        .map(|(block, classified)| match classified.kind {
            AnthropicResponseBlockKind::ToolUse => anthropic_tool_use_item(block).map(Some),
            AnthropicResponseBlockKind::WebSearchCall => anthropic_web_search_call(block).map(Some),
            _ => Ok(None),
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
    Ok((plan.blocks, inputs, plan.items))
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
    let (_, inputs, plan) = response_plan_with_mojo(content)?;

    let mut output = Vec::new();
    for item in plan {
        match item.kind {
            AnthropicResponsePlanKind::Message => {
                let end = item
                    .start
                    .checked_add(item.count)
                    .ok_or_else(|| "Anthropic response plan range overflowed".to_string())?;
                let blocks = content
                    .get(item.start..end)
                    .ok_or_else(|| "Anthropic response plan referenced invalid text".to_string())?;
                output.push(render_response_message(blocks)?);
            }
            AnthropicResponsePlanKind::ToolUse | AnthropicResponsePlanKind::WebSearchCall => {
                let value = inputs
                    .get(item.input_index)
                    .and_then(Clone::clone)
                    .ok_or_else(|| "Anthropic response plan referenced invalid item".to_string())?;
                output.push(value);
            }
            AnthropicResponsePlanKind::WebSearchResult => {
                let block = content.get(item.input_index).ok_or_else(|| {
                    "Anthropic response plan referenced invalid result".to_string()
                })?;
                merge_anthropic_web_search_result(&mut output, block)?;
            }
            AnthropicResponsePlanKind::Reasoning => {
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
        let (actual_blocks, _, actual_plan) = response_plan_with_mojo(&valid).unwrap();
        assert_eq!(
            actual_blocks
                .iter()
                .map(|block| (block.kind, block.has_text))
                .collect::<Vec<_>>(),
            vec![
                (AnthropicResponseBlockKind::Text, true),
                (AnthropicResponseBlockKind::ToolUse, false),
                (AnthropicResponseBlockKind::WebSearchCall, false),
                (AnthropicResponseBlockKind::WebSearchResult, false),
                (AnthropicResponseBlockKind::Thinking, true),
                (AnthropicResponseBlockKind::Thinking, false),
                (AnthropicResponseBlockKind::Thinking, false),
            ]
        );
        assert_eq!(
            actual_plan,
            vec![
                AnthropicResponsePlanItem {
                    kind: AnthropicResponsePlanKind::Message,
                    start: 0,
                    count: 1,
                    input_index: 0
                },
                AnthropicResponsePlanItem {
                    kind: AnthropicResponsePlanKind::ToolUse,
                    start: 0,
                    count: 0,
                    input_index: 1
                },
                AnthropicResponsePlanItem {
                    kind: AnthropicResponsePlanKind::WebSearchCall,
                    start: 0,
                    count: 0,
                    input_index: 2
                },
                AnthropicResponsePlanItem {
                    kind: AnthropicResponsePlanKind::WebSearchResult,
                    start: 0,
                    count: 0,
                    input_index: 3
                },
                AnthropicResponsePlanItem {
                    kind: AnthropicResponsePlanKind::Reasoning,
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
