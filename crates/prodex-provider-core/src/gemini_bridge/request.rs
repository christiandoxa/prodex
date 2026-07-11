//! Gemini request-shape bridge helpers.

mod exact_output;
mod native_project;
mod simple;
mod tools;

pub use self::exact_output::{
    gemini_provider_core_exact_output_generate_chunk, gemini_provider_core_exact_output_sse_stream,
};
pub use self::native_project::gemini_provider_core_native_request_body_with_project;
pub use self::simple::gemini_provider_core_simple_request;
pub use self::tools::{
    gemini_provider_core_builtin_tools_from_request,
    gemini_provider_core_function_declaration_from_openai_tool,
    gemini_provider_core_function_tools_from_chat, gemini_provider_core_request_body_without_tool,
    gemini_provider_core_sanitize_function_schema, gemini_provider_core_tool_config_from_request,
    gemini_provider_core_tools_from_requests, gemini_provider_core_unsupported_tool_fallback_body,
};

use crate::translators::{gemini_contents_from_request, gemini_generation_config_from_request};
use crate::{ProviderTransformResult, provider_core_rewritten_body};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeminiProviderCoreGenerateContentRequest {
    pub body: serde_json::Value,
    pub request: serde_json::Map<String, serde_json::Value>,
    pub model: String,
    pub stream: bool,
}

#[allow(clippy::too_many_arguments)]
pub fn gemini_provider_core_generate_content_request(
    original: &serde_json::Value,
    chat: &serde_json::Value,
    default_model: &str,
    project_id: Option<&str>,
    code_assist: bool,
    thinking_budget_tokens: Option<u64>,
    system_instruction: Option<serde_json::Value>,
    tools: Option<serde_json::Value>,
    tool_config: Option<serde_json::Value>,
) -> GeminiProviderCoreGenerateContentRequest {
    let model = chat
        .get("model")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(default_model)
        .to_string();
    let stream = chat
        .get("stream")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let generation_config = gemini_provider_core_generation_config_from_request(
        original,
        chat,
        &model,
        thinking_budget_tokens,
    );
    let request = gemini_provider_core_generate_content_request_map(
        original,
        system_instruction,
        runtime_gemini_contents_from_chat(chat),
        tools,
        tool_config,
        generation_config,
    );
    let body =
        gemini_provider_core_generate_content_body_value(&model, project_id, code_assist, &request);

    GeminiProviderCoreGenerateContentRequest {
        body,
        request,
        model,
        stream,
    }
}

fn runtime_gemini_contents_from_chat(chat: &serde_json::Value) -> Vec<serde_json::Value> {
    if chat.get("input").is_some() {
        return gemini_contents_from_request(chat);
    }
    if let Some(messages) = chat.get("messages").and_then(serde_json::Value::as_array) {
        return gemini_contents_from_request(&serde_json::json!({ "input": messages }));
    }
    vec![serde_json::json!({"role":"user","parts":[{"text":""}]})]
}

pub fn gemini_provider_core_request_body(
    result: &ProviderTransformResult,
    translated_body: &[u8],
) -> Option<Vec<u8>> {
    let base = provider_core_rewritten_body(Some(result))?;
    let mut base_value = serde_json::from_slice::<serde_json::Value>(&base).ok()?;
    let translated_value = serde_json::from_slice::<serde_json::Value>(translated_body).ok()?;
    let base_object = base_value.as_object_mut()?;
    let translated_object = translated_value.as_object()?;
    if let Some(project) = translated_object.get("project") {
        base_object.insert("project".to_string(), project.clone());
    }
    let translated_request = translated_object.get("request")?.as_object()?;
    let base_request = base_object.get_mut("request")?.as_object_mut()?;
    if let Some(generation_config) = translated_request.get("generationConfig") {
        base_request.insert("generationConfig".to_string(), generation_config.clone());
    }
    serde_json::to_vec(&base_value).ok()
}

pub fn gemini_provider_core_generation_config_from_request(
    original: &serde_json::Value,
    chat: &serde_json::Value,
    model: &str,
    thinking_budget_tokens: Option<u64>,
) -> serde_json::Value {
    gemini_generation_config_from_request(original, chat, model, thinking_budget_tokens)
}

pub fn gemini_provider_core_generate_content_request_map(
    original: &serde_json::Value,
    system_instruction: Option<serde_json::Value>,
    contents: Vec<serde_json::Value>,
    tools: Option<serde_json::Value>,
    tool_config: Option<serde_json::Value>,
    generation_config: serde_json::Value,
) -> serde_json::Map<String, serde_json::Value> {
    let mut request = serde_json::Map::new();
    if let Some(system_instruction) = system_instruction {
        request.insert("systemInstruction".to_string(), system_instruction);
    }
    request.insert("contents".to_string(), serde_json::Value::Array(contents));
    if let Some(tools) = tools {
        request.insert("tools".to_string(), tools);
    }
    if let Some(tool_config) = tool_config {
        request.insert("toolConfig".to_string(), tool_config);
    }
    request.insert("generationConfig".to_string(), generation_config);
    if let Some(settings) = original
        .get("safety_settings")
        .or_else(|| original.get("safetySettings"))
    {
        request.insert("safetySettings".to_string(), settings.clone());
    }
    if let Some(cached_content) = original
        .get("cached_content")
        .or_else(|| original.get("cachedContent"))
        .filter(|value| !value.is_null())
    {
        request.insert("cachedContent".to_string(), cached_content.clone());
    }
    if let Some(labels) = original.get("labels").filter(|value| !value.is_null()) {
        request.insert("labels".to_string(), labels.clone());
    }
    request
}

pub fn gemini_provider_core_generate_content_body_value(
    model: &str,
    project_id: Option<&str>,
    code_assist: bool,
    request: &serde_json::Map<String, serde_json::Value>,
) -> serde_json::Value {
    if code_assist {
        serde_json::json!({
            "model": model,
            "project": project_id,
            "request": serde_json::Value::Object(request.clone()),
        })
    } else {
        serde_json::Value::Object(request.clone())
    }
}
