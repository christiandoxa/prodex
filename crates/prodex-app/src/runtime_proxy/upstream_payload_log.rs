use base64::Engine;
use redaction::{redaction_redact_json, redaction_redact_secret_like_text};

use super::{
    RuntimeRotationProxyShared, RuntimeRouteKind, runtime_proxy_log, runtime_proxy_log_field,
    runtime_proxy_structured_log_message, runtime_route_kind_label,
};

const MAX_UPSTREAM_PAYLOAD_LOG_BYTES: usize = 64 * 1024;

pub(crate) fn log_runtime_upstream_payload_snapshot(
    shared: &RuntimeRotationProxyShared,
    request_id: u64,
    transport: &str,
    route_kind: RuntimeRouteKind,
    profile_name: &str,
    payload: &[u8],
) {
    let bytes = payload.len();
    let Some(payload) = redacted_upstream_payload(payload) else {
        return;
    };
    let (payload, truncated) = truncate_upstream_payload(&payload);
    let payload_b64 = base64::engine::general_purpose::STANDARD.encode(payload);
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "upstream_payload",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("transport", transport),
                runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                runtime_proxy_log_field("profile", profile_name),
                runtime_proxy_log_field("bytes", bytes.to_string()),
                runtime_proxy_log_field("logged_bytes", payload.len().to_string()),
                runtime_proxy_log_field("truncated", truncated.to_string()),
                runtime_proxy_log_field("payload_b64", payload_b64),
            ],
        ),
    );
}

fn redacted_upstream_payload(payload: &[u8]) -> Option<Vec<u8>> {
    if let Ok(mut value) = serde_json::from_slice::<serde_json::Value>(payload) {
        redaction_redact_json(&mut value);
        return serde_json::to_vec(&value).ok();
    }
    let text = std::str::from_utf8(payload).ok()?;
    Some(redaction_redact_secret_like_text(text).into_bytes())
}

fn truncate_upstream_payload(payload: &[u8]) -> (&[u8], bool) {
    if payload.len() <= MAX_UPSTREAM_PAYLOAD_LOG_BYTES {
        return (payload, false);
    }
    let end = std::str::from_utf8(&payload[..MAX_UPSTREAM_PAYLOAD_LOG_BYTES])
        .map(|text| text.len())
        .unwrap_or_else(|_| {
            payload[..MAX_UPSTREAM_PAYLOAD_LOG_BYTES]
                .iter()
                .rposition(|byte| *byte < 0x80)
                .map(|index| index + 1)
                .unwrap_or(0)
        });
    (&payload[..end], true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upstream_payload_redaction_keeps_content_but_removes_credentials() {
        let payload = br#"{"input":"run","access_token":"secret","text":"hello"}"#;
        let redacted = redacted_upstream_payload(payload).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&redacted).unwrap();

        assert_eq!(value["input"], "run");
        assert_eq!(value["text"], "hello");
        assert_eq!(value["access_token"], "<redacted>");
    }

    #[test]
    fn upstream_payload_log_is_bounded_without_splitting_utf8() {
        let payload = "é".repeat(MAX_UPSTREAM_PAYLOAD_LOG_BYTES);
        let (truncated, was_truncated) = truncate_upstream_payload(payload.as_bytes());

        assert!(was_truncated);
        assert!(std::str::from_utf8(truncated).is_ok());
        assert!(truncated.len() <= MAX_UPSTREAM_PAYLOAD_LOG_BYTES);
    }
}
