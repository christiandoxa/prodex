use super::json_utils::{runtime_json_find, runtime_json_string, runtime_json_u64_at};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeTokenUsage {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: u64,
}

pub fn extract_runtime_response_ids_from_payload(payload: &str) -> Vec<String> {
    serde_json::from_str::<serde_json::Value>(payload)
        .ok()
        .map(|value| extract_runtime_response_ids_from_value(&value))
        .unwrap_or_default()
}

pub fn extract_runtime_response_ids_from_body_bytes(body: &[u8]) -> Vec<String> {
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .map(|value| extract_runtime_response_ids_from_value(&value))
        .unwrap_or_default()
}

pub fn extract_runtime_turn_state_from_body_bytes(body: &[u8]) -> Option<String> {
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|value| extract_runtime_turn_state_from_value(&value))
}

pub fn extract_runtime_token_usage_from_body_bytes(body: &[u8]) -> Option<RuntimeTokenUsage> {
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|value| extract_runtime_token_usage_from_value(&value))
}

pub fn push_runtime_response_id(response_ids: &mut Vec<String>, id: Option<&str>) {
    if let Some(id) = id
        && !response_ids.iter().any(|existing| existing == id)
    {
        response_ids.push(id.to_string());
    }
}

pub fn extract_runtime_response_ids_from_value(value: &serde_json::Value) -> Vec<String> {
    let mut response_ids = Vec::new();

    push_runtime_response_id(
        &mut response_ids,
        value
            .get("response")
            .and_then(|response| response.get("id"))
            .and_then(serde_json::Value::as_str),
    );
    push_runtime_response_id(
        &mut response_ids,
        value.get("response_id").and_then(serde_json::Value::as_str),
    );

    if value
        .get("object")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|object| object == "response" || object.ends_with(".response"))
    {
        push_runtime_response_id(
            &mut response_ids,
            value.get("id").and_then(serde_json::Value::as_str),
        );
    }

    response_ids
}

pub fn extract_runtime_turn_state_from_value(value: &serde_json::Value) -> Option<String> {
    value
        .get("response")
        .and_then(|response| response.get("headers"))
        .and_then(extract_runtime_turn_state_from_headers_value)
        .or_else(|| {
            value
                .get("headers")
                .and_then(extract_runtime_turn_state_from_headers_value)
        })
        .or_else(|| {
            value
                .get("response")
                .and_then(|response| response.get("turn_state"))
                .and_then(runtime_json_string)
        })
        .or_else(|| {
            value
                .get("response")
                .and_then(|response| response.get("turnState"))
                .and_then(runtime_json_string)
        })
        .or_else(|| value.get("turn_state").and_then(runtime_json_string))
        .or_else(|| value.get("turnState").and_then(runtime_json_string))
}

pub fn extract_runtime_token_usage_from_value(
    value: &serde_json::Value,
) -> Option<RuntimeTokenUsage> {
    runtime_json_find(value, extract_runtime_token_usage_candidate)
}

fn extract_runtime_token_usage_candidate(value: &serde_json::Value) -> Option<RuntimeTokenUsage> {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(usage) = map.get("usage")
                && let Some(token_usage) = runtime_token_usage_from_usage_value(usage)
            {
                return Some(token_usage);
            }
            runtime_token_usage_from_usage_value(value)
        }
        _ => None,
    }
}

fn runtime_token_usage_from_usage_value(value: &serde_json::Value) -> Option<RuntimeTokenUsage> {
    let input_tokens = runtime_json_u64_at(value, &["input_tokens"])
        .or_else(|| runtime_json_u64_at(value, &["prompt_tokens"]));
    let cached_input_tokens = runtime_json_u64_at(value, &["cached_input_tokens"])
        .or_else(|| runtime_json_u64_at(value, &["input_tokens_details", "cached_tokens"]))
        .or_else(|| runtime_json_u64_at(value, &["input_tokens_details", "cached_input_tokens"]))
        .or_else(|| runtime_json_u64_at(value, &["prompt_tokens_details", "cached_tokens"]));
    let output_tokens = runtime_json_u64_at(value, &["output_tokens"])
        .or_else(|| runtime_json_u64_at(value, &["completion_tokens"]));
    let reasoning_tokens = runtime_json_u64_at(value, &["reasoning_tokens"])
        .or_else(|| runtime_json_u64_at(value, &["output_tokens_details", "reasoning_tokens"]))
        .or_else(|| runtime_json_u64_at(value, &["completion_tokens_details", "reasoning_tokens"]));

    if input_tokens.is_none()
        && cached_input_tokens.is_none()
        && output_tokens.is_none()
        && reasoning_tokens.is_none()
    {
        return None;
    }

    Some(RuntimeTokenUsage {
        input_tokens: input_tokens.unwrap_or_default(),
        cached_input_tokens: cached_input_tokens.unwrap_or_default(),
        output_tokens: output_tokens.unwrap_or_default(),
        reasoning_tokens: reasoning_tokens.unwrap_or_default(),
    })
}

pub fn extract_runtime_turn_state_from_headers_value(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Object(headers) => headers.iter().find_map(|(name, value)| {
            if name.eq_ignore_ascii_case("x-codex-turn-state") {
                extract_runtime_turn_state_header_value(value)
            } else {
                None
            }
        }),
        serde_json::Value::Array(headers) => headers
            .iter()
            .find_map(extract_runtime_turn_state_from_header_entry),
        _ => None,
    }
}

fn extract_runtime_turn_state_from_header_entry(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Array(items) => {
            let name = items.first()?.as_str()?;
            if !name.eq_ignore_ascii_case("x-codex-turn-state") {
                return None;
            }
            items
                .get(1)
                .and_then(extract_runtime_turn_state_header_value)
        }
        serde_json::Value::Object(entry) => {
            let name = entry
                .get("name")
                .or_else(|| entry.get("key"))
                .and_then(serde_json::Value::as_str)?;
            if !name.eq_ignore_ascii_case("x-codex-turn-state") {
                return None;
            }
            entry
                .get("value")
                .or_else(|| entry.get("values"))
                .and_then(extract_runtime_turn_state_header_value)
        }
        _ => None,
    }
}

fn extract_runtime_turn_state_header_value(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(value) => {
            let value = value.trim();
            (!value.is_empty()).then(|| value.to_string())
        }
        serde_json::Value::Array(items) => items
            .iter()
            .find_map(extract_runtime_turn_state_header_value),
        _ => None,
    }
}

pub fn runtime_response_event_type_from_value(value: &serde_json::Value) -> Option<String> {
    value.get("type").and_then(runtime_json_string)
}
