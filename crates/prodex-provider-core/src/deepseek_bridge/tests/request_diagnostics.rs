use crate::deepseek_bridge::*;
use serde_json::{Value, json};

#[test]
fn compiled_request_diagnostics_reach_all_provider_validation_callers() {
    type Validate = fn(&Value, &str) -> Result<(), String>;
    let cases: [(Validate, Value, &str); 12] = [
        (
            deepseek_provider_core_reject_unsupported_request_fields,
            json!({"frequency_penalty": 0.5, "include": false}),
            "frequency_penalty is deprecated and is not forwarded by Prodex",
        ),
        (
            deepseek_provider_core_reject_unsupported_request_fields,
            json!({"truncation": "東京🧭"}),
            "truncation `東京🧭` is not supported",
        ),
        (
            deepseek_provider_core_reject_unsupported_request_fields,
            json!({"text": {"東京🧭": true}}),
            "text.東京🧭 is not supported by this Responses adapter",
        ),
        (
            deepseek_provider_core_reject_beta_completion_fields,
            json!({"prefix": true}),
            "chat prefix completion requires the beta chat endpoint, which is outside this Responses adapter",
        ),
        (
            deepseek_provider_core_validate_reasoning_shape,
            json!({"reasoning": {"東京🧭": true}}),
            "reasoning.東京🧭 is not supported by this Responses adapter",
        ),
        (
            |value, label| {
                deepseek_provider_core_insert_primitive_request_fields(
                    value,
                    &mut serde_json::Map::new(),
                    label,
                )
            },
            json!({"temperature": "bad", "logprobs": "bad"}),
            "temperature must be a number",
        ),
        (
            |value, label| {
                deepseek_provider_core_top_logprobs_from_responses_request(value, label).map(|_| ())
            },
            json!({"top_logprobs": 21}),
            "top_logprobs must be <= 20",
        ),
        (
            |value, label| {
                deepseek_provider_core_stop_from_responses_request(value, label).map(|_| ())
            },
            json!({"stop": [42]}),
            "stop sequences must be strings",
        ),
        (
            |value, label| deepseek_provider_core_validate_tool_choice_shape(value, false, label),
            json!({"tool_choice": "東京🧭"}),
            "tool_choice string `東京🧭` is not supported",
        ),
        (
            |value, label| {
                deepseek_provider_core_response_format_from_responses_request(value, label)
                    .map(|_| ())
            },
            json!({"response_format": {"type": "東京🧭"}}),
            "response_format type `東京🧭` is not supported",
        ),
        (
            |value, label| {
                deepseek_provider_core_response_metadata_from_responses_request(
                    value, label, "provider",
                )
                .map(|_| ())
            },
            json!({"metadata": [], "client_metadata": []}),
            "request metadata must be an object",
        ),
        (
            |value, label| {
                deepseek_provider_core_response_metadata_from_responses_request(
                    value, label, "provider",
                )
                .map(|_| ())
            },
            json!({"metadata": {}, "client_metadata": [], "prompt_cache_key": 7}),
            "client_metadata must be an object",
        ),
    ];
    for label in ["DeepSeek", "Kiro", "Gemini"] {
        for (validate, input, expected) in &cases {
            assert_eq!(
                validate(input, label),
                Err(format!("{label} {expected}")),
                "{input}"
            );
        }
        assert_eq!(
            deepseek_provider_core_response_metadata_from_responses_request(
                &json!({"metadata": {"provider": []}, "client_metadata": []}),
                label,
                "provider"
            ),
            Err(format!(
                "{label} request metadata.provider must be an object"
            )),
        );
    }
}
