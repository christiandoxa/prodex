use crate::{AppPaths, AppState, AppStateIoExt, ProfileProviderExt};
use anyhow::{Context, Result};
use prodex_cli::InspectMcpArgs;
use prodex_mcp_stdio::{read_mcp_message, write_mcp_message};
use serde_json::{Value, json};
use std::env;
use std::fs;
use std::io::{self, BufReader, Read};
use std::path::PathBuf;

const RUNTIME_LATEST_POINTER: &str = "prodex-runtime-latest.path";
const RUNTIME_LOG_TAIL_BYTES: usize = 32 * 1024;
const INSPECT_MCP_INVALID_PARAMS_MESSAGE: &str = "invalid inspect tool parameters";
const INSPECT_MCP_METHOD_NOT_FOUND_MESSAGE: &str = "method not found";

pub(crate) fn handle_inspect_mcp(_args: InspectMcpArgs) -> Result<()> {
    run_inspect_mcp_stdio()
}

fn run_inspect_mcp_stdio() -> Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = stdout.lock();
    while let Some((request, framing)) = read_mcp_message(&mut reader)? {
        let Some(response) = handle_mcp_request(request)? else {
            continue;
        };
        write_mcp_message(&mut writer, &response, framing)?;
    }
    Ok(())
}

fn handle_mcp_request(request: Value) -> Result<Option<Value>> {
    let id = request.get("id").cloned();
    let method = request
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if id.is_none() {
        return Ok(None);
    }
    let id = id.unwrap_or(Value::Null);
    let result = match method {
        "initialize" => json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "prodex-inspect", "version": env!("CARGO_PKG_VERSION") }
        }),
        "ping" => json!({}),
        "tools/list" => json!({ "tools": inspect_tools() }),
        "tools/call" => {
            let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
            match handle_tool_call(params) {
                Ok(result) => result,
                Err(_err) => {
                    return Ok(Some(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": { "code": -32602, "message": INSPECT_MCP_INVALID_PARAMS_MESSAGE }
                    })));
                }
            }
        }
        _ => {
            return Ok(Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": INSPECT_MCP_METHOD_NOT_FOUND_MESSAGE }
            })));
        }
    };
    Ok(Some(
        json!({ "jsonrpc": "2.0", "id": id, "result": result }),
    ))
}

fn inspect_tools() -> Vec<Value> {
    vec![
        json!({
            "name": "prodex_status",
            "description": "Return read-only Prodex state paths, profile counts, and active profile.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "prodex_profiles",
            "description": "List managed Prodex profiles without secrets.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "prodex_latest_runtime_log",
            "description": "Return the latest Prodex runtime log path and a tail excerpt for diagnostics.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "tail_bytes": {
                        "type": "integer",
                        "description": "Maximum log bytes to return from the end of the file.",
                        "default": RUNTIME_LOG_TAIL_BYTES
                    }
                }
            }
        }),
    ]
}

fn handle_tool_call(params: Value) -> Result<Value> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let payload = match name {
        "prodex_status" => prodex_status_json()?,
        "prodex_profiles" => prodex_profiles_json()?,
        "prodex_latest_runtime_log" => prodex_latest_runtime_log_json(&arguments)?,
        _ => anyhow::bail!("unknown inspect tool: {name}"),
    };
    Ok(json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&payload)?
        }]
    }))
}

fn prodex_status_json() -> Result<Value> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    Ok(json!({
        "prodex_home": paths.root,
        "state_file": paths.state_file,
        "managed_profiles_root": paths.managed_profiles_root,
        "shared_codex_home": paths.shared_codex_root,
        "active_profile": state.active_profile,
        "profile_count": state.profiles.len(),
        "response_profile_bindings": state.response_profile_bindings.len(),
        "session_profile_bindings": state.session_profile_bindings.len(),
        "version": env!("CARGO_PKG_VERSION")
    }))
}

fn prodex_profiles_json() -> Result<Value> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    let profiles = state
        .profiles
        .iter()
        .map(|(name, profile)| {
            let auth = profile.provider.auth_summary(&profile.codex_home);
            json!({
                "name": name,
                "active": state.active_profile.as_deref() == Some(name.as_str()),
                "managed": profile.managed,
                "provider": profile.provider.label(),
                "auth": {
                    "label": auth.label,
                    "quota_compatible": auth.quota_compatible
                },
                "email": profile.email,
                "codex_home": profile.codex_home,
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({ "profiles": profiles }))
}

fn prodex_latest_runtime_log_json(arguments: &Value) -> Result<Value> {
    let tail_bytes = arguments
        .get("tail_bytes")
        .and_then(Value::as_u64)
        .unwrap_or(RUNTIME_LOG_TAIL_BYTES as u64)
        .clamp(1, 256 * 1024) as usize;
    let pointer_path = runtime_log_pointer_path();
    let log_path = fs::read_to_string(&pointer_path)
        .ok()
        .map(|raw| PathBuf::from(raw.trim()))
        .filter(|path| !path.as_os_str().is_empty());
    let Some(log_path) = log_path else {
        return Ok(json!({
            "pointer_path": pointer_path,
            "log_path": null,
            "exists": false,
            "tail": "",
            "message": "No latest runtime log pointer has been created."
        }));
    };
    let exists = log_path.exists();
    let tail = if exists {
        read_file_tail_lossy(&log_path, tail_bytes)?
    } else {
        String::new()
    };
    Ok(json!({
        "pointer_path": pointer_path,
        "log_path": log_path,
        "exists": exists,
        "tail_bytes": tail_bytes,
        "tail": tail
    }))
}

fn runtime_log_pointer_path() -> PathBuf {
    runtime_log_dir().join(RUNTIME_LATEST_POINTER)
}

fn runtime_log_dir() -> PathBuf {
    env::var_os("PRODEX_RUNTIME_LOG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(env::temp_dir)
}

fn read_file_tail_lossy(path: &PathBuf, max_bytes: usize) -> Result<String> {
    let mut file =
        fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let len = file
        .metadata()
        .with_context(|| format!("failed to stat {}", path.display()))?
        .len();
    let start = len.saturating_sub(max_bytes as u64);
    if start > 0 {
        use std::io::Seek;
        file.seek(std::io::SeekFrom::Start(start))?;
    }
    let mut bytes = Vec::new();
    file.take(max_bytes as u64)
        .read_to_end(&mut bytes)
        .with_context(|| format!("failed to read {}", path.display()))?;
    Ok(String::from_utf8_lossy(&bytes).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use prodex_mcp_stdio::McpMessageFraming;

    #[test]
    fn inspect_mcp_initialize_returns_server_info() {
        let response = handle_mcp_request(json!({"jsonrpc":"2.0","id":1,"method":"initialize"}))
            .unwrap()
            .unwrap();
        assert_eq!(response["result"]["serverInfo"]["name"], "prodex-inspect");
    }

    #[test]
    fn inspect_mcp_lists_read_only_tools() {
        let response = handle_mcp_request(json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}))
            .unwrap()
            .unwrap();
        let tools = response["result"]["tools"].as_array().unwrap();
        assert!(tools.iter().any(|tool| tool["name"] == "prodex_status"));
        assert!(tools.iter().any(|tool| tool["name"] == "prodex_profiles"));
        assert!(
            tools
                .iter()
                .any(|tool| tool["name"] == "prodex_latest_runtime_log")
        );
    }

    #[test]
    fn inspect_mcp_response_uses_request_framing() {
        let response = json!({"jsonrpc":"2.0","id":1,"result":{}});
        let mut json_line = Vec::new();
        write_mcp_message(&mut json_line, &response, McpMessageFraming::JsonLine).unwrap();
        assert_eq!(json_line.last(), Some(&b'\n'));

        let mut content_length = Vec::new();
        write_mcp_message(
            &mut content_length,
            &response,
            McpMessageFraming::ContentLength,
        )
        .unwrap();
        assert!(content_length.starts_with(b"Content-Length:"));
    }

    #[test]
    fn inspect_mcp_invalid_params_error_is_stable_and_redacted() {
        let response = handle_mcp_request(json!({
            "jsonrpc":"2.0",
            "id":1,
            "method":"tools/call",
            "params": { "name": "secret-token-123", "arguments": {} }
        }))
        .unwrap()
        .unwrap();

        assert_eq!(response["error"]["code"], -32602);
        assert_eq!(
            response["error"]["message"],
            INSPECT_MCP_INVALID_PARAMS_MESSAGE
        );
        assert!(!response.to_string().contains("secret-token-123"));
    }

    #[test]
    fn inspect_mcp_unknown_method_error_is_stable_and_redacted() {
        let response = handle_mcp_request(json!({
            "jsonrpc":"2.0",
            "id":1,
            "method":"secret-token-123"
        }))
        .unwrap()
        .unwrap();

        assert_eq!(response["error"]["code"], -32601);
        assert_eq!(
            response["error"]["message"],
            INSPECT_MCP_METHOD_NOT_FOUND_MESSAGE
        );
        assert!(!response.to_string().contains("secret-token-123"));
    }
}
