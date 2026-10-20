use base64::Engine;
use redaction::{redaction_redact_json, redaction_redact_secret_like_text};

use super::{
    RuntimeRotationProxyShared, RuntimeRouteKind, runtime_proxy_log, runtime_proxy_log_field,
    runtime_proxy_structured_log_message, runtime_route_kind_label,
};

const MAX_UPSTREAM_PAYLOAD_LOG_BYTES: usize = 64 * 1024;
const MAX_UPSTREAM_PAYLOAD_REDACTION_TEXT_BYTES: usize = 1024;
const UPSTREAM_PAYLOAD_OMISSION_MARKER: &str = "<omitted:large-text>";

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
        // Borrow Serde's value strings in its original traversal order.
        // Compiled Mojo owns which values fit the cumulative byte budget.
        let mut strings = Vec::new();
        collect_upstream_payload_string_views(&mut value, &mut strings);
        let mut remaining = MAX_UPSTREAM_PAYLOAD_REDACTION_TEXT_BYTES;
        for batch in strings.chunks_mut(1_048_576) {
            let lengths = batch.iter().map(|text| text.len()).collect::<Vec<_>>();
            let plan =
                prodex_mojo_core::log::upstream_payload_string_budget_plan(&lengths, remaining)
                    .expect("compiled Mojo upstream payload text budget returned invalid output");
            remaining = plan.remaining;
            for (text, omit) in batch.iter_mut().zip(plan.omit) {
                if omit {
                    **text = UPSTREAM_PAYLOAD_OMISSION_MARKER.to_owned();
                }
            }
        }
        drop(strings);
        redaction_redact_json(&mut value);
        return serde_json::to_vec(&value).ok();
    }
    let text = std::str::from_utf8(payload).ok()?;
    if text.len() > MAX_UPSTREAM_PAYLOAD_REDACTION_TEXT_BYTES {
        return Some(UPSTREAM_PAYLOAD_OMISSION_MARKER.as_bytes().to_vec());
    }
    Some(redaction_redact_secret_like_text(text).into_bytes())
}

// Host acquisition only: borrow each Serde string value in the same map/
// array traversal order used by redaction. The byte budget, omit decisions
// and remaining count are exclusively compiled Mojo policy.
fn collect_upstream_payload_string_views<'a>(
    value: &'a mut serde_json::Value,
    strings: &mut Vec<&'a mut String>,
) {
    match value {
        serde_json::Value::Object(map) => {
            for value in map.values_mut() {
                collect_upstream_payload_string_views(value, strings);
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                collect_upstream_payload_string_views(value, strings);
            }
        }
        serde_json::Value::String(text) => strings.push(text),
        _ => {}
    }
}

fn truncate_upstream_payload(payload: &[u8]) -> (&[u8], bool) {
    let (end, truncated) =
        prodex_mojo_core::log::upstream_payload_log_prefix(payload, MAX_UPSTREAM_PAYLOAD_LOG_BYTES)
            .expect("compiled Mojo upstream payload log prefix returned invalid output");
    (&payload[..end], truncated)
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
    fn upstream_payload_redaction_omits_large_string_content_before_redaction() {
        let payload = serde_json::json!({
            "access_token": "super-secret-token",
            "output": "safe output ".repeat(20_000),
            "nested": {"text": "user@example.test"}
        })
        .to_string();
        let redacted = redacted_upstream_payload(payload.as_bytes()).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&redacted).unwrap();

        assert_eq!(value["access_token"], "<redacted>");
        assert_eq!(value["nested"]["text"], "<redacted>");
        let output = value["output"].as_str().unwrap();
        assert_eq!(output, UPSTREAM_PAYLOAD_OMISSION_MARKER);
    }

    #[test]
    fn compiled_upstream_payload_string_budget_preserves_serde_order_skips_and_secret_redaction() {
        let payload = serde_json::json!({
            "a": "x".repeat(900),
            "b": "x".repeat(200),
            "c": "x".repeat(100),
            "d": "x".repeat(30),
            "e": [{"text": ""}, {"text": "x".repeat(20)}],
            "n": 24,
            "access_token": "ignored-for-this-test",
        });
        let output = redacted_upstream_payload(payload.to_string().as_bytes()).unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&output).unwrap();
        // Serde's object order is the same as for the former Rust
        // recursive budget walker; the budget planner may skip an
        // oversized value without consuming the remaining allowance.
        assert_eq!(parsed["a"].as_str().unwrap().len(), 900);
        assert_eq!(parsed["b"], UPSTREAM_PAYLOAD_OMISSION_MARKER);
        assert_eq!(parsed["c"].as_str().unwrap().len(), 100);
        assert_eq!(parsed["d"], UPSTREAM_PAYLOAD_OMISSION_MARKER);
        assert_eq!(parsed["e"][0]["text"], "");
        assert_eq!(parsed["e"][1]["text"], UPSTREAM_PAYLOAD_OMISSION_MARKER);
        assert_eq!(parsed["n"], 24);
        assert_ne!(parsed["access_token"], "ignored-for-this-test");
    }

    #[test]
    fn compiled_upstream_payload_log_prefix_keeps_exact_byte_boundary_semantics() {
        let sample = format!("A{}", "é".repeat(32_768));
        let (prefix, truncated) = truncate_upstream_payload(sample.as_bytes());
        assert!(truncated);
        assert_eq!(
            prefix, b"A",
            "cutting a multi-byte scalar falls back to last ASCII byte"
        );

        let ascii = vec![b'X'; MAX_UPSTREAM_PAYLOAD_LOG_BYTES + 1];
        let (prefix, truncated) = truncate_upstream_payload(&ascii);
        assert!(truncated);
        assert_eq!(prefix.len(), MAX_UPSTREAM_PAYLOAD_LOG_BYTES);

        let exactly_full = vec![b'Y'; MAX_UPSTREAM_PAYLOAD_LOG_BYTES];
        let (prefix, truncated) = truncate_upstream_payload(&exactly_full);
        assert!(!truncated);
        assert_eq!(prefix.len(), exactly_full.len());

        let unicode_only = "雪".repeat(21_846);
        let (prefix, truncated) = truncate_upstream_payload(unicode_only.as_bytes());
        assert!(truncated);
        assert!(
            prefix.is_empty(),
            "the original fallback found no ASCII before the cut"
        );

        let malformed = vec![0xff_u8; MAX_UPSTREAM_PAYLOAD_LOG_BYTES + 1];
        let (prefix, truncated) = truncate_upstream_payload(&malformed);
        assert!(truncated);
        assert!(prefix.is_empty());
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
