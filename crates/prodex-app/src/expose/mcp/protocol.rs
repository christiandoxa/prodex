use super::super::http::{ExposeHttpRequest, expose_valid_token};
use super::super::ui::{ExposeHttpResponse, expose_mcp_json_response};
use super::tools::main_provider;
use super::{
    MCP_CURRENT_PROTOCOL_VERSION, MCP_ERROR_HEADER_MISMATCH, MCP_ERROR_UNSUPPORTED_VERSION,
    MCP_PATH_PREFIX, MCP_PATH_SUFFIX, MCP_PROTOCOL_VERSIONS,
};
use prodex_cli::SuperArgs;
use serde_json::{Map, Value, json};

pub(super) fn validate_mcp_request_headers(
    message: &Map<String, Value>,
    method: &str,
    request: &ExposeHttpRequest,
) -> Option<ExposeHttpResponse> {
    let params = message.get("params").and_then(Value::as_object);
    let body_version = if method == "initialize" {
        params
            .and_then(|params| params.get("protocolVersion"))
            .and_then(Value::as_str)
    } else {
        params
            .and_then(|params| params.get("_meta"))
            .and_then(Value::as_object)
            .and_then(|meta| meta.get("io.modelcontextprotocol/protocolVersion"))
            .and_then(Value::as_str)
    };
    let header_version = request.header("MCP-Protocol-Version");
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
    let current = header_version == Some(MCP_CURRENT_PROTOCOL_VERSION)
        || body_version == Some(MCP_CURRENT_PROTOCOL_VERSION);
    if current {
        if header_version != Some(MCP_CURRENT_PROTOCOL_VERSION)
            || body_version != Some(MCP_CURRENT_PROTOCOL_VERSION)
        {
            return Some(mcp_error_response(
                400,
                request_id(message),
                MCP_ERROR_HEADER_MISMATCH,
                "protocol version metadata is required",
            ));
        }
        if request.header("Mcp-Method") != Some(method) {
            return Some(mcp_error_response(
                400,
                request_id(message),
                MCP_ERROR_HEADER_MISMATCH,
                "Mcp-Method header mismatch",
            ));
        }
        if method == "tools/call" {
            let body_name = params
                .and_then(|params| params.get("name"))
                .and_then(Value::as_str);
            if request.header("Mcp-Name") != body_name {
                return Some(mcp_error_response(
                    400,
                    request_id(message),
                    MCP_ERROR_HEADER_MISMATCH,
                    "Mcp-Name header mismatch",
                ));
            }
        }
    } else if let Some(header) = request.header("Mcp-Method")
        && header != method
    {
        return Some(mcp_error_response(
            400,
            request_id(message),
            MCP_ERROR_HEADER_MISMATCH,
            "Mcp-Method header mismatch",
        ));
    }
    None
}

pub(super) fn mcp_origin_allowed(host: &str, origin: Option<&str>) -> bool {
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

pub(super) fn validate_configured_main_effort(args: &SuperArgs) -> std::result::Result<(), String> {
    let Some(effort) =
        crate::codex_cli_config_override_value(&args.codex_args, "model_reasoning_effort")
    else {
        return Ok(());
    };
    let parsed = effort
        .parse::<prodex_cli::SubAgentReasoningEffort>()
        .map_err(|_| "reasoning_effort is unsupported".to_string())?;
    let provider = main_provider(args);
    let configured_model = crate::codex_cli_config_override_value(&args.codex_args, "model");
    let model = args.local_model.as_deref().or(configured_model.as_deref());
    if crate::canonical_sub_agent_efforts(provider, model).contains(&parsed) {
        Ok(())
    } else {
        Err("reasoning_effort is unsupported for the selected model".to_string())
    }
}

pub(super) fn mcp_json_nesting_within_limit(body: &[u8], limit: usize) -> bool {
    let mut depth = 0;
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

pub(super) fn mcp_capability_segment(target: &str) -> Option<&str> {
    let segment = target
        .strip_prefix(MCP_PATH_PREFIX)?
        .strip_suffix(MCP_PATH_SUFFIX)?;
    (expose_valid_token(segment) && !segment.contains('/')).then_some(segment)
}

pub(super) fn mcp_content_type_allowed(value: Option<&str>) -> bool {
    value
        .and_then(|value| value.split(';').next())
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("application/json"))
}

pub(super) fn mcp_accept_allowed(value: Option<&str>) -> bool {
    let Some(value) = value else {
        return false;
    };
    let mut json = false;
    let mut event_stream = false;
    for media in value.split(',').filter_map(|part| part.split(';').next()) {
        match media.trim().to_ascii_lowercase().as_str() {
            "application/json" => json = true,
            "text/event-stream" => event_stream = true,
            _ => {}
        }
    }
    json && event_stream
}

pub(super) fn request_id(message: &Map<String, Value>) -> Option<Value> {
    message
        .get("id")
        .and_then(|id| (id.is_string() || id.is_number()).then(|| id.clone()))
}

pub(super) fn jsonrpc_result(id: Option<Value>, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

pub(super) fn mcp_error_response(
    status: u16,
    id: Option<Value>,
    code: i64,
    message: &str,
) -> ExposeHttpResponse {
    mcp_json_error(status, id, code, message, None)
}

pub(super) fn mcp_json_error(
    status: u16,
    id: Option<Value>,
    code: i64,
    message: &str,
    data: Option<Value>,
) -> ExposeHttpResponse {
    let mut error = json!({"code": code, "message": message});
    if let Some(data) = data {
        error["data"] = data;
    }
    mcp_json_response(status, json!({"jsonrpc": "2.0", "id": id, "error": error}))
}

pub(super) fn mcp_json_response(status: u16, body: Value) -> ExposeHttpResponse {
    let body = serde_json::to_string(&body).unwrap_or_else(|_| "{}".to_string());
    expose_mcp_json_response(status, &body)
}
