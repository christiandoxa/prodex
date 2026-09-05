use super::*;
use std::ffi::OsString;
use std::time::{Duration, Instant};

#[test]
fn kiro_acp_builds_initialize_and_session_requests() {
    let initialize = runtime_kiro_acp_initialize_request(
        0,
        RuntimeKiroAcpClientInfo {
            name: "acp-test-client",
            title: "ACP Test Client",
            version: "0.1.0",
        },
    );
    assert_eq!(initialize["method"], "initialize");
    assert_eq!(initialize["params"]["protocolVersion"], 1);
    assert_eq!(
        initialize["params"]["clientCapabilities"]["terminal"],
        false
    );

    let session_new = runtime_kiro_acp_session_new_request(1, Path::new("/tmp/work"));
    assert_eq!(session_new["method"], "session/new");
    assert_eq!(session_new["params"]["cwd"], "/tmp/work");
    assert_eq!(session_new["params"]["mcpServers"], json!([]));

    let prompt = runtime_kiro_acp_session_prompt_request(2, "session-1", "hello");
    assert_eq!(prompt["method"], "session/prompt");
    assert_eq!(prompt["params"]["sessionId"], "session-1");
    assert_eq!(
        prompt["params"]["prompt"][0],
        json!({"type":"text","text":"hello"})
    );
}

#[test]
fn kiro_acp_parses_initialize_result_from_captured_agent_line() {
    let envelope = RuntimeKiroAcpEnvelope::parse(
        r#"{"jsonrpc":"2.0","result":{"protocolVersion":1,"agentCapabilities":{"loadSession":true,"promptCapabilities":{"image":true,"audio":false,"embeddedContext":false},"mcpCapabilities":{"http":true,"sse":false},"sessionCapabilities":{},"auth":{}},"authMethods":[{"id":"kiro-login","name":"Kiro Login","description":"Run 'kiro-cli login' in terminal to authenticate. See https://kiro.dev/docs/cli/authentication/"}],"agentInfo":{"name":"Kiro CLI Agent","title":"Kiro CLI Agent","version":"2.10.0"}},"id":0}"#,
    )
    .expect("initialize envelope should parse");
    let result = envelope
        .parse_initialize_result()
        .expect("initialize result should parse");
    assert_eq!(result.protocol_version, 1);
    assert!(result.agent_capabilities.load_session);
    assert_eq!(result.agent_info.version, "2.10.0");
    assert_eq!(result.auth_methods[0].id, "kiro-login");
}

#[test]
fn kiro_acp_parses_new_session_result_and_model_ids() {
    let envelope = RuntimeKiroAcpEnvelope::parse(
        r#"{"jsonrpc":"2.0","result":{"sessionId":"00000000-0000-4000-8000-000000000001","modes":{"currentModeId":"kiro_default","availableModes":[{"id":"kiro_default","name":"kiro_default","description":"The default agent for Kiro CLI"}]},"models":{"currentModelId":"catalog-model-a","availableModels":[{"modelId":"catalog-model-a","name":"Catalog Model A"},{"modelId":"catalog-model-b","name":"Catalog Model B"}]}},"id":1}"#,
    )
    .expect("session/new envelope should parse");
    let result = envelope
        .parse_session_new_result()
        .expect("session/new result should parse");
    assert_eq!(result.session_id, "00000000-0000-4000-8000-000000000001");
    assert_eq!(
        result.model_ids(),
        vec!["catalog-model-a", "catalog-model-b"]
    );
    assert_eq!(
        result
            .modes
            .as_ref()
            .expect("modes should be present")
            .current_mode_id,
        "kiro_default"
    );
}

#[test]
fn kiro_acp_parses_error_notification_line() {
    let envelope = RuntimeKiroAcpEnvelope::parse(
        r#"{"jsonrpc":"2.0","error":{"code":-32603,"message":"Internal error","data":"Encountered an error in the response stream: An unknown error occurred: dispatch failure"},"id":2}"#,
    )
    .expect("error envelope should parse");
    let error = envelope.error.expect("error should be present");
    assert_eq!(error.code, -32603);
    assert_eq!(error.message, "Internal error");
    assert_eq!(
        error.data,
        Some(Value::String(
            "Encountered an error in the response stream: An unknown error occurred: dispatch failure"
                .to_string()
        ))
    );
}

#[test]
fn kiro_acp_accepts_string_request_ids() {
    let envelope = RuntimeKiroAcpEnvelope::parse(
        r#"{"jsonrpc":"2.0","id":"permission-1","method":"session/request_permission","params":{"options":[]}}"#,
    )
    .expect("string request id should parse");
    assert_eq!(envelope.id, Some(json!("permission-1")));
    assert_eq!(envelope.numeric_id(), None);

    let numeric = RuntimeKiroAcpEnvelope::parse(
        r#"{"jsonrpc":"2.0","id":"2","result":{"stopReason":"end_turn"}}"#,
    )
    .expect("numeric string response id should parse");
    assert_eq!(numeric.numeric_id(), Some(2));
}

#[test]
fn kiro_acp_bootstrap_reads_initialize_session_and_notifications() {
    let root = temp_dir("bootstrap");
    let fake_agent = write_fake_kiro_acp_agent(&root);
    let result = runtime_kiro_acp_bootstrap_with_command(fake_agent.as_os_str(), &root, &[])
        .expect("bootstrap should succeed");
    assert_eq!(result.initialize.agent_info.version, "2.10.0");
    assert_eq!(result.session.session_id, "session-1");
    assert_eq!(
        result.session.model_ids(),
        vec!["catalog-model-a", "catalog-model-b"]
    );
    assert_eq!(result.notifications.len(), 1);
    assert_eq!(
        result.notifications[0].method.as_deref(),
        Some("_kiro.dev/subagent/list_update")
    );
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn kiro_acp_bootstrap_times_out_and_terminates_the_agent() {
    use std::os::unix::fs::PermissionsExt;

    let root = temp_dir("bootstrap-timeout");
    let fake_agent = root.join("fake-kiro-timeout");
    fs::write(&fake_agent, "#!/bin/sh\nexec sleep 5\n").unwrap();
    let mut permissions = fs::metadata(&fake_agent).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&fake_agent, permissions).unwrap();
    let started = Instant::now();

    let error = runtime_kiro_acp_bootstrap_with_command_and_timeout(
        fake_agent.as_os_str(),
        &root,
        &[],
        Duration::from_millis(50),
    )
    .unwrap_err();

    assert!(error.to_string().contains("timed out"));
    assert!(started.elapsed() < Duration::from_secs(2));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn kiro_acp_model_catalog_maps_session_models() {
    let session = RuntimeKiroAcpNewSessionResult {
        session_id: "session-1".to_string(),
        modes: None,
        models: Some(RuntimeKiroAcpModelState {
            current_model_id: "catalog-model-a".to_string(),
            available_models: vec![
                RuntimeKiroAcpModelInfo {
                    model_id: "catalog-model-a".to_string(),
                    name: "Catalog Model A".to_string(),
                },
                RuntimeKiroAcpModelInfo {
                    model_id: "catalog-model-b".to_string(),
                    name: "Catalog Model B".to_string(),
                },
            ],
        }),
    };
    let catalog = runtime_kiro_acp_model_catalog(&session).unwrap();
    assert_eq!(catalog.len(), 2);
    assert_eq!(catalog[0]["id"], "catalog-model-a");
    assert_eq!(catalog[0]["owned_by"], "kiro-cli");
}

#[test]
fn kiro_acp_model_catalog_rejects_missing_dynamic_models() {
    let session = RuntimeKiroAcpNewSessionResult {
        session_id: "session-empty".to_string(),
        modes: None,
        models: None,
    };

    let error = runtime_kiro_acp_model_catalog(&session).unwrap_err();

    assert!(error.to_string().contains("no usable models"));
}

#[test]
fn kiro_acp_model_catalog_rejects_oversized_sessions() {
    let session = RuntimeKiroAcpNewSessionResult {
        session_id: "session-example".to_string(),
        modes: None,
        models: Some(RuntimeKiroAcpModelState {
            current_model_id: "model-0".to_string(),
            available_models: (0..=prodex_provider_core::PROVIDER_MODEL_CATALOG_HARD_LIMIT)
                .map(|index| RuntimeKiroAcpModelInfo {
                    model_id: format!("model-{index}"),
                    name: format!("Model {index}"),
                })
                .collect(),
        }),
    };

    let error = runtime_kiro_acp_model_catalog(&session).unwrap_err();

    assert!(error.to_string().contains("hard limit of 1024 entries"));
}

#[test]
fn kiro_acp_model_catalog_dedupes_case_insensitively() {
    let session = RuntimeKiroAcpNewSessionResult {
        session_id: "session-dedupe".to_string(),
        modes: None,
        models: Some(RuntimeKiroAcpModelState {
            current_model_id: "catalog-model-a".to_string(),
            available_models: vec![
                RuntimeKiroAcpModelInfo {
                    model_id: "Catalog-Model-A".to_string(),
                    name: "Catalog Model A".to_string(),
                },
                RuntimeKiroAcpModelInfo {
                    model_id: "catalog-model-a".to_string(),
                    name: "Duplicate".to_string(),
                },
                RuntimeKiroAcpModelInfo {
                    model_id: "catalog-model-b".to_string(),
                    name: "Catalog Model B".to_string(),
                },
            ],
        }),
    };

    let catalog = runtime_kiro_acp_model_catalog(&session).unwrap();

    assert_eq!(
        catalog
            .iter()
            .filter_map(|model| model.get("id").and_then(serde_json::Value::as_str))
            .collect::<Vec<_>>(),
        ["Catalog-Model-A", "catalog-model-b"]
    );
}

#[test]
fn kiro_acp_prompt_turn_sends_prompt_after_session_bootstrap() {
    let root = temp_dir("prompt-turn");
    let fake_agent = write_fake_kiro_prompt_agent(&root);
    let result = runtime_kiro_acp_prompt_turn_with_command(
        fake_agent.as_os_str(),
        &root,
        &[],
        "hello from prodex",
    )
    .expect("prompt turn should succeed");
    assert_eq!(result.initialize.agent_info.version, "2.10.0");
    assert_eq!(result.session.session_id, "session-1");
    assert_eq!(result.prompt_response.id, Some(json!(2)));
    assert_eq!(
        result.prompt_response.result,
        Some(json!({"status":"completed"}))
    );
    assert_eq!(result.notifications.len(), 1);
    assert_eq!(
        result.notifications[0].method.as_deref(),
        Some("_kiro.dev/metadata")
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn kiro_acp_prompt_turn_rejects_unsupported_server_requests() {
    let root = temp_dir("prompt-turn-server-request");
    let fake_agent = write_fake_kiro_prompt_agent(&root);

    let result = runtime_kiro_acp_prompt_turn_with_command(
        fake_agent.as_os_str(),
        &root,
        &[(OsString::from("SERVER_REQUEST"), OsString::from("1"))],
        "hello from prodex",
    )
    .expect("unsupported server request should receive an explicit error");

    assert_eq!(result.prompt_response.id, Some(json!(2)));
    assert!(result.notifications.iter().any(|notification| {
        notification.id == Some(json!(9))
            && notification.method.as_deref() == Some("fs/read_text_file")
    }));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn kiro_acp_sub_agent_selects_one_time_permission() {
    let root = temp_dir("prompt-turn-permission");
    let fake_agent = write_fake_kiro_prompt_agent(&root);
    let _marker = crate::test_support::TestEnvVarGuard::set("PRODEX_SUB_AGENT", "1");

    let result = runtime_kiro_acp_prompt_turn_with_command(
        fake_agent.as_os_str(),
        &root,
        &[
            (OsString::from("PRODEX_SUB_AGENT"), OsString::from("1")),
            (OsString::from("SERVER_PERMISSION"), OsString::from("1")),
        ],
        "hello from prodex",
    )
    .expect("sub-agent permission request should be answered");

    assert_eq!(result.prompt_response.id, Some(json!(2)));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn kiro_acp_prompt_turn_does_not_wait_for_agent_exit() {
    let root = temp_dir("prompt-turn-linger");
    let fake_agent = write_fake_kiro_prompt_agent(&root);
    let started = Instant::now();

    runtime_kiro_acp_prompt_turn_with_command(
        fake_agent.as_os_str(),
        &root,
        &[(OsString::from("LINGER_AFTER_RESPONSE"), OsString::from("1"))],
        "hello from prodex",
    )
    .expect("prompt turn should stop after the terminal response");

    assert!(started.elapsed() < Duration::from_secs(2));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn kiro_acp_internal_activity_smoke_is_non_executable_and_does_not_reconnect() {
    let root = temp_dir("activity-smoke");
    let fake_agent = write_fake_kiro_activity_agent(&root);
    let started = Instant::now();

    let turn = runtime_kiro_acp_prompt_turn_with_command(
        fake_agent.as_os_str(),
        &root,
        &[],
        "inspect one file",
    )
    .expect("local ACP activity smoke should complete");
    let response = runtime_kiro_acp_responses_value_from_prompt_turn(&turn, 11);
    let serialized = serde_json::to_string(&response).unwrap();

    assert!(started.elapsed() < Duration::from_secs(2));
    assert_eq!(turn.prompt_response.id, Some(json!(2)));
    assert!(serialized.contains("final answer"));
    assert!(serialized.contains("phase=started"));
    assert!(serialized.contains("phase=completed"));
    assert!(!serialized.contains("function_call"));
    assert!(!serialized.contains("tool_calls"));
    assert!(!serialized.contains("/home/test-user"));
    assert_eq!(
        fs::read_to_string(root.join("activity-agent-invocations"))
            .unwrap()
            .trim_end(),
        "1"
    );
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn kiro_acp_prompt_turn_times_out_and_terminates_the_agent() {
    use std::os::unix::fs::PermissionsExt;

    let root = temp_dir("prompt-turn-timeout");
    let fake_agent = root.join("fake-kiro-timeout");
    fs::write(&fake_agent, "#!/bin/sh\nexec sleep 5\n").unwrap();
    let mut permissions = fs::metadata(&fake_agent).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&fake_agent, permissions).unwrap();
    let started = Instant::now();

    let error = runtime_kiro_acp_prompt_turn_with_command_and_options_and_timeout(
        fake_agent.as_os_str(),
        &root,
        &[],
        None,
        None,
        "hello from prodex",
        Duration::from_millis(50),
    )
    .unwrap_err();

    assert!(error.to_string().contains("timed out"));
    assert!(started.elapsed() < Duration::from_secs(2));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn kiro_acp_prompt_turn_activity_resets_idle_timeout() {
    let root = temp_dir("prompt-turn-active");
    let fake_agent = write_fake_kiro_prompt_agent(&root);
    let started = Instant::now();

    let result = runtime_kiro_acp_prompt_turn_with_command_and_options_and_timeout(
        fake_agent.as_os_str(),
        &root,
        &[(OsString::from("SLOW_ACTIVITY"), OsString::from("1"))],
        None,
        None,
        "hello from prodex",
        Duration::from_millis(500),
    )
    .expect("active prompt turn should not hit an absolute deadline");

    assert_eq!(result.prompt_response.id, Some(json!(2)));
    assert!(started.elapsed() > Duration::from_millis(500));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn kiro_acp_prompt_turn_passes_selected_model_to_agent() {
    let root = temp_dir("prompt-turn-model");
    let fake_agent = write_fake_kiro_prompt_agent(&root);
    runtime_kiro_acp_prompt_turn_with_command_and_options(
        fake_agent.as_os_str(),
        &root,
        &[(OsString::from("EXPECT_MODEL"), OsString::from("1"))],
        Some("catalog-model-b"),
        Some("medium"),
        "hello from prodex",
    )
    .expect("selected model should be passed to Kiro ACP");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn kiro_acp_parses_prompt_response_stop_reason() {
    let envelope = RuntimeKiroAcpEnvelope::parse(
        r#"{"jsonrpc":"2.0","id":2,"result":{"stopReason":"end_turn"}}"#,
    )
    .expect("prompt response envelope should parse");
    let response = envelope
        .parse_prompt_response()
        .expect("prompt response should parse");
    assert_eq!(response.stop_reason, "end_turn");
}

#[test]
fn kiro_acp_parses_session_update_agent_message_chunk() {
    let envelope = RuntimeKiroAcpEnvelope::parse(
        r#"{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"sess_abc123def456","update":{"sessionUpdate":"agent_message_chunk","messageId":"msg_agent_c42b9","content":{"type":"text","text":"I'll analyze your code for potential issues. Let me examine it..."}}}}"#,
    )
    .expect("session/update envelope should parse");
    let notification = envelope
        .parse_session_notification()
        .expect("session/update should parse");
    assert_eq!(notification.session_id, "sess_abc123def456");
    match notification.update {
        RuntimeKiroAcpSessionUpdate::AgentMessageChunk {
            message_id,
            content,
        } => {
            assert_eq!(message_id.as_deref(), Some("msg_agent_c42b9"));
            assert_eq!(content["type"], "text");
        }
        other => panic!("expected agent message chunk, got {other:?}"),
    }
}

#[test]
fn kiro_acp_parses_session_update_usage_update() {
    let envelope = RuntimeKiroAcpEnvelope::parse(
        r#"{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"sess_abc123def456","update":{"sessionUpdate":"usage_update","used":53000,"size":200000,"cost":{"amount":0.045,"currency":"USD"}}}}"#,
    )
    .expect("usage update envelope should parse");
    let notification = envelope
        .parse_session_notification()
        .expect("usage update should parse");
    match notification.update {
        RuntimeKiroAcpSessionUpdate::UsageUpdate { used, size, cost } => {
            assert_eq!(used, 53_000);
            assert_eq!(size, 200_000);
            assert_eq!(cost.expect("cost should exist").currency, "USD");
        }
        other => panic!("expected usage update, got {other:?}"),
    }
}

#[test]
fn kiro_acp_parses_session_update_tool_call() {
    let envelope = RuntimeKiroAcpEnvelope::parse(
        r#"{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"sess_abc123def456","update":{"sessionUpdate":"tool_call","toolCallId":"call_1","title":"Read file","status":"in_progress","kind":"read","content":[{"type":"resource_link","uri":"file:///tmp/main.py"}],"rawInput":{"path":"/tmp/main.py"},"rawOutput":null,"locations":[]}}}"#,
    )
    .expect("tool call envelope should parse");
    let notification = envelope
        .parse_session_notification()
        .expect("tool call should parse");
    match notification.update {
        RuntimeKiroAcpSessionUpdate::ToolCall {
            tool_call_id,
            title,
            status,
            kind,
            ..
        } => {
            assert_eq!(tool_call_id, "call_1");
            assert_eq!(title, "Read file");
            assert_eq!(status.as_deref(), Some("in_progress"));
            assert_eq!(kind.as_deref(), Some("read"));
        }
        other => panic!("expected tool call, got {other:?}"),
    }
}

#[test]
fn kiro_acp_parses_tool_call_without_initial_status() {
    let envelope = RuntimeKiroAcpEnvelope::parse(
        r#"{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"sess_abc123def456","update":{"sessionUpdate":"tool_call","toolCallId":"call_1","title":"Read file","kind":"read","rawInput":{"path":"/tmp/main.py"}}}}"#,
    )
    .expect("tool call envelope should parse without status");
    let notification = envelope
        .parse_session_notification()
        .expect("tool call without status should parse");
    match notification.update {
        RuntimeKiroAcpSessionUpdate::ToolCall { status, .. } => {
            assert_eq!(status, None);
        }
        other => panic!("expected tool call, got {other:?}"),
    }
}
