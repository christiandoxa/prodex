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

fn write_fake_kiro_compact_agent(root: &Path) -> std::path::PathBuf {
    let script = root.join("fake-kiro-compact");
    fs::write(
        &script,
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
    .expect("fake kiro compact agent should be written");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&script).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script, perms).expect("permissions should update");
    }
    script
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
fn kiro_chat_request_tolerates_token_limit_controls_as_noop() {
    let translated = match runtime_kiro_request_body_for_endpoint(
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
        Ok(translated) => translated,
        Err(_) => panic!("valid token-limit controls should be ignored"),
    };
    let translated: Value = serde_json::from_slice(&translated).unwrap();
    assert!(translated.get("max_output_tokens").is_none());
    assert!(translated.get("max_tokens").is_none());
    assert!(translated.get("max_completion_tokens").is_none());
    assert_eq!(translated["input"][0]["role"], "user");
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
