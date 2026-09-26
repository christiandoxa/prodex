//! DeepSeek request translation.

use std::collections::BTreeMap;

use super::deepseek_passthrough_endpoint;
use crate::translator::{ProviderTransformInput, ProviderTransformResult};
use crate::{ProviderEndpoint, ProviderId, ProviderWireFormat};
use prodex_mojo_core::rich::{DeepSeekKernelInput, DeepSeekKernelOperation};
use serde_json::Value;

type DeepSeekRequestBody = (Vec<u8>, Option<BTreeMap<String, Value>>);

fn deepseek_request_body_from_responses(
    obj: &serde_json::Map<String, Value>,
    value: &Value,
) -> Result<DeepSeekRequestBody, String> {
    let canonical = serde_json::to_string(value)
        .map_err(|error| format!("DeepSeek request serialization failed: {error}"))?;
    crate::deepseek_bridge::deepseek_provider_core_validate_responses_request_params(
        &canonical, "DeepSeek",
    )?;
    let user_id = crate::deepseek_bridge::deepseek_provider_core_user_id_from_responses_request(
        value, "DeepSeek",
    )?;

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
                    "DeepSeek response_format type `{other}` is not supported"
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
    let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::RawCommonRequest);
    input.input = Some(&canonical);
    input.content = user_id.as_deref();
    input.reasoning_content = instructions;
    input.sequence_number = response_format_mode;
    let body = prodex_mojo_core::rich::deepseek_kernel(input)
        .map_err(|error| format!("DeepSeek request kernel failed: {error:?}"))?;
    Ok((body, degraded))
}

pub(super) fn deepseek_transform_request(
    provider: ProviderId,
    input: ProviderTransformInput,
) -> ProviderTransformResult {
    if deepseek_passthrough_endpoint(input.endpoint) {
        return ProviderTransformResult::lossless(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiResponses,
            ProviderWireFormat::OpenAiChatCompletions,
            input.body,
        );
    }
    if !matches!(
        input.endpoint,
        ProviderEndpoint::Responses | ProviderEndpoint::ResponsesCompact
    ) {
        return ProviderTransformResult::unsupported(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiResponses,
            ProviderWireFormat::OpenAiChatCompletions,
            format!(
                "DeepSeek translator does not support {}",
                input.endpoint.label()
            ),
        );
    }
    let value: Value = match serde_json::from_slice(&input.body) {
        Ok(value) => value,
        Err(error) => {
            return ProviderTransformResult::rejected(
                provider,
                input.endpoint,
                ProviderWireFormat::OpenAiResponses,
                ProviderWireFormat::OpenAiChatCompletions,
                format!("failed to parse Responses request JSON: {error}"),
            );
        }
    };
    let Some(obj) = value.as_object() else {
        return ProviderTransformResult::rejected(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiResponses,
            ProviderWireFormat::OpenAiChatCompletions,
            "DeepSeek request body must be a JSON object",
        );
    };
    if matches!(
        obj.get("parallel_tool_calls").and_then(Value::as_bool),
        Some(false)
    ) {
        return ProviderTransformResult::rejected(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiResponses,
            ProviderWireFormat::OpenAiChatCompletions,
            "DeepSeek does not expose a compatible parallel_tool_calls=false control",
        );
    }
    let (body, degraded) = match deepseek_request_body_from_responses(obj, &value) {
        Ok(result) => result,
        Err(reason) => {
            return ProviderTransformResult::rejected(
                provider,
                input.endpoint,
                ProviderWireFormat::OpenAiResponses,
                ProviderWireFormat::OpenAiChatCompletions,
                reason,
            );
        }
    };
    let result = if let Some(details) = degraded {
        ProviderTransformResult::degraded(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiResponses,
            ProviderWireFormat::OpenAiChatCompletions,
            body,
            "DeepSeek degrades JSON schema output to json_object",
            details,
        )
    } else {
        ProviderTransformResult::lossless(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiResponses,
            ProviderWireFormat::OpenAiChatCompletions,
            body,
        )
    };
    let mut metadata = serde_json::Map::new();
    for header in ["x-codex-turn-state", "session_id"] {
        if let Some(value) = input.headers.get(header) {
            metadata.insert(header.to_string(), Value::String(value.clone()));
        }
    }
    if let Some(previous) = obj.get("previous_response_id").and_then(Value::as_str) {
        metadata.insert(
            "previous_response_id".to_string(),
            Value::String(previous.to_string()),
        );
    }
    if metadata.is_empty() {
        result
    } else {
        result.with_metadata("continuation", Value::Object(metadata))
    }
}

#[cfg(test)]
mod tests {
    use super::deepseek_transform_request;
    use crate::translator::{ProviderTransformInput, ProviderTransformLoss};
    use crate::{ProviderEndpoint, ProviderId};
    use prodex_mojo_core::rich::{DeepSeekKernelInput, DeepSeekKernelOperation, deepseek_kernel};
    use serde_json::json;

    #[test]
    fn request_transform_matches_expected_body_and_preserves_boundary_metadata() {
        let mut input = ProviderTransformInput::new(
            ProviderEndpoint::Responses,
            serde_json::to_vec(&json!({
                "model": "deepseek-chat",
                "input": "call search then stop",
                "tools": [
                    {
                        "type": "function",
                        "function": {
                            "name": "search",
                            "description": "Search docs",
                            "parameters": {
                                "type": "object",
                                "properties": {"query": {"type": "string"}}
                            }
                        }
                    },
                    {"type": "web_search_preview"}
                ],
                "tool_choice": {
                    "type": "function",
                    "function": {"name": "search"}
                },
                "temperature": 0.2,
                "top_p": 0.9,
                "max_output_tokens": 128,
                "logprobs": true,
                "top_logprobs": 5,
                "stop_sequences": ["END"],
                "user": " \u{2003}user_123\u{00a0} ",
                "response_format": {
                    "type": "json_schema",
                    "schema": {"type": "object"},
                },
                "previous_response_id": "resp_1"
            }))
            .expect("request fixture serializes"),
        );
        input
            .headers
            .insert("session_id".to_string(), "sess_1".to_string());
        input
            .headers
            .insert("x-codex-turn-state".to_string(), "turn_1".to_string());

        let result = deepseek_transform_request(ProviderId::DeepSeek, input);
        match &result.loss {
            ProviderTransformLoss::DegradedButSafe { reason, details } => {
                assert_eq!(
                    reason,
                    "DeepSeek degrades JSON schema output to json_object"
                );
                assert_eq!(details["from"], "json_schema");
                assert_eq!(details["to"], "json_object");
            }
            loss => panic!("expected degraded request, got {loss:?}"),
        }
        assert_eq!(
            result.metadata["continuation"],
            json!({
                "previous_response_id": "resp_1",
                "session_id": "sess_1",
                "x-codex-turn-state": "turn_1"
            })
        );
        let body: serde_json::Value =
            serde_json::from_slice(result.body.as_ref().expect("request body")).unwrap();
        assert_eq!(
            body,
            json!({
                "model": "deepseek-chat",
                "stream": false,
                "messages": [{"role": "user", "content": "call search then stop"}],
                "tools": [{
                    "type": "function",
                    "function": {
                        "name": "search",
                        "description": "Search docs",
                        "parameters": {
                            "type": "object",
                            "properties": {"query": {"type": "string"}}
                        }
                    }
                }],
                "tool_choice": {
                    "type": "function",
                    "function": {"name": "search"}
                },
                "temperature": 0.2,
                "top_p": 0.9,
                "max_tokens": 128,
                "logprobs": true,
                "top_logprobs": 5,
                "stop": ["END"],
                "user_id": "user_123",
                "response_format": {"type": "json_object"}
            })
        );
    }

    #[test]
    fn request_transform_matches_expected_request_bodies() {
        let mut cases = vec![
            (
                "ASCII whitespace tool choice",
                json!({"input": "hello", "tool_choice": {"type": "function", "name": " \t\r\n "}}),
                json!({
                    "model": "deepseek-chat",
                    "stream": false,
                    "messages": [{"role": "user", "content": "hello"}]
                }),
                None,
            ),
            (
                "Unicode whitespace tool choice",
                json!({"input": "hello", "tool_choice": {"type": "function", "function": {"name": "\u{2003}\u{00a0}"}}}),
                json!({
                    "model": "deepseek-chat",
                    "stream": false,
                    "messages": [{"role": "user", "content": "hello"}]
                }),
                None,
            ),
            (
                "non-string top-level name uses nested function name",
                json!({"input": "hello", "tool_choice": {"type": "function", "name": 7, "function": {"name": "search"}}}),
                json!({
                    "model": "deepseek-chat",
                    "stream": false,
                    "messages": [{"role": "user", "content": "hello"}],
                    "tool_choice": {"type": "function", "function": {"name": "search"}}
                }),
                None,
            ),
            (
                "malformed scalar input",
                json!({"input": 7, "instructions": "system"}),
                json!({
                    "model": "deepseek-chat",
                    "stream": false,
                    "messages": [
                        {"role": "system", "content": "system"},
                        {"role": "user", "content": ""}
                    ]
                }),
                None,
            ),
            (
                "non-object input items",
                json!({
                    "input": [
                        null,
                        7,
                        "plain text",
                        [],
                        true,
                        {"role": "assistant", "content": "kept"}
                    ]
                }),
                json!({
                    "model": "deepseek-chat",
                    "stream": false,
                    "messages": [
                        {"role": "user", "content": ""},
                        {"role": "user", "content": ""},
                        {"role": "user", "content": ""},
                        {"role": "user", "content": ""},
                        {"role": "user", "content": ""},
                        {"role": "assistant", "content": "kept"}
                    ]
                }),
                None,
            ),
            (
                "parameter alias precedence",
                json!({
                    "input": "aliases",
                    "max_output_tokens": 11,
                    "max_tokens": 22,
                    "max_completion_tokens": 33,
                    "stop": ["first"],
                    "stop_sequences": ["second"],
                    "stopSequences": ["third"],
                    "user_id": "first_user",
                    "user": "second_user",
                    "safety_identifier": "third_user"
                }),
                json!({
                    "model": "deepseek-chat",
                    "stream": false,
                    "messages": [{"role": "user", "content": "aliases"}],
                    "max_tokens": 33,
                    "stop": ["first"],
                    "user_id": "first_user"
                }),
                None,
            ),
        ];
        for (format, expected_format, degraded) in [
            ("text", None, false),
            ("json_object", Some(json!({"type": "json_object"})), false),
            ("json_schema", Some(json!({"type": "json_object"})), true),
            ("json", Some(json!({"type": "json_object"})), true),
            (
                "structured_output",
                Some(json!({"type": "json_object"})),
                true,
            ),
        ] {
            let mut expected = json!({
                "model": "deepseek-chat",
                "stream": false,
                "messages": [{"role": "user", "content": "hello"}]
            });
            if let Some(format) = expected_format {
                expected["response_format"] = format;
            }
            cases.push((
                "response format variant",
                json!({"input": "hello", "response_format": {"type": format}}),
                expected,
                degraded.then_some("json_schema"),
            ));
        }

        for (label, request, expected_body, degraded_from) in cases {
            let result = deepseek_transform_request(
                ProviderId::DeepSeek,
                ProviderTransformInput::new(
                    ProviderEndpoint::Responses,
                    serde_json::to_vec(&request).expect("request serializes"),
                ),
            );
            let body: serde_json::Value =
                serde_json::from_slice(result.body.as_ref().expect("request body"))
                    .expect("transformed body is JSON");
            assert_eq!(body, expected_body, "{label}");
            match (degraded_from, result.loss) {
                (Some(from), ProviderTransformLoss::DegradedButSafe { details, .. }) => {
                    assert_eq!(details["from"], from, "{label}");
                    assert_eq!(details["to"], "json_object", "{label}");
                }
                (None, ProviderTransformLoss::Lossless) => {}
                (expected, actual) => panic!("{label}: expected {expected:?}, got {actual:?}"),
            }
        }
    }

    #[test]
    fn raw_request_omits_escaped_unicode_whitespace_tool_choice() {
        let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::RawCommonRequest);
        input.input =
            Some(r#"{"input":"hello","tool_choice":{"type":"function","name":"\u2003\u00a0"}}"#);
        let body = deepseek_kernel(input).expect("raw request kernel");
        let body: serde_json::Value = serde_json::from_slice(&body).expect("JSON body");
        assert_eq!(
            body,
            json!({
                "model": "deepseek-chat",
                "stream": false,
                "messages": [{"role": "user", "content": "hello"}]
            })
        );
    }

    #[test]
    fn request_transform_accepts_near_abi_limit_input() {
        let max_bytes = 4 * 1024 * 1024;
        let overhead = serde_json::to_vec(&json!({"input": ""}))
            .expect("empty request serializes")
            .len();
        let input_text = "a".repeat(max_bytes - overhead - 1);
        let request = json!({"input": &input_text});
        assert_eq!(
            serde_json::to_vec(&request)
                .expect("near-limit request serializes")
                .len(),
            max_bytes - 1
        );

        let result = deepseek_transform_request(
            ProviderId::DeepSeek,
            ProviderTransformInput::new(
                ProviderEndpoint::Responses,
                serde_json::to_vec(&request).expect("near-limit request serializes"),
            ),
        );
        assert!(matches!(result.loss, ProviderTransformLoss::Lossless));
        let body: serde_json::Value =
            serde_json::from_slice(result.body.as_ref().expect("request body"))
                .expect("transformed body is JSON");
        assert_eq!(
            body,
            json!({
                "model": "deepseek-chat",
                "stream": false,
                "messages": [{"role": "user", "content": input_text}]
            })
        );
    }

    #[test]
    fn request_transform_rejects_input_over_the_mojo_abi_limit() {
        let request = json!({"input": "a".repeat(4 * 1024 * 1024)});
        let result = deepseek_transform_request(
            ProviderId::DeepSeek,
            ProviderTransformInput::new(
                ProviderEndpoint::Responses,
                serde_json::to_vec(&request).expect("oversize request serializes"),
            ),
        );

        assert!(matches!(
            result.loss,
            ProviderTransformLoss::Rejected { .. }
        ));
        assert!(result.body.is_none());
    }

    #[test]
    fn request_transform_keeps_parallel_tool_call_rejection() {
        let result = deepseek_transform_request(
            ProviderId::DeepSeek,
            ProviderTransformInput::new(
                ProviderEndpoint::Responses,
                br#"{"input":"hello","parallel_tool_calls":false}"#.to_vec(),
            ),
        );

        assert!(matches!(
            result.loss,
            ProviderTransformLoss::Rejected { .. }
        ));
        assert!(result.body.is_none());
    }

    #[test]
    fn request_transform_rejects_invalid_parameters_with_stable_reasons() {
        for (request, expected) in [
            (
                json!({"input": "hello", "temperature": "warm", "stop": [1]}),
                "DeepSeek temperature must be a number",
            ),
            (
                json!({"input": "hello", "temperature": "warm"}),
                "DeepSeek temperature must be a number",
            ),
            (
                json!({"input": "hello", "max_tokens": 0}),
                "DeepSeek max_tokens must be a positive integer",
            ),
            (
                json!({"input": "hello", "top_logprobs": 21, "logprobs": true}),
                "DeepSeek top_logprobs must be <= 20",
            ),
            (
                json!({"input": "hello", "top_logprobs": 21, "logprobs": true, "stop": [1]}),
                "DeepSeek top_logprobs must be <= 20",
            ),
            (
                json!({"input": "hello", "stop": ["END", 1]}),
                "DeepSeek stop sequences must be strings",
            ),
            (
                json!({"input": "hello", "user_id": "bad!"}),
                "DeepSeek user_id must use only letters, numbers, underscores, or dashes and be at most 512 bytes",
            ),
            (
                json!({"input": "hello", "user_id": "user-é"}),
                "DeepSeek user_id must use only letters, numbers, underscores, or dashes and be at most 512 bytes",
            ),
            (
                json!({"input": "hello", "user_id": "u".repeat(513)}),
                "DeepSeek user_id must use only letters, numbers, underscores, or dashes and be at most 512 bytes",
            ),
        ] {
            let result = deepseek_transform_request(
                ProviderId::DeepSeek,
                ProviderTransformInput::new(
                    ProviderEndpoint::Responses,
                    serde_json::to_vec(&request).expect("request serializes"),
                ),
            );
            let ProviderTransformLoss::Rejected { reason } = result.loss else {
                panic!("request should be rejected: {request}");
            };
            assert_eq!(reason, expected, "request: {request}");
        }
    }

    #[test]
    fn request_transform_accepts_512_byte_user_id_after_unicode_trim() {
        let user_id = "u".repeat(512);
        let result = deepseek_transform_request(
            ProviderId::DeepSeek,
            ProviderTransformInput::new(
                ProviderEndpoint::Responses,
                serde_json::to_vec(&json!({
                    "input": "hello",
                    "user_id": format!(" \u{2003}{user_id}\u{00a0} ")
                }))
                .expect("request serializes"),
            ),
        );

        assert!(matches!(result.loss, ProviderTransformLoss::Lossless));
        let body: serde_json::Value =
            serde_json::from_slice(result.body.as_ref().expect("request body")).unwrap();
        assert_eq!(body["user_id"], user_id);

        let blank = deepseek_transform_request(
            ProviderId::DeepSeek,
            ProviderTransformInput::new(
                ProviderEndpoint::Responses,
                br#"{"input":"hello","user_id":" \u2003\u00a0 "}"#.to_vec(),
            ),
        );
        assert!(matches!(blank.loss, ProviderTransformLoss::Lossless));
        let body: serde_json::Value =
            serde_json::from_slice(blank.body.as_ref().expect("request body")).unwrap();
        assert!(body.get("user_id").is_none());
    }

    #[test]
    fn request_transform_prioritizes_stop_limit_over_item_type() {
        let mut stop = vec![json!(1)];
        stop.extend(std::iter::repeat_n(json!("END"), 16));
        let result = deepseek_transform_request(
            ProviderId::DeepSeek,
            ProviderTransformInput::new(
                ProviderEndpoint::Responses,
                serde_json::to_vec(&json!({"input": "hello", "stop": stop}))
                    .expect("request serializes"),
            ),
        );

        let ProviderTransformLoss::Rejected { reason } = result.loss else {
            panic!("request should be rejected");
        };
        assert_eq!(reason, "DeepSeek supports at most 16 stop sequences");
    }
}
