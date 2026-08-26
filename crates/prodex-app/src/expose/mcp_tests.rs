use super::mcp::ExposeMcpEndpoint;
use super::run_manager::{ExposeRunManager, ExposeRunState};
use super::runtime::{ExposeHttpServer, ExposePty, ExposeShared};
use super::session::ExposeSessionStore;
use crate::ExposeArgs;
use std::collections::BTreeSet;
use std::fs;
use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn expose_send_test_request(listen_addr: SocketAddr, request: &str) -> String {
    let mut stream = TcpStream::connect(listen_addr).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    stream.write_all(request.as_bytes()).unwrap();
    stream.shutdown(Shutdown::Write).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

fn expose_start_mcp_test_server(
    capability: &str,
    instance_id: &str,
    workspace_name: &str,
    public_host: &str,
) -> (SocketAddr, Arc<ExposeShared>, ExposeHttpServer) {
    let args = ExposeArgs {
        command: None,
        cols: 80,
        rows: 24,
        max_clients: 4,
        tunnel: false,
        no_tunnel: false,
        name: Some("test".to_string()),
        invocation: prodex_cli::ExposeInvocation::SuperAlias,
        super_args: None,
    };
    let crate::Commands::Super(super_args) =
        crate::parse_cli_command_from(["prodex", "s"]).expect("Super args should parse")
    else {
        panic!("expected Super args");
    };
    let mcp = ExposeMcpEndpoint::new(
        capability,
        instance_id.to_string(),
        std::env::current_dir().unwrap(),
        workspace_name.to_string(),
        workspace_name.to_string(),
        super_args,
    );
    expose_start_mcp_test_server_with_endpoint(mcp, public_host, args)
}

pub(super) fn expose_start_mcp_test_server_with_endpoint(
    mcp: Arc<ExposeMcpEndpoint>,
    public_host: &str,
    args: ExposeArgs,
) -> (SocketAddr, Arc<ExposeShared>, ExposeHttpServer) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let listen_addr = listener.local_addr().unwrap();
    let shared = Arc::new(ExposeShared {
        sessions: Mutex::new(ExposeSessionStore::new(
            "bootstrap_capability_for_mcp_test",
            4,
            Instant::now(),
        )),
        allowed_hosts: Mutex::new(BTreeSet::from([
            listen_addr.to_string(),
            public_host.to_string(),
        ])),
        mcp_only_hosts: Mutex::new(BTreeSet::from([public_host.to_string()])),
        mcp: Some(mcp),
        pty: ExposePty::spawn(&args).unwrap(),
        shutdown: Arc::new(AtomicBool::new(false)),
        active_clients: AtomicUsize::new(0),
        active_requests: AtomicUsize::new(0),
        peak_requests: AtomicUsize::new(0),
        next_client_id: AtomicU64::new(1),
        max_clients: 4,
    });
    let server = ExposeHttpServer::start(listener, Arc::clone(&shared)).unwrap();
    (listen_addr, shared, server)
}

pub(super) fn expose_mcp_request(
    listen_addr: SocketAddr,
    host: &str,
    target: &str,
    body: &str,
    extra_headers: &str,
) -> String {
    expose_send_test_request(
        listen_addr,
        &format!(
            "POST {target} HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\nAccept: application/json, text/event-stream\r\n{extra_headers}Content-Length: {}\r\n\r\n{body}",
            body.len()
        ),
    )
}

#[test]
fn mcp_start_inherits_frozen_main_and_sub_agent_configuration() {
    let root = crate::test_temp_root().join(format!("prodex-mcp-inherit-{}", std::process::id()));
    let workspace = root.join("workspace");
    fs::create_dir_all(&workspace).unwrap();
    let executable = crate::write_test_python_executable(
        &root,
        "fake-mcp-super",
        r#"#!/usr/bin/env python3
import os
import sys

task = sys.stdin.read()
print("TASK=" + task, flush=True)
print("CWD=" + os.getcwd(), flush=True)
print("ARGV=" + repr(sys.argv), flush=True)
"#,
    );
    let crate::Commands::Super(mut defaults) =
        crate::parse_cli_command_from(["prodex", "s", "--no-sub-agent"])
            .expect("Super defaults should parse")
    else {
        panic!("expected Super defaults");
    };
    defaults.local_model = Some("main-model".to_string());
    defaults.codex_args = vec!["-c".into(), "model_reasoning_effort=max".into()];
    defaults.sub_agent = true;
    defaults.no_sub_agent = false;
    defaults.sub_agent_provider = Some(prodex_provider_core::ProviderId::OpenAi);
    defaults.sub_agent_model = Some("sub-model".to_string());
    defaults.sub_agent_model_reasoning_effort = Some(prodex_cli::SubAgentReasoningEffort::High);
    let manager = ExposeRunManager::new_with_executable(
        workspace.clone(),
        "pdxi_inherit".to_string(),
        "workspace".to_string(),
        executable,
    );
    let endpoint = ExposeMcpEndpoint::new_with_run_manager(
        "abcdefghijklmnopqrstuvwxyz0123456789ABCDEFG",
        "pdxi_inherit".to_string(),
        "workspace".to_string(),
        "workspace".to_string(),
        defaults,
        manager,
    );
    let started = endpoint
        .start_tool(&serde_json::json!({
            "task": "MCP_INHERIT",
            "model": null,
            "reasoning_effort": null,
            "sub_agents": null
        }))
        .unwrap();
    let run_id = started["run_id"].as_str().unwrap();
    for _ in 0..200 {
        if let Some(result) = endpoint.run_manager.result(run_id)
            && result.summary.state.terminal()
        {
            assert_eq!(result.summary.state, ExposeRunState::Succeeded);
            assert!(result.output.contains("TASK=MCP_INHERIT"));
            assert!(result.output.contains("main-model"));
            assert!(result.output.contains("model_reasoning_effort=max"));
            assert!(result.output.contains("sub-model"));
            assert!(
                result
                    .output
                    .contains(&format!("CWD={}", workspace.display()))
            );
            endpoint.run_manager.shutdown();
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("MCP run did not finish");
}

#[test]
fn mcp_json_protocol_and_public_route_isolation_are_enforced() {
    let capability = "abcdefghijklmnopqrstuvwxyz0123456789ABCDEFG";
    let (listen_addr, shared, mut server) = expose_start_mcp_test_server(
        capability,
        "pdxi_test",
        "test",
        "mcp-test.trycloudflare.com",
    );
    let host = "mcp-test.trycloudflare.com";
    let init = expose_mcp_request(
        listen_addr,
        host,
        &format!("/pdx/v1/{capability}/mcp"),
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}"#,
        "MCP-Protocol-Version: 2025-06-18\r\nMcp-Method: initialize\r\n",
    );
    assert!(init.starts_with("HTTP/1.1 200"));
    assert!(init.contains("Content-Type: application/json\r\n"));
    assert!(!init.contains("text/event-stream"));

    let discover = expose_mcp_request(
        listen_addr,
        host,
        &format!("/pdx/v1/{capability}/mcp"),
        r#"{"jsonrpc":"2.0","id":0,"method":"server/discover","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientInfo":{"name":"test","version":"1"},"io.modelcontextprotocol/clientCapabilities":{}}}}"#,
        "MCP-Protocol-Version: 2026-07-28\r\nMcp-Method: server/discover\r\n",
    );
    assert!(discover.starts_with("HTTP/1.1 200"));
    assert!(discover.contains("supportedVersions"));
    assert!(!discover.contains("text/event-stream"));

    let tools = expose_mcp_request(
        listen_addr,
        host,
        &format!("/pdx/v1/{capability}/mcp"),
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#,
        "MCP-Protocol-Version: 2025-06-18\r\nMcp-Method: tools/list\r\n",
    );
    for name in [
        "prodex_super_start",
        "prodex_super_status",
        "prodex_super_events",
        "prodex_super_result",
        "prodex_super_cancel",
        "prodex_super_list",
    ] {
        assert!(tools.contains(name), "missing tool {name}");
    }
    assert!(!tools.contains("text/event-stream"));

    let unknown_method = expose_mcp_request(
        listen_addr,
        host,
        &format!("/pdx/v1/{capability}/mcp"),
        r#"{"jsonrpc":"2.0","id":5,"method":"unknown/method","params":{}}"#,
        "MCP-Protocol-Version: 2025-06-18\r\nMcp-Method: unknown/method\r\n",
    );
    assert!(unknown_method.starts_with("HTTP/1.1 404"));
    assert!(unknown_method.contains("-32601"));

    let malformed = expose_mcp_request(
        listen_addr,
        host,
        &format!("/pdx/v1/{capability}/mcp"),
        "{not-json",
        "",
    );
    assert!(malformed.starts_with("HTTP/1.1 400"));

    let missing_modern_metadata = expose_mcp_request(
        listen_addr,
        host,
        &format!("/pdx/v1/{capability}/mcp"),
        r#"{"jsonrpc":"2.0","id":4,"method":"tools/list","params":{}}"#,
        "MCP-Protocol-Version: 2026-07-28\r\nMcp-Method: tools/list\r\n",
    );
    assert!(missing_modern_metadata.starts_with("HTTP/1.1 400"));
    assert!(missing_modern_metadata.contains("-32020"));

    let body = r#"{"jsonrpc":"2.0","id":4,"method":"ping"}"#;
    let json_only = expose_send_test_request(
        listen_addr,
        &format!(
            "POST /pdx/v1/{capability}/mcp HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\nAccept: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        ),
    );
    assert!(json_only.starts_with("HTTP/1.1 406"));

    let notification = expose_mcp_request(
        listen_addr,
        host,
        &format!("/pdx/v1/{capability}/mcp"),
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        "MCP-Protocol-Version: 2025-06-18\r\nMcp-Method: notifications/initialized\r\n",
    );
    assert!(notification.starts_with("HTTP/1.1 202"));
    assert!(notification.split_once("\r\n\r\n").unwrap().1.is_empty());

    let call = expose_mcp_request(
        listen_addr,
        host,
        &format!("/pdx/v1/{capability}/mcp"),
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"prodex_super_list","arguments":{}}}"#,
        "MCP-Protocol-Version: 2025-06-18\r\nMcp-Method: tools/call\r\nMcp-Name: prodex_super_list\r\n",
    );
    assert!(call.starts_with("HTTP/1.1 200"));
    assert!(call.contains("structuredContent"));

    let get = expose_send_test_request(
        listen_addr,
        &format!(
            "GET /pdx/v1/{capability}/mcp HTTP/1.1\r\nHost: {host}\r\nAccept: application/json\r\n\r\n"
        ),
    );
    assert!(get.starts_with("HTTP/1.1 405"));
    assert!(!get.contains("text/event-stream"));

    let invalid = expose_mcp_request(
        listen_addr,
        host,
        "/pdx/v1/wrong-capability-000000000000000000000000/mcp",
        "not json",
        "",
    );
    assert!(invalid.starts_with("HTTP/1.1 404"));
    assert!(invalid.ends_with("not found"));

    let browser = expose_send_test_request(
        listen_addr,
        &format!("GET /expose HTTP/1.1\r\nHost: {host}\r\n\r\n"),
    );
    assert!(browser.starts_with("HTTP/1.1 404"));
    let local_browser = expose_send_test_request(
        listen_addr,
        &format!("GET /expose HTTP/1.1\r\nHost: {listen_addr}\r\n\r\n"),
    );
    assert!(local_browser.starts_with("HTTP/1.1 200"));

    server.shutdown();
    shared.pty.shutdown();
    shared.mcp.as_ref().unwrap().run_manager.shutdown();
}

#[test]
fn official_rmcp_client_discovers_and_lists_tools_over_json() {
    let capability = "abcdefghijklmnopqrstuvwxyz0123456789ABCDEFG";
    let (listen_addr, shared, mut server) = expose_start_mcp_test_server(
        capability,
        "pdxi_rmcp",
        "rmcp",
        "rmcp-test.trycloudflare.com",
    );
    let uri = format!("http://{listen_addr}/pdx/v1/{capability}/mcp");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("MCP client runtime should build");
    runtime.block_on(async {
        let transport = rmcp::transport::StreamableHttpClientTransport::from_uri(uri);
        let client = rmcp::serve_client_with_lifecycle(
            (),
            transport,
            rmcp::ClientLifecycleMode::Discover {
                preferred_versions: vec![rmcp::model::ProtocolVersion::V_2026_07_28],
            },
        )
        .await
        .expect("official MCP client should complete discovery");
        let tools = client
            .peer()
            .list_tools(None)
            .await
            .expect("official MCP client should list tools");
        assert_eq!(tools.tools.len(), 6);
        assert!(
            tools
                .tools
                .iter()
                .any(|tool| tool.name == "prodex_super_start")
        );
        let list = client
            .peer()
            .call_tool_once(rmcp::model::CallToolRequestParams::new("prodex_super_list"))
            .await
            .expect("official MCP client should call a tool");
        let rmcp::model::CallToolResponse::Complete(list) = list else {
            panic!("expected complete tool result");
        };
        assert!(list.structured_content.is_some());
        client
            .cancel()
            .await
            .expect("official MCP client should close");
    });
    shared.shutdown.store(true, Ordering::SeqCst);
    server.shutdown();
    shared.pty.shutdown();
    shared.mcp.as_ref().unwrap().run_manager.shutdown();
}

#[test]
fn mcp_capabilities_and_runs_are_isolated_between_expose_instances() {
    let capability_a = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
    let capability_b = "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB";
    let (address_a, shared_a, mut server_a) =
        expose_start_mcp_test_server(capability_a, "pdxi_a", "a", "a.trycloudflare.com");
    let (address_b, shared_b, mut server_b) =
        expose_start_mcp_test_server(capability_b, "pdxi_b", "b", "b.trycloudflare.com");
    let host_a = "a.trycloudflare.com";
    let host_b = "b.trycloudflare.com";
    let cross = expose_mcp_request(
        address_b,
        host_b,
        &format!("/pdx/v1/{capability_a}/mcp"),
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#,
        "",
    );
    assert!(cross.starts_with("HTTP/1.1 404"));
    let own = expose_mcp_request(
        address_a,
        host_a,
        &format!("/pdx/v1/{capability_a}/mcp"),
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#,
        "",
    );
    assert!(own.starts_with("HTTP/1.1 200"));
    let other = expose_mcp_request(
        address_b,
        host_b,
        &format!("/pdx/v1/{capability_b}/mcp"),
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#,
        "",
    );
    assert!(other.starts_with("HTTP/1.1 200"));

    shared_a.shutdown.store(true, Ordering::SeqCst);
    server_a.shutdown();
    assert!(TcpStream::connect(address_a).is_err());
    shared_a.pty.shutdown();
    shared_a.mcp.as_ref().unwrap().run_manager.shutdown();
    let remaining = expose_mcp_request(
        address_b,
        host_b,
        &format!("/pdx/v1/{capability_b}/mcp"),
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#,
        "",
    );
    assert!(remaining.starts_with("HTTP/1.1 200"));
    server_b.shutdown();
    shared_b.pty.shutdown();
    shared_b.mcp.as_ref().unwrap().run_manager.shutdown();
}
