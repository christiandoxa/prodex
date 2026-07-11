use self::params::{
    deepseek_insert_primitive_request_fields, deepseek_stop_from_request,
    deepseek_top_logprobs_from_request, deepseek_user_id_from_request,
};
use super::tooling::deepseek_messages_from_request;
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[path = "request/params.rs"]
mod params;

type DeepSeekRequestBody = (Vec<u8>, Option<BTreeMap<String, Value>>);

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

pub(super) fn deepseek_request_body_from_responses(
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
    let body = serde_json::to_vec(&Value::Object(request)).expect("deepseek request serializes");
    Ok((body, degraded))
}
