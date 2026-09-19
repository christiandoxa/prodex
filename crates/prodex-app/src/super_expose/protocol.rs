use super::exec::execute_direct;
use super::run::RunManager;
use serde_json::{Value, json};
use std::path::Path;
use tiny_http::{Header, Response, StatusCode};

const MCP_PROTOCOL_VERSION: &str = "2025-06-18";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExposeMethod {
    Unknown,
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
    Result,
    Cancel,
    List,
    Exec,
}

#[cfg(feature = "mojo-core")]
fn expose_route(method: &str, tool: Option<&str>) -> (ExposeMethod, ExposeTool) {
    use prodex_mojo_core::rich::{SuperExposeMethod, SuperExposeTool};
    let route = prodex_mojo_core::rich::super_expose_route(method, tool)
        .expect("Mojo Super expose route planner returned invalid output");
    (
        match route.method {
            SuperExposeMethod::Unknown => ExposeMethod::Unknown,
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
            SuperExposeTool::Result => ExposeTool::Result,
            SuperExposeTool::Cancel => ExposeTool::Cancel,
            SuperExposeTool::List => ExposeTool::List,
            SuperExposeTool::Exec => ExposeTool::Exec,
        },
    )
}

#[cfg(not(feature = "mojo-core"))]
fn expose_route(method: &str, tool: Option<&str>) -> (ExposeMethod, ExposeTool) {
    let method = match method {
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
        "prodex_super_result" => ExposeTool::Result,
        "prodex_super_cancel" => ExposeTool::Cancel,
        "prodex_super_list" => ExposeTool::List,
        "prodex_super_exec" => ExposeTool::Exec,
        _ => ExposeTool::Unknown,
    };
    (method, tool)
}

pub(super) fn dispatch(
    body: &[u8],
    manager: &RunManager,
    workspace: &Path,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let value: Value = match serde_json::from_slice(body) {
        Ok(value) => value,
        Err(_) => return error(None, -32700, "parse error"),
    };
    let Some(object) = value.as_object() else {
        return error(None, -32600, "invalid request");
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
    let (method_kind, tool_kind) = expose_route(method, tool_name);
    let result = match method_kind {
        ExposeMethod::Initialize => Ok(json!({
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "capabilities": {"tools": {"listChanged": false}},
            "serverInfo": {"name": "prodex-super", "version": env!("CARGO_PKG_VERSION")},
            "instructions": "Full-access Prodex Super endpoint. Use prodex_super_start for autonomous tasks or prodex_super_exec for explicit OS commands."
        })),
        ExposeMethod::Ping => Ok(json!({})),
        ExposeMethod::ToolsList => Ok(json!({"tools": tools()})),
        ExposeMethod::ToolsCall => tool_call(&params, tool_kind, manager, workspace),
        ExposeMethod::Notification => return json_response(202, Value::Null),
        ExposeMethod::Unknown => return error(id, -32601, "method not found"),
    };
    match result {
        Ok(result) => rpc_result(id, result),
        Err(message) => rpc_result(
            id,
            json!({
                "content":[{"type":"text","text":message}],
                "structuredContent":{"error":message},
                "isError":true
            }),
        ),
    }
}

fn tool_call(
    params: &Value,
    tool_kind: ExposeTool,
    manager: &RunManager,
    workspace: &Path,
) -> std::result::Result<Value, String> {
    let object = params
        .as_object()
        .ok_or_else(|| "tool parameters are required".to_string())?;
    if object.get("name").and_then(Value::as_str).is_none() {
        return Err("tool name is required".to_string());
    }
    let empty = json!({});
    let arguments = object.get("arguments").unwrap_or(&empty);
    let result = match tool_kind {
        ExposeTool::Start => {
            let task = required_string(arguments, "task", 65_536)?;
            manager.start(task, arguments)?
        }
        ExposeTool::Status => manager
            .status(&required_run_id(arguments)?)
            .ok_or_else(|| "run not found".to_string())?,
        ExposeTool::Result => manager
            .result(&required_run_id(arguments)?)
            .ok_or_else(|| "run not found".to_string())?,
        ExposeTool::Cancel => manager
            .cancel(&required_run_id(arguments)?)
            .ok_or_else(|| "run not found".to_string())?,
        ExposeTool::List => json!({"runs":manager.list()}),
        ExposeTool::Exec => execute_direct(arguments, workspace)?,
        ExposeTool::Unknown => return Err("tool not found".to_string()),
    };
    Ok(json!({
        "content":[{"type":"text","text":serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string())}],
        "structuredContent":result,
        "isError":false
    }))
}

pub(super) fn tools() -> Vec<Value> {
    vec![
        tool_definition(
            "prodex_super_start",
            "Start a full-access Prodex Super task and return a run id.",
            json!({"type":"object","required":["task"],"properties":{
                "task":{"type":"string","minLength":1,"maxLength":65536},
                "model":{"type":["string","null"]},
                "reasoning_effort":{"type":["string","null"]},
                "provider":{"type":["string","null"]},
                "profile":{"type":["string","null"]},
                "sub_agents":{"type":["boolean","null"]}
            }}),
        ),
        tool_definition("prodex_super_status", "Read task status.", run_id_schema()),
        tool_definition(
            "prodex_super_result",
            "Read task status and captured output.",
            run_id_schema(),
        ),
        tool_definition(
            "prodex_super_cancel",
            "Cancel a running task.",
            run_id_schema(),
        ),
        tool_definition(
            "prodex_super_list",
            "List retained tasks.",
            json!({"type":"object","properties":{}}),
        ),
        tool_definition(
            "prodex_super_exec",
            "Execute one direct OS command under the exposed process user's authority.",
            json!({"type":"object","required":["program"],"properties":{
                "program":{"type":"string","minLength":1,"maxLength":4096},
                "args":{"type":["array","null"],"items":{"type":"string"},"maxItems":256},
                "cwd":{"type":["string","null"],"maxLength":4096},
                "env":{"type":["object","null"],"additionalProperties":{"type":"string"}},
                "stdin":{"type":["string","null"],"maxLength":262144},
                "timeout_ms":{"type":["integer","null"],"minimum":1,"maximum":120000}
            }}),
        ),
    ]
}

fn tool_definition(name: &str, description: &str, schema: Value) -> Value {
    json!({"name":name,"description":description,"inputSchema":schema})
}

fn run_id_schema() -> Value {
    json!({"type":"object","required":["run_id"],"properties":{"run_id":{"type":"string","minLength":5,"maxLength":128}}})
}

fn required_run_id(arguments: &Value) -> std::result::Result<String, String> {
    let value = required_string(arguments, "run_id", 128)?;
    if value.starts_with("spr_")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        Ok(value)
    } else {
        Err("run_id is invalid".to_string())
    }
}

fn required_string(
    arguments: &Value,
    name: &str,
    max_bytes: usize,
) -> std::result::Result<String, String> {
    arguments
        .get(name)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= max_bytes)
        .map(str::to_string)
        .ok_or_else(|| format!("{name} is required or too large"))
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
    fn tool_list_keeps_start_and_direct_exec() {
        let names = tools()
            .into_iter()
            .filter_map(|tool| tool["name"].as_str().map(str::to_string))
            .collect::<Vec<_>>();
        assert!(names.contains(&"prodex_super_start".to_string()));
        assert!(names.contains(&"prodex_super_exec".to_string()));
        assert!(names.contains(&"prodex_super_result".to_string()));
    }
}
