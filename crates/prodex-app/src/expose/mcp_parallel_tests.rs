use super::mcp::ExposeMcpEndpoint;
use super::mcp_tests::{expose_mcp_request, expose_start_mcp_test_server_with_endpoint};
use super::run_manager::{ExposeRunManager, ExposeRunState};
use crate::ExposeArgs;
use std::fs;
use std::net::TcpStream;
use std::sync::atomic::Ordering;
use std::time::Duration;

#[test]
fn three_expose_instances_have_distinct_ports_capabilities_and_identities() {
    let root = crate::test_temp_root().join(format!("prodex-mcp-parallel-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let executable = crate::write_test_python_executable(
        &root,
        "fake-parallel-super",
        r#"#!/usr/bin/env python3
import os
import sys
import time

task = sys.stdin.read()
print("TASK=" + task, flush=True)
print("CWD=" + os.getcwd(), flush=True)
print("ARGV=" + repr(sys.argv), flush=True)
for sentinel in ("A_ONLY", "B_ONLY", "C_ONLY"):
    if os.path.exists(sentinel):
        print("SENTINEL=" + sentinel, flush=True)
if task == "hold":
    time.sleep(30)
else:
    time.sleep(0.4)
"#,
    );

    let mut instances = Vec::new();
    for (label, capability, host) in [
        (
            "api",
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            "api.trycloudflare.com",
        ),
        (
            "ui",
            "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
            "ui.trycloudflare.com",
        ),
        (
            "experiment",
            "CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC",
            "experiment.trycloudflare.com",
        ),
    ] {
        let workspace = root.join(format!("workspace-{label}"));
        fs::create_dir_all(&workspace).unwrap();
        let sentinel = match label {
            "api" => "A_ONLY",
            "ui" => "B_ONLY",
            _ => "C_ONLY",
        };
        fs::write(workspace.join(sentinel), label).unwrap();
        let crate::Commands::Super(mut defaults) =
            crate::parse_cli_command_from(["prodex", "s"]).unwrap()
        else {
            panic!("expected Super args");
        };
        defaults.local_model = Some(format!("model-{label}"));
        defaults.codex_args = vec![
            "-c".into(),
            format!(
                "model_reasoning_effort={}",
                match label {
                    "api" => "high",
                    "ui" => "max",
                    _ => "medium",
                }
            )
            .into(),
        ];
        if label == "ui" {
            defaults.no_sub_agent = true;
        } else {
            defaults.sub_agent = true;
            defaults.sub_agent_provider = Some(prodex_provider_core::ProviderId::OpenAi);
            defaults.sub_agent_model = Some(format!("sub-model-{label}"));
            defaults.sub_agent_model_reasoning_effort = Some(if label == "api" {
                prodex_cli::SubAgentReasoningEffort::Medium
            } else {
                prodex_cli::SubAgentReasoningEffort::High
            });
        }
        let manager = ExposeRunManager::new_with_executable(
            workspace.clone(),
            format!("pdxi_{label}"),
            label.to_string(),
            executable.clone(),
        );
        let endpoint = ExposeMcpEndpoint::new_with_run_manager(
            capability,
            format!("pdxi_{label}"),
            label.to_string(),
            label.to_string(),
            defaults,
            manager,
        );
        let args = ExposeArgs {
            command: None,
            cols: 80,
            rows: 24,
            max_clients: 4,
            tunnel: false,
            no_tunnel: false,
            name: Some(label.to_string()),
            invocation: prodex_cli::ExposeInvocation::SuperAlias,
            super_args: None,
        };
        let (address, shared, server) =
            expose_start_mcp_test_server_with_endpoint(endpoint, host, args);
        instances.push((label, capability, host, workspace, address, shared, server));
    }
    let addresses = instances
        .iter()
        .map(|(_, _, _, _, address, _, _)| *address)
        .collect::<Vec<_>>();
    assert_eq!(
        addresses
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        3
    );
    let mut run_ids = Vec::new();
    for (index, (label, capability, host, workspace, _, shared, _)) in instances.iter().enumerate()
    {
        let own = expose_mcp_request(
            addresses[index],
            host,
            &format!("/pdx/v1/{capability}/mcp"),
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#,
            "",
        );
        assert!(own.starts_with("HTTP/1.1 200"), "{label} should serve MCP");
        assert!(own.contains(&format!("Prodex Super — {label}")));
        assert_ne!(shared.mcp.as_ref().unwrap().instance_id, "");
        assert!(workspace.exists());
        let start = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {"name": "prodex_super_start", "arguments": {"task": "hold"}}
        })
        .to_string();
        let response = expose_mcp_request(
            addresses[index],
            host,
            &format!("/pdx/v1/{capability}/mcp"),
            &start,
            "Mcp-Method: tools/call\r\nMcp-Name: prodex_super_start\r\n",
        );
        let body = response.split_once("\r\n\r\n").unwrap().1;
        let value: serde_json::Value = serde_json::from_str(body).unwrap();
        run_ids.push(
            value["result"]["structuredContent"]["run_id"]
                .as_str()
                .unwrap()
                .to_string(),
        );
    }
    for (index, (_, capability, _host, _, _, shared, _)) in instances.iter().enumerate() {
        assert!(
            !shared
                .mcp
                .as_ref()
                .unwrap()
                .run_manager
                .status(&run_ids[index])
                .unwrap()
                .state
                .terminal()
        );
        for (other_index, (_, _, other_host, _, _, _, _)) in instances.iter().enumerate() {
            if index == other_index {
                continue;
            }
            let cross = expose_mcp_request(
                addresses[other_index],
                other_host,
                &format!("/pdx/v1/{capability}/mcp"),
                r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#,
                "",
            );
            assert!(cross.starts_with("HTTP/1.1 404"));

            let status = serde_json::json!({
                "jsonrpc": "2.0",
                "id": 5,
                "method": "tools/call",
                "params": {"name": "prodex_super_status", "arguments": {"run_id": run_ids[other_index]}}
            })
            .to_string();
            let cross_run = expose_mcp_request(
                addresses[index],
                instances[index].2,
                &format!("/pdx/v1/{capability}/mcp"),
                &status,
                "Mcp-Method: tools/call\r\nMcp-Name: prodex_super_status\r\n",
            );
            let cross_run_body: serde_json::Value =
                serde_json::from_str(cross_run.split_once("\r\n\r\n").unwrap().1).unwrap();
            assert_eq!(
                cross_run_body["result"]["structuredContent"]["state"],
                "unknown"
            );
        }
    }
    let cancel = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {"name": "prodex_super_cancel", "arguments": {"run_id": run_ids[1]}}
    })
    .to_string();
    let cancellation = expose_mcp_request(
        addresses[1],
        instances[1].2,
        &format!("/pdx/v1/{}/mcp", instances[1].1),
        &cancel,
        "Mcp-Method: tools/call\r\nMcp-Name: prodex_super_cancel\r\n",
    );
    assert!(cancellation.starts_with("HTTP/1.1 200"));
    assert!(
        !instances[0]
            .5
            .mcp
            .as_ref()
            .unwrap()
            .run_manager
            .status(&run_ids[0])
            .unwrap()
            .state
            .terminal()
    );
    assert!(
        !instances[2]
            .5
            .mcp
            .as_ref()
            .unwrap()
            .run_manager
            .status(&run_ids[2])
            .unwrap()
            .state
            .terminal()
    );
    for (index, (_, _, _, _, _, shared, _)) in instances.iter().enumerate() {
        if index != 1 {
            shared
                .mcp
                .as_ref()
                .unwrap()
                .run_manager
                .cancel(&run_ids[index]);
        }
        for _ in 0..200 {
            if shared
                .mcp
                .as_ref()
                .unwrap()
                .run_manager
                .status(&run_ids[index])
                .is_some_and(|summary| summary.state.terminal())
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    assert_eq!(
        instances[1]
            .5
            .mcp
            .as_ref()
            .unwrap()
            .run_manager
            .status(&run_ids[1])
            .unwrap()
            .state,
        ExposeRunState::Cancelled
    );
    for (index, label) in ["api", "ui", "experiment"].into_iter().enumerate() {
        let result = instances[index]
            .5
            .mcp
            .as_ref()
            .unwrap()
            .run_manager
            .result(&run_ids[index])
            .unwrap();
        assert!(
            result
                .output
                .contains(&format!("CWD={}", instances[index].3.display()))
        );
        let sentinel = match label {
            "api" => "A_ONLY",
            "ui" => "B_ONLY",
            _ => "C_ONLY",
        };
        assert!(result.output.contains(&format!("SENTINEL={sentinel}")));
        assert!(result.output.contains(&format!("model-{label}")));
        if label == "ui" {
            assert!(!result.output.contains("--sub-agent-model"));
        } else {
            assert!(result.output.contains(&format!("sub-model-{label}")));
        }
    }

    instances[0].5.shutdown.store(true, Ordering::SeqCst);
    instances[0].6.shutdown();
    assert!(TcpStream::connect(addresses[0]).is_err());
    for index in [1, 2] {
        let (_, capability, host, _, address, shared, _) = &instances[index];
        let remaining = expose_mcp_request(
            *address,
            host,
            &format!("/pdx/v1/{capability}/mcp"),
            r#"{"jsonrpc":"2.0","id":4,"method":"tools/list","params":{}}"#,
            "",
        );
        assert!(remaining.starts_with("HTTP/1.1 200"));
        shared.mcp.as_ref().unwrap().run_manager.shutdown();
    }
    for (_, _, _, _, _, shared, mut server) in instances {
        shared.shutdown.store(true, Ordering::SeqCst);
        server.shutdown();
        shared.pty.shutdown();
        shared.mcp.as_ref().unwrap().run_manager.shutdown();
    }
    let _ = fs::remove_dir_all(root);
}
