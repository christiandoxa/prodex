use super::*;

pub fn classify_upstream_headers(
    headers: &[GatewayHttpHeader],
) -> (Vec<GatewayHttpHeader>, Vec<String>) {
    let mut preserved = Vec::new();
    let mut stripped = Vec::new();
    for header in headers {
        let normalized = header.normalized_name();
        if preserve_upstream_header(&normalized) {
            preserved.push(GatewayHttpHeader::new(normalized, header.value.clone()));
        } else if strip_upstream_header(&normalized) {
            stripped.push(normalized);
        }
    }
    (preserved, stripped)
}

fn preserve_upstream_header(name: &str) -> bool {
    matches!(
        name,
        "session_id"
            | "x-openai-subagent"
            | "x-codex-turn-state"
            | "x-codex-turn-metadata"
            | "x-codex-beta-features"
            | "user-agent"
            | "traceparent"
            | "tracestate"
            | "baggage"
    )
}

fn strip_upstream_header(name: &str) -> bool {
    name == "host"
        || name == "connection"
        || name == "content-length"
        || name == "transfer-encoding"
        || name == "upgrade"
        || name.starts_with("sec-websocket-")
        || name == "authorization"
        || name == "chatgpt-account-id"
}
