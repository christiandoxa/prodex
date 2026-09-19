use self::params::{
    deepseek_insert_primitive_request_fields, deepseek_stop_from_request,
    deepseek_top_logprobs_from_request, deepseek_user_id_from_request,
};
#[cfg(not(feature = "mojo"))]
use super::tooling::deepseek_messages_from_request;
use serde_json::Value;
#[cfg(not(feature = "mojo"))]
use serde_json::json;
use std::collections::BTreeMap;

#[cfg(feature = "mojo")]
use prodex_mojo_core::rich::{DeepSeekKernelInput, DeepSeekKernelOperation};

#[path = "request/params.rs"]
mod params;

type DeepSeekRequestBody = (Vec<u8>, Option<BTreeMap<String, Value>>);

#[cfg(not(feature = "mojo"))]
pub(super) fn deepseek_tool_choice_from_request(value: &Value) -> Option<Value> {
    let choice = value.get("tool_choice")?;
    if let Some(choice) = choice.as_str() {
        return matches!(choice, "auto" | "none" | "required")
            .then(|| Value::String(choice.to_string()));
    }
    let object = choice.as_object()?;
    let choice_type = object
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if choice_type != "function" {
        return None;
    }
    let name = object
        .get("name")
        .and_then(Value::as_str)
        .or_else(|| {
            object
                .get("function")
                .and_then(|function| function.get("name"))
                .and_then(Value::as_str)
        })
        .filter(|name| !name.trim().is_empty())?;
    Some(json!({
        "type": "function",
        "function": {
            "name": name,
        },
    }))
}

#[cfg(feature = "mojo")]
fn deepseek_common_request_body_from_responses_mojo(
    obj: &serde_json::Map<String, Value>,
    value: &Value,
) -> Result<DeepSeekRequestBody, String> {
    let mut validated = serde_json::Map::new();
    deepseek_insert_primitive_request_fields(value, &mut validated)?;
    let _ = deepseek_top_logprobs_from_request(value)?;
    let _ = deepseek_stop_from_request(value)?;
    let user_id = deepseek_user_id_from_request(value)?;

    let mut degraded = None;
    let response_format_mode = if let Some(response_format) = obj.get("response_format") {
        match response_format
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("text")
        {
            "text" => 0_u64,
            "json_object" => 1_u64,
            "json_schema" | "json" | "structured_output" => {
                degraded = Some({
                    let mut map = BTreeMap::new();
                    map.insert("from".to_string(), Value::String("json_schema".to_string()));
                    map.insert("to".to_string(), Value::String("json_object".to_string()));
                    map
                });
                1_u64
            }
            other => {
                return Err(format!(
                    "DeepSeek response_format type \x60{other}\x60 is not supported"
                ));
            }
        }
    } else {
        0_u64
    };
    let instructions = value
        .get("instructions")
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty());
    let canonical = serde_json::to_string(value)
        .map_err(|error| format!("DeepSeek request serialization failed: {error}"))?;
    let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::RawCommonRequest);
    input.input = Some(&canonical);
    input.content = user_id.as_deref();
    input.reasoning_content = instructions;
    input.sequence_number = response_format_mode;
    let body = super::deepseek_mojo_body(input);
    Ok((body, degraded))
}

pub(super) fn deepseek_request_body_from_responses(
    obj: &serde_json::Map<String, Value>,
    value: &Value,
) -> Result<DeepSeekRequestBody, String> {
    #[cfg(feature = "mojo")]
    {
        deepseek_common_request_body_from_responses_mojo(obj, value)
    }
    #[cfg(not(feature = "mojo"))]
    {
        deepseek_request_body_from_responses_rust(obj, value)
    }
}

#[cfg(not(feature = "mojo"))]
fn deepseek_request_body_from_responses_rust(
    obj: &serde_json::Map<String, Value>,
    value: &Value,
) -> Result<DeepSeekRequestBody, String> {
    let mut request = serde_json::Map::new();
    request.insert(
        "model".to_string(),
        Value::String(
            obj.get("model")
                .and_then(Value::as_str)
                .unwrap_or("deepseek-chat")
                .to_string(),
        ),
    );
    request.insert(
        "stream".to_string(),
        Value::Bool(obj.get("stream").and_then(Value::as_bool).unwrap_or(false)),
    );
    request.insert(
        "messages".to_string(),
        Value::Array(deepseek_messages_from_request(value)),
    );
    if let Some(tools) = obj.get("tools").and_then(Value::as_array) {
        let function_tools: Vec<Value> = tools
            .iter()
            .filter(|tool| tool.get("type").and_then(Value::as_str) == Some("function"))
            .cloned()
            .collect();
        if !function_tools.is_empty() {
            request.insert("tools".to_string(), Value::Array(function_tools));
        }
    }
    if let Some(tool_choice) = deepseek_tool_choice_from_request(value) {
        request.insert("tool_choice".to_string(), tool_choice);
    }
    deepseek_insert_primitive_request_fields(value, &mut request)?;
    if let Some(top_logprobs) = deepseek_top_logprobs_from_request(value)? {
        request.insert("top_logprobs".to_string(), top_logprobs);
    }
    if let Some(stop) = deepseek_stop_from_request(value)? {
        request.insert("stop".to_string(), stop);
    }
    if let Some(user_id) = deepseek_user_id_from_request(value)? {
        request.insert("user_id".to_string(), Value::String(user_id));
    }
    let mut degraded = None;
    if let Some(response_format) = obj.get("response_format") {
        match response_format
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("text")
        {
            "text" => {}
            "json_object" => {
                request.insert(
                    "response_format".to_string(),
                    json!({"type": "json_object"}),
                );
            }
            "json_schema" | "json" | "structured_output" => {
                request.insert(
                    "response_format".to_string(),
                    json!({"type": "json_object"}),
                );
                degraded = Some({
                    let mut map = BTreeMap::new();
                    map.insert("from".to_string(), Value::String("json_schema".to_string()));
                    map.insert("to".to_string(), Value::String("json_object".to_string()));
                    map
                });
            }
            other => {
                return Err(format!(
                    "DeepSeek response_format type `{other}` is not supported"
                ));
            }
        }
    }
    #[cfg(feature = "mojo")]
    {
        let messages = serde_json::to_string(request.get("messages").expect("messages present"))
            .expect("DeepSeek request messages serialize");
        let tools = request
            .get("tools")
            .map(|value| serde_json::to_string(value).expect("DeepSeek request tools serialize"));
        let tool_choice = request.get("tool_choice").map(|value| {
            serde_json::to_string(value).expect("DeepSeek request tool choice serializes")
        });
        let extra = request
            .iter()
            .filter(|(key, _)| {
                !matches!(
                    key.as_str(),
                    "model" | "stream" | "messages" | "tools" | "tool_choice"
                )
            })
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect::<serde_json::Map<_, _>>();
        let extra = serde_json::to_string(&Value::Object(extra))
            .expect("DeepSeek request extra fields serialize");
        let model = request
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or("deepseek-chat");
        let stream = request
            .get("stream")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::RequestBody);
        input.model = Some(model);
        input.stream = stream;
        input.messages = Some(&messages);
        input.tools = tools.as_deref();
        input.tool_choice = tool_choice.as_deref();
        input.extra = Some(&extra);
        let body = super::deepseek_mojo_body(input);
        Ok((body, degraded))
    }
    #[cfg(not(feature = "mojo"))]
    {
        let body =
            serde_json::to_vec(&Value::Object(request)).expect("deepseek request serializes");
        Ok((body, degraded))
    }
}
