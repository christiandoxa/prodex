use super::mcp_tests::{expose_mcp_request, expose_start_mcp_test_server};

#[test]
fn mcp_super_exec_runs_without_a_super_run_or_plain_session() {
    let capability = "abcdefghijklmnopqrstuvwxyz0123456789ABCDEFG";
    let (listen_addr, shared, mut server) =
        expose_start_mcp_test_server(capability, "pdxi_exec", "exec", "exec.trycloudflare.com");
    let (program, args) = if cfg!(windows) {
        ("cmd.exe", vec!["/C", "echo standalone"])
    } else {
        ("echo", vec!["standalone"])
    };
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "prodex_super_exec",
            "arguments": {"program": program, "args": args}
        }
    });
    let response = expose_mcp_request(
        listen_addr,
        "exec.trycloudflare.com",
        &format!("/pdx/v1/{capability}/mcp"),
        &body.to_string(),
        "Mcp-Method: tools/call\r\nMcp-Name: prodex_super_exec\r\n",
    );
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    let value: serde_json::Value =
        serde_json::from_str(response.split_once("\r\n\r\n").unwrap().1).unwrap();
    let result = &value["result"]["structuredContent"];
    assert_eq!(result["status"], "completed");
    assert_eq!(result["success"], true);
    assert!(result["stdout"].as_str().unwrap().contains("standalone"));
    assert!(result.get("run_id").is_none());
    assert!(result.get("thread_id").is_none());
    assert!(shared.mcp.as_ref().unwrap().run_manager.list().is_empty());
    server.shutdown();
    shared.pty.shutdown();
    shared.mcp.as_ref().unwrap().run_manager.shutdown();
}
