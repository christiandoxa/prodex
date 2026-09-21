use super::*;

pub(super) fn validate_mcp_request_headers(
    message: &serde_json::Map<String, Value>,
    method: &str,
    headers: &McpRequestHeaders,
) -> Option<Response<std::io::Cursor<Vec<u8>>>> {
    let params = message.get("params").and_then(Value::as_object);
    let body_version = request_body_protocol_version(method, params);
    let header_version = headers.protocol_version.as_deref();
    if let Some(response) = validate_protocol_version(message, header_version, body_version) {
        return Some(response);
    }
    validate_current_protocol_metadata(
        message,
        method,
        headers,
        params,
        header_version,
        body_version,
    )
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

fn validate_protocol_version(
    message: &serde_json::Map<String, Value>,
    header_version: Option<&str>,
    body_version: Option<&str>,
) -> Option<Response<std::io::Cursor<Vec<u8>>>> {
    if let Some(version) = header_version.or(body_version)
        && !MCP_PROTOCOL_VERSIONS.contains(&version)
    {
        return Some(mcp_json_error(
            400,
            request_id(message),
            MCP_ERROR_UNSUPPORTED_VERSION,
            "unsupported protocol version",
            Some(json!({
                "supported": MCP_PROTOCOL_VERSIONS,
                "requested": header_version.or(body_version),
            })),
        ));
    }
    if header_version.is_some_and(|header| body_version.is_some_and(|body| body != header)) {
        return Some(mcp_error_response(
            400,
            request_id(message),
            MCP_ERROR_HEADER_MISMATCH,
            "protocol version header mismatch",
        ));
    }
    None
}

fn validate_current_protocol_metadata(
    message: &serde_json::Map<String, Value>,
    method: &str,
    headers: &McpRequestHeaders,
    params: Option<&serde_json::Map<String, Value>>,
    header_version: Option<&str>,
    body_version: Option<&str>,
) -> Option<Response<std::io::Cursor<Vec<u8>>>> {
    let current = header_version == Some(MCP_CURRENT_PROTOCOL_VERSION)
        || body_version == Some(MCP_CURRENT_PROTOCOL_VERSION);
    if !current {
        return validate_legacy_method_header(message, method, headers);
    }
    if header_version != Some(MCP_CURRENT_PROTOCOL_VERSION)
        || body_version != Some(MCP_CURRENT_PROTOCOL_VERSION)
    {
        return Some(header_mismatch(
            message,
            "protocol version metadata is required",
        ));
    }
    if headers.mcp_method.as_deref() != Some(method) {
        return Some(header_mismatch(message, "Mcp-Method header mismatch"));
    }
    if method != "tools/call" {
        return None;
    }
    let body_name = params
        .and_then(|params| params.get("name"))
        .and_then(Value::as_str);
    (headers.mcp_name.as_deref() != body_name)
        .then(|| header_mismatch(message, "Mcp-Name header mismatch"))
}

fn validate_legacy_method_header(
    message: &serde_json::Map<String, Value>,
    method: &str,
    headers: &McpRequestHeaders,
) -> Option<Response<std::io::Cursor<Vec<u8>>>> {
    headers
        .mcp_method
        .as_deref()
        .is_some_and(|header| header != method)
        .then(|| header_mismatch(message, "Mcp-Method header mismatch"))
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
    value
        .and_then(|value| value.split(';').next())
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("application/json"))
}

pub(crate) fn mcp_accept_allowed(value: Option<&str>) -> bool {
    let Some(value) = value else {
        return false;
    };
    value
        .split(',')
        .filter_map(|part| part.split(';').next())
        .any(|media| media.trim().eq_ignore_ascii_case("application/json"))
}

pub(super) fn mcp_json_nesting_within_limit(body: &[u8], limit: usize) -> bool {
    let mut depth = 0usize;
    let mut escaped = false;
    let mut in_string = false;
    for byte in body {
        if in_string {
            if escaped {
                escaped = false;
            } else if *byte == b'\\' {
                escaped = true;
            } else if *byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match *byte {
            b'"' => in_string = true,
            b'{' | b'[' => {
                depth += 1;
                if depth > limit {
                    return false;
                }
            }
            b'}' | b']' => {
                let Some(next_depth) = depth.checked_sub(1) else {
                    return false;
                };
                depth = next_depth;
            }
            _ => {}
        }
    }
    !in_string && !escaped && depth == 0
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
