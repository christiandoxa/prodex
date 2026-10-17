use super::*;
use prodex_mojo_core::rich::SuperExposeParsedOriginUrl;

pub(super) fn validate_mcp_request_headers(
    message: &serde_json::Map<String, Value>,
    method: &str,
    headers: &McpRequestHeaders,
) -> Option<Response<std::io::Cursor<Vec<u8>>>> {
    use prodex_mojo_core::rich::SuperExposeProtocolDecision;

    let params = message.get("params").and_then(Value::as_object);
    let body_version = request_body_protocol_version(method, params);
    let body_name = (method == "tools/call")
        .then(|| {
            params
                .and_then(|params| params.get("name"))
                .and_then(Value::as_str)
        })
        .flatten();
    let decision = prodex_mojo_core::rich::super_expose_protocol_metadata(
        method,
        headers.protocol_version.as_deref(),
        body_version,
        headers.mcp_method.as_deref(),
        headers.mcp_name.as_deref(),
        body_name,
    )
    .expect("Mojo Super expose protocol metadata policy returned invalid output");

    match decision {
        SuperExposeProtocolDecision::Ok => None,
        SuperExposeProtocolDecision::UnsupportedVersion => Some(mcp_json_error(
            400,
            request_id(message),
            MCP_ERROR_UNSUPPORTED_VERSION,
            "unsupported protocol version",
            Some(json!({
                "supported": MCP_PROTOCOL_VERSIONS,
                "requested": headers.protocol_version.as_deref().or(body_version),
            })),
        )),
        SuperExposeProtocolDecision::VersionMismatch => Some(mcp_error_response(
            400,
            request_id(message),
            MCP_ERROR_HEADER_MISMATCH,
            "protocol version header mismatch",
        )),
        SuperExposeProtocolDecision::MetadataRequired => Some(header_mismatch(
            message,
            "protocol version metadata is required",
        )),
        SuperExposeProtocolDecision::MethodMismatch => {
            Some(header_mismatch(message, "Mcp-Method header mismatch"))
        }
        SuperExposeProtocolDecision::NameMismatch => {
            Some(header_mismatch(message, "Mcp-Name header mismatch"))
        }
    }
}

fn request_body_protocol_version<'a>(
    method: &str,
    params: Option<&'a serde_json::Map<String, Value>>,
) -> Option<&'a str> {
    if method == "initialize" {
        return params
            .and_then(|params| params.get("protocolVersion"))
            .and_then(Value::as_str);
    }
    params
        .and_then(|params| params.get("_meta"))
        .and_then(Value::as_object)
        .and_then(|meta| meta.get("io.modelcontextprotocol/protocolVersion"))
        .and_then(Value::as_str)
}

fn header_mismatch(
    message: &serde_json::Map<String, Value>,
    detail: &str,
) -> Response<std::io::Cursor<Vec<u8>>> {
    mcp_error_response(400, request_id(message), MCP_ERROR_HEADER_MISMATCH, detail)
}

pub(crate) fn mcp_origin_allowed(host: &str, origin: Option<&str>) -> bool {
    let parsed = origin.and_then(|value| url::Url::parse(value).ok());
    let url_facts = parsed.as_ref().map(|url| SuperExposeParsedOriginUrl {
        scheme: url.scheme(),
        host: url.host_str(),
        username_empty: url.username().is_empty(),
        password_absent: url.password().is_none(),
        explicit_port_absent: url.port().is_none(),
        root_path: url.path().is_empty() || url.path() == "/",
        query_absent: url.query().is_none(),
        fragment_absent: url.fragment().is_none(),
    });
    prodex_mojo_core::rich::super_expose_origin_allowed(host, origin, url_facts)
        .expect("compiled Mojo MCP Origin trust policy returned invalid output")
}

pub(crate) fn mcp_content_type_allowed(value: Option<&str>) -> bool {
    prodex_mojo_core::rich::super_expose_content_type_allowed(value)
        .expect("Mojo Super expose Content-Type policy returned invalid output")
}

pub(crate) fn mcp_accept_allowed(value: Option<&str>) -> bool {
    prodex_mojo_core::rich::super_expose_accept_allowed(value)
        .expect("Mojo Super expose Accept policy returned invalid output")
}

pub(super) fn mcp_json_nesting_within_limit(body: &[u8], limit: usize) -> bool {
    prodex_mojo_core::rich::super_expose_json_nesting_within_limit(body, limit)
        .expect("Mojo Super expose JSON nesting policy returned invalid output")
}

pub(super) fn request_id(message: &serde_json::Map<String, Value>) -> Option<Value> {
    message
        .get("id")
        .and_then(|id| (id.is_string() || id.is_number()).then(|| id.clone()))
}

pub(crate) fn mcp_error_response(
    status: u16,
    id: Option<Value>,
    code: i64,
    message: &str,
) -> Response<std::io::Cursor<Vec<u8>>> {
    mcp_json_error(status, id, code, message, None)
}

fn mcp_json_error(
    status: u16,
    id: Option<Value>,
    code: i64,
    message: &str,
    data: Option<Value>,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let mut rpc_error = json!({"code": code, "message": message});
    if let Some(data) = data {
        rpc_error["data"] = data;
    }
    json_response(status, json!({"jsonrpc":"2.0","id":id,"error":rpc_error}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_origin_policy_rejects_credential_ports_paths_queries_and_unicode_padding() {
        for (host, origin, allowed) in [
            ("example.com", "https://EXAMPLE.COM/", true),
            ("example.com", "https://example.com", true),
            ("localhost", "https://CHATGPT.COM/", true),
            ("localhost", "https://chat.openai.com/", true),
            ("localhost", "https://chatgpt.com.attacker/", false),
            ("example.com", "http://example.com/", false),
            ("example.com", "https://example.com:9443/", false),
            ("example.com", "https://user@example.com/", false),
            ("example.com", "https://user:pass@example.com/", false),
            ("example.com", "https://example.com/path", false),
            ("example.com", "https://example.com/?q=1", false),
            ("example.com", "https://example.com/#token", false),
            ("example.com", "\u{2003}https://example.com/", false),
            ("example.com", "https://example.com/\u{3000}", false),
            ("127.0.0.1:9876", "http://127.0.0.1:9876", true),
            ("127.0.0.1:9876", "http://127.0.0.1:9876/", false),
            ("127.0.0.1:9876", "http://127.0.0.1:9877", false),
            ("127.0.0.1:9876", "http://localhost:9876", false),
            ("localhost:9876", "http://localhost:9876", false),
            ("localhost", "not-an-origin", false),
        ] {
            assert_eq!(
                mcp_origin_allowed(host, Some(origin)),
                allowed,
                "{host:?}/{origin:?}",
            );
        }
        assert!(mcp_origin_allowed("localhost", None));
    }

    #[test]
    fn origin_policy_uses_exact_mojo_casefold() {
        assert!(mcp_origin_allowed(
            "example.com",
            Some("https://EXAMPLE.COM/")
        ));
        assert!(mcp_origin_allowed(
            "127.0.0.1:9876",
            Some("http://127.0.0.1:9876")
        ));
        assert!(mcp_origin_allowed(
            "localhost",
            Some("https://CHATGPT.COM/")
        ));
        assert!(mcp_origin_allowed(
            "localhost",
            Some("https://chat.openai.com/")
        ));
        assert!(!mcp_origin_allowed(
            "example.com",
            Some(" https://example.com/")
        ));
        assert!(!mcp_origin_allowed(
            "example.com",
            Some("https://example.com/path")
        ));
    }
}
