use super::{
    RuntimeResponsesReply, observe_runtime_prompt_cache_profile_hit,
    observe_runtime_smart_context_token_usage_for_bucket, runtime_route_kind_label,
    runtime_smart_context_normalized_model_name,
};
use crate::{RuntimeRotationProxyShared, runtime_proxy_log};
use prodex_runtime_state::RuntimeRouteKind;
use redaction::{
    redaction_redact_json, redaction_redact_secret_like_text, redaction_redacted_body_snippet,
    redaction_redacted_headers_debug,
};
use runtime_proxy_crate::{
    RuntimeTokenUsage, runtime_proxy_log_field, runtime_proxy_structured_log_message,
};

pub(crate) use runtime_proxy_crate::{
    extract_runtime_proxy_overload_message_from_websocket_payload,
    extract_runtime_proxy_previous_response_message, extract_runtime_proxy_quota_message,
    extract_runtime_proxy_quota_message_from_websocket_payload,
    extract_runtime_response_ids_from_body_bytes, extract_runtime_token_usage_from_body_bytes,
    extract_runtime_turn_state_from_body_bytes, inspect_runtime_sse_buffer,
    runtime_proxy_body_snippet,
};

#[cfg(test)]
pub(crate) use runtime_proxy_crate::{
    extract_runtime_response_ids_from_payload, parse_runtime_sse_payload,
};

pub(crate) fn extract_runtime_proxy_quota_message_from_response_reply(
    response: &RuntimeResponsesReply,
) -> Option<String> {
    match response {
        RuntimeResponsesReply::Buffered(parts) => extract_runtime_proxy_quota_message(&parts.body),
        RuntimeResponsesReply::Streaming(_) => None,
    }
}

pub(crate) fn runtime_proxy_redacted_body_snippet(body: &[u8], max_chars: usize) -> String {
    redaction_redacted_body_snippet(body, max_chars)
}

pub(crate) fn runtime_proxy_body_indicates_token_invalidated(body: &[u8]) -> bool {
    String::from_utf8_lossy(body)
        .to_ascii_lowercase()
        .contains("token invalidated")
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

pub(crate) struct RuntimeTokenUsageLog<'a> {
    pub(crate) shared: &'a RuntimeRotationProxyShared,
    pub(crate) request_id: u64,
    pub(crate) transport: &'static str,
    pub(crate) profile_name: &'a str,
    pub(crate) source: &'static str,
    pub(crate) prompt_cache_key: Option<&'a str>,
    pub(crate) model_name: Option<&'a str>,
    pub(crate) usage: Option<RuntimeTokenUsage>,
    pub(crate) generation_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy)]
enum RuntimeTokenUsageLogEvent {
    Final,
    Progress,
}

impl RuntimeTokenUsageLogEvent {
    fn name(self) -> &'static str {
        match self {
            Self::Final => "token_usage",
            Self::Progress => "token_usage_progress",
        }
    }
}

pub(crate) struct RuntimeStreamPayloadLog<'a> {
    pub(crate) shared: &'a RuntimeRotationProxyShared,
    pub(crate) request_id: u64,
    pub(crate) profile_name: &'a str,
    pub(crate) source: &'a str,
    pub(crate) message: &'a str,
}

pub(crate) fn log_runtime_stream_payload(input: RuntimeStreamPayloadLog<'_>) {
    let RuntimeStreamPayloadLog {
        shared,
        request_id,
        profile_name,
        source,
        message,
    } = input;
    if message.trim().is_empty() {
        return;
    }
    let message = runtime_stream_log_text(message);
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "stream_payload",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("route", "websocket"),
                runtime_proxy_log_field("transport", "websocket"),
                runtime_proxy_log_field("profile", profile_name),
                runtime_proxy_log_field("source", source),
                runtime_proxy_log_field("stream", message),
            ],
        ),
    );
}

fn runtime_stream_log_text(message: &str) -> String {
    let message = redaction_redact_secret_like_text(message);
    let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&message) else {
        return message;
    };
    redaction_redact_json(&mut value);
    serde_json::to_string(&value).unwrap_or(message)
}

pub(crate) fn log_runtime_token_usage(input: RuntimeTokenUsageLog<'_>) {
    log_runtime_token_usage_event(input, RuntimeTokenUsageLogEvent::Final);
}

pub(crate) fn log_runtime_token_usage_progress(input: RuntimeTokenUsageLog<'_>) {
    log_runtime_token_usage_event(input, RuntimeTokenUsageLogEvent::Progress);
}

fn log_runtime_token_usage_event(
    input: RuntimeTokenUsageLog<'_>,
    event: RuntimeTokenUsageLogEvent,
) {
    let RuntimeTokenUsageLog {
        shared,
        request_id,
        transport,
        profile_name,
        source,
        prompt_cache_key,
        model_name,
        usage,
        generation_ms,
    } = input;
    let Some(usage) = usage else {
        return;
    };
    let route_kind = match transport {
        "websocket" => RuntimeRouteKind::Websocket,
        _ => RuntimeRouteKind::Responses,
    };
    let prompt_cache_observation = if matches!(event, RuntimeTokenUsageLogEvent::Final) {
        Some(observe_runtime_prompt_cache_profile_hit(
            shared,
            profile_name,
            prompt_cache_key,
            usage.cached_input_tokens,
            route_kind,
        ))
    } else {
        None
    };
    if matches!(event, RuntimeTokenUsageLogEvent::Final) {
        observe_runtime_smart_context_token_usage_for_bucket(
            shared,
            usage,
            Some(runtime_proxy_crate::SmartContextTokenCalibrationBucketKey {
                route: Some(runtime_route_kind_label(route_kind).to_string()),
                model: runtime_smart_context_normalized_model_name(model_name),
                profile: Some(profile_name.to_string()),
                transport: Some(transport.to_string()),
            }),
        );
    }
    let uncached_input_tokens = usage.input_tokens.saturating_sub(usage.cached_input_tokens);
    let mut fields = vec![
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
        runtime_proxy_log_field(
            "prompt_cache_owner",
            prompt_cache_observation.map_or("not_recorded", |value| value.log_label()),
        ),
        runtime_proxy_log_field("input_tokens", usage.input_tokens.to_string()),
        runtime_proxy_log_field("uncached_input_tokens", uncached_input_tokens.to_string()),
        runtime_proxy_log_field("cached_input_tokens", usage.cached_input_tokens.to_string()),
        runtime_proxy_log_field("output_tokens", usage.output_tokens.to_string()),
        runtime_proxy_log_field("reasoning_tokens", usage.reasoning_tokens.to_string()),
    ];
    if matches!(event, RuntimeTokenUsageLogEvent::Final)
        && usage.output_tokens > 0
        && let Some(generation_ms) = generation_ms.filter(|value| *value > 0)
    {
        fields.push(runtime_proxy_log_field(
            "generation_ms",
            generation_ms.to_string(),
        ));
        let output_tokens_per_second = usage.output_tokens as f64 * 1_000.0 / generation_ms as f64;
        if output_tokens_per_second.is_finite() {
            fields.push(runtime_proxy_log_field(
                "output_tokens_per_second",
                format!("{output_tokens_per_second:.1}"),
            ));
        }
    }
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(event.name(), fields),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_invalidated_classifier_matches_backend_body_text() {
        assert!(runtime_proxy_body_indicates_token_invalidated(
            br#"{"error":{"message":"Token invalidated. Please login again."}}"#
        ));
        assert!(!runtime_proxy_body_indicates_token_invalidated(
            b"temporary server unavailable"
        ));
    }

    #[test]
    fn stream_log_text_redacts_json_credentials() {
        let redacted = runtime_stream_log_text(r#"{"access_token":"secret","text":"hello"}"#);

        assert!(redacted.contains("<redacted>"));
        assert!(redacted.contains("hello"));
        assert!(!redacted.contains("secret"));
    }
}
