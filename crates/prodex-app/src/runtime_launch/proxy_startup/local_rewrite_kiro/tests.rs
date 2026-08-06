use super::*;
use crate::runtime_anthropic::translate_runtime_anthropic_messages_request;
use crate::runtime_launch::proxy_startup::chat_compatible_rewrite::runtime_provider_chat_compatible_request_body;
use crate::runtime_launch::proxy_startup::provider_bridge::RuntimeProviderBridgeKind;
use serde_json::{Value, json};
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn kiro_models_endpoint_augments_canonical_models_and_serves_each_entry() {
    let auth = RuntimeKiroProfileAuth {
        profile_name: "kiro-example".to_string(),
        codex_home: PathBuf::from("/synthetic/kiro-home"),
        model_catalog: vec![json!({
            "id": "account-only-model",
            "name": "Account Only Model",
            "object": "model",
            "owned_by": "kiro-cli"
        })],
        command: None,
    };

    let list = runtime_kiro_models_buffered_response(&auth, "GET", "/v1/models").unwrap();
    let body: Value = serde_json::from_slice(&list.body).unwrap();
    let models = body["data"].as_array().unwrap();
    assert_eq!(models[0]["id"], "auto");
    assert!(
        models
            .iter()
            .any(|model| model["id"] == "account-only-model")
    );

    for model_id in ["auto", "account-only-model"] {
        let response =
            runtime_kiro_models_buffered_response(&auth, "GET", &format!("/v1/models/{model_id}"))
                .unwrap();
        assert_eq!(response.status, 200);
        let model: Value = serde_json::from_slice(&response.body).unwrap();
        assert_eq!(model["id"], model_id);
    }
}

#[test]
fn oversized_kiro_catalog_only_rejects_model_routes() {
    let auth = RuntimeKiroProfileAuth {
        profile_name: "kiro-example".to_string(),
        codex_home: PathBuf::from("/synthetic/kiro-home"),
        model_catalog: (0..prodex_provider_core::PROVIDER_MODEL_CATALOG_HARD_LIMIT)
            .map(|index| json!({"id": format!("account-model-{index}")}))
            .collect(),
        command: None,
    };

    assert!(runtime_kiro_models_buffered_response(&auth, "GET", "/health").is_none());
    let response = runtime_kiro_models_buffered_response(&auth, "GET", "/v1/models").unwrap();
    let body: Value = serde_json::from_slice(&response.body).unwrap();
    assert_eq!(response.status, 503);
    assert_eq!(body["error"]["code"], "model_catalog_limit_exceeded");
}

#[test]
fn kiro_streaming_reader_times_out_while_the_worker_is_silent() {
    let (_sender, receiver) = mpsc::channel();
    let mut reader = RuntimeKiroStreamingReader {
        receiver,
        pending: Cursor::new(Vec::new()),
        finished: false,
        idle_timeout: Duration::from_millis(10),
    };

    let error = reader.read(&mut [0_u8; 1]).unwrap_err();

    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
}

#[test]
fn kiro_streaming_queue_applies_backpressure() {
    let (sender, _receiver) = mpsc::sync_channel(16);
    for _ in 0..16 {
        sender
            .try_send(RuntimeKiroStreamingChunk::End)
            .expect("queue should accept work up to its capacity");
    }

    assert!(matches!(
        sender.try_send(RuntimeKiroStreamingChunk::End),
        Err(mpsc::TrySendError::Full(_))
    ));
}

#[test]
fn kiro_streaming_internal_activity_is_ordered_non_executable_text() {
    let start = RuntimeKiroAcpEnvelope::parse(
        r#"{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"session-example","update":{"sessionUpdate":"tool_call","toolCallId":"activity-example","title":"Read file","status":"in_progress","kind":"read","rawInput":{"path":"/home/test-user/private.txt"}}}}"#,
    )
    .unwrap()
    .parse_session_notification()
    .unwrap();
    let done = RuntimeKiroAcpEnvelope::parse(
        r#"{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"session-example","update":{"sessionUpdate":"tool_call_update","toolCallId":"activity-example","status":"completed","rawOutput":{"path":"/home/test-user/private.txt"}}}}"#,
    )
    .unwrap()
    .parse_session_notification()
    .unwrap();
    let (sender, receiver) = mpsc::sync_channel(8);
    let mut state = RuntimeKiroStreamingState::new(1, Some("model-example"));

    for notification in [&start, &done] {
        super::stream::runtime_kiro_stream_notification(
            &sender,
            notification,
            &state.response_id,
            &state.chat_completion_id,
            &state.stream_model,
            state.created_at,
            &state.message_item_id,
            &mut state.sequence_number,
            &mut state.message_item_open,
            &mut state.assistant_text,
            &mut state.tool_activities,
            false,
            &mut state.chat_delta_started,
        )
        .unwrap();
    }

    let mut body = Vec::new();
    while let Ok(RuntimeKiroStreamingChunk::Data(bytes)) = receiver.try_recv() {
        body.extend(bytes);
    }
    let body = String::from_utf8(body).unwrap();
    assert!(body.find("phase=started").unwrap() < body.find("phase=completed").unwrap());
    assert!(!body.contains("function_call"));
    assert!(!body.contains("tool_calls"));
    assert!(!body.contains("arguments.delta"));
    assert!(!body.contains("/home/test-user"));
}

#[test]
fn kiro_chat_streaming_preserves_reasoning_chunks() {
    let thought = RuntimeKiroAcpEnvelope::parse(
        r#"{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"session-example","update":{"sessionUpdate":"agent_thought_chunk","messageId":"thought-example","content":{"type":"text","text":"inspect code"}}}}"#,
    )
    .unwrap()
    .parse_session_notification()
    .unwrap();
    let (sender, receiver) = mpsc::sync_channel(2);
    let mut state = RuntimeKiroStreamingState::new(1, Some("model-example"));

    super::stream::runtime_kiro_stream_notification(
        &sender,
        &thought,
        &state.response_id,
        &state.chat_completion_id,
        &state.stream_model,
        state.created_at,
        &state.message_item_id,
        &mut state.sequence_number,
        &mut state.message_item_open,
        &mut state.assistant_text,
        &mut state.tool_activities,
        true,
        &mut state.chat_delta_started,
    )
    .unwrap();

    let RuntimeKiroStreamingChunk::Data(bytes) = receiver.try_recv().unwrap() else {
        panic!("reasoning update should emit a chat chunk");
    };
    let body = String::from_utf8(bytes).unwrap();
    assert!(body.contains(r#""reasoning_content":"inspect code""#));
    assert!(body.contains(r#""role":"assistant""#));
}

#[test]
fn kiro_streaming_total_timeout_is_bounded_without_reconnect_after_output() {
    let (sender, _receiver) = mpsc::sync_channel(1);
    let (_line_sender, line_receiver) = mpsc::channel::<io::Result<String>>();
    let mut state = RuntimeKiroStreamingState::new(1, Some("model-example"));
    state.prompt_sent = true;
    let started = std::time::Instant::now();

    let error = runtime_kiro_receive_stream(
        &sender,
        &mut Vec::new(),
        line_receiver,
        "prompt",
        &mut state,
        false,
        Duration::from_secs(1),
        Duration::from_millis(10),
    )
    .unwrap_err();

    assert!(started.elapsed() < Duration::from_secs(1));
    assert!(error.to_string().contains("after output began"));
    assert!(error.to_string().contains("did not reconnect or replay"));
}

#[test]
fn kiro_responses_stream_preserves_terminal_status() {
    for (status, event_type, response) in [
        (
            "completed",
            "response.completed",
            json!({"id": "resp_1", "status": "completed"}),
        ),
        (
            "failed",
            "response.failed",
            json!({
                "id": "resp_1",
                "status": "failed",
                "error": {"code": "-1", "message": "ACP failed"}
            }),
        ),
        (
            "incomplete",
            "response.incomplete",
            json!({
                "id": "resp_1",
                "status": "incomplete",
                "incomplete_details": {
                    "reason": "max_output_tokens",
                    "message": "ACP reached the token limit"
                }
            }),
        ),
    ] {
        let (sender, receiver) = mpsc::sync_channel(4);
        let mut state = RuntimeKiroStreamingState::new(1, Some("claude-sonnet-4"));
        super::stream::runtime_kiro_send_final_stream(&sender, &response, &mut state, false)
            .expect("Kiro terminal event should be emitted");

        let mut body = Vec::new();
        while let Ok(chunk) = receiver.try_recv() {
            if let RuntimeKiroStreamingChunk::Data(bytes) = chunk {
                body.extend(bytes);
            }
        }
        let body = String::from_utf8(body).expect("Kiro SSE should be UTF-8");
        assert!(
            body.contains(&format!("event: {event_type}")),
            "{status}: {body}"
        );
        assert!(
            body.contains(&format!("\"status\":\"{status}\"")),
            "{status}: {body}"
        );
        if status != "completed" {
            assert!(
                !body.contains("event: response.completed"),
                "{status}: {body}"
            );
        }
    }
}

fn test_kiro_anthropic_request() -> RuntimeAnthropicMessagesRequest {
    translate_runtime_anthropic_messages_request(&RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages".to_string(),
        headers: vec![("anthropic-version".to_string(), "2023-06-01".to_string())],
        body: serde_json::to_vec(&json!({
            "model": "claude-sonnet-4",
            "max_tokens": 128,
            "stream": false,
            "messages": [{"role": "user", "content": "hello"}]
        }))
        .unwrap(),
    })
    .expect("Anthropic request should translate")
}

fn test_kiro_anthropic_response(value: Value) -> RuntimeLocalRewriteUpstreamResponse {
    RuntimeLocalRewriteUpstreamResponse::Buffered(RuntimeHeapTrimmedBufferedResponseParts {
        status: 200,
        headers: vec![("content-type".to_string(), b"application/json".to_vec())],
        body: serde_json::to_vec(&value).unwrap().into(),
    })
}

#[test]
fn kiro_anthropic_buffered_translation_preserves_terminal_status() {
    let request = test_kiro_anthropic_request();
    let cases = [
        (
            json!({
                "status": "failed",
                "error": {"code": "-1", "message": "ACP failed"}
            }),
            502,
            Some("ACP failed"),
            None,
        ),
        (
            json!({
                "status": "incomplete",
                "incomplete_details": {
                    "reason": "cancelled",
                    "message": "ACP stopped early"
                }
            }),
            502,
            Some("ACP stopped early"),
            None,
        ),
        (
            json!({
                "status": "incomplete",
                "incomplete_details": {
                    "reason": "max_output_tokens",
                    "message": "ACP reached the token limit"
                },
                "output": [{
                    "type": "message",
                    "content": [{"type": "output_text", "text": "limited"}]
                }]
            }),
            200,
            None,
            Some("max_tokens"),
        ),
        (
            json!({
                "status": "completed",
                "output": [{
                    "type": "message",
                    "content": [{"type": "output_text", "text": "done"}]
                }]
            }),
            200,
            None,
            Some("end_turn"),
        ),
    ];

    for (value, expected_status, error_message, stop_reason) in cases {
        let response = test_kiro_anthropic_response(value);
        let parts = runtime_kiro_anthropic_message_parts_from_response(&response, &request);
        assert_eq!(parts.status, expected_status);
        let body: Value = serde_json::from_slice(&parts.body).expect("Anthropic body should parse");
        if let Some(error_message) = error_message {
            assert_eq!(body["type"], "error");
            assert_eq!(body["error"]["message"], error_message);
        } else {
            assert_eq!(body["type"], "message");
            assert_eq!(body["stop_reason"], stop_reason.unwrap());
        }
    }
}

fn write_fake_kiro_compact_agent(root: &Path) -> std::path::PathBuf {
    crate::test_support::write_test_python_executable(
        root,
        "fake-kiro-compact",
        r#"#!/usr/bin/env python3
import json, sys
first = json.loads(sys.stdin.readline())
second = json.loads(sys.stdin.readline())
assert first["method"] == "initialize"
assert second["method"] == "session/new"
print(json.dumps({"jsonrpc":"2.0","result":{"protocolVersion":1,"agentCapabilities":{"loadSession":True,"promptCapabilities":{"image":False,"audio":False,"embeddedContext":False},"mcpCapabilities":{"http":True,"sse":False},"sessionCapabilities":{},"auth":{}},"authMethods":[],"agentInfo":{"name":"Kiro CLI Agent","title":"Kiro CLI Agent","version":"2.10.0"}},"id":0}), flush=True)
print(json.dumps({"jsonrpc":"2.0","result":{"sessionId":"session-1","models":{"currentModelId":"claude-sonnet-4","availableModels":[{"modelId":"claude-sonnet-4","name":"claude-sonnet-4"}]}},"id":1}), flush=True)
third = json.loads(sys.stdin.readline())
assert third["method"] == "session/prompt"
prompt = third["params"]["prompt"][0]["text"]
print(prompt, file=sys.stderr, flush=True)
print(json.dumps({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"session-1","update":{"sessionUpdate":"agent_message_chunk","messageId":"msg_1","content":{"type":"text","text":"FAKE NATIVE KIRO COMPACT SUMMARY"}}}}), flush=True)
print(json.dumps({"jsonrpc":"2.0","result":{"stopReason":"end_turn"},"id":2}), flush=True)
"#,
    )
}

#[test]
fn kiro_chat_tool_message_maps_only_to_function_call_output() {
    let items = runtime_kiro_responses_items_from_chat_message(&json!({
        "role": "tool",
        "tool_call_id": "call_1",
        "content": "ok",
    }));

    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["type"], "function_call_output");
    assert_eq!(items[0]["call_id"], "call_1");
    assert_eq!(items[0]["output"], "ok");
}

#[test]
fn kiro_chat_assistant_legacy_function_call_maps_to_function_call_item() {
    let items = runtime_kiro_responses_items_from_chat_message(&json!({
        "role": "assistant",
        "content": null,
        "function_call": {
            "name": "read_file",
            "arguments": "{\"path\":\"/tmp/main.py\"}"
        }
    }));

    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["type"], "function_call");
    assert_eq!(items[0]["call_id"], "read_file");
    assert_eq!(items[0]["name"], "read_file");
    assert_eq!(items[0]["arguments"], "{\"path\":\"/tmp/main.py\"}");
}

#[test]
fn kiro_chat_legacy_function_role_maps_to_function_call_output() {
    let items = runtime_kiro_responses_items_from_chat_message(&json!({
        "role": "function",
        "name": "read_file",
        "content": "ok",
    }));

    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["type"], "function_call_output");
    assert_eq!(items[0]["call_id"], "read_file");
    assert_eq!(items[0]["output"], "ok");
}

#[test]
fn kiro_messages_translation_preserves_anthropic_user_text() {
    let request = crate::RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages".to_string(),
        headers: vec![("anthropic-version".to_string(), "2023-06-01".to_string())],
        body: serde_json::to_vec(&json!({
            "model": "claude-sonnet-4",
            "max_tokens": 128,
            "stream": false,
            "messages": [{
                "role": "user",
                "content": "start tool"
            }]
        }))
        .unwrap(),
    };
    let translated_request =
        translate_runtime_anthropic_messages_request(&request).expect("anthropic request");
    let conversations = RuntimeDeepSeekConversationStore::default();
    let translated = runtime_provider_chat_compatible_request_body(
        &translated_request.translated_request.body,
        &conversations,
        RuntimeProviderBridgeKind::Kiro,
        "",
        false,
        Default::default(),
    )
    .expect("kiro translated request");
    let request_body = String::from_utf8(translated_request.translated_request.body.clone())
        .expect("translated request should be utf8");
    let messages_json = serde_json::to_string(&translated.messages).unwrap();
    assert!(
        messages_json.contains("start tool"),
        "{request_body}\n{messages_json}"
    );
    assert!(runtime_kiro_prompt_from_messages(&translated.messages).contains("start tool"));
}

#[test]
fn kiro_translation_accepts_codex_web_search_tool() {
    let body = serde_json::to_vec(&json!({
        "model": "auto",
        "stream": true,
        "input": [{
            "type": "message",
            "role": "user",
            "content": [{"type": "input_text", "text": "hello"}]
        }],
        "tools": [{"type": "web_search", "search_context_size": "medium"}]
    }))
    .unwrap();
    let translated = runtime_provider_chat_compatible_request_body(
        &body,
        &RuntimeDeepSeekConversationStore::default(),
        RuntimeProviderBridgeKind::Kiro,
        "",
        false,
        runtime_kiro_rewrite_options(),
    )
    .expect("Kiro should accept Codex web search metadata");
    assert_eq!(translated.messages[0]["content"], "hello");
}

#[test]
fn kiro_chat_request_tolerates_default_noop_controls() {
    let translated = match runtime_kiro_request_body_for_endpoint(
        ProviderEndpoint::ChatCompletions,
        serde_json::to_vec(&json!({
            "model": "claude-sonnet-4",
            "messages": [{
                "role": "user",
                "content": "hello"
            }],
            "stop": [],
            "temperature": 1,
            "top_p": 1,
            "presence_penalty": 0,
            "frequency_penalty": 0,
            "parallel_tool_calls": true,
            "user": "user-123"
        }))
        .unwrap(),
    ) {
        Ok(translated) => translated,
        Err(_) => panic!("default chat controls should be ignored"),
    };
    let translated: Value = serde_json::from_slice(&translated).unwrap();
    assert!(translated.get("stop").is_none());
    assert!(translated.get("temperature").is_none());
    assert!(translated.get("top_p").is_none());
    assert!(translated.get("presence_penalty").is_none());
    assert!(translated.get("frequency_penalty").is_none());
    assert!(translated.get("parallel_tool_calls").is_none());
    assert!(translated.get("user").is_none());
    assert_eq!(translated["input"][0]["role"], "user");
}

#[test]
fn kiro_chat_request_rejects_semantic_parallel_tool_calls_control() {
    let error = match runtime_kiro_request_body_for_endpoint(
        ProviderEndpoint::ChatCompletions,
        serde_json::to_vec(&json!({
            "model": "claude-sonnet-4",
            "messages": [{
                "role": "user",
                "content": "hello"
            }],
            "parallel_tool_calls": false
        }))
        .unwrap(),
    ) {
        Ok(_) => panic!("parallel_tool_calls=false should still fail"),
        Err(error) => error,
    };
    let body: Value = serde_json::from_slice(&error.body).unwrap();
    assert_eq!(body["error"]["code"], "unsupported_parallel_tool_calls");
}

#[test]
fn kiro_chat_request_rejects_unenforceable_token_limit_controls() {
    let error = match runtime_kiro_request_body_for_endpoint(
        ProviderEndpoint::ChatCompletions,
        serde_json::to_vec(&json!({
            "model": "claude-sonnet-4",
            "messages": [{
                "role": "user",
                "content": "hello"
            }],
            "max_output_tokens": 64,
            "max_tokens": 32,
            "max_completion_tokens": 16
        }))
        .unwrap(),
    ) {
        Ok(_) => panic!("unenforceable token-limit controls should fail"),
        Err(error) => error,
    };
    let body: Value = serde_json::from_slice(&error.body).unwrap();
    assert_eq!(body["error"]["code"], "unsupported_token_limit");
}

#[test]
fn kiro_chat_request_rejects_invalid_token_limit_controls() {
    let error = match runtime_kiro_request_body_for_endpoint(
        ProviderEndpoint::ChatCompletions,
        serde_json::to_vec(&json!({
            "model": "claude-sonnet-4",
            "messages": [{
                "role": "user",
                "content": "hello"
            }],
            "max_tokens": 0
        }))
        .unwrap(),
    ) {
        Ok(_) => panic!("invalid token-limit controls should fail"),
        Err(error) => error,
    };
    let body: Value = serde_json::from_slice(&error.body).unwrap();
    assert_eq!(body["error"]["code"], "unsupported_token_limit");
}

#[test]
fn kiro_responses_request_rejects_unenforceable_generation_control() {
    let error = runtime_kiro_request_body_for_endpoint(
        ProviderEndpoint::Responses,
        serde_json::to_vec(&json!({
            "model": "auto",
            "input": "hello",
            "temperature": 0.2
        }))
        .unwrap(),
    )
    .expect_err("Kiro Responses temperature should fail before ACP launch");
    let body: Value = serde_json::from_slice(&error.body).unwrap();
    assert_eq!(body["error"]["code"], "unsupported_generation_control");
}

#[test]
fn kiro_parent_request_removes_external_tool_declarations() {
    let result = runtime_kiro_request_body_for_endpoint(
        ProviderEndpoint::Responses,
        serde_json::to_vec(&json!({
            "model": "claude-sonnet-4.5",
            "input": "inspect the repository",
            "tools": [{"type": "function", "name": "shell"}],
            "functions": [{"name": "shell"}],
            "parallel_tool_calls": true,
            "tool_choice": "auto"
        }))
        .unwrap(),
    );
    let Ok(result) = result else {
        panic!("parent Kiro request should let ACP own tools");
    };
    let value: Value = serde_json::from_slice(&result).unwrap();
    for field in ["tools", "functions"] {
        assert!(value.get(field).is_none(), "{field}");
    }
}

#[test]
fn kiro_sub_agent_request_removes_external_tool_controls() {
    let result = super::request_validation::runtime_kiro_request_body_for_endpoint_with_sub_agent(
        ProviderEndpoint::Responses,
        serde_json::to_vec(&json!({
            "model": "claude-sonnet-4.5",
            "input": "inspect the repository",
            "parallel_tool_calls": true,
            "tool_choice": "auto",
            "tools": [{"type": "function", "name": "shell"}],
            "functions": [{"name": "shell"}],
            "function_call": "auto",
            "web_search_options": {}
        }))
        .unwrap(),
        true,
    );
    let Ok(body) = result else {
        panic!("sub-agent Kiro request should let ACP own tools");
    };
    let value: Value = serde_json::from_slice(&body).unwrap();
    for field in [
        "parallel_tool_calls",
        "tool_choice",
        "tools",
        "functions",
        "function_call",
        "web_search_options",
    ] {
        assert!(value.get(field).is_none(), "{field}");
    }
}

#[test]
fn kiro_semantic_compact_summary_uses_acp_turn() {
    let root = std::env::temp_dir().join(format!(
        "prodex-kiro-compact-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should move forward")
            .as_nanos()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("temp root should exist");
    #[cfg(unix)]
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
        .expect("temp root should be private");
    let codex_home = root.join("kiro-home");
    secret_store::SecretManager::new(secret_store::FileSecretBackend::new())
        .write_text(
            &secret_store::SecretLocation::file(codex_home.join("kiro_auth.json")),
            serde_json::json!({
                "auth_key": "kirocli:social:token",
                "auth_kind": "social",
                "auth_json": "{\"token\":\"abc\"}",
                "email": "kiro@example.com",
                "profile_arn": null,
                "profile_name": null,
                "start_url": null,
                "region": "us-east-1"
            })
            .to_string(),
        )
        .expect("kiro auth secret should be written");
    let auth = RuntimeKiroProfileAuth {
        profile_name: "kiro-main".to_string(),
        codex_home: codex_home.clone(),
        model_catalog: vec![json!({
            "id": "claude-sonnet-4",
            "name": "claude-sonnet-4",
            "object": "model",
            "owned_by": "kiro-cli"
        })],
        command: Some(write_fake_kiro_compact_agent(&root)),
    };
    let async_runtime = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .expect("Kiro compact test runtime should build"),
    );
    let summary = runtime_kiro_semantic_compact_summary(
        7,
        &serde_json::to_vec(&json!({
            "model": "claude-sonnet-4",
            "input": [
                {
                    "type": "message",
                    "role": "user",
                    "content": [{"type": "input_text", "text": "keep implementing parity"}]
                },
                {
                    "type": "function_call",
                    "call_id": "call_1",
                    "name": "read_file",
                    "arguments": "{\"path\":\"src/main.rs\"}"
                },
                {
                    "type": "function_call_output",
                    "call_id": "call_1",
                    "output": "fn main() {}"
                }
            ]
        }))
        .expect("request body"),
        &async_runtime,
        &auth,
    )
    .expect("semantic compact summary should succeed");
    assert_eq!(summary, "FAKE NATIVE KIRO COMPACT SUMMARY");
    let _ = fs::remove_dir_all(&root);
}
