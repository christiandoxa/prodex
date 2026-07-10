//! DeepSeek request parameter validation and mapping.

use serde_json::Value;

pub(super) fn deepseek_insert_primitive_request_fields(
    value: &Value,
    request: &mut serde_json::Map<String, Value>,
) -> Result<(), String> {
    for field in ["temperature", "top_p"] {
        if let Some(next) = value.get(field) {
            if !next.is_number() {
                return Err(format!("DeepSeek {field} must be a number"));
            }
            request.insert(field.to_string(), next.clone());
        }
    }
    for field in ["max_output_tokens", "max_tokens", "max_completion_tokens"] {
        if let Some(next) = value.get(field) {
            if next.as_u64().is_none_or(|count| count == 0) {
                return Err(format!("DeepSeek {field} must be a positive integer"));
            }
            request.insert("max_tokens".to_string(), next.clone());
        }
    }
    if let Some(logprobs) = value.get("logprobs") {
        if !logprobs.is_boolean() {
            return Err("DeepSeek logprobs must be a boolean".to_string());
        }
        request.insert("logprobs".to_string(), logprobs.clone());
    }
    Ok(())
}

pub(super) fn deepseek_top_logprobs_from_request(value: &Value) -> Result<Option<Value>, String> {
    let Some(top_logprobs) = value.get("top_logprobs") else {
        return Ok(None);
    };
    let Some(count) = top_logprobs.as_u64() else {
        return Err("DeepSeek top_logprobs must be an integer".to_string());
    };
    if count > 20 {
        return Err("DeepSeek top_logprobs must be <= 20".to_string());
    }
    if value.get("logprobs").and_then(Value::as_bool) != Some(true) {
        return Err("DeepSeek top_logprobs requires logprobs=true".to_string());
    }
    Ok(Some(top_logprobs.clone()))
}

pub(super) fn deepseek_stop_from_request(value: &Value) -> Result<Option<Value>, String> {
    let Some(stop) = value
        .get("stop")
        .or_else(|| value.get("stop_sequences"))
        .or_else(|| value.get("stopSequences"))
    else {
        return Ok(None);
    };
    if stop.as_str().is_some() {
        return Ok(Some(stop.clone()));
    }
    let Some(stops) = stop.as_array() else {
        return Err("DeepSeek stop must be a string or array of strings".to_string());
    };
    if stops.len() > 16 {
        return Err("DeepSeek supports at most 16 stop sequences".to_string());
    }
    if stops.iter().any(|stop| !stop.is_string()) {
        return Err("DeepSeek stop sequences must be strings".to_string());
    }
    Ok(Some(stop.clone()))
}

pub(super) fn deepseek_user_id_from_request(value: &Value) -> Result<Option<String>, String> {
    let Some(user_id) = value
        .get("user_id")
        .or_else(|| value.get("user"))
        .or_else(|| value.get("safety_identifier"))
    else {
        return Ok(None);
    };
    let Some(user_id) = user_id.as_str() else {
        return Err("DeepSeek user_id must be a string".to_string());
    };
    let user_id = user_id.trim();
    if user_id.is_empty() {
        return Ok(None);
    }
    if user_id.len() > 512
        || !user_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(
            "DeepSeek user_id must use only letters, numbers, underscores, or dashes and be at most 512 bytes".to_string(),
        );
    }
    Ok(Some(user_id.to_string()))
}
