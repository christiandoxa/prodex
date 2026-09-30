use super::*;

struct ParsedDispatchRequest {
    id: Option<Value>,
    params: Value,
    method_kind: ExposeMethod,
    tool_kind: ExposeTool,
}

enum MethodDispatch {
    Rpc(std::result::Result<Value, String>),
    Immediate(Response<std::io::Cursor<Vec<u8>>>),
}

pub(in crate::super_expose) fn dispatch(
    body: &[u8],
    headers: &McpRequestHeaders,
    context: &DispatchContext<'_>,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let request = match parse_dispatch_request(body, headers, context) {
        Ok(request) => request,
        Err(response) => return response,
    };
    let server_name = format!("Prodex Super — {}", context.display_name);
    let instructions = expose_instructions(
        context.workspace,
        context.instance_id,
        context.mode,
        context.optional_tools,
    );
    match dispatch_method(&request, context, &server_name, &instructions) {
        MethodDispatch::Immediate(response) => response,
        MethodDispatch::Rpc(result) => finish_dispatch_result(request, result, context),
    }
}

fn parse_dispatch_request(
    body: &[u8],
    headers: &McpRequestHeaders,
    context: &DispatchContext<'_>,
) -> std::result::Result<ParsedDispatchRequest, Response<std::io::Cursor<Vec<u8>>>> {
    if !mcp_json_nesting_within_limit(body, MCP_MAX_JSON_NESTING) {
        return Err(error(None, -32700, "parse error"));
    }
    let value = parse_dispatch_json(body, context)?;
    let object = require_dispatch_object(value, context)?;
    let id = request_id(&object);
    if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Err(error(id, -32600, "invalid request"));
    }
    let Some(method) = object.get("method").and_then(Value::as_str) else {
        return Err(error(id, -32600, "method is required"));
    };
    let params = object.get("params").cloned().unwrap_or(Value::Null);
    if let Some(response) = validate_mcp_request_headers(&object, method, headers) {
        return Err(response);
    }

    let params_object = params.as_object();
    let tool_name = params_object
        .and_then(|params| params.get("name"))
        .and_then(Value::as_str);
    let protocol_version_present = params_object
        .and_then(|params| params.get("protocolVersion"))
        .and_then(Value::as_str)
        .is_some();
    let tool_arguments_kind = match params_object.and_then(|params| params.get("arguments")) {
        None => 0,
        Some(arguments) if arguments.is_object() => 1,
        Some(_) => 2,
    };
    let validation = prodex_mojo_core::rich::super_expose_dispatch_validation(
        method,
        object.contains_key("id"),
        id.is_some(),
        params_object.is_some(),
        protocol_version_present,
        tool_name.is_some(),
        tool_arguments_kind,
    )
    .expect("Mojo Super expose dispatch validation returned invalid output");

    use prodex_mojo_core::rich::SuperExposeDispatchDecision;
    match validation {
        SuperExposeDispatchDecision::NotificationAccepted => {
            return Err(json_response(202, Value::Null));
        }
        SuperExposeDispatchDecision::NotificationUnsupported => {
            return Err(error(None, -32601, "notification is unsupported"));
        }
        SuperExposeDispatchDecision::InvalidRequestId => {
            return Err(error(None, -32600, "invalid request id"));
        }
        SuperExposeDispatchDecision::Ok
        | SuperExposeDispatchDecision::InitializeParamsRequired
        | SuperExposeDispatchDecision::ProtocolVersionRequired
        | SuperExposeDispatchDecision::ToolParamsRequired
        | SuperExposeDispatchDecision::ToolNameRequired
        | SuperExposeDispatchDecision::ToolArgumentsObjectRequired => {}
    }

    let (method_kind, tool_kind) = expose_route(method, tool_name);
    audit_dispatch_request(context, method_kind, tool_kind, body.len());

    match validation {
        SuperExposeDispatchDecision::InitializeParamsRequired => {
            return Err(mcp_error_response(
                400,
                id,
                -32602,
                "initialize params are required",
            ));
        }
        SuperExposeDispatchDecision::ProtocolVersionRequired => {
            return Err(mcp_error_response(
                400,
                id,
                -32602,
                "protocolVersion is required",
            ));
        }
        SuperExposeDispatchDecision::ToolParamsRequired => {
            return Err(mcp_error_response(
                400,
                id,
                -32602,
                "tool parameters are required",
            ));
        }
        SuperExposeDispatchDecision::ToolNameRequired => {
            return Err(mcp_error_response(400, id, -32602, "tool name is required"));
        }
        SuperExposeDispatchDecision::ToolArgumentsObjectRequired => {
            return Err(mcp_error_response(
                400,
                id,
                -32602,
                "tool arguments must be an object",
            ));
        }
        SuperExposeDispatchDecision::Ok => {}
        SuperExposeDispatchDecision::NotificationAccepted
        | SuperExposeDispatchDecision::NotificationUnsupported
        | SuperExposeDispatchDecision::InvalidRequestId => {
            unreachable!("handled before dispatch audit")
        }
    }

    if method_kind == ExposeMethod::ToolsCall {
        let empty_arguments = Value::Object(Default::default());
        let arguments = params_object
            .and_then(|params| params.get("arguments"))
            .unwrap_or(&empty_arguments);
        validate_tool_arguments(
            tool_name.expect("Mojo validated tools/call name"),
            arguments,
        )
        .map_err(|message| mcp_error_response(400, id.clone(), -32602, &message))?;
    }

    Ok(ParsedDispatchRequest {
        id,
        params,
        method_kind,
        tool_kind,
    })
}

fn parse_dispatch_json(
    body: &[u8],
    context: &DispatchContext<'_>,
) -> std::result::Result<Value, Response<std::io::Cursor<Vec<u8>>>> {
    serde_json::from_slice(body).map_err(|_| {
        context.audit.event(
            "super_expose_rpc_rejected",
            [
                crate::runtime_proxy_log_field("mode", context.mode.as_str()),
                crate::runtime_proxy_log_field("reason", "parse_error"),
            ],
        );
        error(None, -32700, "parse error")
    })
}

fn require_dispatch_object(
    value: Value,
    context: &DispatchContext<'_>,
) -> std::result::Result<serde_json::Map<String, Value>, Response<std::io::Cursor<Vec<u8>>>> {
    match value {
        Value::Object(object) => Ok(object),
        value => {
            context.audit.event(
                "super_expose_rpc_rejected",
                [
                    crate::runtime_proxy_log_field("mode", context.mode.as_str()),
                    crate::runtime_proxy_log_field("reason", "invalid_request"),
                ],
            );
            Err(error(
                None,
                -32600,
                if value.is_array() {
                    "batch requests are unsupported"
                } else {
                    "parse error"
                },
            ))
        }
    }
}

fn audit_dispatch_request(
    context: &DispatchContext<'_>,
    method_kind: ExposeMethod,
    tool_kind: ExposeTool,
    body_bytes: usize,
) {
    context.audit.event(
        "super_expose_rpc",
        [
            crate::runtime_proxy_log_field("mode", context.mode.as_str()),
            crate::runtime_proxy_log_field("method", method_kind.as_str()),
            crate::runtime_proxy_log_field("tool", tool_kind.as_str()),
            crate::runtime_proxy_log_field("body_bytes", body_bytes.to_string()),
        ],
    );
}

fn dispatch_method(
    request: &ParsedDispatchRequest,
    context: &DispatchContext<'_>,
    server_name: &str,
    instructions: &str,
) -> MethodDispatch {
    let result = match request.method_kind {
        ExposeMethod::ServerDiscover => Ok(server_discover_result(server_name, instructions)),
        ExposeMethod::Initialize => initialize_result(&request.params, server_name, instructions),
        ExposeMethod::Ping => Ok(json!({})),
        ExposeMethod::ToolsList => Ok(tools_list_result(
            context.mode,
            server_name,
            context.optional_tools,
        )),
        ExposeMethod::ToolsCall => tool_call(&request.params, request.tool_kind, context),
        ExposeMethod::Notification => {
            audit_dispatch_completion(context, request.method_kind, request.tool_kind, true);
            return MethodDispatch::Immediate(json_response(202, Value::Null));
        }
        ExposeMethod::Unknown => {
            context.audit.event(
                "super_expose_rpc_rejected",
                [
                    crate::runtime_proxy_log_field("mode", context.mode.as_str()),
                    crate::runtime_proxy_log_field("reason", "method_not_found"),
                ],
            );
            return MethodDispatch::Immediate(mcp_error_response(
                404,
                request.id.clone(),
                -32601,
                "method not found",
            ));
        }
    };
    MethodDispatch::Rpc(result)
}

fn server_discover_result(server_name: &str, instructions: &str) -> Value {
    json!({
        "resultType": "complete",
        "supportedVersions": [MCP_CURRENT_PROTOCOL_VERSION],
        "capabilities": {"tools": {"listChanged": false}},
        "instructions": instructions,
        "ttlMs": 300_000,
        "cacheScope": "private",
        "_meta": {"io.modelcontextprotocol/serverInfo": {
            "name": server_name,
            "version": env!("CARGO_PKG_VERSION")
        }}
    })
}

fn tools_list_result(
    mode: SuperExposeMode,
    server_name: &str,
    optional_tools: &ExposeOptionalTools,
) -> Value {
    json!({
        "resultType": "complete",
        "tools": tools(mode, optional_tools),
        "ttlMs": 300_000,
        "cacheScope": "private",
        "_meta": {"io.modelcontextprotocol/serverInfo": {
            "name": server_name,
            "version": env!("CARGO_PKG_VERSION")
        }}
    })
}

fn finish_dispatch_result(
    request: ParsedDispatchRequest,
    result: std::result::Result<Value, String>,
    context: &DispatchContext<'_>,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let success = result.is_ok();
    audit_dispatch_completion(context, request.method_kind, request.tool_kind, success);
    match result {
        Ok(result) => rpc_result(request.id, result),
        Err(message) => rpc_result(
            request.id,
            json!({
                "resultType": "complete",
                "content":[{"type":"text","text":serde_json::to_string(&json!({"error": message})).unwrap_or_else(|_| "{}".to_string())}],
                "structuredContent":{"error":message},
                "isError":true
            }),
        ),
    }
}

fn audit_dispatch_completion(
    context: &DispatchContext<'_>,
    method_kind: ExposeMethod,
    tool_kind: ExposeTool,
    success: bool,
) {
    context.audit.event(
        "super_expose_rpc_completed",
        [
            crate::runtime_proxy_log_field("mode", context.mode.as_str()),
            crate::runtime_proxy_log_field("method", method_kind.as_str()),
            crate::runtime_proxy_log_field("tool", tool_kind.as_str()),
            crate::runtime_proxy_log_field("success", success.to_string()),
        ],
    );
}
