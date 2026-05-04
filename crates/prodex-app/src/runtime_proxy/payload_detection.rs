use super::*;

pub(crate) use runtime_proxy_crate::{
    extract_runtime_proxy_overload_message,
    extract_runtime_proxy_overload_message_from_websocket_payload,
    extract_runtime_proxy_previous_response_message, extract_runtime_proxy_quota_message,
    extract_runtime_proxy_quota_message_from_websocket_payload,
    extract_runtime_response_ids_from_body_bytes, extract_runtime_token_usage_from_body_bytes,
    extract_runtime_turn_state_from_body_bytes, inspect_runtime_sse_buffer,
};

#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use runtime_proxy_crate::{
    extract_runtime_proxy_overload_message_from_text,
    extract_runtime_proxy_previous_response_message_from_text,
    extract_runtime_proxy_quota_message_candidate, extract_runtime_proxy_quota_message_from_text,
    extract_runtime_response_ids_from_value, extract_runtime_turn_state_from_headers_value,
    parse_runtime_sse_event, parse_runtime_sse_payload, push_runtime_response_id,
    runtime_proxy_overload_message, runtime_proxy_tool_context_missing_message,
    runtime_proxy_usage_limit_message, runtime_sse_trimmed_line_bytes,
};

#[cfg(test)]
pub(crate) fn extract_runtime_response_ids_from_payload(payload: &str) -> Vec<String> {
    serde_json::from_str::<serde_json::Value>(payload)
        .ok()
        .map(|value| extract_runtime_response_ids_from_value(&value))
        .unwrap_or_default()
}

pub(crate) fn extract_runtime_proxy_quota_message_from_response_reply(
    response: &RuntimeResponsesReply,
) -> Option<String> {
    match response {
        RuntimeResponsesReply::Buffered(parts) => extract_runtime_proxy_quota_message(&parts.body),
        RuntimeResponsesReply::Streaming(_) => None,
    }
}

pub(crate) fn runtime_proxy_body_snippet(body: &[u8], max_chars: usize) -> String {
    redaction_text_snippet(String::from_utf8_lossy(body).as_ref(), max_chars)
}

pub(crate) fn runtime_proxy_redacted_body_snippet(body: &[u8], max_chars: usize) -> String {
    redaction_redacted_body_snippet(body, max_chars)
}

pub(crate) fn runtime_proxy_redacted_headers_debug(headers: &[(String, String)]) -> String {
    redaction_redacted_headers_debug(headers)
}

fn runtime_prompt_cache_key_log_label(prompt_cache_key: Option<&str>) -> &'static str {
    if prompt_cache_key
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
    {
        "present"
    } else {
        "absent"
    }
}

fn runtime_prompt_cache_key_hash_label(prompt_cache_key: Option<&str>) -> String {
    prompt_cache_key
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(runtime_proxy_crate::smart_context_hash_text)
        .unwrap_or_else(|| "none".to_string())
}

pub(crate) fn log_runtime_token_usage(
    shared: &RuntimeRotationProxyShared,
    request_id: u64,
    transport: &'static str,
    profile_name: &str,
    source: &'static str,
    prompt_cache_key: Option<&str>,
    usage: Option<RuntimeTokenUsage>,
) {
    let Some(usage) = usage else {
        return;
    };
    let route_kind = match transport {
        "websocket" => RuntimeRouteKind::Websocket,
        _ => RuntimeRouteKind::Responses,
    };
    let prompt_cache_observation = observe_runtime_prompt_cache_profile_hit(
        shared,
        profile_name,
        prompt_cache_key,
        usage.cached_input_tokens,
        route_kind,
    );
    observe_runtime_smart_context_token_usage(shared, usage);
    let uncached_input_tokens = usage.input_tokens.saturating_sub(usage.cached_input_tokens);
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "token_usage",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                runtime_proxy_log_field("transport", transport),
                runtime_proxy_log_field("profile", profile_name),
                runtime_proxy_log_field("source", source),
                runtime_proxy_log_field(
                    "prompt_cache_key",
                    runtime_prompt_cache_key_log_label(prompt_cache_key),
                ),
                runtime_proxy_log_field(
                    "prompt_cache_key_hash",
                    runtime_prompt_cache_key_hash_label(prompt_cache_key),
                ),
                runtime_proxy_log_field("prompt_cache_owner", prompt_cache_observation.log_label()),
                runtime_proxy_log_field("input_tokens", usage.input_tokens.to_string()),
                runtime_proxy_log_field("uncached_input_tokens", uncached_input_tokens.to_string()),
                runtime_proxy_log_field(
                    "cached_input_tokens",
                    usage.cached_input_tokens.to_string(),
                ),
                runtime_proxy_log_field("output_tokens", usage.output_tokens.to_string()),
                runtime_proxy_log_field("reasoning_tokens", usage.reasoning_tokens.to_string()),
            ],
        ),
    );
}
