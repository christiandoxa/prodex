use super::*;

pub fn runtime_anthropic_computer_key_combo_from_output_action(
    action: &serde_json::Map<String, serde_json::Value>,
) -> Option<String> {
    let keys = action
        .get("keys")
        .and_then(serde_json::Value::as_array)?
        .iter()
        .filter_map(|key| {
            key.as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| value.to_ascii_lowercase())
        })
        .collect::<Vec<_>>();
    (!keys.is_empty()).then_some(keys.join("+"))
}

pub fn runtime_anthropic_computer_tool_input_from_output_item(
    item: &serde_json::Value,
) -> Option<serde_json::Value> {
    #[cfg(feature = "mojo")]
    {
        let actions = item.get("actions")?.as_array()?;
        if actions.len() != 1 {
            return None;
        }
        let action = actions.first()?.as_object()?;
        let action_type = action
            .get("type")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let button = action
            .get("button")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let text = action
            .get("text")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let coordinates = action
            .get("x")
            .and_then(runtime_proxy_anthropic_coordinate_component)
            .zip(
                action
                    .get("y")
                    .and_then(runtime_proxy_anthropic_coordinate_component),
            )
            .and_then(|(x, y)| serde_json::to_string(&[x, y]).ok());
        let keys = action
            .get("keys")
            .and_then(serde_json::Value::as_array)
            .map(|keys| {
                keys.iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .collect::<Vec<_>>()
                    .join("+")
            })
            .filter(|keys| !keys.is_empty());
        let mut input = prodex_mojo_core::rich::RuntimeAnthropicKernelInput::new(
            prodex_mojo_core::rich::RuntimeAnthropicKernelOperation::ComputerToolInput,
        );
        input.block_type = action_type;
        input.name = button;
        input.text = text;
        input.input = coordinates.as_deref();
        input.output = keys.as_deref();
        let value = crate::mojo::json(input);
        (!value.is_null()).then_some(value)
    }
    #[cfg(not(feature = "mojo"))]
    runtime_anthropic_computer_tool_input_from_output_item_rust(item)
}

#[cfg(any(not(feature = "mojo"), test))]
fn runtime_anthropic_computer_tool_input_from_output_item_rust(
    item: &serde_json::Value,
) -> Option<serde_json::Value> {
    let actions = item.get("actions").and_then(serde_json::Value::as_array)?;
    if actions.len() != 1 {
        return None;
    }
    let action = actions.first()?.as_object()?;
    let action_type = action
        .get("type")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let input = match action_type {
        "screenshot" => serde_json::json!({ "action": "screenshot" }),
        "click" => {
            let button = action
                .get("button")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("left");
            let button_action = match button {
                "left" => "left_click",
                "right" => "right_click",
                "middle" => "middle_click",
                _ => return None,
            };
            let x = runtime_proxy_anthropic_coordinate_component(action.get("x")?)?;
            let y = runtime_proxy_anthropic_coordinate_component(action.get("y")?)?;
            serde_json::json!({
                "action": button_action,
                "coordinate": [x, y],
            })
        }
        "double_click" => {
            let x = runtime_proxy_anthropic_coordinate_component(action.get("x")?)?;
            let y = runtime_proxy_anthropic_coordinate_component(action.get("y")?)?;
            serde_json::json!({
                "action": "double_click",
                "coordinate": [x, y],
            })
        }
        "move" => {
            let x = runtime_proxy_anthropic_coordinate_component(action.get("x")?)?;
            let y = runtime_proxy_anthropic_coordinate_component(action.get("y")?)?;
            serde_json::json!({
                "action": "mouse_move",
                "coordinate": [x, y],
            })
        }
        "type" => serde_json::json!({
            "action": "type",
            "text": action
                .get("text")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())?,
        }),
        "keypress" => serde_json::json!({
            "action": "key",
            "key": runtime_anthropic_computer_key_combo_from_output_action(action)?,
        }),
        "wait" => serde_json::json!({
            "action": "wait",
        }),
        _ => return None,
    };
    Some(input)
}

pub fn runtime_anthropic_raw_computer_tool_input_from_output_item(
    item: &serde_json::Value,
) -> serde_json::Value {
    item.get("actions")
        .filter(|value| value.is_array())
        .cloned()
        .map(|actions| serde_json::json!({ "actions": actions }))
        .unwrap_or_else(|| serde_json::json!({}))
}

#[cfg(feature = "mojo")]
pub fn runtime_anthropic_computer_tool_use_block_from_output_item(
    item: &serde_json::Value,
) -> serde_json::Value {
    let call_id = item
        .get("call_id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("computer_call");
    let input = runtime_anthropic_computer_tool_input_from_output_item(item)
        .unwrap_or_else(|| runtime_anthropic_raw_computer_tool_input_from_output_item(item));
    let input = serde_json::to_string(&input).expect("Anthropic computer input serializes");
    crate::mojo::json({
        let mut kernel_input = prodex_mojo_core::rich::RuntimeAnthropicKernelInput::new(
            prodex_mojo_core::rich::RuntimeAnthropicKernelOperation::ToolUseBlock,
        );
        kernel_input.id = Some(call_id);
        kernel_input.name = Some("computer");
        kernel_input.input = Some(&input);
        kernel_input
    })
}

#[cfg(all(test, feature = "mojo"))]
mod tests {
    use super::*;

    #[test]
    fn mojo_computer_tool_inputs_match_rust_oracle() {
        let cases = [
            serde_json::json!({"actions":[{"type":"screenshot"}]}),
            serde_json::json!({"actions":[{"type":"click","x":-1,"y":2}]}),
            serde_json::json!({"actions":[{"type":"click","button":"right","x":3,"y":4}]}),
            serde_json::json!({"actions":[{"type":"click","button":"middle","x":5,"y":6}]}),
            serde_json::json!({"actions":[{"type":"double_click","x":7,"y":8}]}),
            serde_json::json!({"actions":[{"type":"move","x":9,"y":10}]}),
            serde_json::json!({"actions":[{"type":"type","text":" \u{1f980}\n "}]}),
            serde_json::json!({"actions":[{"type":"keypress","keys":[" CTRL ",null,"Shift","\u{1f980}"]}]}),
            serde_json::json!({"actions":[{"type":"wait"}]}),
            serde_json::json!({"actions":[{"type":"click","button":"other","x":1,"y":2}]}),
            serde_json::json!({"actions":[]}),
            serde_json::json!({}),
        ];
        for item in cases {
            assert_eq!(
                runtime_anthropic_computer_tool_input_from_output_item(&item),
                runtime_anthropic_computer_tool_input_from_output_item_rust(&item),
                "{item}"
            );
        }
    }
}

#[cfg(not(feature = "mojo"))]
pub fn runtime_anthropic_computer_tool_use_block_from_output_item(
    item: &serde_json::Value,
) -> serde_json::Value {
    let call_id = item
        .get("call_id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("computer_call");
    serde_json::json!({
        "type": "tool_use",
        "id": call_id,
        "name": "computer",
        "input": runtime_anthropic_computer_tool_input_from_output_item(item)
            .unwrap_or_else(|| runtime_anthropic_raw_computer_tool_input_from_output_item(item)),
    })
}
