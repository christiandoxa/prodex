use serde_json::Value;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeTokenUsage {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: u64,
}

struct RuntimeResponseMetadata {
    response_ids: Vec<String>,
    turn_state: Option<String>,
    headers_turn_state: Option<String>,
    token_usage: Option<RuntimeTokenUsage>,
    event_type: Option<String>,
}

pub fn extract_runtime_response_ids_from_payload(payload: &str) -> Vec<String> {
    serde_json::from_str::<Value>(payload)
        .ok()
        .map(|value| runtime_response_metadata_from_value(&value).response_ids)
        .unwrap_or_default()
}

pub fn extract_runtime_response_ids_from_body_bytes(body: &[u8]) -> Vec<String> {
    serde_json::from_slice::<Value>(body)
        .ok()
        .map(|value| runtime_response_metadata_from_value(&value).response_ids)
        .unwrap_or_default()
}

pub fn extract_runtime_turn_state_from_body_bytes(body: &[u8]) -> Option<String> {
    serde_json::from_slice::<Value>(body)
        .ok()
        .and_then(|value| runtime_response_metadata_from_value(&value).turn_state)
}

pub fn extract_runtime_token_usage_from_body_bytes(body: &[u8]) -> Option<RuntimeTokenUsage> {
    serde_json::from_slice::<Value>(body)
        .ok()
        .and_then(|value| runtime_response_metadata_from_value(&value).token_usage)
}

pub fn extract_runtime_response_ids_from_value(value: &Value) -> Vec<String> {
    runtime_response_metadata_from_value(value).response_ids
}

pub fn extract_runtime_turn_state_from_value(value: &Value) -> Option<String> {
    runtime_response_metadata_from_value(value).turn_state
}

pub fn extract_runtime_token_usage_from_value(value: &Value) -> Option<RuntimeTokenUsage> {
    runtime_response_metadata_from_value(value).token_usage
}

pub fn extract_runtime_turn_state_from_headers_value(value: &Value) -> Option<String> {
    runtime_response_metadata_from_value(value).headers_turn_state
}

pub fn runtime_response_event_type_from_value(value: &Value) -> Option<String> {
    runtime_response_metadata_from_value(value).event_type
}

fn runtime_response_metadata_from_value(value: &Value) -> RuntimeResponseMetadata {
    let raw = serde_json::to_string(value).expect("runtime response value serializes");
    let plan = prodex_mojo_core::json::runtime_response_metadata_json(&raw)
        .expect("Mojo response metadata returned invalid output");

    let decode_string = |span: Option<(usize, usize)>, trim: bool| {
        span.map(|(start, end)| {
            let value = serde_json::from_str::<String>(&raw[start..end])
                .expect("Mojo response metadata selected a string token");
            if trim {
                value.trim().to_string()
            } else {
                value
            }
        })
    };
    let response_ids = plan
        .response_ids
        .into_iter()
        .flatten()
        .map(|(start, end)| {
            serde_json::from_str::<String>(&raw[start..end])
                .expect("Mojo response metadata selected a response-id string")
        })
        .collect();
    let token_usage = plan.token_usage_present.then(|| {
        let token_count = |slot: usize| {
            plan.token_usage_spans[slot]
                .map(|(start, end)| runtime_response_metadata_u64(&raw[start..end]))
                .unwrap_or_default()
        };
        RuntimeTokenUsage {
            input_tokens: token_count(0),
            cached_input_tokens: token_count(1),
            output_tokens: token_count(2),
            reasoning_tokens: token_count(3),
        }
    });

    RuntimeResponseMetadata {
        response_ids,
        turn_state: decode_string(plan.turn_state, true),
        headers_turn_state: decode_string(plan.headers_turn_state, true),
        token_usage,
        event_type: decode_string(plan.event_type, true),
    }
}

fn runtime_response_metadata_u64(text: &str) -> u64 {
    if text == "-0" {
        return 0;
    }
    text.parse()
        .expect("Mojo response metadata selected an invalid unsigned integer")
}
