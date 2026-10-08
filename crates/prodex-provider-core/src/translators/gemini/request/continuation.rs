//! Gemini continuation metadata comes from the Mojo request-content policy.

use std::collections::BTreeMap;

use serde_json::Value;

use prodex_mojo_core::provider_constraints::{
    GeminiRequestContentKernelInput, GeminiRequestContentOperation, gemini_request_content_kernel,
};

pub(crate) fn gemini_continuation_metadata(
    headers: &BTreeMap<String, String>,
    request: &serde_json::Map<String, Value>,
) -> Option<Value> {
    // Generic shallow ABI projection: only field names and JSON string-type
    // flags cross to Mojo. Large body members/headers never enter the 4-MiB
    // JSON kernel. Mojo alone selects keys and chooses header/request precedence.
    let header_fields: BTreeMap<&str, bool> =
        headers.keys().map(|key| (key.as_str(), true)).collect();
    let request_fields: BTreeMap<&str, bool> = request
        .iter()
        .map(|(key, value)| (key.as_str(), value.is_string()))
        .collect();
    let header_fields = serde_json::to_vec(&header_fields).expect("Gemini header names serialize");
    let request_fields = serde_json::to_vec(&request_fields).expect("Gemini field names serialize");
    let mut input =
        GeminiRequestContentKernelInput::new(GeminiRequestContentOperation::ContinuationMetadata);
    input.primary = Some(&header_fields);
    input.secondary = Some(&request_fields);
    let body = gemini_request_content_kernel(input)
        .expect("Mojo Gemini continuation metadata plan returned an invalid status");
    let plan: Option<BTreeMap<String, String>> =
        serde_json::from_slice(&body).expect("Mojo Gemini continuation plan must be JSON");
    let plan = plan?;
    let mut metadata = serde_json::Map::new();
    for (key, source) in plan {
        let value = match source.as_str() {
            "headers" => headers.get(&key).map(|value| Value::String(value.clone())),
            "request" => request.get(&key).cloned(),
            _ => panic!("Mojo Gemini continuation metadata plan returned an invalid source"),
        }
        .expect("Mojo Gemini continuation metadata plan referred to a missing field");
        metadata.insert(key, value);
    }
    (!metadata.is_empty()).then_some(Value::Object(metadata))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn mojo_continuation_metadata_filters_and_orders_provider_fields() {
        let headers = BTreeMap::from([
            ("x-codex-turn-state".into(), "state:α\"quoted".into()),
            ("session_id".into(), "".into()),
            ("authorization".into(), "must-not-forward".into()),
        ]);
        let request = json!({
            "previous_response_id": "resp-A\nB",
            "extra": "must-not-forward",
            "input": []
        });
        let metadata = gemini_continuation_metadata(&headers, request.as_object().unwrap());
        assert_eq!(
            metadata,
            Some(json!({
                "x-codex-turn-state": "state:α\"quoted",
                "session_id": "",
                "previous_response_id": "resp-A\nB",
            }))
        );
    }

    #[test]
    fn mojo_continuation_metadata_handles_large_unrelated_production_input() {
        let request = json!({
            "input": "x".repeat(5 * 1024 * 1024),
            "previous_response_id": "resp-large",
        });
        assert_eq!(
            gemini_continuation_metadata(&BTreeMap::new(), request.as_object().unwrap()),
            Some(json!({"previous_response_id": "resp-large"})),
        );
    }

    #[test]
    fn mojo_continuation_metadata_preserves_absent_and_wrong_type_inputs() {
        let empty = serde_json::Map::new();
        assert_eq!(gemini_continuation_metadata(&BTreeMap::new(), &empty), None);
        let request = json!({"previous_response_id": 3});
        assert_eq!(
            gemini_continuation_metadata(&BTreeMap::new(), request.as_object().unwrap()),
            None,
        );
        let headers = BTreeMap::from([("session_id".into(), "id-Ω".into())]);
        let request = json!({"previous_response_id": null, "session_id": "wrong-layer"});
        assert_eq!(
            gemini_continuation_metadata(&headers, request.as_object().unwrap()),
            Some(json!({"session_id": "id-Ω"})),
        );
        let request = json!({"previous_response_id": ""});
        assert_eq!(
            gemini_continuation_metadata(&BTreeMap::new(), request.as_object().unwrap()),
            Some(json!({"previous_response_id": ""})),
        );
    }
}
