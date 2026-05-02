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

pub(crate) fn log_runtime_token_usage(
    shared: &RuntimeRotationProxyShared,
    request_id: u64,
    transport: &'static str,
    profile_name: &str,
    source: &'static str,
    usage: Option<RuntimeTokenUsage>,
) {
    let Some(usage) = usage else {
        return;
    };
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "token_usage",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("transport", transport),
                runtime_proxy_log_field("profile", profile_name),
                runtime_proxy_log_field("source", source),
                runtime_proxy_log_field("input_tokens", usage.input_tokens.to_string()),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_secret(parts: &[&str]) -> String {
        parts.concat()
    }

    fn fake_named_secret(name: &str) -> String {
        fake_secret(&["fixture_", name, "_notreal_", "12345"])
    }

    fn fake_api_key(prefix: &str, name: &str) -> String {
        fake_secret(&[prefix, "fixture-", name, "-notreal-", "123456789"])
    }

    #[test]
    fn redacted_headers_debug_removes_sensitive_header_values() {
        let authorization_token = fake_named_secret("authorization");
        let api_key = fake_api_key("sk-ant-", "header");
        let headers = vec![
            (
                "authorization".to_string(),
                format!("Bearer {authorization_token}"),
            ),
            ("x-api-key".to_string(), api_key.clone()),
            ("anthropic-version".to_string(), "2023-06-01".to_string()),
        ];

        let redacted = runtime_proxy_redacted_headers_debug(&headers);

        assert!(redacted.contains("authorization"));
        assert!(redacted.contains("Bearer <redacted>"));
        assert!(redacted.contains("anthropic-version"));
        assert!(!redacted.contains(authorization_token.as_str()));
        assert!(!redacted.contains(api_key.as_str()));
    }

    #[test]
    fn redacted_body_snippet_removes_json_secret_fields_and_token_values() {
        let api_key = fake_api_key("sk-ant-", "json");
        let bearer_token = fake_named_secret("body_bearer");
        let body = serde_json::to_vec(&serde_json::json!({
            "model": "claude-sonnet-4-6",
            "api_key": api_key.clone(),
            "messages": [
                {
                    "role": "user",
                    "content": format!("Use Bearer {bearer_token}")
                }
            ]
        }))
        .expect("test body should serialize");

        let redacted = runtime_proxy_redacted_body_snippet(&body, 4096);

        assert!(redacted.contains("claude-sonnet-4-6"));
        assert!(redacted.contains("\"api_key\":\"<redacted>\""));
        assert!(redacted.contains("Bearer <redacted>"));
        assert!(!redacted.contains(api_key.as_str()));
        assert!(!redacted.contains(bearer_token.as_str()));
    }
}
