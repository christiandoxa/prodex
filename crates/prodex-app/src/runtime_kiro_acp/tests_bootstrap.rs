use super::*;

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
        r#"{"jsonrpc":"2.0","result":{"sessionId":"43dc1cdb-4376-442a-afa8-b12238274893","modes":{"currentModeId":"kiro_default","availableModes":[{"id":"kiro_default","name":"kiro_default","description":"The default agent for Kiro CLI"}]},"models":{"currentModelId":"claude-sonnet-4","availableModels":[{"modelId":"claude-sonnet-4","name":"claude-sonnet-4"},{"modelId":"claude-sonnet-4.5","name":"claude-sonnet-4.5"}]}},"id":1}"#,
    )
    .expect("session/new envelope should parse");
    let result = envelope
        .parse_session_new_result()
        .expect("session/new result should parse");
    assert_eq!(result.session_id, "43dc1cdb-4376-442a-afa8-b12238274893");
    assert_eq!(
        result.model_ids(),
        vec!["claude-sonnet-4", "claude-sonnet-4.5"]
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
fn kiro_acp_bootstrap_reads_initialize_session_and_notifications() {
    let root = temp_dir("bootstrap");
    let fake_agent = write_fake_kiro_acp_agent(&root);
    let result = runtime_kiro_acp_bootstrap_with_command(fake_agent.as_os_str(), &root, &[])
        .expect("bootstrap should succeed");
    assert_eq!(result.initialize.agent_info.version, "2.10.0");
    assert_eq!(result.session.session_id, "session-1");
    assert_eq!(
        result.session.model_ids(),
        vec!["claude-sonnet-4", "claude-sonnet-4.5"]
    );
    assert_eq!(result.notifications.len(), 1);
    assert_eq!(
        result.notifications[0].method.as_deref(),
        Some("_kiro.dev/subagent/list_update")
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn kiro_acp_model_catalog_maps_session_models() {
    let session = RuntimeKiroAcpNewSessionResult {
        session_id: "session-1".to_string(),
        modes: None,
        models: Some(RuntimeKiroAcpModelState {
            current_model_id: "claude-sonnet-4".to_string(),
            available_models: vec![
                RuntimeKiroAcpModelInfo {
                    model_id: "claude-sonnet-4".to_string(),
                    name: "claude-sonnet-4".to_string(),
                },
                RuntimeKiroAcpModelInfo {
                    model_id: "claude-sonnet-4.5".to_string(),
                    name: "claude-sonnet-4.5".to_string(),
                },
            ],
        }),
    };
    let catalog = runtime_kiro_acp_model_catalog(&session);
    assert_eq!(catalog.len(), 2);
    assert_eq!(catalog[0]["id"], "claude-sonnet-4");
    assert_eq!(catalog[0]["owned_by"], "kiro-cli");
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
    assert_eq!(result.prompt_response.id, Some(2));
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
            assert_eq!(status, "in_progress");
            assert_eq!(kind.as_deref(), Some("read"));
        }
        other => panic!("expected tool call, got {other:?}"),
    }
}
