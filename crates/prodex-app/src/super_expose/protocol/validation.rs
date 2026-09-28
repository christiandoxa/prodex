use super::*;

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
    let Some(origin) = origin else {
        return true;
    };
    if origin != origin.trim() {
        return false;
    }
    let Ok(parsed) = url::Url::parse(origin) else {
        return false;
    };
    let local_http = host.starts_with("127.0.0.1:") && origin == format!("http://{host}");
    let trusted_https = parsed.scheme() == "https"
        && parsed.host_str().is_some_and(|origin_host| {
            origin_host.eq_ignore_ascii_case(host)
                || matches!(
                    origin_host.to_ascii_lowercase().as_str(),
                    "chatgpt.com" | "chat.openai.com"
                )
        })
        && parsed.port().is_none();
    (local_http || trusted_https)
        && parsed.username().is_empty()
        && parsed.password().is_none()
        && (parsed.path().is_empty() || parsed.path() == "/")
        && parsed.query().is_none()
        && parsed.fragment().is_none()
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
