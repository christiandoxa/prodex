#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeResponseForwardingBodyKind {
    Unary,
    Sse,
}

impl RuntimeResponseForwardingBodyKind {
    pub fn is_sse(self) -> bool {
        matches!(self, Self::Sse)
    }

    pub fn as_log_label(self) -> &'static str {
        match self {
            Self::Unary => "unary",
            Self::Sse => "sse",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeBufferedResponseMetadata<'a> {
    pub status: u16,
    pub content_type: Option<&'a str>,
    pub body_bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeSseForwardingCommitDetail {
    pub prelude_bytes: usize,
    pub response_id_count: usize,
}

pub fn should_skip_runtime_response_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "connection"
            | "content-encoding"
            | "content-length"
            | "date"
            | "server"
            | "transfer-encoding"
    )
}

pub fn runtime_forward_text_response_header(name: &str, value: &str) -> Option<(String, String)> {
    (!should_skip_runtime_response_header(name)).then(|| (name.to_string(), value.to_string()))
}

pub fn runtime_forward_binary_response_header(
    name: &str,
    value: &[u8],
) -> Option<(String, Vec<u8>)> {
    (!should_skip_runtime_response_header(name)).then(|| (name.to_string(), value.to_vec()))
}

pub fn runtime_forward_text_response_headers<'a>(
    headers: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> Vec<(String, String)> {
    headers
        .into_iter()
        .filter_map(|(name, value)| runtime_forward_text_response_header(name, value))
        .collect()
}

pub fn runtime_forward_binary_response_headers<'a>(
    headers: impl IntoIterator<Item = (&'a str, &'a [u8])>,
) -> Vec<(String, Vec<u8>)> {
    headers
        .into_iter()
        .filter_map(|(name, value)| runtime_forward_binary_response_header(name, value))
        .collect()
}

pub fn runtime_response_content_type_from_binary_headers<'a>(
    headers: impl IntoIterator<Item = (&'a str, &'a [u8])>,
) -> Option<&'a str> {
    headers.into_iter().find_map(|(name, value)| {
        name.eq_ignore_ascii_case("content-type")
            .then(|| std::str::from_utf8(value).ok())
            .flatten()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    })
}

pub fn runtime_response_forwarding_body_kind(
    content_type: Option<&str>,
) -> RuntimeResponseForwardingBodyKind {
    if content_type.is_some_and(|value| value.contains("text/event-stream")) {
        RuntimeResponseForwardingBodyKind::Sse
    } else {
        RuntimeResponseForwardingBodyKind::Unary
    }
}

pub fn runtime_response_content_type_is_sse(content_type: Option<&str>) -> bool {
    runtime_response_forwarding_body_kind(content_type).is_sse()
}

pub fn runtime_buffered_response_metadata<'a>(
    status: u16,
    headers: impl IntoIterator<Item = (&'a str, &'a [u8])>,
    body_bytes: usize,
) -> RuntimeBufferedResponseMetadata<'a> {
    RuntimeBufferedResponseMetadata {
        status,
        content_type: runtime_response_content_type_from_binary_headers(headers),
        body_bytes,
    }
}

pub fn runtime_sse_forwarding_commit_detail(
    prelude_bytes: usize,
    response_id_count: usize,
) -> RuntimeSseForwardingCommitDetail {
    RuntimeSseForwardingCommitDetail {
        prelude_bytes,
        response_id_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_runtime_response_headers_case_insensitively() {
        assert!(should_skip_runtime_response_header("Connection"));
        assert!(should_skip_runtime_response_header("content-length"));
        assert!(should_skip_runtime_response_header("TRANSFER-ENCODING"));
        assert!(!should_skip_runtime_response_header("x-codex-turn-state"));
        assert!(!should_skip_runtime_response_header("content-type"));
    }

    #[test]
    fn filters_text_response_headers_without_rewriting_values() {
        let headers = runtime_forward_text_response_headers([
            ("Content-Type", "text/event-stream"),
            ("Server", "upstream"),
            ("x-codex-turn-state", " ts-1 "),
        ]);

        assert_eq!(
            headers,
            vec![
                ("Content-Type".to_string(), "text/event-stream".to_string()),
                ("x-codex-turn-state".to_string(), " ts-1 ".to_string()),
            ]
        );
    }

    #[test]
    fn filters_binary_response_headers_without_utf8_requirement() {
        let headers = runtime_forward_binary_response_headers([
            ("Date", b"today".as_slice()),
            ("x-binary", b"\xff\x00".as_slice()),
        ]);

        assert_eq!(
            headers,
            vec![("x-binary".to_string(), b"\xff\x00".to_vec())]
        );
    }

    #[test]
    fn extracts_buffered_response_metadata_from_binary_headers() {
        let headers = [
            ("x-extra", b"ignored".as_slice()),
            ("content-type", b" application/json ".as_slice()),
        ];

        let metadata = runtime_buffered_response_metadata(201, headers, 42);

        assert_eq!(
            metadata,
            RuntimeBufferedResponseMetadata {
                status: 201,
                content_type: Some("application/json"),
                body_bytes: 42,
            }
        );
    }

    #[test]
    fn classifies_sse_content_type_with_existing_case_sensitive_match() {
        assert_eq!(
            runtime_response_forwarding_body_kind(Some("text/event-stream; charset=utf-8")),
            RuntimeResponseForwardingBodyKind::Sse
        );
        assert_eq!(
            runtime_response_forwarding_body_kind(Some("TEXT/EVENT-STREAM")),
            RuntimeResponseForwardingBodyKind::Unary
        );
        assert_eq!(
            runtime_response_forwarding_body_kind(None),
            RuntimeResponseForwardingBodyKind::Unary
        );
    }

    #[test]
    fn builds_sse_commit_detail_for_logs() {
        assert_eq!(
            runtime_sse_forwarding_commit_detail(17, 2),
            RuntimeSseForwardingCommitDetail {
                prelude_bytes: 17,
                response_id_count: 2,
            }
        );
    }
}
