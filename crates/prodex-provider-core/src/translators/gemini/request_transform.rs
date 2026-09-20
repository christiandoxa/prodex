//! Gemini request transform orchestration.

use crate::translator::{ProviderTransformInput, ProviderTransformResult};
use crate::{ProviderEndpoint, ProviderId, ProviderWireFormat};
use serde_json::{Value, json};

#[cfg(not(feature = "mojo"))]
use super::request::{
    gemini_apply_optional_request_fields, gemini_apply_response_format,
    gemini_validate_candidate_count,
};
#[cfg(not(feature = "mojo"))]
use super::request::{
    gemini_apply_text_format, gemini_insert_basic_generation_config,
    gemini_insert_extended_generation_config, gemini_thinking_config_from_request,
};
use super::request::{
    gemini_builtin_tools_from_request, gemini_continuation_metadata,
    gemini_is_supported_builtin_tool, gemini_tool_config_from_request,
    gemini_tool_from_openai_tool, gemini_validate_openai_tools,
};
#[cfg(not(feature = "mojo"))]
use super::request_contents::gemini_contains_local_media_path;
#[cfg(feature = "mojo")]
use super::request_contents::gemini_text_contents_from_request_mojo;
use super::request_contents::{
    gemini_contents_from_request, gemini_system_instruction_from_request,
};

#[cfg(feature = "mojo")]
fn gemini_translator_validation_error(
    plan: &crate::gemini_bridge::GeminiTranslatorValidationPlan,
) -> Option<String> {
    let index = plan.index.unwrap_or(0);
    Some(match plan.tag {
        0 | 1 | 16 => return None,
        2 => "invalid_candidate_count: Gemini request fields `candidate_count` and `candidateCount` conflict".to_string(),
        3 => "invalid_candidate_count: Gemini request field `candidate_count` must be omitted, null, or 1".to_string(),
        4 => "invalid_candidate_count: Gemini request field `candidateCount` must be omitted, null, or 1".to_string(),
        5 => "invalid_tool_declaration: Gemini request field `tools` must be an array".to_string(),
        6 => format!("invalid_tool_declaration: Gemini request field `tools[{index}]` must be an object"),
        7 => format!("invalid_tool_declaration: Gemini request field `tools[{index}].function` must be an object"),
        8 => format!("invalid_tool_declaration: Gemini request field `tools[{index}].function.name` must be a non-empty string"),
        9 => format!("invalid_tool_declaration: Gemini request field `tools[{index}].name` must be a non-empty string"),
        10 => format!("invalid_tool_declaration: Gemini request field `tools[{index}].function.parameters` is required"),
        11 => format!("invalid_tool_declaration: Gemini request field `tools[{index}].parameters` is required"),
        12 => format!("invalid_tool_declaration: Gemini request field `tools[{index}].function.parameters` must be an object"),
        13 => format!("invalid_tool_declaration: Gemini request field `tools[{index}].parameters` must be an object"),
        14 => format!("invalid_tool_declaration: Gemini request field `tools[{index}].function.description` must be a string"),
        15 => format!("invalid_tool_declaration: Gemini request field `tools[{index}].description` must be a string"),
        17 => format!("Gemini response_format type `{}` is not supported", plan.detail.as_deref().unwrap_or_default()),
        _ => "Gemini translator validation returned an unknown result".to_string(),
    })
}

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

    #[cfg(feature = "mojo")]
    let validation = match gemini_validate_request_mojo(&input, obj) {
        Ok(plan) => plan,
        Err(issue) => return gemini_issue_result(input.endpoint, issue),
    };
    #[cfg(not(feature = "mojo"))]
    if let Err(issue) = gemini_validate_request_rust(&value, obj) {
        return gemini_issue_result(input.endpoint, issue);
    }

    let (system_instruction, contents) = gemini_request_contents(&value);
    let model = obj
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("gemini-2.5-pro")
        .to_string();

    #[cfg(feature = "mojo")]
    let body = match gemini_build_body_mojo(
        &value,
        obj,
        &model,
        system_instruction.as_ref(),
        &contents,
        &validation,
    ) {
        Ok(body) => body,
        Err(issue) => return gemini_issue_result(input.endpoint, issue),
    };
    #[cfg(not(feature = "mojo"))]
    let body = match gemini_build_body_rust(&value, obj, &model, system_instruction, contents) {
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

#[cfg(feature = "mojo")]
fn gemini_validate_request_mojo(
    input: &ProviderTransformInput,
    obj: &serde_json::Map<String, Value>,
) -> Result<crate::gemini_bridge::GeminiTranslatorValidationPlan, GeminiTransformIssue> {
    let plan = crate::gemini_bridge::gemini_bridge_validate_translator(&input.body);
    if plan.tag == 1 {
        return Err(GeminiTransformIssue::Unsupported(
            "Gemini translator does not support local media path inputs".to_string(),
        ));
    }
    if let Some(reason) = gemini_translator_validation_error(&plan) {
        return Err(GeminiTransformIssue::Rejected(reason));
    }
    if plan.tag == 16
        && let Some(tools) = obj.get("tools")
    {
        gemini_validate_openai_tools(tools).map_err(GeminiTransformIssue::Rejected)?;
    }
    Ok(plan)
}

#[cfg(not(feature = "mojo"))]
fn gemini_validate_request_rust(
    value: &Value,
    obj: &serde_json::Map<String, Value>,
) -> Result<(), GeminiTransformIssue> {
    if gemini_contains_local_media_path(value) {
        return Err(GeminiTransformIssue::Unsupported(
            "Gemini translator does not support local media path inputs".to_string(),
        ));
    }
    gemini_validate_candidate_count(value).map_err(GeminiTransformIssue::Rejected)?;
    if let Some(tools) = obj.get("tools") {
        gemini_validate_openai_tools(tools).map_err(GeminiTransformIssue::Rejected)?;
    }
    Ok(())
}

#[cfg(feature = "mojo")]
fn gemini_request_contents(value: &Value) -> (Option<Value>, Vec<Value>) {
    gemini_text_contents_from_request_mojo(value).unwrap_or_else(|| {
        (
            gemini_system_instruction_from_request(value),
            gemini_contents_from_request(value),
        )
    })
}

#[cfg(not(feature = "mojo"))]
fn gemini_request_contents(value: &Value) -> (Option<Value>, Vec<Value>) {
    (
        gemini_system_instruction_from_request(value),
        gemini_contents_from_request(value),
    )
}

#[cfg(feature = "mojo")]
fn gemini_build_body_mojo(
    value: &Value,
    obj: &serde_json::Map<String, Value>,
    model: &str,
    system_instruction: Option<&Value>,
    contents: &[Value],
    plan: &crate::gemini_bridge::GeminiTranslatorValidationPlan,
) -> Result<Vec<u8>, GeminiTransformIssue> {
    let tools = if plan.tag == 16 {
        let mut tool_request = serde_json::Map::new();
        gemini_apply_tools(obj, &mut tool_request).map_err(GeminiTransformIssue::Rejected)?;
        tool_request.remove("tools")
    } else {
        None
    };
    let tool_config = gemini_tool_config_from_request(value);
    Ok(crate::gemini_bridge::gemini_bridge_raw_translator_request(
        value,
        system_instruction,
        contents,
        tools.as_ref(),
        tool_config.as_ref(),
        model,
    ))
}

#[cfg(not(feature = "mojo"))]
fn gemini_build_body_rust(
    value: &Value,
    obj: &serde_json::Map<String, Value>,
    model: &str,
    system_instruction: Option<Value>,
    contents: Vec<Value>,
) -> Result<Vec<u8>, GeminiTransformIssue> {
    let mut request = serde_json::Map::new();
    if let Some(system_instruction) = system_instruction {
        request.insert("systemInstruction".to_string(), system_instruction);
    }
    request.insert("contents".to_string(), Value::Array(contents));
    let mut generation_config = gemini_translator_generation_config(value, obj, model);
    if let Some(response_format) = obj.get("response_format") {
        gemini_apply_response_format(response_format, &mut generation_config)
            .map_err(GeminiTransformIssue::Rejected)?;
    }
    if !generation_config.is_empty() {
        request.insert(
            "generationConfig".to_string(),
            Value::Object(generation_config),
        );
    }
    gemini_apply_tools(obj, &mut request).map_err(GeminiTransformIssue::Rejected)?;
    if let Some(tool_config) = gemini_tool_config_from_request(value) {
        request.insert("toolConfig".to_string(), tool_config);
    }
    gemini_apply_optional_request_fields(obj, &mut request);
    Ok(serde_json::to_vec(&json!({
        "model": model,
        "request": Value::Object(request)
    }))
    .expect("gemini request serializes"))
}

#[cfg(not(feature = "mojo"))]
fn gemini_translator_generation_config(
    _value: &Value,
    obj: &serde_json::Map<String, Value>,
    model: &str,
) -> serde_json::Map<String, Value> {
    let mut generation_config = serde_json::Map::new();
    gemini_insert_basic_generation_config(obj, &mut generation_config);
    gemini_insert_extended_generation_config(obj, &mut generation_config);
    gemini_apply_text_format(obj, &mut generation_config);
    if let Some(thinking_config) = gemini_thinking_config_from_request(obj, model) {
        generation_config.insert("thinkingConfig".to_string(), thinking_config);
    }
    generation_config
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
}
