use super::{anthropic_mojo_value, anthropic_tool_name, json_fragment};
use prodex_mojo_core::rich::{AnthropicRequestKernelInput, AnthropicRequestKernelOperation};
use serde_json::{Value, json};

pub(super) fn anthropic_tools(value: &Value) -> Result<Vec<Value>, String> {
    let Some(tools) = value.as_array() else {
        return Err("Responses `tools` must be an array".to_string());
    };
    tools
        .iter()
        .map(|tool| {
            let object = tool
                .as_object()
                .ok_or_else(|| "Responses function tool must be an object".to_string())?;
            let function = object
                .get("function")
                .and_then(Value::as_object)
                .unwrap_or(object);
            let name = function
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| "Responses function tool must contain name".to_string())?;
            let name = anthropic_tool_name(function.get("namespace").and_then(Value::as_str), name);
            let input_schema = function
                .get("parameters")
                .cloned()
                .unwrap_or_else(|| json!({"type": "object", "properties": {}}));
            let name = json_fragment(&Value::String(name))?;
            let input_schema = json_fragment(&input_schema)?;
            let description = function.get("description").map(json_fragment).transpose()?;
            let mut input =
                AnthropicRequestKernelInput::new(AnthropicRequestKernelOperation::ToolDeclaration);
            input.name = Some(&name);
            input.input = Some(&input_schema);
            input.content = description.as_deref();
            anthropic_mojo_value(input)
        })
        .collect()
}

pub(super) fn anthropic_tool_choice(value: &Value) -> Result<Option<Value>, String> {
    let (choice_kind, name) = match value {
        Value::String(choice) => match choice.as_str() {
            "auto" => (1, None),
            "required" => (2, None),
            "none" => return Ok(None),
            _ => return Err(format!("unsupported Responses tool_choice `{choice}")),
        },
        Value::Object(object) if object.get("type").and_then(Value::as_str) == Some("function") => {
            let name = object
                .get("name")
                .or_else(|| {
                    object
                        .get("function")
                        .and_then(|function| function.get("name"))
                })
                .and_then(Value::as_str)
                .ok_or_else(|| "function tool_choice must contain name".to_string())?;
            (
                3,
                Some(anthropic_tool_name(
                    object.get("namespace").and_then(Value::as_str),
                    name,
                )),
            )
        }
        _ => return Err("unsupported Responses tool_choice shape".to_string()),
    };
    let name = name
        .map(|name| json_fragment(&Value::String(name)))
        .transpose()?;
    let mut input = AnthropicRequestKernelInput::new(AnthropicRequestKernelOperation::ToolChoice);
    input.choice_kind = choice_kind;
    input.name = name.as_deref();
    anthropic_mojo_value(input).map(Some)
}
