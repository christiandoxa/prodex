use super::deepseek_rewrite::{
    RuntimeDeepSeekConversationStore, RuntimeDeepSeekRewriteOptions,
    RuntimeDeepSeekTranslatedRequest, runtime_deepseek_apply_web_search_mode,
    runtime_deepseek_dedup_and_validate_function_tools,
    runtime_deepseek_messages_from_responses_request,
    runtime_deepseek_tool_choice_from_responses_request,
    runtime_deepseek_tools_from_responses_request,
    runtime_deepseek_web_search_options_from_responses_request,
};
use super::provider_bridge::{RuntimeProviderBridgeKind, runtime_provider_canonical_model};
use anyhow::{Context, Result};
use prodex_provider_core::{
    deepseek_provider_core_apply_reasoning_from_responses_request,
    deepseek_provider_core_ensure_json_prompt_instruction,
    deepseek_provider_core_function_tool_name,
    deepseek_provider_core_insert_primitive_request_fields,
    deepseek_provider_core_normalize_thinking_tool_call_messages,
    deepseek_provider_core_note_thinking_tool_choice_omission,
    deepseek_provider_core_reject_beta_completion_fields,
    deepseek_provider_core_reject_unsupported_request_fields,
    deepseek_provider_core_repair_tool_call_adjacency,
    deepseek_provider_core_response_format_from_responses_request,
    deepseek_provider_core_response_metadata_from_responses_request,
    deepseek_provider_core_stop_from_responses_request, deepseek_provider_core_thinking_enabled,
    deepseek_provider_core_top_logprobs_from_responses_request,
    deepseek_provider_core_user_id_from_responses_request,
    deepseek_provider_core_validate_reasoning_shape,
    deepseek_provider_core_validate_tool_choice_name,
    deepseek_provider_core_validate_tool_choice_shape,
    deepseek_provider_core_validate_tool_choice_target,
    deepseek_provider_core_validate_tools_shape,
    gemini_provider_core_preserve_tool_call_signatures,
    provider_core_chat_compatible_validate_top_level_request_shape,
};
use std::collections::BTreeSet;

pub(super) fn runtime_provider_chat_compatible_request_body(
    body: &[u8],
    conversations: &RuntimeDeepSeekConversationStore,
    provider_kind: RuntimeProviderBridgeKind,
    default_model: &str,
    include_reasoning_params: bool,
    options: RuntimeDeepSeekRewriteOptions,
) -> Result<RuntimeDeepSeekTranslatedRequest> {
    let gemini_compat = provider_kind == RuntimeProviderBridgeKind::Gemini;
    let provider_label = provider_kind.chat_compatible_adapter_label();
    let provider_key = provider_kind.provider_id().label();
    let value: serde_json::Value =
        serde_json::from_slice(body).context("failed to parse Codex Responses request JSON")?;
    deepseek_provider_core_reject_beta_completion_fields(&value, provider_label)
        .map_err(anyhow::Error::msg)?;
    provider_core_chat_compatible_validate_top_level_request_shape(&value, provider_label)
        .map_err(anyhow::Error::msg)?;
    deepseek_provider_core_validate_reasoning_shape(&value, provider_label)
        .map_err(anyhow::Error::msg)?;
    let mut request = serde_json::Map::new();
    let model = value
        .get("model")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(default_model);
    let model = runtime_provider_canonical_model(provider_kind, model);
    let stream = value
        .get("stream")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    request.insert("model".to_string(), serde_json::Value::String(model));
    request.insert("stream".to_string(), serde_json::Value::Bool(stream));
    let thinking_enabled = deepseek_provider_core_thinking_enabled(&value);
    let mut messages = runtime_deepseek_messages_from_responses_request(
        &value,
        conversations,
        gemini_compat,
        provider_kind,
    )?
    .unwrap_or_else(|| {
        vec![serde_json::json!({
            "role": "user",
            "content": "",
        })]
    });
    deepseek_provider_core_repair_tool_call_adjacency(&mut messages);
    if provider_kind == RuntimeProviderBridgeKind::Gemini {
        gemini_provider_core_preserve_tool_call_signatures(&mut messages);
    }
    if messages.is_empty() {
        messages.push(serde_json::json!({
            "role": "user",
            "content": "",
        }));
    }
    if thinking_enabled {
        deepseek_provider_core_normalize_thinking_tool_call_messages(&mut messages);
    }
    let response_format =
        deepseek_provider_core_response_format_from_responses_request(&value, provider_label)
            .map_err(anyhow::Error::msg)?;
    let mut response_metadata = deepseek_provider_core_response_metadata_from_responses_request(
        &value,
        provider_label,
        provider_key,
    )
    .map_err(anyhow::Error::msg)?;
    deepseek_provider_core_note_thinking_tool_choice_omission(
        &value,
        thinking_enabled,
        provider_label,
        provider_key,
        &mut response_metadata,
    );
    if response_format.is_some() {
        deepseek_provider_core_ensure_json_prompt_instruction(&mut messages);
    }
    request.insert(
        "messages".to_string(),
        serde_json::Value::Array(messages.clone()),
    );
    if include_reasoning_params {
        deepseek_provider_core_apply_reasoning_from_responses_request(
            &value,
            &mut request,
            provider_label,
            gemini_compat,
        )
        .map_err(anyhow::Error::msg)?;
    }
    let mut tool_names = BTreeSet::new();
    deepseek_provider_core_validate_tools_shape(&value, gemini_compat, provider_label)
        .map_err(anyhow::Error::msg)?;
    if let Some(tools) = runtime_deepseek_tools_from_responses_request(&value) {
        let tools =
            runtime_deepseek_dedup_and_validate_function_tools(tools, options, provider_kind)?;
        if tools.len() > 128 {
            anyhow::bail!(
                "{} supports at most 128 function tools",
                provider_kind.chat_compatible_adapter_label()
            );
        }
        tool_names.extend(
            tools
                .iter()
                .filter_map(deepseek_provider_core_function_tool_name),
        );
        request.insert("tools".to_string(), serde_json::Value::Array(tools));
    }
    if !gemini_compat
        && let Some(web_search_options) =
            runtime_deepseek_web_search_options_from_responses_request(&value, provider_kind)?
    {
        runtime_deepseek_apply_web_search_mode(
            &mut request,
            web_search_options,
            options,
            provider_kind,
        )?;
    }
    deepseek_provider_core_validate_tool_choice_shape(&value, thinking_enabled, provider_label)
        .map_err(anyhow::Error::msg)?;
    if let Some(tool_choice) =
        runtime_deepseek_tool_choice_from_responses_request(&value, thinking_enabled)
    {
        deepseek_provider_core_validate_tool_choice_name(&tool_choice, provider_label)
            .map_err(anyhow::Error::msg)?;
        deepseek_provider_core_validate_tool_choice_target(
            &tool_choice,
            &tool_names,
            provider_label,
        )
        .map_err(anyhow::Error::msg)?;
        request.insert("tool_choice".to_string(), tool_choice);
    }
    deepseek_provider_core_insert_primitive_request_fields(&value, &mut request, provider_label)
        .map_err(anyhow::Error::msg)?;
    if !gemini_compat {
        deepseek_provider_core_reject_unsupported_request_fields(&value, provider_label)
            .map_err(anyhow::Error::msg)?;
    }
    if let Some(stop) = deepseek_provider_core_stop_from_responses_request(&value, provider_label)
        .map_err(anyhow::Error::msg)?
    {
        request.insert("stop".to_string(), stop);
    }
    if let Some(top_logprobs) =
        deepseek_provider_core_top_logprobs_from_responses_request(&value, provider_label)
            .map_err(anyhow::Error::msg)?
    {
        request.insert("top_logprobs".to_string(), top_logprobs);
    }
    if stream {
        request.insert(
            "stream_options".to_string(),
            serde_json::json!({"include_usage": true}),
        );
    }
    if let Some(response_format) = response_format {
        request.insert("response_format".to_string(), response_format);
    }
    if let Some(user_id) =
        deepseek_provider_core_user_id_from_responses_request(&value, provider_label)
            .map_err(anyhow::Error::msg)?
    {
        request.insert("user_id".to_string(), serde_json::Value::String(user_id));
    }
    let body = serde_json::to_vec(&serde_json::Value::Object(request)).with_context(|| {
        format!(
            "failed to serialize {} chat request JSON",
            provider_kind.chat_compatible_adapter_label()
        )
    })?;
    Ok(RuntimeDeepSeekTranslatedRequest {
        body,
        messages,
        response_metadata,
    })
}
