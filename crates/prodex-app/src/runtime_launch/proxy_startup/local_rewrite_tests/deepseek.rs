use super::super::deepseek_rewrite::{
    RuntimeDeepSeekChatSseReader, RuntimeDeepSeekConversationStore, RuntimeDeepSeekRewriteOptions,
    runtime_deepseek_chat_request_body, runtime_deepseek_chat_request_body_with_options,
};
use std::io::{Cursor, Read};

fn deepseek_conversation_store() -> RuntimeDeepSeekConversationStore {
    RuntimeDeepSeekConversationStore::default()
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
            }
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
    let metadata = translated
        .response_metadata
        .clone()
        .expect("tool_choice omission should be recorded");
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
    assert_eq!(
        metadata["deepseek"]["omitted_tool_choice"]["from"]["name"],
        "shell"
    );
}

#[test]
fn deepseek_request_translation_accepts_untyped_role_content_items() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": [
            {
                "role": "user",
                "content": "hello from anthropic-compat"
            }
        ]
    });

    let translated =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect("request should translate");
    let translated: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

    assert_eq!(translated["messages"][0]["role"], "user");
    assert_eq!(
        translated["messages"][0]["content"],
        "hello from anthropic-compat"
    );
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
fn deepseek_request_translation_replays_custom_apply_patch_call_and_output() {
    let conversations = deepseek_conversation_store();
    let patch = "*** Begin Patch\n*** Add File: note.txt\n+hello\n*** End Patch";
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": [{
            "type": "custom_tool_call",
            "call_id": "call_patch_1",
            "name": "apply_patch",
            "input": patch
        }, {
            "type": "custom_tool_call_output",
            "call_id": "call_patch_1",
            "output": "ok"
        }]
    });

    let translated =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect("request should translate");
    let translated: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
    let messages = translated["messages"].as_array().unwrap();
    let arguments = messages[0]["tool_calls"][0]["function"]["arguments"]
        .as_str()
        .expect("apply_patch arguments should be a JSON string");
    let arguments: serde_json::Value = serde_json::from_str(arguments).unwrap();

    assert_eq!(messages[0]["role"], "assistant");
    assert_eq!(
        messages[0]["tool_calls"][0]["function"]["name"],
        "apply_patch"
    );
    assert_eq!(arguments["input"], patch);
    assert_eq!(messages[1]["role"], "tool");
    assert_eq!(messages[1]["tool_call_id"], "call_patch_1");
    assert_eq!(messages[1]["content"], "ok");
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
fn deepseek_request_translation_preserves_strict_false_function_tool() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "call the tool",
        "tools": [{
            "type": "function",
            "name": "lookup",
            "strict": false,
            "parameters": {"type": "object"}
        }]
    });

    let translated =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect("strict false should translate");
    let translated: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

    assert_eq!(translated["tools"][0]["function"]["strict"], false);
}

#[test]
fn deepseek_request_translation_rejects_strict_true_without_strict_mode() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "call the tool",
        "tools": [{
            "type": "function",
            "name": "lookup",
            "strict": true,
            "parameters": {"type": "object"}
        }]
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("strict true should require strict mode");

    assert!(
        error
            .to_string()
            .contains("requires deepseek.strict_tools=true")
    );
}

#[test]
fn deepseek_request_translation_rejects_non_bool_function_strict() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "call the tool",
        "tools": [{
            "type": "function",
            "name": "lookup",
            "strict": "true",
            "parameters": {"type": "object"}
        }]
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("non-bool strict should fail");

    assert!(error.to_string().contains("tool strict must be a boolean"));
}

#[test]
fn deepseek_request_translation_rejects_strict_true_custom_tool() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "edit",
        "tools": [{
            "type": "custom",
            "name": "apply_patch",
            "strict": true
        }]
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("strict custom tool should fail clearly");

    assert!(
        error
            .to_string()
            .contains("custom tools cannot preserve strict=true")
    );
}

#[test]
fn deepseek_request_translation_preserves_custom_tool_format_in_wrapper() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "emit a selector",
        "tools": [{
            "type": "custom",
            "name": "css_selector",
            "description": "Emit a CSS selector.",
            "format": {
                "type": "grammar",
                "syntax": "lark",
                "definition": "start: /[a-z]+/"
            }
        }]
    });

    let translated =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect("custom format should translate through wrapper description");
    let translated: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
    let description = translated["tools"][0]["function"]["description"]
        .as_str()
        .unwrap();

    assert!(description.contains("Original custom tool format JSON"));
    assert!(description.contains("\"type\":\"grammar\""));
    assert!(description.contains("\"syntax\":\"lark\""));
}

#[test]
fn deepseek_request_translation_rejects_malformed_custom_tool_format() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "emit a selector",
        "tools": [{
            "type": "custom",
            "name": "css_selector",
            "format": "grammar"
        }]
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("malformed custom format should fail");

    assert!(
        error
            .to_string()
            .contains("custom tool format must be an object")
    );
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
        "safety_identifier": "user-safe-123",
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
fn deepseek_request_translation_maps_max_completion_tokens() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "max_completion_tokens": 77
    });

    let translated =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect("max_completion_tokens should translate");
    let translated: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

    assert_eq!(translated["max_tokens"], 77);
}

#[test]
fn deepseek_request_translation_rejects_invalid_top_level_shapes() {
    for (body, expected) in [
        (
            serde_json::json!(["hello"]),
            "request body must be a JSON object",
        ),
        (
            serde_json::json!({
                "model": 7,
                "input": "hello"
            }),
            "model must be a string",
        ),
        (
            serde_json::json!({
                "model": "",
                "input": "hello"
            }),
            "model must not be empty",
        ),
        (
            serde_json::json!({
                "model": "deepseek-v4-pro",
                "stream": "true",
                "input": "hello"
            }),
            "stream must be a boolean",
        ),
        (
            serde_json::json!({
                "model": "deepseek-v4-pro",
                "instructions": ["be concise"],
                "input": "hello"
            }),
            "instructions must be a string",
        ),
        (
            serde_json::json!({
                "model": "deepseek-v4-pro",
                "previous_response_id": 7,
                "input": "hello"
            }),
            "previous_response_id must be a string",
        ),
    ] {
        let conversations = deepseek_conversation_store();
        let error =
            runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
                .expect_err("invalid top-level shape should fail clearly");

        assert!(
            error.to_string().contains(expected),
            "expected `{expected}`, got `{error}`"
        );
    }
}

#[test]
fn deepseek_request_translation_rejects_unknown_reasoning_effort() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "reasoning": {"effort": "extreme"}
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("unknown reasoning effort should fail");

    assert!(
        error
            .to_string()
            .contains("reasoning effort is not supported")
    );
}

#[test]
fn deepseek_request_translation_rejects_invalid_reasoning_shape() {
    for (body, expected) in [
        (
            serde_json::json!({
                "model": "deepseek-v4-pro",
                "input": "hello",
                "reasoning": "high"
            }),
            "reasoning must be an object",
        ),
        (
            serde_json::json!({
                "model": "deepseek-v4-pro",
                "input": "hello",
                "reasoning": {"effort": 7}
            }),
            "reasoning.effort must be a string",
        ),
        (
            serde_json::json!({
                "model": "deepseek-v4-pro",
                "input": "hello",
                "reasoning": {"effort": "high", "summary": "auto"}
            }),
            "reasoning.summary is not supported",
        ),
        (
            serde_json::json!({
                "model": "deepseek-v4-pro",
                "input": "hello",
                "reasoning_effort": 7
            }),
            "reasoning_effort must be a string",
        ),
    ] {
        let conversations = deepseek_conversation_store();
        let error =
            runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
                .expect_err("invalid reasoning shape should fail");

        assert!(error.to_string().contains(expected));
    }
}

#[test]
fn deepseek_request_translation_rejects_invalid_chat_parameter_shapes() {
    for (field, value, message) in [
        (
            "temperature",
            serde_json::json!("0.2"),
            "temperature must be a number",
        ),
        ("top_p", serde_json::json!("0.9"), "top_p must be a number"),
        (
            "max_output_tokens",
            serde_json::json!(0),
            "max_output_tokens must be a positive integer",
        ),
        (
            "max_tokens",
            serde_json::json!("321"),
            "max_tokens must be a positive integer",
        ),
        (
            "logprobs",
            serde_json::json!("true"),
            "logprobs must be a boolean",
        ),
    ] {
        let conversations = deepseek_conversation_store();
        let mut body = serde_json::json!({
            "model": "deepseek-v4-pro",
            "input": "hello"
        });
        body[field] = value;

        let error =
            runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
                .expect_err("request should reject invalid chat parameter shape");

        assert!(
            error.to_string().contains(message),
            "{field} error should contain `{message}`, got `{error}`"
        );
    }
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
fn deepseek_request_translation_rejects_message_cache_control() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": [{
            "type": "message",
            "role": "user",
            "content": [{
                "type": "input_text",
                "text": "hello",
                "cache_control": {"type": "ephemeral"}
            }]
        }]
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("per-message cache_control should fail clearly");

    assert!(error.to_string().contains("cache_control is not supported"));
}

#[test]
fn deepseek_request_translation_rejects_text_part_without_text() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": [{
            "type": "message",
            "role": "user",
            "content": [{"type": "input_text"}]
        }]
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("text content without text should fail");

    assert!(
        error
            .to_string()
            .contains("input_text content parts require a text field")
    );
}

#[test]
fn deepseek_request_translation_rejects_unknown_object_content_part() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": [{
            "type": "message",
            "role": "user",
            "content": [{"unknown": "value"}]
        }]
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("unknown object content should fail");

    assert!(
        error
            .to_string()
            .contains("object message content parts without text or type")
    );
}

#[test]
fn deepseek_request_translation_rejects_unknown_input_item_type() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": [{
            "type": "file_search_call",
            "query": "README"
        }]
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("unsupported input item type should fail clearly");

    assert!(
        error
            .to_string()
            .contains("input item type `file_search_call` is not supported")
    );
}

#[test]
fn deepseek_request_translation_rejects_invalid_input_item_shapes() {
    for (input, expected) in [
        (
            serde_json::json!("loose text"),
            "input items must be objects",
        ),
        (
            serde_json::json!({
                "type": "message",
                "role": "critic",
                "content": "hello"
            }),
            "message role `critic` is not supported",
        ),
        (
            serde_json::json!({
                "type": "message",
                "role": 7,
                "content": "hello"
            }),
            "message role must be a string",
        ),
    ] {
        let conversations = deepseek_conversation_store();
        let body = serde_json::json!({
            "model": "deepseek-v4-pro",
            "input": [input]
        });

        let error =
            runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
                .expect_err("invalid input item shape should fail clearly");

        assert!(
            error.to_string().contains(expected),
            "expected `{expected}`, got `{error}`"
        );
    }
}

#[test]
fn deepseek_request_translation_rejects_invalid_input_and_content_shapes() {
    for (body, expected) in [
        (
            serde_json::json!({
                "model": "deepseek-v4-pro",
                "input": {"text": "hello"}
            }),
            "input must be a string or array of input items",
        ),
        (
            serde_json::json!({
                "model": "deepseek-v4-pro",
                "input": [{
                    "type": "message",
                    "role": "user",
                    "content": 7
                }]
            }),
            "message content must be a string, object, or array",
        ),
    ] {
        let conversations = deepseek_conversation_store();
        let error =
            runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
                .expect_err("invalid input/content shape should fail clearly");

        assert!(
            error.to_string().contains(expected),
            "expected `{expected}`, got `{error}`"
        );
    }
}

#[test]
fn deepseek_request_translation_rejects_malformed_input_tool_items() {
    for (input, expected) in [
        (
            serde_json::json!({
                "type": "function_call",
                "call_id": "call_1",
                "arguments": "{}"
            }),
            "input tool call items require a function name",
        ),
        (
            serde_json::json!({
                "type": "function_call",
                "name": "shell",
                "arguments": "{}"
            }),
            "input tool items require a call_id",
        ),
        (
            serde_json::json!({
                "type": "function_call_output",
                "output": "ok"
            }),
            "input tool items require a call_id",
        ),
        (
            serde_json::json!({
                "type": "function_call_output",
                "call_id": "call_1"
            }),
            "input tool output items require output content",
        ),
        (
            serde_json::json!({
                "type": "local_shell_call",
                "call_id": "call_shell_1",
                "action": {"command": ["cargo", 7]}
            }),
            "local_shell_call action.command must be an array of strings",
        ),
        (
            serde_json::json!({
                "type": "local_shell_call",
                "call_id": "call_shell_1",
                "action": {"command": []}
            }),
            "local_shell_call action.command must be an array of strings",
        ),
        (
            serde_json::json!({
                "type": "local_shell_call",
                "call_id": "call_shell_1"
            }),
            "local_shell_call requires a command",
        ),
    ] {
        let conversations = deepseek_conversation_store();
        let body = serde_json::json!({
            "model": "deepseek-v4-pro",
            "input": [input]
        });

        let error =
            runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
                .expect_err("malformed input tool item should fail clearly");

        assert!(
            error.to_string().contains(expected),
            "expected `{expected}`, got `{error}`"
        );
    }
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
fn deepseek_request_translation_preserves_request_metadata() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "metadata": {
            "ticket": "DS-123",
            "deepseek": {
                "caller": "codex"
            }
        },
        "client_metadata": {
            "session_id": "sess-123",
            "turn_id": "turn-456"
        },
        "prompt_cache_key": "cache-stable-123",
        "prompt_cache_retention": "24h"
    });

    let translated =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect("request metadata should translate");
    let metadata = translated
        .response_metadata
        .expect("request metadata should be preserved");

    assert_eq!(metadata["ticket"], "DS-123");
    assert_eq!(metadata["deepseek"]["caller"], "codex");
    assert_eq!(metadata["client_metadata"]["session_id"], "sess-123");
    assert_eq!(metadata["client_metadata"]["turn_id"], "turn-456");
    assert_eq!(metadata["prompt_cache_key"], "cache-stable-123");
    assert_eq!(metadata["prompt_cache_retention"], "24h");
}

#[test]
fn deepseek_request_translation_rejects_invalid_metadata_shape() {
    for (body, expected) in [
        (
            serde_json::json!({
                "model": "deepseek-v4-pro",
                "input": "hello",
                "metadata": "ticket"
            }),
            "request metadata must be an object",
        ),
        (
            serde_json::json!({
                "model": "deepseek-v4-pro",
                "input": "hello",
                "metadata": {"deepseek": "caller"}
            }),
            "request metadata.deepseek must be an object",
        ),
        (
            serde_json::json!({
                "model": "deepseek-v4-pro",
                "input": "hello",
                "client_metadata": "session"
            }),
            "client_metadata must be an object",
        ),
        (
            serde_json::json!({
                "model": "deepseek-v4-pro",
                "input": "hello",
                "prompt_cache_key": 7
            }),
            "prompt_cache_key must be a string",
        ),
        (
            serde_json::json!({
                "model": "deepseek-v4-pro",
                "input": "hello",
                "prompt_cache_retention": 7
            }),
            "prompt_cache_retention must be a string",
        ),
    ] {
        let conversations = deepseek_conversation_store();
        let error =
            runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
                .expect_err("invalid metadata should fail");

        assert!(error.to_string().contains(expected));
    }
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
fn deepseek_request_translation_allows_text_response_format_default() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "response_format": {"type": "text"}
    });

    let translated =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect("text response format should translate as default");
    let translated: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

    assert!(translated.get("response_format").is_none());
}

#[test]
fn deepseek_request_translation_rejects_unsupported_text_options() {
    for (text, expected) in [
        (
            serde_json::json!({"format": {"type": "text"}, "verbosity": "low"}),
            "text.verbosity is not supported",
        ),
        (
            serde_json::json!({"extra": true}),
            "text.extra is not supported",
        ),
        (serde_json::json!("json"), "text must be an object"),
    ] {
        let conversations = deepseek_conversation_store();
        let body = serde_json::json!({
            "model": "deepseek-v4-pro",
            "input": "hello",
            "text": text
        });

        let error =
            runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
                .expect_err("unsupported text option should fail clearly");

        assert!(error.to_string().contains(expected));
    }
}

#[test]
fn deepseek_request_translation_rejects_unknown_response_format() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "response_format": {"type": "xml"}
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("unknown response format should fail");

    assert!(
        error
            .to_string()
            .contains("response_format type `xml` is not supported")
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
fn deepseek_request_translation_rejects_invalid_stop_shape() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "stop": {"sequence": "DONE"}
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("request should reject invalid stop shape");

    assert!(error.to_string().contains("stop must be a string or array"));
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
fn deepseek_request_translation_rejects_invalid_top_logprobs_shape() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "logprobs": true,
        "top_logprobs": "3"
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("request should reject invalid top_logprobs shape");

    assert!(
        error
            .to_string()
            .contains("top_logprobs must be an integer")
    );
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
fn deepseek_request_translation_rejects_unsupported_generation_knobs() {
    for (field, value) in [
        ("n", serde_json::json!(2)),
        ("seed", serde_json::json!(7)),
        ("service_tier", serde_json::json!("flex")),
        (
            "prediction",
            serde_json::json!({"type": "content", "content": "hello"}),
        ),
        ("logit_bias", serde_json::json!({"42": 10})),
        (
            "functions",
            serde_json::json!([{"name": "legacy", "parameters": {"type": "object"}}]),
        ),
        ("function_call", serde_json::json!("auto")),
    ] {
        let conversations = deepseek_conversation_store();
        let body = serde_json::json!({
            "model": "deepseek-v4-pro",
            "input": "hello",
            field: value
        });

        let error =
            runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
                .expect_err("request should reject unsupported generation knobs");

        assert!(error.to_string().contains("not supported"));
        assert!(error.to_string().contains(field));
    }
}

#[test]
fn deepseek_request_translation_tolerates_empty_include() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "include": []
    });

    let translated =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect("empty include should be a no-op");
    let translated: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

    assert!(translated.get("include").is_none());
}

#[test]
fn deepseek_request_translation_ignores_include_expansions() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "include": ["message.output_text.logprobs"]
    });

    let translated =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect("include expansions should be a no-op");
    let translated: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

    assert!(translated.get("include").is_none());
}

#[test]
fn deepseek_request_translation_tolerates_default_response_controls() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "store": false,
        "background": false,
        "truncation": "disabled"
    });

    let translated =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect("default response controls should be no-ops");
    let translated: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

    assert!(translated.get("background").is_none());
    assert!(translated.get("store").is_none());
    assert!(translated.get("truncation").is_none());
}

#[test]
fn deepseek_request_translation_rejects_unsupported_response_controls() {
    for (field, value) in [
        ("background", serde_json::json!(true)),
        ("truncation", serde_json::json!("auto")),
        ("max_tool_calls", serde_json::json!(1)),
    ] {
        let conversations = deepseek_conversation_store();
        let body = serde_json::json!({
            "model": "deepseek-v4-pro",
            "input": "hello",
            field: value
        });

        let error =
            runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
                .expect_err("unsupported response control should fail clearly");

        assert!(error.to_string().contains(field));
        assert!(error.to_string().contains("not supported"));
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
fn deepseek_request_translation_rejects_invalid_parallel_tool_calls_shape() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "parallel_tool_calls": "true"
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("request should reject invalid parallel tool call control");

    assert!(
        error
            .to_string()
            .contains("parallel_tool_calls must be a boolean")
    );
}

#[test]
fn deepseek_request_translation_accepts_include_usage_stream_options() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "stream": true,
        "stream_options": {"include_usage": true}
    });

    let translated =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect("supported stream_options should translate");
    let translated: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

    assert_eq!(translated["stream_options"]["include_usage"], true);
}

#[test]
fn deepseek_request_translation_rejects_include_usage_false() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "stream": true,
        "stream_options": {"include_usage": false}
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("include_usage=false should fail clearly");

    assert!(error.to_string().contains("include_usage=true"));
}

#[test]
fn deepseek_request_translation_rejects_invalid_include_usage_shape() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "stream": true,
        "stream_options": {"include_usage": "true"}
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("non-boolean include_usage should fail clearly");

    assert!(
        error
            .to_string()
            .contains("stream_options.include_usage must be a boolean")
    );
}

#[test]
fn deepseek_request_translation_rejects_unknown_stream_options() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "stream": true,
        "stream_options": {"include_usage": true, "extra": true}
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("unknown stream_options should fail clearly");

    assert!(
        error
            .to_string()
            .contains("stream_options.extra is not supported")
    );
}

#[test]
fn deepseek_request_translation_allows_text_only_modalities() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "modalities": ["text"]
    });

    let translated =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect("text-only modalities should translate");
    let translated: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

    assert!(translated.get("modalities").is_none());
}

#[test]
fn deepseek_request_translation_rejects_audio_modality() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "modalities": ["text", "audio"]
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("audio modality should fail");

    assert!(error.to_string().contains("only supports text modality"));
}

#[test]
fn deepseek_request_translation_rejects_audio_output_options() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "audio": {"voice": "alloy", "format": "mp3"}
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("audio options should fail");

    assert!(error.to_string().contains("does not support audio output"));
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
fn deepseek_request_translation_rejects_non_string_user_id() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "user_id": 123
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("request should reject non-string DeepSeek user_id");

    assert!(
        error
            .to_string()
            .contains("DeepSeek user_id must be a string")
    );
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
fn deepseek_request_translation_rejects_non_array_tools() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "tools": {"type": "function", "name": "shell"}
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("request should reject non-array tools");

    assert!(error.to_string().contains("tools must be an array"));
}

#[test]
fn deepseek_request_translation_rejects_non_object_tool_entries() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "tools": ["shell"]
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("request should reject non-object tool entries");

    assert!(error.to_string().contains("tools entries must be objects"));
}

#[test]
fn deepseek_request_translation_rejects_unknown_tool_type() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "tools": [{"type": "computer_use", "name": "computer"}]
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("request should reject unknown tool type");

    assert!(
        error
            .to_string()
            .contains("tool type `computer_use` is not supported")
    );
}

#[test]
fn deepseek_request_translation_rejects_function_tool_without_name() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "tools": [{"type": "function", "parameters": {"type": "object"}}]
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("request should reject unnamed function tool");

    assert!(error.to_string().contains("function tools require a name"));
}

#[test]
fn deepseek_request_translation_rejects_custom_tool_without_name() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "tools": [{"type": "custom"}]
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("request should reject unnamed custom tool");

    assert!(error.to_string().contains("custom tools require a name"));
}

#[test]
fn deepseek_request_translation_rejects_function_tool_with_non_object_parameters() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "tools": [{
            "type": "function",
            "name": "shell",
            "parameters": "not a schema object"
        }]
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("request should reject malformed function parameters");

    assert!(
        error
            .to_string()
            .contains("function tool `shell` parameters must be an object")
    );
}

#[test]
fn deepseek_request_translation_rejects_non_string_tool_description() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "tools": [{
            "type": "function",
            "name": "shell",
            "description": {"text": "run a command"},
            "parameters": {"type": "object"}
        }]
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("request should reject non-string tool description");

    assert!(
        error
            .to_string()
            .contains("tool description must be a string")
    );
}

#[test]
fn deepseek_request_translation_rejects_namespace_tool_without_name() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "tools": [{
            "type": "namespace",
            "tools": [{
                "type": "function",
                "name": "spawn_agent",
                "parameters": {"type": "object"}
            }]
        }]
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("request should reject unnamed namespace tool");

    assert!(error.to_string().contains("namespace tools require a name"));
}

#[test]
fn deepseek_request_translation_rejects_namespace_tool_without_tool_array() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "tools": [{
            "type": "namespace",
            "name": "agents"
        }]
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("request should reject namespace tool without tools array");

    assert!(
        error
            .to_string()
            .contains("namespace tool `agents` requires a tools array")
    );
}

#[test]
fn deepseek_request_translation_rejects_namespace_tool_without_function_name() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "tools": [{
            "type": "namespace",
            "name": "agents",
            "tools": [{
                "type": "function",
                "parameters": {"type": "object"}
            }]
        }]
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("request should reject unnamed namespace function tool");

    assert!(
        error
            .to_string()
            .contains("namespace tool `agents` function entries require a name")
    );
}

#[test]
fn deepseek_request_translation_rejects_non_string_namespace_function_description() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "tools": [{
            "type": "namespace",
            "name": "agents",
            "tools": [{
                "type": "function",
                "name": "spawn_agent",
                "description": ["spawn"],
                "parameters": {"type": "object"}
            }]
        }]
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("request should reject non-string namespace function description");

    assert!(
        error
            .to_string()
            .contains("namespace function description must be a string")
    );
}

#[test]
fn deepseek_request_translation_rejects_mcp_toolset_without_server_name() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "tools": [{
            "type": "mcp",
            "allowed_tools": ["status"]
        }]
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("request should reject MCP toolset without server name");

    assert!(
        error
            .to_string()
            .contains("MCP toolsets require a server name")
    );
}

#[test]
fn deepseek_request_translation_rejects_mcp_toolset_without_enabled_tools() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "tools": [{
            "type": "mcp",
            "server_label": "git tools",
            "allowed_tools": [],
            "configs": {
                "status": {"enabled": false}
            }
        }]
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("request should reject MCP toolset without enabled tools");

    assert!(
        error
            .to_string()
            .contains("MCP toolset `git tools` requires allowed_tools or enabled configs")
    );
}

#[test]
fn deepseek_request_translation_rejects_mcp_function_tool_without_name() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "tools": [{
            "type": "mcp_tool",
            "input_schema": {"type": "object"}
        }]
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("request should reject unnamed MCP function tool");

    assert!(
        error
            .to_string()
            .contains("MCP function tools require a name")
    );
}

#[test]
fn deepseek_request_translation_rejects_mcp_function_tool_without_schema() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "tools": [{
            "type": "mcp_tool",
            "name": "mcp__prodex_sqz__compress"
        }]
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("request should reject MCP function tool without schema");

    assert!(
        error
            .to_string()
            .contains("MCP function tool `mcp__prodex_sqz__compress` requires a schema")
    );
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
fn deepseek_request_translation_rejects_named_tool_choice_without_name() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "tool_choice": {
            "type": "function"
        }
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("request should reject named tool_choice without a name");

    assert!(
        error
            .to_string()
            .contains("named tool_choice requires a function name")
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
fn deepseek_request_translation_maps_string_tool_choice_modes() {
    for mode in ["none", "auto", "required"] {
        let conversations = deepseek_conversation_store();
        let body = serde_json::json!({
            "model": "deepseek-v4-pro",
            "input": "hello",
            "tools": [{
                "type": "function",
                "name": "shell",
                "parameters": {"type": "object"}
            }],
            "tool_choice": mode
        });

        let translated =
            runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
                .expect("request should translate");
        let translated: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

        assert_eq!(translated["tool_choice"], mode);
    }
}

#[test]
fn deepseek_request_translation_rejects_unknown_string_tool_choice() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "tool_choice": "always"
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("unknown tool_choice string should fail");

    assert!(
        error
            .to_string()
            .contains("tool_choice string `always` is not supported")
    );
}

#[test]
fn deepseek_request_translation_rejects_unknown_object_tool_choice() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "tool_choice": {"type": "web_search"}
    });

    let error =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect_err("unknown tool_choice object should fail");

    assert!(
        error
            .to_string()
            .contains("tool_choice type `web_search` is not supported")
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
    conversations.insert(
        "resp_prev",
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
