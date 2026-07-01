use redaction::redaction_redact_secret_like_text;

pub(super) fn runtime_provider_sse_event(event: &str, data: serde_json::Value) -> String {
    let data = serde_json::to_string(&data).unwrap_or_else(|_| "{}".to_string());
    format!("event: {event}\r\ndata: {data}\r\n\r\n")
}

pub(super) fn runtime_provider_sse_failed_event(
    sequence_number: u64,
    created_at: u64,
    response_id: &str,
    code: &str,
    message: &str,
) -> String {
    let message = redaction_redact_secret_like_text(message);
    runtime_provider_sse_event(
        "response.failed",
        serde_json::json!({
            "type": "response.failed",
            "sequence_number": sequence_number,
            "created_at": created_at,
            "response": {
                "id": response_id,
                "error": {
                    "code": code,
                    "message": message,
                },
            },
        }),
    )
}

pub(super) fn runtime_provider_sse_output_text_item_added_event(
    sequence_number: u64,
    response_id: &str,
    item_id: &str,
) -> String {
    runtime_provider_sse_event(
        "response.output_item.added",
        serde_json::json!({
            "type": "response.output_item.added",
            "sequence_number": sequence_number,
            "response_id": response_id,
            "item": {
                "id": item_id,
                "type": "message",
                "role": "assistant",
                "content": [],
            },
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_sse_failed_event_redacts_secret_like_message_material() {
        let event = runtime_provider_sse_failed_event(
            1,
            2,
            "resp_1",
            "provider_stream_error",
            "upstream failed: Authorization: Bearer fixture_token_123 url=https://example.test?api_key=sk-fixture-123456",
        );

        assert!(event.contains("Authorization: Bearer <redacted>"));
        assert!(event.contains("api_key=<redacted>"));
        assert!(!event.contains("fixture_token_123"));
        assert!(!event.contains("sk-fixture-123456"));
    }
}

pub(super) fn runtime_provider_sse_output_text_item_done_event(
    sequence_number: u64,
    response_id: &str,
    item_id: &str,
    text: &str,
) -> String {
    runtime_provider_sse_event(
        "response.output_item.done",
        serde_json::json!({
            "type": "response.output_item.done",
            "sequence_number": sequence_number,
            "response_id": response_id,
            "item": {
                "id": item_id,
                "type": "message",
                "role": "assistant",
                "content": [{
                    "type": "output_text",
                    "text": text,
                }],
            },
        }),
    )
}
