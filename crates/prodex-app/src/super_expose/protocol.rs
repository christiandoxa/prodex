use super::exec::execute_direct;
use super::logging::ExposeAuditLog;
use super::run::RunManager;
use super::session_prompt_write::{
    ExistingSessionPromptWrite, PromptOutputReadRequest, SESSION_PROMPT_WRITE_MAX_MESSAGE_BYTES,
    SessionPreemptRequest, SessionPromptWriteRequest,
};
use prodex_cli::SuperExposeMode;
use serde_json::{Value, json};
use std::path::Path;
use tiny_http::{Header, Response, StatusCode};
use uuid::Uuid;

const MCP_CURRENT_PROTOCOL_VERSION: &str = "2026-07-28";
const MCP_PROTOCOL_VERSIONS: [&str; 5] = [
    MCP_CURRENT_PROTOCOL_VERSION,
    "2025-11-25",
    "2025-06-18",
    "2025-03-26",
    "2024-11-05",
];
const MCP_MAX_EVENT_PAGE: usize = 64;
const MCP_MAX_OUTPUT_EVENTS: usize = 200;
const MCP_MAX_OUTPUT_WAIT_MS: u64 = 10_000;
const MCP_MAX_CURSOR_BYTES: usize = 16 * 1024;

#[path = "protocol/tool_contract.rs"]
mod tool_contract;
#[path = "protocol/validation.rs"]
mod validation;

use tool_contract::*;
pub(super) use validation::{
    mcp_accept_allowed, mcp_content_type_allowed, mcp_error_response, mcp_origin_allowed,
};
use validation::{mcp_json_nesting_within_limit, validate_mcp_request_headers};
const MCP_MAX_JSON_NESTING: usize = 64;
const MCP_ERROR_UNSUPPORTED_VERSION: i64 = -32022;
const MCP_ERROR_HEADER_MISMATCH: i64 = -32020;

pub(super) struct McpRequestHeaders {
    pub(super) protocol_version: Option<String>,
    pub(super) mcp_method: Option<String>,
    pub(super) mcp_name: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExposeMethod {
    Unknown,
    ServerDiscover,
    Initialize,
    Ping,
    ToolsList,
    ToolsCall,
    Notification,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExposeTool {
    Unknown,
    Start,
    Status,
    Events,
    Result,
    Cancel,
    List,
    Exec,
    SessionPromptWrite,
    SessionPreempt,
    SessionOutputRead,
}

impl ExposeMethod {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::ServerDiscover => "server_discover",
            Self::Initialize => "initialize",
            Self::Ping => "ping",
            Self::ToolsList => "tools_list",
            Self::ToolsCall => "tools_call",
            Self::Notification => "notification",
        }
    }
}

impl ExposeTool {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Start => "start",
            Self::Status => "status",
            Self::Events => "events",
            Self::Result => "result",
            Self::Cancel => "cancel",
            Self::List => "list",
            Self::Exec => "exec",
            Self::SessionPromptWrite => "session_prompt_write",
            Self::SessionPreempt => "session_preempt",
            Self::SessionOutputRead => "session_output_read",
        }
    }
}

#[cfg(feature = "mojo-core")]
fn expose_route(method: &str, tool: Option<&str>) -> (ExposeMethod, ExposeTool) {
    use prodex_mojo_core::rich::{SuperExposeMethod, SuperExposeTool};
    let route = prodex_mojo_core::rich::super_expose_route(method, tool)
        .expect("Mojo Super expose route planner returned invalid output");
    (
        match route.method {
            SuperExposeMethod::Unknown => ExposeMethod::Unknown,
            SuperExposeMethod::ServerDiscover => ExposeMethod::ServerDiscover,
            SuperExposeMethod::Initialize => ExposeMethod::Initialize,
            SuperExposeMethod::Ping => ExposeMethod::Ping,
            SuperExposeMethod::ToolsList => ExposeMethod::ToolsList,
            SuperExposeMethod::ToolsCall => ExposeMethod::ToolsCall,
            SuperExposeMethod::Notification => ExposeMethod::Notification,
        },
        match route.tool {
            SuperExposeTool::Unknown => ExposeTool::Unknown,
            SuperExposeTool::Start => ExposeTool::Start,
            SuperExposeTool::Status => ExposeTool::Status,
            SuperExposeTool::Events => ExposeTool::Events,
            SuperExposeTool::Result => ExposeTool::Result,
            SuperExposeTool::Cancel => ExposeTool::Cancel,
            SuperExposeTool::List => ExposeTool::List,
            SuperExposeTool::Exec => ExposeTool::Exec,
            SuperExposeTool::SessionPromptWrite => ExposeTool::SessionPromptWrite,
            SuperExposeTool::SessionPreempt => ExposeTool::SessionPreempt,
            SuperExposeTool::SessionOutputRead => ExposeTool::SessionOutputRead,
        },
    )
}

#[cfg(not(feature = "mojo-core"))]
fn expose_route(method: &str, tool: Option<&str>) -> (ExposeMethod, ExposeTool) {
    let method = match method {
        "server/discover" => ExposeMethod::ServerDiscover,
        "initialize" => ExposeMethod::Initialize,
        "ping" => ExposeMethod::Ping,
        "tools/list" => ExposeMethod::ToolsList,
        "tools/call" => ExposeMethod::ToolsCall,
        "notifications/initialized" | "notifications/cancelled" => ExposeMethod::Notification,
        _ => ExposeMethod::Unknown,
    };
    let tool = match tool.unwrap_or_default() {
        "prodex_super_start" => ExposeTool::Start,
        "prodex_super_status" => ExposeTool::Status,
        "prodex_super_events" => ExposeTool::Events,
        "prodex_super_result" => ExposeTool::Result,
        "prodex_super_cancel" => ExposeTool::Cancel,
        "prodex_super_list" => ExposeTool::List,
        "prodex_super_exec" => ExposeTool::Exec,
        "prodex_session_prompt_write" => ExposeTool::SessionPromptWrite,
        "prodex_session_preempt" => ExposeTool::SessionPreempt,
        "prodex_session_output_read" => ExposeTool::SessionOutputRead,
        _ => ExposeTool::Unknown,
    };
    (method, tool)
}

#[cfg(feature = "mojo-core")]
fn tool_allowed(mode: SuperExposeMode, tool_name: &str) -> bool {
    prodex_mojo_core::rich::super_expose_tool_allowed(mode.exec_only(), tool_name)
        .expect("Mojo Super expose tool policy returned invalid output")
}

#[cfg(not(feature = "mojo-core"))]
fn tool_allowed(mode: SuperExposeMode, tool_name: &str) -> bool {
    if mode.exec_only() {
        return tool_name == "prodex_super_exec";
    }
    matches!(
        tool_name,
        "prodex_super_start"
            | "prodex_super_status"
            | "prodex_super_events"
            | "prodex_super_result"
            | "prodex_super_cancel"
            | "prodex_super_list"
            | "prodex_super_exec"
            | "prodex_session_prompt_write"
            | "prodex_session_preempt"
            | "prodex_session_output_read"
    )
}

pub(super) struct DispatchContext<'a> {
    pub(super) manager: &'a RunManager,
    pub(super) session_prompt_write: &'a dyn ExistingSessionPromptWrite,
    pub(super) instance_id: &'a str,
    pub(super) display_name: &'a str,
    pub(super) workspace: &'a Path,
    pub(super) mode: SuperExposeMode,
    pub(super) audit: &'a ExposeAuditLog,
}

pub(super) fn dispatch(
    body: &[u8],
    headers: &McpRequestHeaders,
    context: &DispatchContext<'_>,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let instance_id = context.instance_id;
    let display_name = context.display_name;
    let workspace = context.workspace;
    let mode = context.mode;
    let audit = context.audit;
    if !mcp_json_nesting_within_limit(body, MCP_MAX_JSON_NESTING) {
        return error(None, -32700, "parse error");
    }
    let value: Value = match serde_json::from_slice(body) {
        Ok(value) => value,
        Err(_) => {
            audit.event(
                "super_expose_rpc_rejected",
                [
                    crate::runtime_proxy_log_field("mode", mode.as_str()),
                    crate::runtime_proxy_log_field("reason", "parse_error"),
                ],
            );
            return error(None, -32700, "parse error");
        }
    };
    let Some(object) = value.as_object() else {
        audit.event(
            "super_expose_rpc_rejected",
            [
                crate::runtime_proxy_log_field("mode", mode.as_str()),
                crate::runtime_proxy_log_field("reason", "invalid_request"),
            ],
        );
        return error(
            None,
            -32600,
            if value.is_array() {
                "batch requests are unsupported"
            } else {
                "parse error"
            },
        );
    };
    let id = object
        .get("id")
        .filter(|id| id.is_number() || id.is_string())
        .cloned();
    if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return error(id, -32600, "invalid request");
    }
    let Some(method) = object.get("method").and_then(Value::as_str) else {
        return error(id, -32600, "method is required");
    };
    let params = object.get("params").cloned().unwrap_or(Value::Null);
    let tool_name = params
        .as_object()
        .and_then(|params| params.get("name"))
        .and_then(Value::as_str);
    if let Some(response) = validate_mcp_request_headers(object, method, headers) {
        return response;
    }
    if !object.contains_key("id") {
        return if matches!(
            method,
            "notifications/initialized" | "notifications/cancelled"
        ) {
            json_response(202, Value::Null)
        } else {
            error(None, -32601, "notification is unsupported")
        };
    }
    if id.is_none() {
        return error(None, -32600, "invalid request id");
    }

    let (method_kind, tool_kind) = expose_route(method, tool_name);
    audit.event(
        "super_expose_rpc",
        [
            crate::runtime_proxy_log_field("mode", mode.as_str()),
            crate::runtime_proxy_log_field("method", method_kind.as_str()),
            crate::runtime_proxy_log_field("tool", tool_kind.as_str()),
            crate::runtime_proxy_log_field("body_bytes", body.len().to_string()),
        ],
    );
    if method_kind == ExposeMethod::Initialize {
        let Some(params_object) = params.as_object() else {
            return mcp_error_response(400, id, -32602, "initialize params are required");
        };
        if params_object
            .get("protocolVersion")
            .and_then(Value::as_str)
            .is_none()
        {
            return mcp_error_response(400, id, -32602, "protocolVersion is required");
        }
    }
    if method_kind == ExposeMethod::ToolsCall {
        let Some(params_object) = params.as_object() else {
            return mcp_error_response(400, id, -32602, "tool parameters are required");
        };
        let Some(name) = params_object.get("name").and_then(Value::as_str) else {
            return mcp_error_response(400, id, -32602, "tool name is required");
        };
        let empty_arguments = Value::Object(Default::default());
        let arguments = params_object.get("arguments").unwrap_or(&empty_arguments);
        if !arguments.is_object() {
            return mcp_error_response(400, id, -32602, "tool arguments must be an object");
        }
        if let Err(message) = validate_tool_arguments(name, arguments) {
            return mcp_error_response(400, id, -32602, &message);
        }
    }

    let server_name = format!("Prodex Super — {display_name}");
    let instructions = expose_instructions(workspace, instance_id, mode);
    let result = match method_kind {
        ExposeMethod::ServerDiscover => Ok(json!({
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
        })),
        ExposeMethod::Initialize => initialize_result(&params, &server_name, &instructions),
        ExposeMethod::Ping => Ok(json!({})),
        ExposeMethod::ToolsList => Ok(json!({
            "resultType": "complete",
            "tools": tools(mode),
            "ttlMs": 300_000,
            "cacheScope": "private",
            "_meta": {"io.modelcontextprotocol/serverInfo": {
                "name": server_name,
                "version": env!("CARGO_PKG_VERSION")
            }}
        })),
        ExposeMethod::ToolsCall => tool_call(&params, tool_kind, context),
        ExposeMethod::Notification => {
            audit.event(
                "super_expose_rpc_completed",
                [
                    crate::runtime_proxy_log_field("mode", mode.as_str()),
                    crate::runtime_proxy_log_field("method", method_kind.as_str()),
                    crate::runtime_proxy_log_field("tool", tool_kind.as_str()),
                    crate::runtime_proxy_log_field("success", "true"),
                ],
            );
            return json_response(202, Value::Null);
        }
        ExposeMethod::Unknown => {
            audit.event(
                "super_expose_rpc_rejected",
                [
                    crate::runtime_proxy_log_field("mode", mode.as_str()),
                    crate::runtime_proxy_log_field("reason", "method_not_found"),
                ],
            );
            return mcp_error_response(404, id, -32601, "method not found");
        }
    };
    match result {
        Ok(result) => {
            audit.event(
                "super_expose_rpc_completed",
                [
                    crate::runtime_proxy_log_field("mode", mode.as_str()),
                    crate::runtime_proxy_log_field("method", method_kind.as_str()),
                    crate::runtime_proxy_log_field("tool", tool_kind.as_str()),
                    crate::runtime_proxy_log_field("success", "true"),
                ],
            );
            rpc_result(id, result)
        }
        Err(message) => {
            audit.event(
                "super_expose_rpc_completed",
                [
                    crate::runtime_proxy_log_field("mode", mode.as_str()),
                    crate::runtime_proxy_log_field("method", method_kind.as_str()),
                    crate::runtime_proxy_log_field("tool", tool_kind.as_str()),
                    crate::runtime_proxy_log_field("success", "false"),
                ],
            );
            rpc_result(
                id,
                json!({
                    "resultType": "complete",
                    "content":[{"type":"text","text":serde_json::to_string(&json!({"error": message})).unwrap_or_else(|_| "{}".to_string())}],
                    "structuredContent":{"error":message},
                    "isError":true
                }),
            )
        }
    }
}

fn tool_call(
    params: &Value,
    tool_kind: ExposeTool,
    context: &DispatchContext<'_>,
) -> std::result::Result<Value, String> {
    let manager = context.manager;
    let session_prompt_write = context.session_prompt_write;
    let instance_id = context.instance_id;
    let workspace = context.workspace;
    let mode = context.mode;
    let audit = context.audit;
    let object = params
        .as_object()
        .ok_or_else(|| "tool parameters are required".to_string())?;
    let Some(tool_name) = object.get("name").and_then(Value::as_str) else {
        return Err("tool name is required".to_string());
    };
    if !tool_allowed(mode, tool_name) {
        return Err("tool is not exposed by this endpoint".to_string());
    }
    let empty = json!({});
    let arguments = object.get("arguments").unwrap_or(&empty);
    if !arguments.is_object() {
        return Err("tool arguments must be an object".to_string());
    }
    validate_tool_arguments(tool_name, arguments)?;
    let result = match tool_kind {
        ExposeTool::Start => {
            let task = required_string(arguments, "task", 65_536)?;
            let started = manager.start(task, arguments)?;
            json!({
                "run_id": started.get("run_id").cloned().unwrap_or(Value::Null),
                "state": started.get("state").cloned().unwrap_or(Value::Null),
            })
        }
        ExposeTool::Status => {
            let run_id = required_run_id(arguments)?;
            let mut value = manager
                .status(&run_id)
                .unwrap_or_else(|| json!({"run_id": run_id, "state": "unknown"}));
            value["instance_id"] = json!(instance_id);
            value
        }
        ExposeTool::Events => {
            let run_id = required_run_id(arguments)?;
            let after_seq = arguments
                .get("after_seq")
                .map(value_u64)
                .transpose()?
                .unwrap_or(0);
            let limit = arguments
                .get("limit")
                .map(value_usize)
                .transpose()?
                .unwrap_or(MCP_MAX_EVENT_PAGE);
            if !(1..=MCP_MAX_EVENT_PAGE).contains(&limit) {
                return Err(format!("limit must be between 1 and {MCP_MAX_EVENT_PAGE}"));
            }
            let mut value = manager.events(&run_id, after_seq, limit).map_or_else(
                || {
                    json!({
                        "run_id": run_id,
                        "state": "unknown",
                        "events": [],
                        "next_seq": 0,
                        "truncated": false,
                    })
                },
                |events| {
                    json!({
                        "run_id": run_id,
                        "events": events.events.iter().map(|event| json!({
                            "seq": event.seq,
                            "type": event.event_type,
                            "text": event.text,
                        })).collect::<Vec<_>>(),
                        "next_seq": events.next_seq,
                        "truncated": events.truncated,
                    })
                },
            );
            value["instance_id"] = json!(instance_id);
            value
        }
        ExposeTool::Result => {
            let run_id = required_run_id(arguments)?;
            let mut value = manager
                .result(&run_id)
                .unwrap_or_else(|| json!({"run_id": run_id, "state": "unknown"}));
            value["instance_id"] = json!(instance_id);
            value
        }
        ExposeTool::Cancel => {
            let run_id = required_run_id(arguments)?;
            let mut value = manager
                .cancel(&run_id)
                .unwrap_or_else(|| json!({"run_id": run_id, "state": "unknown"}));
            value["instance_id"] = json!(instance_id);
            value
        }
        ExposeTool::List => json!({"instance_id": instance_id, "runs":manager.list()}),
        ExposeTool::Exec => execute_direct(arguments, workspace, audit)?,
        ExposeTool::SessionPromptWrite => {
            let message =
                required_string(arguments, "message", SESSION_PROMPT_WRITE_MAX_MESSAGE_BYTES)?;
            if message.as_bytes().contains(&0) {
                return Err("message must not contain NUL".to_string());
            }
            let prodex_pid = optional_process_id(arguments)?;
            let thread_id = normalized_thread_id(arguments)?;
            let result = session_prompt_write
                .write(SessionPromptWriteRequest {
                    workspace_root: workspace.to_path_buf(),
                    message,
                    cwd: optional_string(arguments, "cwd", 4096)?,
                    prodex_pid,
                    thread_id: thread_id.clone(),
                    binding_key: session_binding_key(instance_id, prodex_pid, thread_id.as_deref()),
                })
                .map_err(|error| error.as_str().to_string())?;
            json!({
                "status": "written",
                "prodex_pid": result.prodex_pid,
                "codex_pid": result.codex_pid,
                "thread_id": result.thread_id,
                "message_id": result.message_id,
                "submission_id": result.submission_id,
                "output_cursor": result.output_cursor,
                "queue_exit": result.queue_exit,
                "verification": result.verification,
                "recovery_generation": result.recovery_generation,
                "last_prompt_requeued": result.last_prompt_requeued,
                "requeue_reason": result.requeue_reason,
            })
        }
        ExposeTool::SessionPreempt => {
            let prodex_pid = optional_process_id(arguments)?;
            let thread_id = normalized_thread_id(arguments)?;
            let result = session_prompt_write
                .preempt(SessionPreemptRequest {
                    workspace_root: workspace.to_path_buf(),
                    cwd: optional_string(arguments, "cwd", 4096)?,
                    prodex_pid,
                    thread_id: thread_id.clone(),
                    binding_key: session_binding_key(instance_id, prodex_pid, thread_id.as_deref()),
                })
                .map_err(|error| error.as_str().to_string())?;
            let current_turn_found = result.current_turn_id.is_some();
            let cancelled_count = result.cancelled_submission_ids.len();
            let remaining_count = result.remaining_submission_ids.len();
            json!({
                "status": "preempted",
                "preempted": true,
                "prodex_pid": result.prodex_pid,
                "codex_pid": result.codex_pid,
                "thread_id": result.thread_id,
                "current_turn_id": result.current_turn_id,
                "current_turn_found": current_turn_found,
                "current_turn_interrupted": result.current_turn_interrupted,
                "cancelled_submission_ids": result.cancelled_submission_ids,
                "cancelled_count": cancelled_count,
                "remaining_submission_ids": result.remaining_submission_ids,
                "remaining_count": remaining_count,
                "queue_empty_at_boundary": result.queue_empty_at_boundary,
                "session_ready": result.session_ready,
                "generation": result.generation,
                "generation_boundary": result.generation,
            })
        }
        ExposeTool::SessionOutputRead => {
            let limit = arguments
                .get("limit")
                .map(value_usize)
                .transpose()?
                .unwrap_or(MCP_MAX_OUTPUT_EVENTS);
            if !(1..=MCP_MAX_OUTPUT_EVENTS).contains(&limit) {
                return Err(format!(
                    "limit must be between 1 and {MCP_MAX_OUTPUT_EVENTS}"
                ));
            }
            let wait_ms = arguments
                .get("wait_ms")
                .map(value_u64)
                .transpose()?
                .unwrap_or_default();
            if wait_ms > MCP_MAX_OUTPUT_WAIT_MS {
                return Err(format!(
                    "wait_ms must be between 0 and {MCP_MAX_OUTPUT_WAIT_MS}"
                ));
            }
            let prodex_pid = optional_process_id(arguments)?;
            let thread_id = normalized_thread_id(arguments)?;
            let result = session_prompt_write
                .read_output(PromptOutputReadRequest {
                    workspace_root: workspace.to_path_buf(),
                    cursor: optional_string(arguments, "cursor", MCP_MAX_CURSOR_BYTES)?,
                    limit,
                    wait_ms,
                    prodex_pid,
                    thread_id: thread_id.clone(),
                    binding_key: session_binding_key(instance_id, prodex_pid, thread_id.as_deref()),
                    shutdown: None,
                })
                .map_err(|error| error.as_str().to_string())?;
            json!({
                "status": "ok",
                "prodex_pid": result.prodex_pid,
                "codex_pid": result.codex_pid,
                "thread_id": result.thread_id,
                "source": result.source,
                "events": result.events.into_iter().map(|event| json!({
                    "sequence": event.sequence,
                    "timestamp": event.timestamp,
                    "kind": event.kind,
                    "name": event.name,
                    "status": event.status,
                    "text": event.text,
                })).collect::<Vec<_>>(),
                "next_cursor": result.next_cursor,
                "has_more": result.has_more,
            })
        }
        ExposeTool::Unknown => return Err("tool not found".to_string()),
    };
    Ok(json!({
        "resultType": "complete",
        "content":[{"type":"text","text":serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string())}],
        "structuredContent":result,
        "isError":false
    }))
}

fn initialize_result(
    params: &Value,
    server_name: &str,
    instructions: &str,
) -> std::result::Result<Value, String> {
    let params = params
        .as_object()
        .ok_or_else(|| "initialize params are required".to_string())?;
    let version = params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .ok_or_else(|| "protocolVersion is required".to_string())?;
    if !MCP_PROTOCOL_VERSIONS.contains(&version) {
        return Err("unsupported protocol version".to_string());
    }
    Ok(json!({
        "protocolVersion": version,
        "capabilities": {"tools": {"listChanged": false}},
        "serverInfo": {"name": server_name, "version": env!("CARGO_PKG_VERSION")},
        "instructions": instructions
    }))
}

fn expose_instructions(workspace: &Path, instance_id: &str, mode: SuperExposeMode) -> String {
    if mode.exec_only() {
        return "Exec-only Prodex Super endpoint. Only prodex_super_exec is exposed.".to_string();
    }
    let workspace_name = workspace
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("workspace");
    format!(
        "This is a local full-access Prodex Super runtime starting in {:?} (instance {}). The initial directory is context, not a filesystem jail: runs retain normal OS-user filesystem, process, network, Git, and local-tool authority. For development requests, resolve one compatible existing plain prodex s with prodex_session_prompt_write first, then read its exact returned PID, thread_id, and cursor. Use prodex_session_preempt for one exact session when the current turn and all pending queued prompts must stop; it never kills a process. Start one prodex_super_start fallback only after authoritative no_session; never run both paths in parallel or treat ambiguity, stale identity, addressability, queue, source, or verification errors as no_session. A fresh idle prodex s needs no manual bootstrap prompt. Include consequential external actions in the user's task, and poll an existing run instead of starting duplicates. The expose URL is ephemeral capability authentication; anyone with it can control this instance.",
        workspace_name, instance_id
    )
}

fn rpc_result(id: Option<Value>, result: Value) -> Response<std::io::Cursor<Vec<u8>>> {
    json_response(200, json!({"jsonrpc":"2.0","id":id,"result":result}))
}

fn error(id: Option<Value>, code: i64, message: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    json_response(
        400,
        json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}}),
    )
}

pub(super) fn json_response(status: u16, body: Value) -> Response<std::io::Cursor<Vec<u8>>> {
    let bytes = serde_json::to_vec(&body).unwrap_or_else(|_| b"{}".to_vec());
    let mut response = Response::from_data(bytes).with_status_code(StatusCode(status));
    if let Ok(header) = Header::from_bytes("Content-Type", "application/json") {
        response = response.with_header(header);
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_mode_exposes_the_exact_preserved_tool_contract() {
        let tools = tools(SuperExposeMode::Full);
        let names = tools
            .iter()
            .filter_map(|tool| tool["name"].as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec![
                "prodex_super_start",
                "prodex_super_status",
                "prodex_super_events",
                "prodex_super_result",
                "prodex_super_cancel",
                "prodex_super_list",
                "prodex_super_exec",
                "prodex_session_prompt_write",
                "prodex_session_preempt",
                "prodex_session_output_read",
            ]
        );
        for tool in &tools {
            assert!(tool.get("title").is_some());
            assert!(tool.get("description").is_some());
            assert!(tool.get("inputSchema").is_some());
            assert!(tool.get("outputSchema").is_some());
            assert!(tool.get("annotations").is_some());
        }
        let events = tools
            .iter()
            .find(|tool| tool["name"] == "prodex_super_events")
            .expect("events tool");
        assert_eq!(events["inputSchema"]["properties"]["limit"]["maximum"], 64);
        assert_eq!(run_id_schema()["properties"]["run_id"]["minLength"], 1);
        assert_eq!(run_id_schema()["additionalProperties"], false);
    }

    #[test]
    fn exec_mode_exposes_only_direct_exec() {
        let names = tools(SuperExposeMode::Exec)
            .into_iter()
            .filter_map(|tool| tool["name"].as_str().map(str::to_string))
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["prodex_super_exec".to_string()]);
        assert!(tool_allowed(SuperExposeMode::Exec, "prodex_super_exec"));
        assert!(!tool_allowed(SuperExposeMode::Exec, "prodex_super_start"));
        assert!(!tool_allowed(
            SuperExposeMode::Exec,
            "prodex_session_prompt_write"
        ));
    }

    #[test]
    fn preserved_protocol_versions_match_04294() {
        assert_eq!(
            MCP_PROTOCOL_VERSIONS,
            [
                "2026-07-28",
                "2025-11-25",
                "2025-06-18",
                "2025-03-26",
                "2024-11-05",
            ]
        );
        assert_eq!(MCP_CURRENT_PROTOCOL_VERSION, "2026-07-28");
    }

    #[test]
    fn tool_arguments_fail_closed_on_unknown_keys() {
        let arguments = json!({"run_id":"spr_test","extra":true});
        assert_eq!(
            validate_tool_arguments("prodex_super_status", &arguments),
            Err("unknown tool argument: extra".to_string())
        );
    }

    #[test]
    fn optional_strings_reject_control_characters() {
        let arguments = json!({"model":"bad\nmodel"});
        assert!(optional_string(&arguments, "model", 256).is_err());
    }
}
