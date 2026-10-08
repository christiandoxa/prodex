//! DeepSeek request translation.

use std::collections::BTreeMap;

use super::deepseek_passthrough_endpoint;
use crate::mojo_json::Document;
use crate::translator::{ProviderTransformInput, ProviderTransformResult};
use crate::{ProviderEndpoint, ProviderId, ProviderWireFormat};
use prodex_mojo_core::rich::deepseek_responses_request_transform;
use serde_json::Value;

struct DeepSeekRequestPlan {
    body: Vec<u8>,
    degraded: bool,
    continuation: Option<Value>,
    omitted_tool_choice: Option<Value>,
}

fn deepseek_request_plan_from_responses(
    value: Value,
    model: Option<&str>,
    turn_state: Option<&str>,
    session_id: Option<&str>,
) -> Result<DeepSeekRequestPlan, String> {
    let model = value
        .get("model")
        .and_then(Value::as_str)
        .or(model)
        .filter(|model| !model.trim().is_empty())
        .map(|model| crate::provider_canonical_model(ProviderId::DeepSeek, model));
    let mut context = serde_json::Map::new();
    context.insert("request".to_string(), value);
    if let Some(model) = model {
        context.insert("canonical_model".to_string(), Value::String(model));
    }
    if let Some(turn_state) = turn_state {
        context.insert(
            "turn_state".to_string(),
            Value::String(turn_state.to_string()),
        );
    }
    if let Some(session_id) = session_id {
        context.insert(
            "session_id".to_string(),
            Value::String(session_id.to_string()),
        );
    }
    let context = Value::Object(context);
    let mut document = Document::default();
    document.push(&context, None, "");
    let source = std::str::from_utf8(&document.raw)
        .map_err(|error| format!("DeepSeek request serialization failed: {error}"))?;
    let bytes = deepseek_responses_request_transform(&document.nodes, source)
        .map_err(|error| format!("DeepSeek request kernel failed: {error:?}"))?;
    let plan: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("DeepSeek request plan returned invalid JSON: {error}"))?;
    let issue = plan
        .get("issue")
        .and_then(Value::as_u64)
        .ok_or_else(|| "DeepSeek request plan omitted issue code".to_string())?;
    match issue {
        0 => {}
        1 => {
            return Err(
                "DeepSeek does not expose a compatible parallel_tool_calls=false control"
                    .to_string(),
            );
        }
        2 => {
            let detail = plan
                .get("detail")
                .and_then(Value::as_str)
                .unwrap_or_default();
            return Err(format!(
                "DeepSeek response_format type `{detail}` is not supported"
            ));
        }
        3 => return Err("DeepSeek user_id must be a string".to_string()),
        4 => {
            return Err(
                "DeepSeek user_id must use only letters, numbers, underscores, or dashes and be at most 512 bytes"
                    .to_string(),
            );
        }
        5 => return Err("DeepSeek reasoning must be an object".to_string()),
        6 => {
            let detail = plan
                .get("detail")
                .and_then(Value::as_str)
                .unwrap_or_default();
            return Err(format!(
                "DeepSeek reasoning.{detail} is not supported by this Responses adapter"
            ));
        }
        7 => return Err("DeepSeek reasoning.effort must be a string".to_string()),
        8 => return Err("DeepSeek reasoning_effort must be a string".to_string()),
        9 => return Err("DeepSeek reasoning effort is not supported".to_string()),
        10 => return Err("DeepSeek temperature must be a number".to_string()),
        11 => return Err("DeepSeek top_p must be a number".to_string()),
        12 => return Err("DeepSeek max_output_tokens must be a positive integer".to_string()),
        13 => return Err("DeepSeek max_tokens must be a positive integer".to_string()),
        14 => {
            return Err("DeepSeek max_completion_tokens must be a positive integer".to_string());
        }
        15 => return Err("DeepSeek logprobs must be a boolean".to_string()),
        16 => return Err("DeepSeek top_logprobs must be an integer".to_string()),
        17 => return Err("DeepSeek top_logprobs must be <= 20".to_string()),
        18 => return Err("DeepSeek top_logprobs requires logprobs=true".to_string()),
        19 => return Err("DeepSeek stop must be a string or array of strings".to_string()),
        20 => return Err("DeepSeek supports at most 16 stop sequences".to_string()),
        21 => return Err("DeepSeek stop sequences must be strings".to_string()),
        other => {
            return Err(format!(
                "DeepSeek request plan returned unknown issue code {other}"
            ));
        }
    }
    let body = plan
        .get("body")
        .ok_or_else(|| "DeepSeek request plan omitted body".to_string())?;
    let body = serde_json::to_vec(body)
        .map_err(|error| format!("DeepSeek request plan body serialization failed: {error}"))?;
    let degraded = plan
        .get("degraded")
        .and_then(Value::as_bool)
        .ok_or_else(|| "DeepSeek request plan omitted degradation state".to_string())?;
    let continuation = plan
        .get("continuation")
        .filter(|value| !value.is_null())
        .cloned();
    let omitted_tool_choice = plan.get("omitted_tool_choice").cloned();
    Ok(DeepSeekRequestPlan {
        body,
        degraded,
        continuation,
        omitted_tool_choice,
    })
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
    if !value.is_object() {
        return ProviderTransformResult::rejected(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiResponses,
            ProviderWireFormat::OpenAiChatCompletions,
            "DeepSeek request body must be a JSON object",
        );
    };
    let turn_state = input.headers.get("x-codex-turn-state").map(String::as_str);
    let session_id = input.headers.get("session_id").map(String::as_str);
    let DeepSeekRequestPlan {
        body,
        degraded,
        continuation,
        omitted_tool_choice,
    } = match deepseek_request_plan_from_responses(
        value,
        input.model.as_deref(),
        turn_state,
        session_id,
    ) {
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
    let result = if degraded {
        ProviderTransformResult::degraded(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiResponses,
            ProviderWireFormat::OpenAiChatCompletions,
            body,
            "DeepSeek degrades JSON schema output to json_object",
            BTreeMap::from([
                ("from".to_string(), Value::String("json_schema".to_string())),
                ("to".to_string(), Value::String("json_object".to_string())),
            ]),
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
    let result = if let Some(tool_choice) = omitted_tool_choice {
        result.with_metadata(
            "deepseek",
            serde_json::json!({
                "omitted_tool_choice": {
                    "from": tool_choice,
                    "reason": "DeepSeek thinking mode currently rejects explicit tool_choice on the OpenAI Chat route, so Prodex omits it while preserving translated function tools"
                }
            }),
        )
    } else {
        result
    };
    if let Some(continuation) = continuation {
        result.with_metadata("continuation", continuation)
    } else {
        result
    }
}

#[cfg(test)]
mod tests {
    use super::deepseek_transform_request;
    use crate::translator::{ProviderTransformInput, ProviderTransformLoss};
    use crate::{ProviderEndpoint, ProviderId};
    use prodex_mojo_core::rich::{DeepSeekKernelInput, DeepSeekKernelOperation, deepseek_kernel};
    use serde_json::{Value, json};

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
    fn request_transform_maps_models_and_reasoning_in_the_mojo_plan() {
        for (request, model, expected_reasoning, omitted_choice) in [
            (
                json!({
                    "model": "pro",
                    "input": "think this through",
                    "reasoning": {"effort": "xhigh"},
                    "tool_choice": {"type": "function", "name": "search"}
                }),
                "deepseek-v4-pro",
                json!({"thinking": {"type": "enabled"}, "reasoning_effort": "max"}),
                true,
            ),
            (
                json!({
                    "input": "think this through",
                    "reasoning_effort": "low",
                    "tool_choice": "required"
                }),
                "deepseek-v4-flash",
                json!({"thinking": {"type": "enabled"}, "reasoning_effort": "high"}),
                true,
            ),
            (
                json!({
                    "input": "think this through",
                    "reasoning_effort": "minimal",
                    "tool_choice": "required"
                }),
                "deepseek-chat",
                json!({"thinking": {"type": "disabled"}}),
                false,
            ),
        ] {
            let mut input = ProviderTransformInput::new(
                ProviderEndpoint::Responses,
                serde_json::to_vec(&request).expect("request serializes"),
            );
            if request.get("model").is_none() && model != "deepseek-chat" {
                input.model = Some("flash".to_string());
            }
            let result = deepseek_transform_request(ProviderId::DeepSeek, input);
            let body: Value =
                serde_json::from_slice(result.body.as_ref().expect("translated request body"))
                    .expect("translated body is JSON");
            assert_eq!(body["model"], model);
            for (key, value) in expected_reasoning.as_object().expect("reasoning object") {
                assert_eq!(body[key], *value, "effort request: {request}");
            }
            if omitted_choice {
                assert!(body.get("tool_choice").is_none(), "{request}");
                assert_eq!(
                    result.metadata["deepseek"]["omitted_tool_choice"]["from"],
                    request["tool_choice"]
                );
            } else {
                assert_eq!(body["tool_choice"], request["tool_choice"]);
                assert!(result.metadata.get("deepseek").is_none());
            }
        }
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
