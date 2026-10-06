//! Gemini request transform orchestration.

use crate::translator::{ProviderTransformInput, ProviderTransformResult};
use crate::{ProviderEndpoint, ProviderId, ProviderWireFormat};
use serde_json::Value;

use super::request::{
    gemini_continuation_metadata, gemini_tool_config_from_request, gemini_validate_openai_tools,
};

pub(super) fn gemini_transform_request(input: ProviderTransformInput) -> ProviderTransformResult {
    if super::gemini_passthrough_endpoint(input.endpoint) {
        return ProviderTransformResult::lossless(
            ProviderId::Gemini,
            input.endpoint,
            ProviderWireFormat::OpenAiResponses,
            ProviderWireFormat::GeminiGenerateContent,
            input.body,
        );
    }
    if !matches!(
        input.endpoint,
        ProviderEndpoint::Responses | ProviderEndpoint::ResponsesCompact
    ) {
        return gemini_unsupported(
            input.endpoint,
            format!(
                "Gemini translator does not support {}",
                input.endpoint.label()
            ),
        );
    }

    let value = match gemini_parse_request(&input) {
        Ok(value) => value,
        Err(issue) => return gemini_issue_result(input.endpoint, issue),
    };
    let obj = value.as_object().expect("validated Gemini request object");

    let validation = match gemini_validate_request_mojo(&input, obj) {
        Ok(plan) => plan,
        Err(issue) => return gemini_issue_result(input.endpoint, issue),
    };

    let (system_instruction, contents) = match gemini_request_contents(&value) {
        Ok(contents) => contents,
        Err(issue) => return gemini_issue_result(input.endpoint, issue),
    };
    let model = obj
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("gemini-2.5-pro")
        .to_string();

    let body = match gemini_build_body_mojo(
        &value,
        &model,
        system_instruction.as_ref(),
        &contents,
        &validation,
    ) {
        Ok(body) => body,
        Err(issue) => return gemini_issue_result(input.endpoint, issue),
    };

    let result = ProviderTransformResult::lossless(
        ProviderId::Gemini,
        input.endpoint,
        ProviderWireFormat::OpenAiResponses,
        ProviderWireFormat::GeminiGenerateContent,
        body,
    );
    if let Some(metadata) = gemini_continuation_metadata(&input.headers, obj) {
        result.with_metadata("continuation", metadata)
    } else {
        result
    }
}

#[derive(Debug)]
enum GeminiTransformIssue {
    Rejected(String),
    Unsupported(String),
}

fn gemini_parse_request(input: &ProviderTransformInput) -> Result<Value, GeminiTransformIssue> {
    let value: Value = serde_json::from_slice(&input.body).map_err(|error| {
        GeminiTransformIssue::Rejected(format!("failed to parse Responses request JSON: {error}"))
    })?;
    if !value.is_object() {
        return Err(GeminiTransformIssue::Rejected(
            "Gemini request body must be a JSON object".to_string(),
        ));
    }
    Ok(value)
}

fn gemini_issue_result(
    endpoint: ProviderEndpoint,
    issue: GeminiTransformIssue,
) -> ProviderTransformResult {
    match issue {
        GeminiTransformIssue::Rejected(reason) => gemini_rejected(endpoint, reason),
        GeminiTransformIssue::Unsupported(reason) => gemini_unsupported(endpoint, reason),
    }
}

fn gemini_rejected(
    endpoint: ProviderEndpoint,
    reason: impl Into<String>,
) -> ProviderTransformResult {
    ProviderTransformResult::rejected(
        ProviderId::Gemini,
        endpoint,
        ProviderWireFormat::OpenAiResponses,
        ProviderWireFormat::GeminiGenerateContent,
        reason.into(),
    )
}

fn gemini_unsupported(
    endpoint: ProviderEndpoint,
    reason: impl Into<String>,
) -> ProviderTransformResult {
    ProviderTransformResult::unsupported(
        ProviderId::Gemini,
        endpoint,
        ProviderWireFormat::OpenAiResponses,
        ProviderWireFormat::GeminiGenerateContent,
        reason.into(),
    )
}

fn gemini_validate_request_mojo(
    input: &ProviderTransformInput,
    obj: &serde_json::Map<String, Value>,
) -> Result<crate::gemini_bridge::GeminiTranslatorValidationPlan, GeminiTransformIssue> {
    let plan = crate::gemini_bridge::gemini_bridge_validate_translator(&input.body)
        .map_err(GeminiTransformIssue::Rejected)?;
    if plan.tag == 1 {
        return Err(GeminiTransformIssue::Unsupported(
            "Gemini translator does not support local media path inputs".to_string(),
        ));
    }
    if let Some(reason) = plan.reason.clone() {
        return Err(GeminiTransformIssue::Rejected(reason));
    }
    if plan.tag == 16
        && let Some(tools) = obj.get("tools")
    {
        gemini_validate_openai_tools(tools).map_err(GeminiTransformIssue::Rejected)?;
    }
    Ok(plan)
}

fn gemini_request_contents(
    value: &Value,
) -> Result<(Option<Value>, Vec<Value>), GeminiTransformIssue> {
    super::request_contents::gemini_request_contents_from_request_mojo(value)
        .map_err(GeminiTransformIssue::Rejected)
}

fn gemini_build_body_mojo(
    value: &Value,
    model: &str,
    system_instruction: Option<&Value>,
    contents: &[Value],
    _plan: &crate::gemini_bridge::GeminiTranslatorValidationPlan,
) -> Result<Vec<u8>, GeminiTransformIssue> {
    let function_tools =
        crate::chat_tools_bridge::provider_core_chat_tools_from_responses_request(value)
            .map(Value::Array);
    let tool_config =
        gemini_tool_config_from_request(value).map_err(GeminiTransformIssue::Rejected)?;
    crate::gemini_bridge::gemini_bridge_raw_translator_request(
        value,
        system_instruction,
        contents,
        function_tools.as_ref(),
        tool_config.as_ref(),
        model,
    )
    .map_err(GeminiTransformIssue::Rejected)
}

#[cfg(test)]
mod tests {
    use super::{gemini_request_contents, gemini_transform_request};
    use crate::translator::{ProviderTransformInput, ProviderTransformLoss};
    use crate::{ProviderEndpoint, ProviderId};
    use serde_json::json;

    fn transform(request: serde_json::Value) -> crate::ProviderTransformResult {
        gemini_transform_request(ProviderTransformInput::new(
            ProviderEndpoint::Responses,
            serde_json::to_vec(&request).unwrap(),
        ))
    }

    fn rejection_reason(result: crate::ProviderTransformResult) -> String {
        assert_eq!(result.provider, ProviderId::Gemini);
        let ProviderTransformLoss::Rejected { reason } = result.loss else {
            panic!("request should be rejected");
        };
        reason
    }

    #[test]
    fn request_contents_preserve_tool_history_when_text_kernel_declines() {
        let request = json!({
            "input": [
                {
                    "role": "assistant",
                    "content": "Looking up records.",
                    "tool_calls": [
                        {
                            "id": "call-1",
                            "function": {
                                "name": "lookup",
                                "arguments": "{\"query\":\"synthetic\"}"
                            }
                        },
                        {
                            "id": "call-2",
                            "function": {
                                "name": "count",
                                "arguments": "not-json"
                            }
                        }
                    ]
                },
                {"role": "tool", "tool_call_id": "call-1", "content": "{\"found\":true}"},
                {"role": "tool", "tool_call_id": "call-2", "content": "plain result"}
            ]
        });

        let (_, contents) =
            gemini_request_contents(&request).expect("valid Gemini request contents");
        assert_eq!(
            serde_json::Value::Array(contents),
            json!([
                {
                    "role": "model",
                    "parts": [
                        {"text": "Looking up records."},
                        {"functionCall": {
                            "name": "lookup",
                            "args": {"query": "synthetic"},
                            "id": "call-1"
                        }},
                        {"functionCall": {
                            "name": "count",
                            "args": {},
                            "id": "call-2"
                        }}
                    ]
                },
                {
                    "role": "user",
                    "parts": [
                        {"functionResponse": {
                            "name": "lookup",
                            "response": {"found": true},
                            "id": "call-1"
                        }},
                        {"functionResponse": {
                            "name": "count",
                            "response": {"output": "plain result"},
                            "id": "call-2"
                        }}
                    ]
                }
            ])
        );
    }

    #[test]
    fn system_instruction_uses_fixed_mojo_result() {
        let request = json!({
            "input": [
                {"role": "system", "content": "system instruction"},
                {"role": "user", "content": "<environment_context>synthetic</environment_context>"},
                {"role": "user", "content": "actual request"}
            ]
        });

        assert_eq!(
            gemini_request_contents(&request).expect("valid Gemini request contents"),
            (
                Some(
                    json!({"parts": [{"text": "system instruction\n\n<environment_context>synthetic</environment_context>"}]})
                ),
                vec![json!({"role": "user", "parts": [{"text": "actual request"}]})]
            )
        );
    }

    #[test]
    fn candidate_count_accepts_only_one_and_preserves_the_canonical_field() {
        let result = transform(json!({
            "model": "gemini-2.5-pro",
            "candidate_count": 1,
            "candidateCount": 1,
        }));
        let body: serde_json::Value = serde_json::from_slice(&result.body.unwrap()).unwrap();

        assert_eq!(body["request"]["generationConfig"]["candidateCount"], 1);
    }

    #[test]
    fn candidate_count_conflicts_and_invalid_values_are_rejected() {
        for request in [
            json!({"candidate_count": 0}),
            json!({"candidate_count": -1}),
            json!({"candidateCount": 2}),
            json!({"candidateCount": 1.0}),
            json!({"candidateCount": "1"}),
            json!({"candidateCount": []}),
            json!({"candidateCount": {}}),
        ] {
            for stream in [false, true] {
                let mut request = request.clone();
                request["stream"] = json!(stream);
                let reason = rejection_reason(transform(request));
                assert!(reason.contains("invalid_candidate_count"), "{reason}");
                assert!(reason.contains("candidate"), "{reason}");
            }
        }
        for (request, expected) in [
            (
                json!({"candidate_count": 1, "candidateCount": 2}),
                "candidate_count` and `candidateCount` conflict",
            ),
            (
                json!({"candidate_count": 2}),
                "candidate_count` must be omitted, null, or 1",
            ),
        ] {
            let reason = rejection_reason(transform(request));
            assert!(reason.contains("invalid_candidate_count"), "{reason}");
            assert!(reason.contains(expected), "{reason}");
        }
    }

    #[test]
    fn null_candidate_count_is_omitted() {
        for request in [
            json!({"candidateCount": null}),
            json!({"candidate_count": null}),
            json!({"candidateCount": null, "candidate_count": null}),
            json!({"candidateCount": null, "candidate_count": 1}),
        ] {
            let result = transform(request);
            let body: serde_json::Value = serde_json::from_slice(&result.body.unwrap()).unwrap();

            assert!(
                body["request"]["generationConfig"]
                    .get("candidateCount")
                    .is_none()
            );
        }
    }

    #[test]
    fn malformed_function_tools_are_rejected_with_their_path() {
        for (tools, expected) in [
            (
                json!([{"type": "function", "function": {"description": "missing name", "parameters": {}}}]),
                "tools[0].function.name",
            ),
            (
                json!([{"type": "function", "function": {"name": "missing_schema"}}]),
                "tools[0].function.parameters",
            ),
            (
                json!([
                    {"type": "function", "function": {"name": "valid", "parameters": {"type": "object"}}},
                    {"type": "function", "function": {"name": "invalid", "parameters": true}}
                ]),
                "tools[1].function.parameters",
            ),
        ] {
            let reason = rejection_reason(transform(json!({"tools": tools})));
            assert!(reason.contains(expected), "{reason}");
        }
    }

    #[test]
    fn malformed_tools_array_is_rejected_instead_of_dropped() {
        let reason = rejection_reason(transform(json!({"tools": {"type": "function"}})));

        assert!(reason.contains("tools` must be an array"), "{reason}");
    }

    #[test]
    fn valid_function_tool_declaration_is_preserved() {
        let result = transform(json!({
            "tools": [{
                "type": "function",
                "function": {
                    "name": "lookup",
                    "description": "Look up a record",
                    "parameters": {
                        "type": "object",
                        "properties": {"query": {"type": "string"}},
                        "required": ["query"]
                    }
                }
            }]
        }));
        let body: serde_json::Value = serde_json::from_slice(&result.body.unwrap()).unwrap();
        let declaration = &body["request"]["tools"][0]["functionDeclarations"][0];

        assert_eq!(declaration["name"], "lookup");
        assert_eq!(declaration["description"], "Look up a record");
        assert_eq!(declaration["parameters"]["type"], "object");
        assert_eq!(declaration["parameters"]["required"][0], "query");
    }

    #[test]
    fn function_tools_keep_order_duplicates_unicode_and_schema_arrays() {
        let declaration = json!({
            "type": "function",
            "function": {
                "name": "検索🙂",
                "description": "Unicode tool description: β",
                "parameters": {
                    "$schema": "discarded",
                    "strict": true,
                    "type": "object",
                    "required": ["β", "alpha", "β"],
                    "properties": {
                        "β": {"type": "string", "additionalProperties": false},
                        "alpha": {"type": "integer"}
                    }
                }
            }
        });
        let result = transform(json!({
            "tools": [declaration.clone(), declaration]
        }));
        let body: serde_json::Value = serde_json::from_slice(&result.body.unwrap()).unwrap();

        assert_eq!(
            body["request"]["tools"],
            json!([{
                "functionDeclarations": [
                    {
                        "name": "検索🙂",
                        "description": "Unicode tool description: β",
                        "parameters": {
                            "type": "object",
                            "required": ["β", "alpha", "β"],
                            "properties": {
                                "β": {"type": "string"},
                                "alpha": {"type": "integer"}
                            }
                        }
                    },
                    {
                        "name": "検索🙂",
                        "description": "Unicode tool description: β",
                        "parameters": {
                            "type": "object",
                            "required": ["β", "alpha", "β"],
                            "properties": {
                                "β": {"type": "string"},
                                "alpha": {"type": "integer"}
                            }
                        }
                    }
                ]
            }])
        );
    }

    #[test]
    fn tool_choice_keeps_nested_string_precedence_and_legacy_outer_fallback() {
        for (choice, expected) in [
            (
                json!({"function": {"name": 7}, "name": "fallback🙂"}),
                Some(json!({
                    "functionCallingConfig": {
                        "mode": "ANY",
                        "allowedFunctionNames": ["fallback🙂"]
                    }
                })),
            ),
            (
                json!({"function": {"name": "nested"}, "name": "outer"}),
                Some(json!({
                    "functionCallingConfig": {
                        "mode": "ANY",
                        "allowedFunctionNames": ["nested"]
                    }
                })),
            ),
            (
                json!({"function": false, "name": "outer"}),
                Some(json!({
                    "functionCallingConfig": {
                        "mode": "ANY",
                        "allowedFunctionNames": ["outer"]
                    }
                })),
            ),
            (
                json!({"name": "outer-only"}),
                Some(json!({
                    "functionCallingConfig": {
                        "mode": "ANY",
                        "allowedFunctionNames": ["outer-only"]
                    }
                })),
            ),
            (
                json!({"function": {"name": ""}, "name": "outer"}),
                Some(json!({
                    "functionCallingConfig": {
                        "mode": "ANY",
                        "allowedFunctionNames": [""]
                    }
                })),
            ),
            (json!({"function": {"name": 7}, "name": false}), None),
        ] {
            let result = transform(json!({"tool_choice": choice}));
            let body: serde_json::Value = serde_json::from_slice(&result.body.unwrap()).unwrap();
            assert_eq!(body["request"].get("toolConfig").cloned(), expected);
        }
        for (choice, expected) in [
            ("auto", None),
            (
                "none",
                Some(json!({"functionCallingConfig": {"mode": "NONE"}})),
            ),
            (
                "required",
                Some(json!({"functionCallingConfig": {"mode": "ANY"}})),
            ),
        ] {
            let result = transform(json!({"tool_choice": choice}));
            let body: serde_json::Value = serde_json::from_slice(&result.body.unwrap()).unwrap();
            assert_eq!(body["request"].get("toolConfig").cloned(), expected);
        }
    }

    #[test]
    fn valid_custom_tool_is_translated_instead_of_discarded() {
        let result = transform(json!({
            "tools": [{
                "type": "custom",
                "name": "apply_patch",
                "description": "Edit files.",
                "format": {"type": "grammar"}
            }]
        }));
        let body: serde_json::Value = serde_json::from_slice(&result.body.unwrap()).unwrap();
        let declaration = &body["request"]["tools"][0]["functionDeclarations"][0];

        assert_eq!(declaration["name"], "apply_patch");
        assert_eq!(declaration["parameters"]["required"][0], "input");
    }

    #[test]
    fn aliased_generation_and_optional_fields_keep_existing_precedence() {
        let result = transform(json!({
            "top_k": 1,
            "topK": 2,
            "presence_penalty": 0.1,
            "presencePenalty": 0.3,
            "safety_settings": null,
            "safetySettings": [{"category": "synthetic"}],
            "cached_content": null,
            "cachedContent": "synthetic-cache",
            "labels": {"suite": "synthetic"},
        }));
        let body: serde_json::Value = serde_json::from_slice(&result.body.unwrap()).unwrap();

        assert_eq!(body["request"]["generationConfig"]["topK"], 2);
        assert_eq!(body["request"]["generationConfig"]["presencePenalty"], 0.3);
        assert!(body["request"]["safetySettings"].is_null());
        assert!(body["request"].get("cachedContent").is_none());
        assert_eq!(body["request"]["labels"]["suite"], "synthetic");
    }

    #[test]
    fn null_aliases_unicode_schemas_and_optional_fields_keep_the_expected_values() {
        let result = transform(json!({
            "model": "gemini-3-pro",
            "temperature": 0.25,
            "top_p": 0.8,
            "max_tokens": 123,
            "top_k": 1,
            "topK": null,
            "presence_penalty": 0.1,
            "presencePenalty": null,
            "response_schema": {"title": "東京 🦀"},
            "responseSchema": null,
            "candidate_count": 1,
            "candidateCount": null,
            "stop": null,
            "stop_sequences": ["ignored"],
            "reasoning": {"effort": "low"},
            "text": {"format": {"type": "json_schema", "schema": {"description": "λ"}}},
            "safety_settings": null,
            "safetySettings": [{"category": "ignored"}],
            "cached_content": null,
            "cachedContent": "ignored-cache",
            "labels": {"suite": "日本語"}
        }));
        let body: serde_json::Value = serde_json::from_slice(&result.body.unwrap()).unwrap();

        assert_eq!(
            body["request"]["generationConfig"],
            json!({
                "temperature": 0.25,
                "topP": 0.8,
                "maxOutputTokens": 123,
                "topK": 1,
                "presencePenalty": 0.1,
                "responseSchema": {"title": "東京 🦀"},
                "responseMimeType": "application/json",
                "responseJsonSchema": {"description": "λ"},
                "thinkingConfig": {"includeThoughts": true, "thinkingLevel": "LOW"}
            })
        );
        assert!(body["request"]["safetySettings"].is_null());
        assert!(body["request"].get("cachedContent").is_none());
        assert_eq!(body["request"]["labels"], json!({"suite": "日本語"}));
    }

    #[test]
    fn malformed_request_json_is_rejected_before_mojo_translation() {
        let result = gemini_transform_request(ProviderTransformInput::new(
            ProviderEndpoint::Responses,
            b"{\"model\":".to_vec(),
        ));
        assert!(rejection_reason(result).starts_with("failed to parse Responses request JSON:"),);
    }
}
