use super::super::deepseek_rewrite::{
    RuntimeDeepSeekChatSseReader, RuntimeDeepSeekConversationStore, RuntimeDeepSeekRewriteOptions,
    runtime_deepseek_chat_request_body, runtime_deepseek_chat_request_body_with_options,
};
use std::collections::BTreeMap;
use std::io::{Cursor, Read};
use std::sync::{Arc, Mutex};

fn deepseek_conversation_store() -> RuntimeDeepSeekConversationStore {
    Arc::new(Mutex::new(BTreeMap::new()))
}

#[test]
fn deepseek_request_translation_maps_responses_input_and_tools() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "stream": true,
        "instructions": "Be concise.",
        "input": [
            {
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "List files"}]
            },
            {
                "type": "function_call_output",
                "call_id": "call_1",
                "output": "README.md"
            }
        ],
        "tools": [
            {
                "type": "function",
                "name": "shell",
                "description": "Run shell",
                "parameters": {"type": "object"}
            },
            {"type": "web_search_preview"}
        ],
        "tool_choice": {
            "type": "function",
            "name": "shell"
        },
        "reasoning": {
            "effort": "xhigh"
        },
        "max_output_tokens": 123
    });

    let translated =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect("request should translate");
    let translated: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

    assert_eq!(translated["model"], "deepseek-v4-pro");
    assert_eq!(translated["stream"], true);
    assert_eq!(translated["max_tokens"], 123);
    assert_eq!(translated["messages"][0]["role"], "system");
    assert_eq!(translated["messages"][1]["content"], "List files");
    assert_eq!(translated["messages"].as_array().unwrap().len(), 2);
    assert_eq!(translated["tools"].as_array().unwrap().len(), 1);
    assert_eq!(translated["tools"][0]["function"]["name"], "shell");
    assert!(translated.get("tool_choice").is_none());
    assert_eq!(translated["thinking"]["type"], "enabled");
    assert_eq!(translated["reasoning_effort"], "max");
}

#[test]
fn deepseek_request_translation_preserves_local_shell_call_fields() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": [{
            "type": "local_shell_call",
            "call_id": "call_shell_1",
            "action": {
                "command": ["cargo", "test"],
                "cwd": "/repo",
                "timeout": 120,
                "env": {
                    "RUST_LOG": "debug"
                }
            }
        }, {
            "type": "function_call_output",
            "call_id": "call_shell_1",
            "output": "ok"
        }]
    });

    let translated =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect("request should translate");
    let translated: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
    let arguments = translated["messages"][0]["tool_calls"][0]["function"]["arguments"]
        .as_str()
        .expect("shell arguments should be a JSON string");
    let arguments: serde_json::Value = serde_json::from_str(arguments).unwrap();

    assert_eq!(arguments["command"], "cargo test");
    assert_eq!(arguments["cwd"], "/repo");
    assert_eq!(arguments["timeout"], 120);
    assert_eq!(arguments["env"]["RUST_LOG"], "debug");
}

#[test]
fn deepseek_strict_tools_sets_strict_and_sanitizes_schema() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "call the tool",
        "tools": [{
            "type": "function",
            "name": "lookup",
            "description": "Lookup records",
            "parameters": {
                "type": "object",
                "properties": {
                    "query": {"type": "string"},
                    "limit": {"type": "integer"}
                }
            }
        }]
    });

    let translated = runtime_deepseek_chat_request_body_with_options(
        &serde_json::to_vec(&body).unwrap(),
        &conversations,
        RuntimeDeepSeekRewriteOptions {
            strict_tools: true,
            ..RuntimeDeepSeekRewriteOptions::default()
        },
    )
    .expect("strict tool request should translate");
    let translated: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
    let function = &translated["tools"][0]["function"];

    assert_eq!(function["strict"], true);
    assert_eq!(function["parameters"]["required"][0], "limit");
    assert_eq!(function["parameters"]["required"][1], "query");
    assert_eq!(function["parameters"]["additionalProperties"], false);
}

#[test]
fn deepseek_strict_tools_rejects_unsupported_schema_keywords() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "call the tool",
        "tools": [{
            "type": "function",
            "name": "lookup",
            "parameters": {
                "type": "object",
                "properties": {
                    "query": {"type": "string", "pattern": "^[a-z]+$"}
                }
            }
        }]
    });

    let error = runtime_deepseek_chat_request_body_with_options(
        &serde_json::to_vec(&body).unwrap(),
        &conversations,
        RuntimeDeepSeekRewriteOptions {
            strict_tools: true,
            ..RuntimeDeepSeekRewriteOptions::default()
        },
    )
    .expect_err("unsupported strict schema keyword should fail");

    assert!(error.to_string().contains("unsupported keyword `pattern`"));
}

#[test]
fn deepseek_request_translation_maps_chat_completion_parameters() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "stream": true,
        "input": "Return JSON.",
        "temperature": 0.2,
        "top_p": 0.9,
        "max_output_tokens": 321,
        "stop": ["DONE"],
        "logprobs": true,
        "top_logprobs": 3,
        "user_id": "user-safe-123",
        "response_format": {"type": "json_object"}
    });

    let translated =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect("request should translate");
    let translated: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

    assert_eq!(translated["temperature"], 0.2);
    assert_eq!(translated["top_p"], 0.9);
    assert_eq!(translated["max_tokens"], 321);
    assert_eq!(translated["stop"][0], "DONE");
    assert_eq!(translated["logprobs"], true);
    assert_eq!(translated["top_logprobs"], 3);
    assert_eq!(translated["user_id"], "user-safe-123");
    assert_eq!(translated["response_format"]["type"], "json_object");
    assert_eq!(translated["stream_options"]["include_usage"], true);
}

#[test]
fn deepseek_request_translation_maps_single_stop_string() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "stop": "DONE"
    });

    let translated =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect("request should translate");
    let translated: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

    assert_eq!(translated["stop"], "DONE");
}

#[test]
fn deepseek_request_translation_rejects_beta_prompt_completion_shape() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "prompt": "fn main() {"
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("request should reject prompt completions on Responses adapter");

    assert!(
        error
            .to_string()
            .contains("prompt completions require the beta /completions endpoint")
    );
}

#[test]
fn deepseek_request_translation_rejects_beta_fim_suffix_shape() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "prompt": "fn main() {",
        "suffix": "}"
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("request should reject FIM suffix completions on Responses adapter");

    assert!(
        error
            .to_string()
            .contains("FIM suffix completion requires the beta /completions endpoint")
    );
}

#[test]
fn deepseek_request_translation_rejects_beta_chat_prefix_shape() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "prefix": true,
        "input": "finish this assistant prefix"
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("request should reject chat prefix completion markers");

    assert!(
        error
            .to_string()
            .contains("chat prefix completion requires the beta chat endpoint")
    );
}

#[test]
fn deepseek_request_translation_rejects_assistant_prefix_message() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": [{
            "type": "message",
            "role": "assistant",
            "prefix": true,
            "content": [{"type": "output_text", "text": "Partial answer"}]
        }]
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("request should reject assistant prefix completion markers");

    assert!(
        error
            .to_string()
            .contains("chat prefix completion requires the beta chat endpoint")
    );
}

#[test]
fn deepseek_request_translation_rejects_image_message_content() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": [{
            "type": "message",
            "role": "user",
            "content": [{
                "type": "input_image",
                "image_url": "data:image/png;base64,AAAA"
            }]
        }]
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("request should reject image content");

    assert!(
        error
            .to_string()
            .contains("does not support message content part type `input_image`")
    );
}

#[test]
fn deepseek_request_translation_rejects_document_message_content() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": [{
            "type": "message",
            "role": "user",
            "content": [{
                "type": "input_file",
                "file_url": "file:///tmp/example.pdf"
            }]
        }]
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("request should reject document content");

    assert!(
        error
            .to_string()
            .contains("does not support message content part type `input_file`")
    );
}

#[test]
fn deepseek_request_translation_degrades_json_schema_to_json_object() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "Return JSON.",
        "text": {
            "format": {
                "type": "json_schema",
                "name": "answer",
                "schema": {
                    "type": "object",
                    "properties": {"answer": {"type": "string"}},
                    "required": ["answer"]
                }
            }
        }
    });

    let translated =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect("request should translate");
    let metadata = translated
        .response_metadata
        .clone()
        .expect("JSON schema degradation should be recorded");
    let translated: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

    assert_eq!(translated["response_format"]["type"], "json_object");
    assert!(translated["response_format"].get("schema").is_none());
    assert_eq!(
        metadata["deepseek"]["degraded_response_format"]["from"],
        "json_schema"
    );
    assert_eq!(
        metadata["deepseek"]["degraded_response_format"]["to"],
        "json_object"
    );
}

#[test]
fn deepseek_request_translation_adds_json_prompt_guard_when_missing() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "Return an object with answer set to ok.",
        "response_format": {"type": "json_object"}
    });

    let translated =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect("request should translate");
    let translated: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

    assert_eq!(translated["messages"][0]["role"], "system");
    assert_eq!(
        translated["messages"][0]["content"],
        "Respond with valid JSON only."
    );
    assert_eq!(
        translated["messages"][1]["content"],
        "Return an object with answer set to ok."
    );
}

#[test]
fn deepseek_request_translation_does_not_duplicate_json_prompt_guard() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "Return valid JSON with answer set to ok.",
        "response_format": {"type": "json_object"}
    });

    let translated =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect("request should translate");
    let translated: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

    assert_eq!(translated["messages"].as_array().unwrap().len(), 1);
    assert_eq!(
        translated["messages"][0]["content"],
        "Return valid JSON with answer set to ok."
    );
}

#[test]
fn deepseek_request_translation_rejects_too_many_stop_sequences() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "stop": (0..17).map(|index| format!("stop-{index}")).collect::<Vec<_>>()
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("request should reject unsupported stop sequence count");

    assert!(error.to_string().contains("at most 16 stop sequences"));
}

#[test]
fn deepseek_request_translation_rejects_non_string_stop_sequences() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "stop": ["DONE", 42]
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("request should reject non-string stop sequences");

    assert!(error.to_string().contains("stop sequences must be strings"));
}

#[test]
fn deepseek_request_translation_rejects_too_many_top_logprobs() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "logprobs": true,
        "top_logprobs": 21
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("request should reject unsupported top_logprobs count");

    assert!(error.to_string().contains("top_logprobs must be <= 20"));
}

#[test]
fn deepseek_request_translation_rejects_top_logprobs_without_logprobs() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "top_logprobs": 3
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("request should reject top_logprobs without logprobs");

    assert!(
        error
            .to_string()
            .contains("top_logprobs requires logprobs=true")
    );
}

#[test]
fn deepseek_request_translation_rejects_deprecated_penalties() {
    for field in ["frequency_penalty", "presence_penalty"] {
        let conversations = deepseek_conversation_store();
        let body = serde_json::json!({
            "model": "deepseek-v4-pro",
            "input": "hello",
            field: 0.2
        });

        let error =
            runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
                .expect_err("request should reject deprecated DeepSeek penalty fields");

        assert!(error.to_string().contains("deprecated"));
        assert!(error.to_string().contains(field));
    }
}

#[test]
fn deepseek_request_translation_tolerates_parallel_tool_calls_true() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "parallel_tool_calls": true
    });

    let translated =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect("request should translate");
    let translated: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

    assert!(translated.get("parallel_tool_calls").is_none());
}

#[test]
fn deepseek_request_translation_rejects_parallel_tool_calls_false() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "parallel_tool_calls": false
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("request should reject unsupported parallel tool call control");

    assert!(error.to_string().contains("parallel_tool_calls=false"));
}

#[test]
fn deepseek_request_translation_rejects_invalid_user_id() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "user_id": "alice@example.com"
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("request should reject invalid DeepSeek user_id");

    assert!(error.to_string().contains("DeepSeek user_id must use only"));
}

#[test]
fn deepseek_request_translation_rejects_more_than_128_function_tools() {
    let conversations = deepseek_conversation_store();
    let tools = (0..129)
        .map(|index| {
            serde_json::json!({
                "type": "function",
                "name": format!("tool_{index}"),
                "parameters": {"type": "object"}
            })
        })
        .collect::<Vec<_>>();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "tools": tools
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("request should reject unsupported tool count");

    assert!(error.to_string().contains("at most 128 function tools"));
}

#[test]
fn deepseek_request_translation_rejects_duplicate_function_tool_names() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "tools": [
            {
                "type": "function",
                "name": "shell",
                "parameters": {"type": "object"}
            },
            {
                "type": "function",
                "name": "shell",
                "description": "Different shell wrapper",
                "parameters": {"type": "object"}
            }
        ]
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("request should reject duplicate function tool names");

    assert!(
        error
            .to_string()
            .contains("function tool name `shell` is duplicated")
    );
}

#[test]
fn deepseek_request_translation_rejects_translated_namespace_tool_name_collision() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "tools": [
            {
                "type": "function",
                "name": "agents--spawn_agent",
                "description": "Direct flattened function",
                "parameters": {"type": "object"}
            },
            {
                "type": "namespace",
                "name": "agents",
                "tools": [{
                    "type": "function",
                    "name": "spawn_agent",
                    "parameters": {"type": "object"}
                }]
            }
        ]
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("request should reject translated tool name collisions");

    assert!(
        error
            .to_string()
            .contains("function tool name `agents--spawn_agent` is duplicated")
    );
}

#[test]
fn deepseek_request_translation_rejects_invalid_function_tool_name() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "tools": [{
            "type": "function",
            "name": "bad.name",
            "parameters": {"type": "object"}
        }]
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("request should reject invalid DeepSeek function tool names");

    assert!(
        error
            .to_string()
            .contains("function tool names must use only")
    );
}

#[test]
fn deepseek_request_translation_rejects_invalid_named_tool_choice() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "tool_choice": {
            "type": "function",
            "name": "bad.name"
        }
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("request should reject invalid DeepSeek named tool choice");

    assert!(
        error
            .to_string()
            .contains("function tool names must use only")
    );
}

#[test]
fn deepseek_request_translation_rejects_named_tool_choice_without_tools() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "tool_choice": {
            "type": "function",
            "name": "shell"
        }
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("request should reject named tool choice without tools");

    assert!(
        error
            .to_string()
            .contains("named tool_choice `shell` does not match")
    );
}

#[test]
fn deepseek_request_translation_rejects_named_tool_choice_missing_target() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "tools": [{
            "type": "function",
            "name": "shell",
            "parameters": {"type": "object"}
        }],
        "tool_choice": {
            "type": "function",
            "name": "missing"
        }
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("request should reject named tool choice with no matching tool");

    assert!(
        error
            .to_string()
            .contains("named tool_choice `missing` does not match")
    );
}

#[test]
fn deepseek_request_translation_flattens_namespace_tool_choice() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "spawn",
        "tools": [{
            "type": "namespace",
            "name": "agents",
            "tools": [{
                "type": "function",
                "name": "spawn_agent",
                "parameters": {"type": "object"}
            }]
        }],
        "tool_choice": {
            "type": "function",
            "namespace": "agents",
            "name": "spawn_agent"
        }
    });

    let translated =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect("request should translate");
    let translated: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

    assert_eq!(
        translated["tools"][0]["function"]["name"],
        "agents--spawn_agent"
    );
    assert_eq!(
        translated["tool_choice"]["function"]["name"],
        "agents--spawn_agent"
    );
}

#[test]
fn deepseek_request_translation_prepends_previous_response_history() {
    let conversations = deepseek_conversation_store();
    conversations.lock().unwrap().insert(
        "resp_prev".to_string(),
        vec![
            serde_json::json!({"role": "user", "content": "old prompt"}),
            serde_json::json!({"role": "assistant", "content": "old answer"}),
        ],
    );
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "previous_response_id": "resp_prev",
        "input": "new prompt"
    });

    let translated =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect("request should translate");
    let translated: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

    assert_eq!(translated["messages"][0]["content"], "old prompt");
    assert_eq!(translated["messages"][1]["content"], "old answer");
    assert_eq!(translated["messages"][2]["content"], "new prompt");
}

#[test]
fn deepseek_sse_reader_maps_text_and_tool_calls_to_responses_events() {
    let conversations = deepseek_conversation_store();
    let stream = concat!(
        "data: {\"id\":\"chatcmpl_1\",\"model\":\"deepseek-v4-pro\",\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n",
        "data: {\"id\":\"chatcmpl_1\",\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"function\":{\"name\":\"shell\",\"arguments\":\"{\\\"cmd\\\":\"}}]}}]}\n\n",
        "data: {\"id\":\"chatcmpl_1\",\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"\\\"ls\\\"}\"}}]},\"finish_reason\":\"tool_calls\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let mut reader = RuntimeDeepSeekChatSseReader::new(
        Cursor::new(stream.as_bytes()),
        7,
        Vec::new(),
        None,
        conversations,
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains("event: response.created"));
    assert!(output.contains("\"type\":\"response.output_text.delta\""));
    assert!(output.contains("\"delta\":\"hi\""));
    assert!(output.contains("\"type\":\"response.output_item.added\""));
    assert!(output.contains("\"type\":\"response.function_call_arguments.delta\""));
    assert!(output.contains("\"type\":\"response.output_item.done\""));
    assert!(output.contains("\"arguments\":\"{\\\"cmd\\\":\\\"rtk ls\\\"}\""));
    assert!(output.contains("event: response.completed"));
}
