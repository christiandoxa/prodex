//! Gemini request transform orchestration.

use crate::translator::{ProviderTransformInput, ProviderTransformResult};
use crate::{ProviderEndpoint, ProviderId, ProviderWireFormat};
use serde_json::{Value, json};

use super::request::{
    gemini_apply_optional_request_fields, gemini_apply_response_format, gemini_apply_text_format,
    gemini_builtin_tools_from_request, gemini_continuation_metadata,
    gemini_insert_basic_generation_config, gemini_insert_extended_generation_config,
    gemini_thinking_config_from_request, gemini_tool_config_from_request,
    gemini_tool_from_openai_tool,
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
    if input.endpoint != ProviderEndpoint::Responses {
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
    if let Some(response_format) = obj.get("response_format") {
        if let Err(reason) = gemini_apply_response_format(response_format, &mut generation_config) {
            return ProviderTransformResult::rejected(
                ProviderId::Gemini,
                input.endpoint,
                ProviderWireFormat::OpenAiResponses,
                ProviderWireFormat::GeminiGenerateContent,
                reason,
            );
        }
    }
    if !generation_config.is_empty() {
        request.insert(
            "generationConfig".to_string(),
            Value::Object(generation_config),
        );
    }
    if let Some(tools) = obj.get("tools").and_then(Value::as_array) {
        let mut translated_tools = gemini_builtin_tools_from_request(tools);
        let declarations: Vec<Value> = tools
            .iter()
            .filter_map(gemini_tool_from_openai_tool)
            .collect();
        if !declarations.is_empty() {
            translated_tools.push(json!({"functionDeclarations": declarations}));
        }
        if !translated_tools.is_empty() {
            request.insert("tools".to_string(), Value::Array(translated_tools));
        }
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
