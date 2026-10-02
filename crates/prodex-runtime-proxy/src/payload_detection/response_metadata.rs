use prodex_mojo_core::json::JsonStringSpan;
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
    let nodes = crate::runtime_request_json_nodes(value);
    let mut number_texts = Vec::new();
    runtime_response_metadata_number_texts(value, &mut number_texts);

    let plan = prodex_mojo_core::json::runtime_response_metadata(&nodes, &number_texts)
        .expect("Mojo response metadata returned invalid output");
    let string = |span: Option<JsonStringSpan>| {
        span.map(|span| {
            let end = span.start + span.length;
            nodes[span.node]
                .text
                .get(span.start..end)
                .expect("Mojo response metadata returned an invalid string span")
                .to_string()
        })
    };
    let response_ids = plan
        .response_ids
        .into_iter()
        .flatten()
        .map(|index| nodes[index].text.to_string())
        .collect();
    let token_usage = plan.token_usage_present.then(|| {
        let token_count = |slot: usize| {
            plan.token_usage_nodes[slot]
                .map(|index| {
                    let text = number_texts[index].as_deref().unwrap_or(nodes[index].text);
                    runtime_response_metadata_u64(text)
                })
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
        turn_state: string(plan.turn_state),
        headers_turn_state: string(plan.headers_turn_state),
        token_usage,
        event_type: string(plan.event_type),
    }
}

fn runtime_response_metadata_u64(text: &str) -> u64 {
    if text == "-0" {
        return 0;
    }
    text.parse()
        .expect("Mojo response metadata selected an invalid unsigned integer")
}

fn runtime_response_metadata_number_texts(value: &Value, number_texts: &mut Vec<Option<String>>) {
    number_texts.push(value.as_number().map(ToString::to_string));
    match value {
        Value::Array(values) => {
            for value in values {
                runtime_response_metadata_number_texts(value, number_texts);
            }
        }
        Value::Object(map) => {
            for value in map.values() {
                runtime_response_metadata_number_texts(value, number_texts);
            }
        }
        _ => {}
    }
}
