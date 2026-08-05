use super::*;

#[test]
fn kiro_acp_prompt_turn_translates_text_response() {
    let turn = RuntimeKiroAcpPromptTurnResult {
        initialize: RuntimeKiroAcpInitializeResult {
            protocol_version: 1,
            agent_capabilities: RuntimeKiroAcpAgentCapabilities {
                load_session: true,
                prompt_capabilities: RuntimeKiroAcpPromptCapabilities {
                    image: false,
                    audio: false,
                    embedded_context: false,
                },
                mcp_capabilities: RuntimeKiroAcpMcpCapabilities {
                    http: true,
                    sse: false,
                },
                session_capabilities: json!({}),
                auth: json!({}),
            },
            auth_methods: Vec::new(),
            agent_info: RuntimeKiroAcpAgentInfo {
                name: "Kiro CLI Agent".to_string(),
                title: "Kiro CLI Agent".to_string(),
                version: "2.10.0".to_string(),
            },
        },
        session: RuntimeKiroAcpNewSessionResult {
            session_id: "session-1".to_string(),
            modes: None,
            models: Some(RuntimeKiroAcpModelState {
                current_model_id: "claude-sonnet-4".to_string(),
                available_models: vec![RuntimeKiroAcpModelInfo {
                    model_id: "claude-sonnet-4".to_string(),
                    name: "claude-sonnet-4".to_string(),
                }],
            }),
        },
        prompt_response: RuntimeKiroAcpEnvelope::parse(
            r#"{"jsonrpc":"2.0","id":2,"result":{"stopReason":"end_turn"}}"#,
        )
        .expect("prompt response should parse"),
        notifications: vec![
            RuntimeKiroAcpEnvelope::parse(
                r#"{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"session-1","update":{"sessionUpdate":"agent_message_chunk","messageId":"msg_1","content":{"type":"text","text":"hello "}}}}"#,
            )
            .expect("text chunk should parse"),
            RuntimeKiroAcpEnvelope::parse(
                r#"{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"session-1","update":{"sessionUpdate":"agent_message_chunk","messageId":"msg_1","content":{"type":"text","text":"world"}}}}"#,
            )
            .expect("text chunk should parse"),
            RuntimeKiroAcpEnvelope::parse(
                r#"{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"session-1","update":{"sessionUpdate":"agent_thought_chunk","messageId":"thought_1","content":{"type":"text","text":"inspect code"}}}}"#,
            )
            .expect("thought chunk should parse"),
        ],
    };

    let response = runtime_kiro_acp_responses_value_from_prompt_turn(&turn, 7);
    assert_eq!(response["id"], "resp_kiro_7");
    assert_eq!(response["object"], "response");
    assert_eq!(response["model"], "claude-sonnet-4");
    assert_eq!(response["output"][0]["type"], "message");
    assert_eq!(response["output"][0]["content"][0]["type"], "output_text");
    assert_eq!(response["output"][0]["content"][0]["text"], "hello world");
    assert_eq!(
        response["metadata"]["kiro"]["reasoning_content"],
        "inspect code"
    );
    assert_eq!(response["metadata"]["kiro"]["stop_reason"], "end_turn");
}

#[test]
fn kiro_acp_prompt_turn_translates_tool_activity_as_non_executable_progress() {
    let turn = RuntimeKiroAcpPromptTurnResult {
        initialize: RuntimeKiroAcpInitializeResult {
            protocol_version: 1,
            agent_capabilities: RuntimeKiroAcpAgentCapabilities {
                load_session: true,
                prompt_capabilities: RuntimeKiroAcpPromptCapabilities {
                    image: false,
                    audio: false,
                    embedded_context: false,
                },
                mcp_capabilities: RuntimeKiroAcpMcpCapabilities {
                    http: true,
                    sse: false,
                },
                session_capabilities: json!({}),
                auth: json!({}),
            },
            auth_methods: Vec::new(),
            agent_info: RuntimeKiroAcpAgentInfo {
                name: "Kiro CLI Agent".to_string(),
                title: "Kiro CLI Agent".to_string(),
                version: "2.10.0".to_string(),
            },
        },
        session: RuntimeKiroAcpNewSessionResult {
            session_id: "session-1".to_string(),
            modes: None,
            models: Some(RuntimeKiroAcpModelState {
                current_model_id: "claude-sonnet-4".to_string(),
                available_models: vec![RuntimeKiroAcpModelInfo {
                    model_id: "claude-sonnet-4".to_string(),
                    name: "claude-sonnet-4".to_string(),
                }],
            }),
        },
        prompt_response: RuntimeKiroAcpEnvelope::parse(
            r#"{"jsonrpc":"2.0","id":2,"result":{"stopReason":"end_turn"}}"#,
        )
        .expect("prompt response should parse"),
        notifications: vec![
            RuntimeKiroAcpEnvelope::parse(
                r#"{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"session-1","update":{"sessionUpdate":"tool_call","toolCallId":"call_1","title":"Read file","status":"in_progress","kind":"read","rawInput":{"path":"/tmp/main.py"}}}}"#,
            )
            .expect("tool call should parse"),
            RuntimeKiroAcpEnvelope::parse(
                r#"{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"session-1","update":{"sessionUpdate":"tool_call_update","toolCallId":"call_1","status":"completed","rawOutput":{"content":"ok"}}}}"#,
            )
            .expect("tool call update should parse"),
        ],
    };

    let response = runtime_kiro_acp_responses_value_from_prompt_turn(&turn, 8);
    assert_eq!(response["output"][0]["type"], "message");
    let progress = response["output"][0]["content"][0]["text"]
        .as_str()
        .unwrap();
    let started = progress.find("phase=started").unwrap();
    let completed = progress.find("phase=completed").unwrap();
    assert!(started < completed);
    assert!(progress.contains("Read file"));
    let activities = response["metadata"]["kiro"]["tool_activities"]
        .as_array()
        .unwrap();
    assert_eq!(activities.len(), 2);
    assert_eq!(activities[0]["name"], "Read file");
    assert_eq!(activities[0]["phase"], "started");
    assert_eq!(activities[1]["name"], "Read file");
    assert_eq!(activities[1]["phase"], "completed");
    assert_eq!(activities[1]["details_omitted"], true);
    let serialized = serde_json::to_string(&response).unwrap();
    assert!(!serialized.contains("function_call"));
    assert!(!serialized.contains("/tmp/main.py"));
    assert!(!serialized.contains("rawOutput"));

    let chat = runtime_kiro_acp_chat_assistant_messages_from_prompt_turn(&turn);
    assert_eq!(chat.len(), 1);
    assert!(chat[0].get("tool_calls").is_none());
    assert!(chat[0]["content"].as_str().unwrap().contains("Read file"));
}

#[test]
fn kiro_acp_prompt_turn_carries_usage_and_incomplete_metadata() {
    let turn = RuntimeKiroAcpPromptTurnResult {
        initialize: RuntimeKiroAcpInitializeResult {
            protocol_version: 1,
            agent_capabilities: RuntimeKiroAcpAgentCapabilities {
                load_session: true,
                prompt_capabilities: RuntimeKiroAcpPromptCapabilities {
                    image: false,
                    audio: false,
                    embedded_context: false,
                },
                mcp_capabilities: RuntimeKiroAcpMcpCapabilities {
                    http: true,
                    sse: false,
                },
                session_capabilities: json!({}),
                auth: json!({}),
            },
            auth_methods: Vec::new(),
            agent_info: RuntimeKiroAcpAgentInfo {
                name: "Kiro CLI Agent".to_string(),
                title: "Kiro CLI Agent".to_string(),
                version: "2.10.0".to_string(),
            },
        },
        session: RuntimeKiroAcpNewSessionResult {
            session_id: "session-1".to_string(),
            modes: None,
            models: Some(RuntimeKiroAcpModelState {
                current_model_id: "claude-sonnet-4".to_string(),
                available_models: vec![RuntimeKiroAcpModelInfo {
                    model_id: "claude-sonnet-4".to_string(),
                    name: "claude-sonnet-4".to_string(),
                }],
            }),
        },
        prompt_response: RuntimeKiroAcpEnvelope::parse(
            r#"{"jsonrpc":"2.0","id":2,"result":{"stopReason":"max_tokens"}}"#,
        )
        .expect("prompt response should parse"),
        notifications: vec![RuntimeKiroAcpEnvelope::parse(
            r#"{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"session-1","update":{"sessionUpdate":"usage_update","used":53000,"size":200000,"cost":{"amount":0.045,"currency":"USD"}}}}"#,
        )
        .expect("usage update should parse")],
    };

    let response = runtime_kiro_acp_responses_value_from_prompt_turn(&turn, 9);
    assert_eq!(response["status"], "incomplete");
    assert_eq!(
        response["incomplete_details"]["reason"],
        "max_output_tokens"
    );
    assert_eq!(response["metadata"]["kiro"]["usage_update"]["used"], 53_000);
    assert_eq!(
        response["metadata"]["kiro"]["usage_update"]["size"],
        200_000
    );
    assert_eq!(
        response["metadata"]["kiro"]["usage_update"]["remaining"],
        147_000
    );
    assert_eq!(
        response["metadata"]["kiro"]["usage_update"]["cost"]["currency"],
        "USD"
    );
}
