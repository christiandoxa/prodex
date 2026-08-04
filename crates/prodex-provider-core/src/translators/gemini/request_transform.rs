//! Gemini request transform orchestration.

use crate::translator::{ProviderTransformInput, ProviderTransformResult};
use crate::{ProviderEndpoint, ProviderId, ProviderWireFormat};
use serde_json::{Value, json};

use super::request::{
    gemini_apply_optional_request_fields, gemini_apply_response_format, gemini_apply_text_format,
    gemini_builtin_tools_from_request, gemini_continuation_metadata,
    gemini_insert_basic_generation_config, gemini_insert_extended_generation_config,
    gemini_is_supported_builtin_tool, gemini_thinking_config_from_request,
    gemini_tool_config_from_request, gemini_tool_from_openai_tool, gemini_validate_candidate_count,
    gemini_validate_openai_tools,
};
use super::request_contents::{
    gemini_contains_local_media_path, gemini_contents_from_request,
    gemini_system_instruction_from_request,
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
        return ProviderTransformResult::unsupported(
            ProviderId::Gemini,
            input.endpoint,
            ProviderWireFormat::OpenAiResponses,
            ProviderWireFormat::GeminiGenerateContent,
            format!(
                "Gemini translator does not support {}",
                input.endpoint.label()
            ),
        );
    }
    let value: Value = match serde_json::from_slice(&input.body) {
        Ok(value) => value,
        Err(error) => {
            return ProviderTransformResult::rejected(
                ProviderId::Gemini,
                input.endpoint,
                ProviderWireFormat::OpenAiResponses,
                ProviderWireFormat::GeminiGenerateContent,
                format!("failed to parse Responses request JSON: {error}"),
            );
        }
    };
    let Some(obj) = value.as_object() else {
        return ProviderTransformResult::rejected(
            ProviderId::Gemini,
            input.endpoint,
            ProviderWireFormat::OpenAiResponses,
            ProviderWireFormat::GeminiGenerateContent,
            "Gemini request body must be a JSON object",
        );
    };
    if gemini_contains_local_media_path(&value) {
        return ProviderTransformResult::unsupported(
            ProviderId::Gemini,
            input.endpoint,
            ProviderWireFormat::OpenAiResponses,
            ProviderWireFormat::GeminiGenerateContent,
            "Gemini translator does not support local media path inputs",
        );
    }
    if let Err(reason) = gemini_validate_candidate_count(&value) {
        return ProviderTransformResult::rejected(
            ProviderId::Gemini,
            input.endpoint,
            ProviderWireFormat::OpenAiResponses,
            ProviderWireFormat::GeminiGenerateContent,
            reason,
        );
    }
    if let Some(tools) = obj.get("tools")
        && let Err(reason) = gemini_validate_openai_tools(tools)
    {
        return ProviderTransformResult::rejected(
            ProviderId::Gemini,
            input.endpoint,
            ProviderWireFormat::OpenAiResponses,
            ProviderWireFormat::GeminiGenerateContent,
            reason,
        );
    }
    let mut request = serde_json::Map::new();
    if let Some(system_instruction) = gemini_system_instruction_from_request(&value) {
        request.insert("systemInstruction".to_string(), system_instruction);
    }
    request.insert(
        "contents".to_string(),
        Value::Array(gemini_contents_from_request(&value)),
    );
    let model = obj
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("gemini-2.5-pro")
        .to_string();
    let mut generation_config = serde_json::Map::new();
    gemini_insert_basic_generation_config(obj, &mut generation_config);
    gemini_insert_extended_generation_config(obj, &mut generation_config);
    gemini_apply_text_format(obj, &mut generation_config);
    if let Some(thinking_config) = gemini_thinking_config_from_request(obj, &model) {
        generation_config.insert("thinkingConfig".to_string(), thinking_config);
    }
    if let Some(response_format) = obj.get("response_format")
        && let Err(reason) = gemini_apply_response_format(response_format, &mut generation_config)
    {
        return ProviderTransformResult::rejected(
            ProviderId::Gemini,
            input.endpoint,
            ProviderWireFormat::OpenAiResponses,
            ProviderWireFormat::GeminiGenerateContent,
            reason,
        );
    }
    if !generation_config.is_empty() {
        request.insert(
            "generationConfig".to_string(),
            Value::Object(generation_config),
        );
    }
    if let Err(reason) = gemini_apply_tools(obj, &mut request) {
        return ProviderTransformResult::rejected(
            ProviderId::Gemini,
            input.endpoint,
            ProviderWireFormat::OpenAiResponses,
            ProviderWireFormat::GeminiGenerateContent,
            reason,
        );
    }
    if let Some(tool_config) = gemini_tool_config_from_request(&value) {
        request.insert("toolConfig".to_string(), tool_config);
    }
    gemini_apply_optional_request_fields(obj, &mut request);
    let body = serde_json::to_vec(&json!({"model": model, "request": Value::Object(request)}))
        .expect("gemini request serializes");
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

fn gemini_apply_tools(
    obj: &serde_json::Map<String, Value>,
    request: &mut serde_json::Map<String, Value>,
) -> Result<(), String> {
    let Some(tools) = obj.get("tools").and_then(Value::as_array) else {
        return Ok(());
    };
    let mut translated_tools = gemini_builtin_tools_from_request(tools);
    let mut declarations = Vec::new();
    for (index, tool) in tools.iter().enumerate() {
        if tool.get("function").is_some()
            || tool.get("type").and_then(Value::as_str) == Some("function")
        {
            declarations.push(gemini_tool_from_openai_tool(tool, index)?);
            continue;
        }
        if gemini_is_supported_builtin_tool(tool) {
            continue;
        }
        if let Some(translated) =
            crate::chat_tools_bridge::provider_core_chat_tools_from_responses_request(
                &json!({"tools": [tool]}),
            )
        {
            for translated_tool in translated {
                declarations.push(gemini_tool_from_openai_tool(&translated_tool, index)?);
            }
        }
    }
    if !declarations.is_empty() {
        translated_tools.push(json!({"functionDeclarations": declarations}));
    }
    if !translated_tools.is_empty() {
        request.insert("tools".to_string(), Value::Array(translated_tools));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::gemini_transform_request;
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
}
