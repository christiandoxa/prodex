use super::mcp::{ExposeMcpEndpoint, ExposeMcpEndpointInit, PublicMcpEndpoint};
use super::mcp_tests::{
    SyntheticSessionBridge, expose_mcp_request, expose_start_mcp_test_server_with_endpoint,
    expose_test_args,
};
use super::run_manager::ExposeRunManager;
use super::session_prompt_write::{
    ExistingSessionPromptWrite, PromptOutputReadRequest, PromptOutputReadSuccess,
    SessionPromptWriteError, SessionPromptWriteRequest, SessionPromptWriteSuccess,
};
use super::session_prompt_write_tests::{fixture, queue, service};
use serde_json::Value;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

fn structured(response: &str) -> Value {
    let value: Value = serde_json::from_str(response.split_once("\r\n\r\n").unwrap().1).unwrap();
    value["result"]["structuredContent"].clone()
}

fn endpoint(
    instance_id: &str,
    capability: &str,
    bridge: Arc<dyn ExistingSessionPromptWrite>,
) -> (
    std::net::SocketAddr,
    Arc<super::runtime::ExposeShared>,
    super::runtime::ExposeHttpServer,
) {
    endpoint_at(
        instance_id,
        capability,
        std::env::current_dir().unwrap(),
        bridge,
    )
}

fn endpoint_at(
    instance_id: &str,
    capability: &str,
    workspace: PathBuf,
    bridge: Arc<dyn ExistingSessionPromptWrite>,
) -> (
    std::net::SocketAddr,
    Arc<super::runtime::ExposeShared>,
    super::runtime::ExposeHttpServer,
) {
    let crate::Commands::Super(defaults) = crate::parse_cli_command_from(["prodex", "s"]).unwrap()
    else {
        panic!("expected Super defaults");
    };
    let endpoint = ExposeMcpEndpoint::new_with_run_manager_and_writer(ExposeMcpEndpointInit {
        capability: capability.to_string(),
        instance_id: instance_id.to_string(),
        workspace_name: instance_id.to_string(),
        display_name: instance_id.to_string(),
        defaults,
        run_manager: ExposeRunManager::new(
            workspace.clone(),
            instance_id.to_string(),
            instance_id.to_string(),
        ),
        workspace_root: workspace,
        session_prompt_write: bridge,
    });
    expose_start_mcp_test_server_with_endpoint(
        endpoint,
        "e2e.trycloudflare.com",
        expose_test_args(),
    )
}

fn call_tool(
    address: std::net::SocketAddr,
    target: &str,
    id: u64,
    name: &str,
    arguments: Value,
    session: &str,
) -> Value {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": {"name": name, "arguments": arguments}
    })
    .to_string();
    structured(&expose_mcp_request(
        address,
        "e2e.trycloudflare.com",
        target,
        &body,
        &format!("Mcp-Session-Id: {session}\r\nMcp-Method: tools/call\r\nMcp-Name: {name}\r\n"),
    ))
}

fn append_records(path: &std::path::Path, records: &[Value]) {
    let mut output = std::fs::OpenOptions::new().append(true).open(path).unwrap();
    for record in records {
        writeln!(output, "{record}").unwrap();
    }
}

#[derive(Default)]
struct RejectingSessionBridge;

impl ExistingSessionPromptWrite for RejectingSessionBridge {
    fn write(
        &self,
        _request: SessionPromptWriteRequest,
    ) -> Result<SessionPromptWriteSuccess, SessionPromptWriteError> {
        Err(SessionPromptWriteError::NoSession)
    }

    fn read_output(
        &self,
        _request: PromptOutputReadRequest,
    ) -> Result<PromptOutputReadSuccess, SessionPromptWriteError> {
        Err(SessionPromptWriteError::NoSession)
    }
}

#[test]
fn mcp_prompt_write_output_read_is_deterministic_and_fail_closed() {
    let bridge = Arc::new(SyntheticSessionBridge::default());
    let (address, shared, mut server) = endpoint(
        "pdxi_e2e",
        "EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE",
        bridge.clone(),
    );
    let target = "/pdx/v1/EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE/mcp";
    let initialize = expose_mcp_request(
        address,
        "e2e.trycloudflare.com",
        target,
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"e2e","version":"1"}}}"#,
        "MCP-Protocol-Version: 2025-06-18\r\nMcp-Method: initialize\r\n",
    );
    assert!(initialize.starts_with("HTTP/1.1 200"), "{initialize}");
    let tools = expose_mcp_request(
        address,
        "e2e.trycloudflare.com",
        target,
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#,
        "Mcp-Method: tools/list\r\n",
    );
    assert!(tools.contains("prodex_session_prompt_write"));

    let message = format!("mcp-e2e-exact-{}", std::process::id());
    let write_body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "prodex_session_prompt_write",
            "arguments": {"message": message}
        }
    })
    .to_string();
    let write = expose_mcp_request(
        address,
        "e2e.trycloudflare.com",
        target,
        &write_body,
        "Mcp-Session-Id: first-session\r\nMcp-Method: tools/call\r\nMcp-Name: prodex_session_prompt_write\r\n",
    );
    let write_value = structured(&write);
    let cursor = write_value["output_cursor"].as_str().unwrap();
    assert_eq!(write_value["status"], "written");
    assert_eq!(write_value["output_cursor"], "cursor-anchor");
    assert_eq!(
        write_value["thread_id"],
        "019f3b59-7771-7ea1-a9a1-3cd638f216c4"
    );
    assert_eq!(bridge.written.lock().unwrap()[0].message, message);
    assert!(shared.mcp.as_ref().unwrap().run_manager.list().is_empty());

    let cloudflare_binding = bridge.written.lock().unwrap()[0].binding_key.clone();
    let local_write = expose_mcp_request(
        address,
        &address.to_string(),
        target,
        &write_body,
        "Mcp-Session-Id: local-a\r\nMcp-Method: tools/call\r\nMcp-Name: prodex_session_prompt_write\r\n",
    );
    assert_eq!(structured(&local_write)["status"], "written");
    let local_output = expose_mcp_request(
        address,
        &address.to_string(),
        target,
        r#"{"jsonrpc":"2.0","id":11,"method":"tools/call","params":{"name":"prodex_session_output_read","arguments":{}}}"#,
        "Mcp-Session-Id: local-b\r\nMcp-Method: tools/call\r\nMcp-Name: prodex_session_output_read\r\n",
    );
    assert!(local_output.contains("synthetic output"));
    let relay = PublicMcpEndpoint::new(
        &format!("http://{address}"),
        "EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE",
    )
    .unwrap();
    let opaque = shared
        .mcp
        .as_ref()
        .unwrap()
        .install_openai_relay(&relay)
        .unwrap();
    let relay_path = url::Url::parse(&opaque).unwrap().path().to_string();
    let openai_write = expose_mcp_request(
        address,
        &address.to_string(),
        &relay_path,
        &write_body,
        "Mcp-Session-Id: openai-a\r\nMcp-Method: tools/call\r\nMcp-Name: prodex_session_prompt_write\r\n",
    );
    assert_eq!(structured(&openai_write)["status"], "written");
    let openai_output = expose_mcp_request(
        address,
        &address.to_string(),
        &relay_path,
        r#"{"jsonrpc":"2.0","id":12,"method":"tools/call","params":{"name":"prodex_session_output_read","arguments":{}}}"#,
        "Mcp-Session-Id: openai-b\r\nMcp-Method: tools/call\r\nMcp-Name: prodex_session_output_read\r\n",
    );
    assert!(openai_output.contains("synthetic output"));
    assert_eq!(
        cloudflare_binding,
        bridge.written.lock().unwrap()[1].binding_key
    );
    assert_eq!(
        cloudflare_binding,
        bridge.written.lock().unwrap()[2].binding_key
    );

    let page = |id: u64, session: &str, cursor: &str| {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": {
                "name": "prodex_session_output_read",
                "arguments": {"cursor": cursor, "limit": 1}
            }
        })
        .to_string();
        structured(&expose_mcp_request(
            address,
            "e2e.trycloudflare.com",
            target,
            &body,
            &format!(
                "Mcp-Session-Id: {session}\r\nMcp-Method: tools/call\r\nMcp-Name: prodex_session_output_read\r\n"
            ),
        ))
    };
    let first = page(4, "reconnect-a", cursor);
    assert_eq!(first["events"][0]["text"], "assistant output");
    let repeated = page(5, "reconnect-b", cursor);
    assert_eq!(first, repeated);
    let second = page(6, "reconnect-c", first["next_cursor"].as_str().unwrap());
    assert_eq!(second["events"][0]["text"], "tool result");
    let third = page(7, "reconnect-d", second["next_cursor"].as_str().unwrap());
    assert_eq!(third["events"][0]["text"], "turn completed");
    assert!(!third["has_more"].as_bool().unwrap());
    assert_eq!(bridge.read.lock().unwrap().len(), 6);

    let upper_thread = "019F3B59-7771-7EA1-A9A1-3CD638F216C4";
    let explicit = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 9,
        "method": "tools/call",
        "params": {
            "name": "prodex_session_prompt_write",
            "arguments": {
                "message": "explicit selector",
                "prodex_pid": 123,
                "thread_id": upper_thread
            }
        }
    })
    .to_string();
    let explicit_write = expose_mcp_request(
        address,
        "e2e.trycloudflare.com",
        target,
        &explicit,
        "Mcp-Session-Id: selector-a\r\nMcp-Method: tools/call\r\nMcp-Name: prodex_session_prompt_write\r\n",
    );
    assert_eq!(structured(&explicit_write)["status"], "written");
    let explicit_binding = bridge
        .written
        .lock()
        .unwrap()
        .last()
        .unwrap()
        .binding_key
        .clone();
    assert!(explicit_binding.contains(":pid=123:thread=019f3b59-7771-7ea1-a9a1-3cd638f216c4"));
    let explicit_read = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 10,
        "method": "tools/call",
        "params": {
            "name": "prodex_session_output_read",
            "arguments": {"prodex_pid": 123, "thread_id": upper_thread}
        }
    })
    .to_string();
    assert!(expose_mcp_request(
        address,
        "e2e.trycloudflare.com",
        target,
        &explicit_read,
        "Mcp-Session-Id: selector-b\r\nMcp-Method: tools/call\r\nMcp-Name: prodex_session_output_read\r\n",
    )
    .contains("synthetic output"));
    assert_eq!(
        explicit_binding,
        bridge.read.lock().unwrap().last().unwrap().binding_key
    );

    server.shutdown();
    shared.pty.shutdown();
    shared.mcp.as_ref().unwrap().run_manager.shutdown();

    let (stale_address, stale_shared, mut stale_server) = endpoint(
        "pdxi_stale",
        "SSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSS",
        Arc::new(RejectingSessionBridge),
    );
    let stale_target = "/pdx/v1/SSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSS/mcp";
    let stale = expose_mcp_request(
        stale_address,
        "e2e.trycloudflare.com",
        stale_target,
        &write_body,
        "Mcp-Method: tools/call\r\nMcp-Name: prodex_session_prompt_write\r\n",
    );
    assert!(stale.contains("no_session"));
    let healthy = expose_mcp_request(
        stale_address,
        "e2e.trycloudflare.com",
        stale_target,
        r#"{"jsonrpc":"2.0","id":8,"method":"tools/list","params":{}}"#,
        "Mcp-Method: tools/list\r\n",
    );
    assert!(healthy.starts_with("HTTP/1.1 200"));
    stale_server.shutdown();
    stale_shared.pty.shutdown();
    stale_shared.mcp.as_ref().unwrap().run_manager.shutdown();
}

#[test]
fn mcp_prompt_write_and_output_read_follow_the_real_session_service() {
    let fixture = fixture();
    let message = format!("real-session-e2e-{}", std::process::id());
    let queue_db_before = std::fs::read(&fixture._queue_db).unwrap();
    let mut queue_control = queue(&fixture, Some("019f3b59-7771-7ea1-a9a1-3cd638f216c5"));
    queue_control.consumed_message = Some(message.clone());
    let calls = Arc::clone(&queue_control.calls);
    let bridge = Arc::new(service(&fixture, queue_control));
    let capability = "RRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRR";
    let target = format!("/pdx/v1/{capability}/mcp");
    let (address, shared, mut server) = endpoint_at(
        "pdxi_real_e2e",
        capability,
        fixture.workspace.clone(),
        bridge,
    );

    let initialize = expose_mcp_request(
        address,
        "e2e.trycloudflare.com",
        &target,
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"e2e","version":"1"}}}"#,
        "MCP-Protocol-Version: 2025-06-18\r\nMcp-Method: initialize\r\n",
    );
    assert!(initialize.starts_with("HTTP/1.1 200"), "{initialize}");
    let tools = expose_mcp_request(
        address,
        "e2e.trycloudflare.com",
        &target,
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#,
        "Mcp-Method: tools/list\r\n",
    );
    assert!(tools.contains("prodex_session_prompt_write"));
    assert!(tools.contains("prodex_session_output_read"));

    let written = call_tool(
        address,
        &target,
        3,
        "prodex_session_prompt_write",
        serde_json::json!({"message": message}),
        "writer-a",
    );
    assert_eq!(written["status"], "written");
    assert_eq!(written["prodex_pid"], 100);
    assert_eq!(written["codex_pid"], 200);
    assert_eq!(written["thread_id"], "019f3b59-7771-7ea1-a9a1-3cd638f216c4");
    assert_eq!(written["verification"], "rollout_user_event_observed");
    let cursor = written["output_cursor"].as_str().unwrap();
    assert_eq!(
        calls.lock().unwrap().as_slice(),
        [(
            "019f3b59-7771-7ea1-a9a1-3cd638f216c4".to_string(),
            message.clone()
        )]
    );

    append_records(
        &fixture.rollout,
        &[
            serde_json::json!({
                "timestamp": "2026-09-03T10:00:04Z",
                "type": "session_meta",
                "payload": {"base_instructions": {"text": "hidden-system-secret"}}
            }),
            serde_json::json!({
                "timestamp": "2026-09-03T10:00:05Z",
                "type": "response_item",
                "payload": {
                    "type": "message",
                    "role": "user",
                    "content": [{"type": "input_text", "text": "hidden-context-secret"}],
                    "internal_chat_message_metadata_passthrough": {
                        "content_item_kinds": ["agents_md.instructions"]
                    }
                }
            }),
            serde_json::json!({
                "timestamp": "2026-09-03T10:00:06Z",
                "type": "event_msg",
                "payload": {"type": "agent_message", "message": "assistant visible"}
            }),
            serde_json::json!({
                "timestamp": "2026-09-03T10:00:07Z",
                "type": "response_item",
                "payload": {"type": "function_call", "name": "shell", "arguments": "{\"cmd\":\"true\"}"}
            }),
            serde_json::json!({
                "timestamp": "2026-09-03T10:00:08Z",
                "type": "response_item",
                "payload": {"type": "function_call_output", "output": "tool visible"}
            }),
            serde_json::json!({
                "timestamp": "2026-09-03T10:00:09Z",
                "type": "event_msg",
                "payload": {"type": "turn_completed", "status": "completed"}
            }),
        ],
    );
    let other_thread = "019f3b59-7771-7ea1-a9a1-3cd638f216d0";
    let other_rollout = fixture
        .rollout
        .parent()
        .unwrap()
        .join(format!("rollout-{other_thread}.jsonl"));
    std::fs::write(
        other_rollout,
        "{\"timestamp\":\"2026-09-03T10:00:10Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"agent_message\",\"message\":\"other-session-secret\"}}\n",
    )
    .unwrap();

    let first = call_tool(
        address,
        &target,
        4,
        "prodex_session_output_read",
        serde_json::json!({"cursor": cursor, "limit": 2}),
        "reader-a",
    );
    let repeated = call_tool(
        address,
        &target,
        5,
        "prodex_session_output_read",
        serde_json::json!({"cursor": cursor, "limit": 2}),
        "reader-reconnected",
    );
    assert_eq!(first, repeated);
    assert_eq!(first["events"][0]["kind"], "user");
    assert_eq!(first["events"][0]["text"], message);
    assert_eq!(first["events"][1]["text"], "assistant visible");
    let second = call_tool(
        address,
        &target,
        6,
        "prodex_session_output_read",
        serde_json::json!({"cursor": first["next_cursor"], "limit": 2}),
        "reader-b",
    );
    assert_eq!(second["events"][0]["kind"], "tool");
    assert_eq!(second["events"][0]["status"], "started");
    assert_eq!(second["events"][1]["text"], "tool visible");
    let third = call_tool(
        address,
        &target,
        7,
        "prodex_session_output_read",
        serde_json::json!({"cursor": second["next_cursor"], "limit": 2}),
        "reader-c",
    );
    assert_eq!(third["events"][0]["kind"], "terminal");
    let visible = format!("{first}{second}{third}");
    assert!(!visible.contains("hidden-system-secret"));
    assert!(!visible.contains("hidden-context-secret"));
    assert!(!visible.contains("other-session-secret"));
    assert_eq!(std::fs::read(&fixture._queue_db).unwrap(), queue_db_before);
    assert!(shared.mcp.as_ref().unwrap().run_manager.list().is_empty());

    server.shutdown();
    shared.pty.shutdown();
    shared.mcp.as_ref().unwrap().run_manager.shutdown();
}

#[test]
fn mcp_busy_prompt_is_queued_once_and_read_in_rollout_order() {
    let fixture = fixture();
    let message = "queued while turn A is active";
    let mut queue_control = queue(&fixture, Some("019f3b59-7771-7ea1-a9a1-3cd638f216c6"));
    queue_control.invocation.queued = true;
    let calls = Arc::clone(&queue_control.calls);
    let capability = "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB";
    let target = format!("/pdx/v1/{capability}/mcp");
    let (address, shared, mut server) = endpoint_at(
        "pdxi_busy_e2e",
        capability,
        fixture.workspace.clone(),
        Arc::new(service(&fixture, queue_control)),
    );

    let written = call_tool(
        address,
        &target,
        20,
        "prodex_session_prompt_write",
        serde_json::json!({"message": message}),
        "busy-writer",
    );
    assert_eq!(written["verification"], "queue_pending_observed");
    let cursor = written["output_cursor"].as_str().unwrap();
    append_records(
        &fixture.rollout,
        &[
            serde_json::json!({
                "timestamp": "2026-09-03T10:01:00Z",
                "type": "event_msg",
                "payload": {"type": "turn_completed", "status": "completed"}
            }),
            serde_json::json!({
                "timestamp": "2026-09-03T10:01:01Z",
                "type": "event_msg",
                "payload": {"type": "user_message", "message": message}
            }),
            serde_json::json!({
                "timestamp": "2026-09-03T10:01:02Z",
                "type": "event_msg",
                "payload": {"type": "agent_message", "message": "turn B output"}
            }),
            serde_json::json!({
                "timestamp": "2026-09-03T10:01:03Z",
                "type": "event_msg",
                "payload": {"type": "turn_completed", "status": "completed"}
            }),
        ],
    );
    let output = call_tool(
        address,
        &target,
        21,
        "prodex_session_output_read",
        serde_json::json!({"cursor": cursor, "limit": 10}),
        "busy-reader",
    );
    assert_eq!(
        output["events"]
            .as_array()
            .unwrap()
            .iter()
            .map(|event| event["kind"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["terminal", "user", "assistant", "terminal"]
    );
    assert_eq!(output["events"][1]["text"], message);
    assert_eq!(output["events"][2]["text"], "turn B output");
    assert_eq!(calls.lock().unwrap().len(), 1);
    assert!(shared.mcp.as_ref().unwrap().run_manager.list().is_empty());

    server.shutdown();
    shared.pty.shutdown();
    shared.mcp.as_ref().unwrap().run_manager.shutdown();
}
