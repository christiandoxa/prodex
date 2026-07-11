use super::gemini_rewrite_test_support::{conversation_store, gemini_test_function_response};
use super::runtime_gemini_generate_request_body;

#[test]
fn gemini_request_translation_structures_running_exec_command_output() {
    let output = "Chunk ID: abc123\nWall time: 1.0000 seconds\nProcess running with session ID 48274\nOriginal token count: 12\nOutput:\nCloning into '/tmp/gemini-cli'...\n";
    let body = serde_json::json!({
        "model": "gemini-2.5-pro",
        "input": [
            {
                "type": "function_call",
                "call_id": "call_exec_1",
                "name": "exec_command",
                "arguments": "{\"cmd\":\"git clone https://github.com/google-gemini/gemini-cli.git /tmp/gemini-cli\"}"
            },
            {
                "type": "function_call_output",
                "call_id": "call_exec_1",
                "output": output
            }
        ]
    });

    let translated = runtime_gemini_generate_request_body(
        &serde_json::to_vec(&body).unwrap(),
        &conversation_store(),
        false,
        None,
        None,
    )
    .expect("request should translate");
    let value: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
    let response =
        &gemini_test_function_response(&value, "call_exec_1")["functionResponse"]["response"];

    assert_eq!(response["output"], output);
    assert_eq!(response["status"], "running");
    assert_eq!(response["codex_tool_status"], "running");
    assert_eq!(response["running_session_id"], 48274_u64);
    assert!(
        response["next_required_action"]
            .as_str()
            .unwrap()
            .contains("write_stdin")
    );
}

#[test]
fn gemini_request_translation_marks_failed_clone_output_as_unverified_path() {
    let output = "Chunk ID: abc123\nWall time: 1.0000 seconds\nProcess exited with code 2\nOutput:\nCloning into '/tmp/refs/gemini-cli'...\nerror: RPC failed; HTTP 404 curl 22 The requested URL returned error: 404\nfatal: error reading section header 'ack'\n";
    let body = serde_json::json!({
        "model": "gemini-2.5-pro",
        "input": [
            {
                "type": "function_call",
                "call_id": "call_exec_1",
                "name": "exec_command",
                "arguments": "{\"cmd\":\"git clone https://github.com/google-gemini/gemini-cli.git /tmp/refs/gemini-cli\"}"
            },
            {
                "type": "function_call_output",
                "call_id": "call_exec_1",
                "output": output
            }
        ]
    });

    let translated = runtime_gemini_generate_request_body(
        &serde_json::to_vec(&body).unwrap(),
        &conversation_store(),
        false,
        None,
        None,
    )
    .expect("request should translate");
    let value: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
    let response =
        &gemini_test_function_response(&value, "call_exec_1")["functionResponse"]["response"];

    assert_eq!(response["status"], "failed");
    assert_eq!(response["codex_tool_status"], "failed");
    assert_eq!(response["failure_kind"], "clone_or_network");
    assert_eq!(response["affected_path"], "/tmp/refs/gemini-cli");
    assert_eq!(response["path_verified"], false);
    assert!(
        response["next_required_action"]
            .as_str()
            .unwrap()
            .contains("Do not use files or paths")
    );
}

#[test]
fn gemini_request_translation_marks_missing_path_command_output() {
    let output = "Chunk ID: abc123\nWall time: 1.0000 seconds\nProcess exited with code 2\nOutput:\nls: cannot access '/tmp/refs/gemini-cli': No such file or directory\n";
    let body = serde_json::json!({
        "model": "gemini-2.5-pro",
        "input": [
            {
                "type": "function_call",
                "call_id": "call_exec_1",
                "name": "exec_command",
                "arguments": "{\"cmd\":\"ls /tmp/refs/gemini-cli\"}"
            },
            {
                "type": "function_call_output",
                "call_id": "call_exec_1",
                "output": output
            }
        ]
    });

    let translated = runtime_gemini_generate_request_body(
        &serde_json::to_vec(&body).unwrap(),
        &conversation_store(),
        false,
        None,
        None,
    )
    .expect("request should translate");
    let value: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
    let response =
        &gemini_test_function_response(&value, "call_exec_1")["functionResponse"]["response"];

    assert_eq!(response["status"], "failed");
    assert_eq!(response["failure_kind"], "missing_path");
    assert_eq!(response["affected_path"], "/tmp/refs/gemini-cli");
    assert_eq!(response["path_verified"], false);
}
