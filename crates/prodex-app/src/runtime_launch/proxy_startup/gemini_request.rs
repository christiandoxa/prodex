use super::super::chat_compatible_rewrite::{
    RuntimeChatCompatibleConversationStore, RuntimeDeepSeekRewriteOptions,
    runtime_provider_chat_compatible_request_body,
};
use super::super::provider_bridge::RuntimeProviderBridgeKind;
use super::RuntimeGeminiTranslatedRequest;
use anyhow::{Context, Result, bail};
use prodex_provider_core::{
    PRODEX_GEMINI_DEFAULT_MODEL as GEMINI_DEFAULT_MODEL, ProviderEndpoint, ProviderId,
    ProviderTransformInput, gemini_provider_core_generation_config_from_request,
    provider_translator,
};

mod gemini_request_chat_source;
use gemini_request_chat_source::runtime_gemini_chat_source_request;

pub(in super::super) fn runtime_gemini_generate_request_body_with_config(
    body: &[u8],
    conversations: &RuntimeChatCompatibleConversationStore,
    code_assist: bool,
    project_id: Option<&str>,
    thinking_budget_tokens: Option<u64>,
    _allow_local_file_access: bool,
    _config: &crate::RuntimeGeminiConfig,
) -> Result<RuntimeGeminiTranslatedRequest> {
    let original: serde_json::Value =
        serde_json::from_slice(body).context("failed to parse Codex Responses request JSON")?;

    // Keep the conversation projection needed by continuation/affinity bookkeeping,
    // but make provider-core the single request-translation authority.
    let chat_source = runtime_gemini_chat_source_request(&original);
    let chat = runtime_provider_chat_compatible_request_body(
        &serde_json::to_vec(&chat_source).context("failed to serialize Gemini chat source JSON")?,
        conversations,
        RuntimeProviderBridgeKind::Gemini,
        GEMINI_DEFAULT_MODEL,
        true,
        RuntimeDeepSeekRewriteOptions::default(),
    )?;

    let translated = provider_translator(ProviderId::Gemini).transform_request(
        ProviderTransformInput::new(ProviderEndpoint::Responses, body.to_vec()),
    );
    let transform_status = prodex_provider_core::TransformStatus::from(&translated.loss);
    if let Some(reason) = transform_status.reason() {
        bail!("Gemini request translation failed: {reason}");
    }
    let translated_body = translated
        .body
        .context("Gemini request translation returned no body")?;
    let mut envelope: serde_json::Value = serde_json::from_slice(&translated_body)
        .context("Gemini request translation returned invalid JSON")?;
    let model = envelope
        .get("model")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(GEMINI_DEFAULT_MODEL)
        .to_string();

    if let Some(budget) = thinking_budget_tokens
        && let Some(request) = envelope
            .get_mut("request")
            .and_then(serde_json::Value::as_object_mut)
    {
        request.insert(
            "generationConfig".to_string(),
            gemini_provider_core_generation_config_from_request(
                &original,
                &original,
                &model,
                Some(budget),
            ),
        );
    }

    let body_value = if code_assist {
        if let Some(object) = envelope.as_object_mut() {
            object.insert(
                "project".to_string(),
                project_id.map_or(serde_json::Value::Null, |value| {
                    serde_json::Value::String(value.to_string())
                }),
            );
        }
        envelope
    } else {
        envelope
            .get("request")
            .cloned()
            .context("Gemini translator response is missing request")?
    };
    let stream = original
        .get("stream")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);

    Ok(RuntimeGeminiTranslatedRequest {
        body: serde_json::to_vec(&body_value)
            .context("failed to serialize Gemini generateContent request JSON")?,
        messages: chat.messages,
        model,
        stream,
    })
}
