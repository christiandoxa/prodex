//! Gemini request-shape bridge helpers.

mod exact_output;
mod native_project;
#[path = "request_contents.rs"]
mod request_contents;
mod simple;
mod tools;

pub use self::exact_output::{
    gemini_provider_core_exact_output_generate_chunk, gemini_provider_core_exact_output_sse_stream,
};
pub use self::native_project::gemini_provider_core_native_request_body_with_project;
#[cfg(feature = "mojo")]
pub(crate) use self::request_contents::{
    GeminiTranslatorValidationPlan, gemini_bridge_raw_translator_request,
    gemini_bridge_validate_translator,
};
pub use self::simple::gemini_provider_core_simple_request;
pub use self::tools::{
    gemini_provider_core_function_tools_from_chat,
    gemini_provider_core_function_tools_from_chat_checked,
    gemini_provider_core_request_body_without_tool, gemini_provider_core_sanitize_function_schema,
    gemini_provider_core_tool_config_from_request, gemini_provider_core_tools_from_requests,
    gemini_provider_core_tools_from_requests_checked,
    gemini_provider_core_unsupported_tool_fallback_body,
    gemini_provider_core_validate_request_tools,
};

use crate::translators::gemini_contents_from_request;
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
) -> Result<GeminiProviderCoreGenerateContentRequest, String> {
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
    )?;
    let request = gemini_provider_core_generate_content_request_map(
        original,
        system_instruction,
        runtime_gemini_contents_from_chat(chat),
        tools,
        tool_config,
        generation_config,
    )?;
    let body = gemini_provider_core_generate_content_body_value(
        &model,
        project_id,
        code_assist,
        &request,
    )?;

    Ok(GeminiProviderCoreGenerateContentRequest {
        body,
        request,
        model,
        stream,
    })
}

fn runtime_gemini_contents_from_chat(chat: &serde_json::Value) -> Vec<serde_json::Value> {
    if chat.get("input").is_some() {
        return gemini_contents_from_request(chat);
    }
    if let Some(messages) = chat.get("messages").and_then(serde_json::Value::as_array) {
        return gemini_contents_from_request(&serde_json::json!({ "input": messages }));
    }
    #[cfg(feature = "mojo")]
    {
        vec![request_contents::gemini_request_content_value(
            prodex_mojo_core::provider_constraints::GeminiRequestContentOperation::Content,
            Some(b"\"user\""),
            Some(br#"[{"text":""}]"#),
            None,
            None,
            0,
        )]
    }
    #[cfg(not(feature = "mojo"))]
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
) -> Result<serde_json::Value, String> {
    request_contents::gemini_bridge_request_generation_config(
        original,
        chat,
        model,
        thinking_budget_tokens,
    )
}

pub fn gemini_provider_core_validate_candidate_count(
    value: &serde_json::Value,
) -> Result<(), String> {
    #[cfg(feature = "mojo")]
    {
        request_contents::gemini_bridge_request_candidate_count(value)
    }
    #[cfg(not(feature = "mojo"))]
    crate::translators::gemini_validate_candidate_count(value)
}

pub fn gemini_provider_core_generate_content_request_map(
    original: &serde_json::Value,
    system_instruction: Option<serde_json::Value>,
    contents: Vec<serde_json::Value>,
    tools: Option<serde_json::Value>,
    tool_config: Option<serde_json::Value>,
    generation_config: serde_json::Value,
) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    request_contents::gemini_bridge_request_map(
        original,
        system_instruction.as_ref(),
        &contents,
        tools.as_ref(),
        tool_config.as_ref(),
        &generation_config,
    )
}

pub fn gemini_provider_core_generate_content_body_value(
    model: &str,
    project_id: Option<&str>,
    code_assist: bool,
    request: &serde_json::Map<String, serde_json::Value>,
) -> Result<serde_json::Value, String> {
    request_contents::gemini_bridge_request_body(model, project_id, code_assist, request)
}

#[cfg(test)]
mod tests {
    use super::{
        gemini_provider_core_generate_content_request_map,
        gemini_provider_core_generation_config_from_request,
        gemini_provider_core_tool_config_from_request,
    };
    use serde_json::json;

    #[test]
    fn generation_config_keeps_alias_defaults_and_u64_budget() {
        let config = gemini_provider_core_generation_config_from_request(
            &json!({
                "top_k": 1,
                "topK": null,
                "response_schema": {"title": "日本語"},
                "responseSchema": null,
                "candidate_count": 1,
                "reasoning": {"effort": "xhigh"}
            }),
            &json!({"temperature": null, "top_p": 0.25, "max_tokens": u64::MAX}),
            "gemini-2.5-pro",
            Some(u64::MAX),
        )
        .expect("valid generation config");

        assert_eq!(
            config,
            json!({
                "temperature": null,
                "topP": 0.25,
                "maxOutputTokens": u64::MAX,
                "topK": 1,
                "responseSchema": {"title": "日本語"},
                "candidateCount": 1,
                "thinkingConfig": {
                    "includeThoughts": true,
                    "thinkingBudget": u64::MAX
                }
            })
        );
    }

    #[test]
    fn request_map_preserves_optional_field_null_and_alias_rules() {
        let request = gemini_provider_core_generate_content_request_map(
            &json!({
                "safety_settings": null,
                "safetySettings": [{"category": "ignored"}],
                "cached_content": null,
                "cachedContent": "ignored-cache",
                "labels": {"suite": "日本語"}
            }),
            None,
            Vec::new(),
            None,
            None,
            json!({}),
        )
        .expect("valid request map");

        assert_eq!(request["contents"], json!([]));
        assert_eq!(request["generationConfig"], json!({}));
        assert!(request["safetySettings"].is_null());
        assert!(request.get("cachedContent").is_none());
        assert_eq!(request["labels"], json!({"suite": "日本語"}));
    }

    fn request_with_serialized_size(size: usize) -> serde_json::Value {
        let mut request = json!({"padding": ""});
        let empty_size = serde_json::to_vec(&request).unwrap().len();
        request["padding"] = serde_json::Value::String("x".repeat(size - empty_size));
        assert_eq!(serde_json::to_vec(&request).unwrap().len(), size);
        request
    }

    #[test]
    fn bridge_helpers_accept_the_mojo_fragment_limit_and_reject_one_byte_over() {
        const MAX_BYTES: usize = 4 * 1024 * 1024;

        let at_limit = request_with_serialized_size(MAX_BYTES);
        assert!(
            gemini_provider_core_generation_config_from_request(
                &at_limit,
                &json!({}),
                "gemini",
                None,
            )
            .is_ok()
        );
        assert_eq!(
            gemini_provider_core_generate_content_request_map(
                &at_limit,
                None,
                Vec::new(),
                None,
                None,
                json!({}),
            )
            .unwrap()["contents"],
            json!([])
        );

        let over_limit = request_with_serialized_size(MAX_BYTES + 1);
        assert!(
            gemini_provider_core_generation_config_from_request(
                &over_limit,
                &json!({}),
                "gemini",
                None,
            )
            .is_err()
        );
        assert!(
            gemini_provider_core_generate_content_request_map(
                &over_limit,
                None,
                Vec::new(),
                None,
                None,
                json!({}),
            )
            .is_err()
        );
        assert!(gemini_provider_core_tool_config_from_request(&over_limit).is_err());
    }
}
