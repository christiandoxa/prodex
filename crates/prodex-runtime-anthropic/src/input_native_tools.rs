use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeAnthropicNativeClientToolKind {
    Shell,
    Computer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeAnthropicNativeClientToolCall {
    pub kind: RuntimeAnthropicNativeClientToolKind,
    pub max_output_length: Option<u64>,
}

pub fn runtime_proxy_translate_anthropic_shell_tool_call(
    block: &serde_json::Value,
) -> Option<(
    String,
    serde_json::Value,
    RuntimeAnthropicNativeClientToolCall,
)> {
    let name = block
        .get("name")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    if runtime_proxy_anthropic_client_tool_name(name) != Some("bash") {
        return None;
    }
    let call_id = block
        .get("id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?
        .to_string();
    let input = block.get("input")?.as_object()?;
    if input
        .get("restart")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        return None;
    }
    let command = input
        .get("command")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let mut action = serde_json::Map::new();
    action.insert(
        "commands".to_string(),
        serde_json::json!([command.to_string()]),
    );
    if let Some(timeout) = input.get("timeout_ms").and_then(serde_json::Value::as_u64) {
        action.insert(
            "timeout_ms".to_string(),
            serde_json::Value::Number(timeout.into()),
        );
    }
    if let Some(max_output_length) = input
        .get("max_output_length")
        .and_then(serde_json::Value::as_u64)
    {
        action.insert(
            "max_output_length".to_string(),
            serde_json::Value::Number(max_output_length.into()),
        );
    }
    Some((
        call_id.clone(),
        serde_json::json!({
            "type": "shell_call",
            "call_id": call_id,
            "action": serde_json::Value::Object(action),
            "status": "completed",
        }),
        RuntimeAnthropicNativeClientToolCall {
            kind: RuntimeAnthropicNativeClientToolKind::Shell,
            max_output_length: input
                .get("max_output_length")
                .and_then(serde_json::Value::as_u64),
        },
    ))
}

pub fn runtime_proxy_anthropic_coordinate_component(value: &serde_json::Value) -> Option<i64> {
    value.as_i64().or_else(|| {
        value
            .as_u64()
            .and_then(|component| i64::try_from(component).ok())
    })
}

pub fn runtime_proxy_anthropic_coordinate_pair(
    value: Option<&serde_json::Value>,
) -> Option<(i64, i64)> {
    let coordinates = value?.as_array()?;
    if coordinates.len() < 2 {
        return None;
    }
    let x = runtime_proxy_anthropic_coordinate_component(coordinates.first()?)?;
    let y = runtime_proxy_anthropic_coordinate_component(coordinates.get(1)?)?;
    Some((x, y))
}

#[cfg(any(not(feature = "mojo"), test))]
fn runtime_proxy_anthropic_computer_keypress_keys(key_combo: &str) -> Option<Vec<String>> {
    let keys = key_combo
        .split('+')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_uppercase())
        .collect::<Vec<_>>();
    (!keys.is_empty()).then_some(keys)
}

pub fn runtime_proxy_translate_anthropic_computer_action(
    input: &serde_json::Map<String, serde_json::Value>,
) -> Option<serde_json::Value> {
    #[cfg(feature = "mojo")]
    {
        let action = input
            .get("action")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let text = input
            .get("text")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let key = input
            .get("key")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let coordinate = runtime_proxy_anthropic_coordinate_pair(input.get("coordinate"))
            .and_then(|(x, y)| serde_json::to_string(&[x, y]).ok());
        let mut kernel = prodex_mojo_core::rich::RuntimeAnthropicKernelInput::new(
            prodex_mojo_core::rich::RuntimeAnthropicKernelOperation::ComputerAction,
        );
        kernel.block_type = action;
        kernel.text = text;
        kernel.name = key;
        kernel.input = coordinate.as_deref();
        let value = crate::mojo::json(kernel);
        (!value.is_null()).then_some(value)
    }
    #[cfg(not(feature = "mojo"))]
    runtime_proxy_translate_anthropic_computer_action_rust(input)
}

#[cfg(any(not(feature = "mojo"), test))]
fn runtime_proxy_translate_anthropic_computer_action_rust(
    input: &serde_json::Map<String, serde_json::Value>,
) -> Option<serde_json::Value> {
    let action = input
        .get("action")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    match action {
        "screenshot" => Some(serde_json::json!({ "type": "screenshot" })),
        "left_click" | "right_click" | "middle_click" => {
            let (x, y) = runtime_proxy_anthropic_coordinate_pair(input.get("coordinate"))?;
            let button = match action {
                "left_click" => "left",
                "right_click" => "right",
                "middle_click" => "middle",
                _ => unreachable!(),
            };
            Some(serde_json::json!({
                "type": "click",
                "button": button,
                "x": x,
                "y": y,
            }))
        }
        "double_click" => {
            let (x, y) = runtime_proxy_anthropic_coordinate_pair(input.get("coordinate"))?;
            Some(serde_json::json!({
                "type": "double_click",
                "x": x,
                "y": y,
            }))
        }
        "mouse_move" => {
            let (x, y) = runtime_proxy_anthropic_coordinate_pair(input.get("coordinate"))?;
            Some(serde_json::json!({
                "type": "move",
                "x": x,
                "y": y,
            }))
        }
        "type" => input
            .get("text")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|text| {
                serde_json::json!({
                    "type": "type",
                    "text": text,
                })
            }),
        "key" => input
            .get("key")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .and_then(runtime_proxy_anthropic_computer_keypress_keys)
            .map(|keys| {
                serde_json::json!({
                    "type": "keypress",
                    "keys": keys,
                })
            }),
        "wait" => Some(serde_json::json!({ "type": "wait" })),
        _ => None,
    }
}

pub fn runtime_proxy_translate_anthropic_computer_tool_call(
    block: &serde_json::Value,
) -> Option<(
    String,
    serde_json::Value,
    RuntimeAnthropicNativeClientToolCall,
)> {
    let name = block
        .get("name")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    if runtime_proxy_anthropic_client_tool_name(name) != Some("computer") {
        return None;
    }
    let call_id = block
        .get("id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?
        .to_string();
    let input = block.get("input")?.as_object()?;
    let action = runtime_proxy_translate_anthropic_computer_action(input)?;
    Some((
        call_id.clone(),
        serde_json::json!({
            "type": "computer_call",
            "call_id": call_id,
            "actions": [action],
            "status": "completed",
        }),
        RuntimeAnthropicNativeClientToolCall {
            kind: RuntimeAnthropicNativeClientToolKind::Computer,
            max_output_length: None,
        },
    ))
}

#[cfg(all(test, feature = "mojo"))]
mod tests {
    use super::*;

    #[test]
    fn mojo_computer_actions_match_rust_oracle() {
        let cases = [
            serde_json::json!({"action":"screenshot"}),
            serde_json::json!({"action":"left_click","coordinate":[-1,2]}),
            serde_json::json!({"action":"right_click","coordinate":[i64::MAX,i64::MIN,3]}),
            serde_json::json!({"action":"middle_click","coordinate":[0,0]}),
            serde_json::json!({"action":"double_click","coordinate":[3,4]}),
            serde_json::json!({"action":"mouse_move","coordinate":[5,6]}),
            serde_json::json!({"action":"type","text":" \u{1f980}\n "}),
            serde_json::json!({"action":"key","key":" ctrl + Shift + \u{1f980} "}),
            serde_json::json!({"action":"wait"}),
            serde_json::json!({"action":"key","key":" + "}),
            serde_json::json!({"action":"left_click","coordinate":[1]}),
            serde_json::json!({"action":"unknown"}),
            serde_json::json!({}),
        ];
        for value in cases {
            let input = value.as_object().unwrap();
            assert_eq!(
                runtime_proxy_translate_anthropic_computer_action(input),
                runtime_proxy_translate_anthropic_computer_action_rust(input),
                "{value}"
            );
        }
    }
}
