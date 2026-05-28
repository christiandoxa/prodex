#[cfg(test)]
mod tests {
    use super::super::*;
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};

    fn conversation_store() -> RuntimeDeepSeekConversationStore {
        Arc::new(Mutex::new(BTreeMap::new()))
    }

    #[test]
    fn deepseek_tool_turn_replays_reasoning_content_from_previous_response() {
        let conversations = conversation_store();
        let first_turn_messages = vec![serde_json::json!({
            "role": "user",
            "content": "read package metadata",
        })];
        let response = serde_json::json!({
            "id": "chatcmpl_1",
            "model": "deepseek-v4-pro",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "reasoning_content": "Need to inspect the package file before answering.",
                    "content": "I will inspect it.",
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {
                            "name": "shell",
                            "arguments": "{\"cmd\":\"cat package.json\"}"
                        }
                    }]
                }
            }]
        });
        runtime_deepseek_store_conversation(
            &conversations,
            "chatcmpl_1",
            first_turn_messages,
            runtime_deepseek_chat_assistant_messages_from_response_value(&response),
        );

        let next_request = serde_json::json!({
            "model": "deepseek-v4-pro",
            "stream": true,
            "previous_response_id": "chatcmpl_1",
            "input": [{
                "type": "function_call_output",
                "call_id": "call_1",
                "output": "{\"name\":\"prodex\"}"
            }],
            "reasoning": {"effort": "xhigh"}
        });
        let translated = runtime_deepseek_chat_request_body(
            &serde_json::to_vec(&next_request).unwrap(),
            &conversations,
        )
        .expect("request should translate");
        let body: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

        assert_eq!(
            body["messages"][1]["reasoning_content"],
            "Need to inspect the package file before answering."
        );
        assert_eq!(body["messages"][1]["content"], "I will inspect it.");
        assert_eq!(body["messages"][1]["tool_calls"][0]["id"], "call_1");
        assert_eq!(body["messages"][2]["role"], "tool");
        assert_eq!(body["reasoning_effort"], "max");
        assert_eq!(body["thinking"]["type"], "enabled");
    }

    #[test]
    fn deepseek_tool_turn_keeps_tool_output_adjacent_when_instructions_repeat() {
        let conversations = conversation_store();
        let instructions = "You are Codex. Keep answers concise.";
        runtime_deepseek_store_conversation(
            &conversations,
            "chatcmpl_1",
            vec![
                serde_json::json!({
                    "role": "system",
                    "content": instructions,
                }),
                serde_json::json!({
                    "role": "user",
                    "content": "read recent commits",
                }),
            ],
            vec![serde_json::json!({
                "role": "assistant",
                "reasoning_content": "Need commit history.",
                "content": null,
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": {
                        "name": "shell",
                        "arguments": "{\"cmd\":\"git log --oneline -10\"}"
                    }
                }]
            })],
        );

        let next_request = serde_json::json!({
            "model": "deepseek-v4-pro",
            "stream": true,
            "instructions": instructions,
            "previous_response_id": "chatcmpl_1",
            "input": [{
                "type": "function_call_output",
                "call_id": "call_1",
                "output": "a6f12b1 chore(release): prepare 0.125.0"
            }],
            "reasoning": {"effort": "xhigh"}
        });
        let translated = runtime_deepseek_chat_request_body(
            &serde_json::to_vec(&next_request).unwrap(),
            &conversations,
        )
        .expect("request should translate");
        let body: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
        let messages = body["messages"].as_array().unwrap();

        assert_eq!(messages[0]["role"], "system");
        assert_eq!(messages[1]["role"], "user");
        assert_eq!(messages[2]["role"], "assistant");
        assert_eq!(messages[2]["content"], "");
        assert_eq!(messages[2]["tool_calls"][0]["id"], "call_1");
        assert_eq!(messages[3]["role"], "tool");
        assert_eq!(messages[3]["tool_call_id"], "call_1");
        assert_eq!(messages.len(), 4);
    }

    #[test]
    fn deepseek_tool_output_without_previous_response_id_replays_history_by_call_id() {
        let conversations = conversation_store();
        runtime_deepseek_store_conversation(
            &conversations,
            "chatcmpl_1",
            vec![serde_json::json!({
                "role": "user",
                "content": "read commit history",
            })],
            vec![serde_json::json!({
                "role": "assistant",
                "reasoning_content": "Need to inspect recent commits before answering.",
                "content": null,
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": {
                        "name": "shell",
                        "arguments": "{\"cmd\":\"git log --oneline -10\"}"
                    }
                }]
            })],
        );

        let next_request = serde_json::json!({
            "model": "deepseek-v4-pro",
            "stream": true,
            "input": [{
                "type": "function_call_output",
                "call_id": "call_1",
                "output": "befa38d chore(release): prepare 0.124.0"
            }],
            "reasoning": {"effort": "xhigh"}
        });
        let translated = runtime_deepseek_chat_request_body(
            &serde_json::to_vec(&next_request).unwrap(),
            &conversations,
        )
        .expect("request should translate");
        let body: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

        assert_eq!(body["messages"][0]["content"], "read commit history");
        assert_eq!(
            body["messages"][1]["reasoning_content"],
            "Need to inspect recent commits before answering."
        );
        assert_eq!(body["messages"][1]["content"], "");
        assert_eq!(body["messages"][1]["tool_calls"][0]["id"], "call_1");
        assert_eq!(body["messages"][2]["tool_call_id"], "call_1");
    }

    #[test]
    fn deepseek_tool_output_skips_duplicate_function_call_from_codex_replay() {
        let conversations = conversation_store();
        runtime_deepseek_store_conversation(
            &conversations,
            "chatcmpl_1",
            vec![serde_json::json!({
                "role": "user",
                "content": "read commit history",
            })],
            vec![serde_json::json!({
                "role": "assistant",
                "reasoning_content": "Need to inspect recent commits before answering.",
                "content": null,
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": {
                        "name": "shell",
                        "arguments": "{\"cmd\":\"git log --oneline -3\"}"
                    }
                }]
            })],
        );

        let next_request = serde_json::json!({
            "model": "deepseek-v4-pro",
            "stream": true,
            "previous_response_id": "chatcmpl_1",
            "input": [
                {
                    "type": "function_call",
                    "call_id": "call_1",
                    "name": "shell",
                    "arguments": "{\"cmd\":\"git log --oneline -3\"}"
                },
                {
                    "type": "function_call_output",
                    "call_id": "call_1",
                    "output": "a6f12b1 chore(release): prepare 0.125.0"
                }
            ],
            "reasoning": {"effort": "xhigh"}
        });
        let translated = runtime_deepseek_chat_request_body(
            &serde_json::to_vec(&next_request).unwrap(),
            &conversations,
        )
        .expect("request should translate");
        let body: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
        let messages = body["messages"].as_array().unwrap();

        assert_eq!(messages[0]["role"], "user");
        assert_eq!(messages[1]["role"], "assistant");
        assert_eq!(messages[1]["tool_calls"][0]["id"], "call_1");
        assert_eq!(messages[2]["role"], "tool");
        assert_eq!(messages[2]["tool_call_id"], "call_1");
        assert_eq!(messages.len(), 3);
    }

    #[test]
    fn deepseek_tool_output_skips_duplicate_context_messages_from_codex_replay() {
        let conversations = conversation_store();
        runtime_deepseek_store_conversation(
            &conversations,
            "chatcmpl_1",
            vec![
                serde_json::json!({
                    "role": "system",
                    "content": "You are Codex.",
                }),
                serde_json::json!({
                    "role": "user",
                    "content": "read commit history",
                }),
            ],
            vec![serde_json::json!({
                "role": "assistant",
                "reasoning_content": "Need to inspect recent commits before answering.",
                "content": null,
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": {
                        "name": "shell",
                        "arguments": "{\"cmd\":\"git log --oneline -3\"}"
                    }
                }]
            })],
        );

        let next_request = serde_json::json!({
            "model": "deepseek-v4-pro",
            "stream": true,
            "previous_response_id": "chatcmpl_1",
            "input": [
                {
                    "type": "message",
                    "role": "system",
                    "content": [{"type": "input_text", "text": "You are Codex."}]
                },
                {
                    "type": "message",
                    "role": "user",
                    "content": [{"type": "input_text", "text": "read commit history"}]
                },
                {
                    "type": "function_call_output",
                    "call_id": "call_1",
                    "output": "a6f12b1 chore(release): prepare 0.125.0"
                }
            ],
            "reasoning": {"effort": "xhigh"}
        });
        let translated = runtime_deepseek_chat_request_body(
            &serde_json::to_vec(&next_request).unwrap(),
            &conversations,
        )
        .expect("request should translate");
        let body: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
        let messages = body["messages"].as_array().unwrap();

        assert_eq!(messages[0]["role"], "system");
        assert_eq!(messages[1]["role"], "user");
        assert_eq!(messages[2]["role"], "assistant");
        assert_eq!(messages[2]["tool_calls"][0]["id"], "call_1");
        assert_eq!(messages[3]["role"], "tool");
        assert_eq!(messages[3]["tool_call_id"], "call_1");
        assert_eq!(messages.len(), 4);
    }

    #[test]
    fn deepseek_thinking_tool_turn_adds_empty_reasoning_content_when_missing() {
        let conversations = conversation_store();
        runtime_deepseek_store_conversation(
            &conversations,
            "chatcmpl_1",
            vec![serde_json::json!({
                "role": "user",
                "content": "read commit history",
            })],
            vec![serde_json::json!({
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": {
                        "name": "shell",
                        "arguments": "{\"cmd\":\"git log --oneline -3\"}"
                    }
                }]
            })],
        );

        let next_request = serde_json::json!({
            "model": "deepseek-v4-pro",
            "stream": true,
            "previous_response_id": "chatcmpl_1",
            "input": [{
                "type": "function_call_output",
                "call_id": "call_1",
                "output": "f05b28c chore(release): prepare 0.127.0"
            }],
            "reasoning": {"effort": "xhigh"}
        });
        let translated = runtime_deepseek_chat_request_body(
            &serde_json::to_vec(&next_request).unwrap(),
            &conversations,
        )
        .expect("request should translate");
        let body: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
        let messages = body["messages"].as_array().unwrap();

        assert_eq!(messages[1]["role"], "assistant");
        assert_eq!(messages[1]["content"], "");
        assert_eq!(messages[1]["reasoning_content"], "");
        assert_eq!(messages[1]["tool_calls"][0]["id"], "call_1");
        assert_eq!(messages[2]["role"], "tool");
    }

    #[test]
    fn deepseek_tool_turn_trims_unanswered_parallel_tool_calls() {
        let conversations = conversation_store();
        runtime_deepseek_store_conversation(
            &conversations,
            "chatcmpl_1",
            vec![serde_json::json!({
                "role": "user",
                "content": "inspect repo and package metadata",
            })],
            vec![serde_json::json!({
                "role": "assistant",
                "reasoning_content": "Need two local reads.",
                "content": null,
                "tool_calls": [
                    {
                        "id": "call_1",
                        "type": "function",
                        "function": {
                            "name": "shell",
                            "arguments": "{\"cmd\":\"git status --short\"}"
                        }
                    },
                    {
                        "id": "call_2",
                        "type": "function",
                        "function": {
                            "name": "shell",
                            "arguments": "{\"cmd\":\"cat package.json\"}"
                        }
                    }
                ]
            })],
        );

        let next_request = serde_json::json!({
            "model": "deepseek-v4-pro",
            "stream": true,
            "previous_response_id": "chatcmpl_1",
            "input": [{
                "type": "function_call_output",
                "call_id": "call_1",
                "output": "M crates/prodex-app/src/runtime_launch/proxy_startup/deepseek_rewrite.rs"
            }],
            "reasoning": {"effort": "xhigh"}
        });
        let translated = runtime_deepseek_chat_request_body(
            &serde_json::to_vec(&next_request).unwrap(),
            &conversations,
        )
        .expect("request should translate");
        let body: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
        let messages = body["messages"].as_array().unwrap();

        assert_eq!(messages[0]["role"], "user");
        assert_eq!(messages[1]["role"], "assistant");
        assert_eq!(messages[1]["tool_calls"].as_array().unwrap().len(), 1);
        assert_eq!(messages[1]["tool_calls"][0]["id"], "call_1");
        assert_eq!(messages[2]["role"], "tool");
        assert_eq!(messages[2]["tool_call_id"], "call_1");
        assert_eq!(messages.len(), 3);
        assert!(!serde_json::to_string(messages).unwrap().contains("call_2"));
    }

    #[test]
    fn deepseek_tool_turn_moves_late_tool_output_next_to_tool_call() {
        let conversations = conversation_store();
        runtime_deepseek_store_conversation(
            &conversations,
            "chatcmpl_1",
            vec![serde_json::json!({
                "role": "user",
                "content": "read commit history",
            })],
            vec![serde_json::json!({
                "role": "assistant",
                "reasoning_content": "Need recent commits.",
                "content": null,
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": {
                        "name": "shell",
                        "arguments": "{\"cmd\":\"git log --oneline -3\"}"
                    }
                }]
            })],
        );

        let next_request = serde_json::json!({
            "model": "deepseek-v4-pro",
            "stream": true,
            "previous_response_id": "chatcmpl_1",
            "input": [
                {
                    "type": "message",
                    "role": "user",
                    "content": [{"type": "input_text", "text": "continue after reading it"}]
                },
                {
                    "type": "function_call_output",
                    "call_id": "call_1",
                    "output": "7828df3 fix(deepseek): tolerate missing thinking replay"
                }
            ],
            "reasoning": {"effort": "xhigh"}
        });
        let translated = runtime_deepseek_chat_request_body(
            &serde_json::to_vec(&next_request).unwrap(),
            &conversations,
        )
        .expect("request should translate");
        let body: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
        let messages = body["messages"].as_array().unwrap();

        assert_eq!(messages[0]["role"], "user");
        assert_eq!(messages[1]["role"], "assistant");
        assert_eq!(messages[1]["tool_calls"][0]["id"], "call_1");
        assert_eq!(messages[2]["role"], "tool");
        assert_eq!(messages[2]["tool_call_id"], "call_1");
        assert_eq!(messages[3]["role"], "user");
        assert_eq!(messages[3]["content"], "continue after reading it");
        assert_eq!(messages.len(), 4);
    }

    #[test]
    fn deepseek_followup_skips_replayed_tool_output_after_final_answer() {
        let conversations = conversation_store();
        runtime_deepseek_store_conversation(
            &conversations,
            "chatcmpl_2",
            vec![
                serde_json::json!({
                    "role": "user",
                    "content": "read commit history",
                }),
                serde_json::json!({
                    "role": "assistant",
                    "reasoning_content": "Need to inspect recent commits before answering.",
                    "content": "",
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {
                            "name": "shell",
                            "arguments": "{\"cmd\":\"git log --oneline -3\"}"
                        }
                    }]
                }),
                serde_json::json!({
                    "role": "tool",
                    "tool_call_id": "call_1",
                    "content": "335a5dd chore(release): prepare 0.126.0",
                }),
            ],
            vec![serde_json::json!({
                "role": "assistant",
                "content": "335a5dd chore(release): prepare 0.126.0",
            })],
        );

        let next_request = serde_json::json!({
            "model": "deepseek-v4-pro",
            "stream": true,
            "previous_response_id": "chatcmpl_2",
            "input": [
                {
                    "type": "function_call_output",
                    "call_id": "call_1",
                    "output": "335a5dd chore(release): prepare 0.126.0"
                },
                {
                    "type": "message",
                    "role": "user",
                    "content": [{"type": "input_text", "text": "what else can be improved?"}]
                }
            ],
            "reasoning": {"effort": "xhigh"}
        });
        let translated = runtime_deepseek_chat_request_body(
            &serde_json::to_vec(&next_request).unwrap(),
            &conversations,
        )
        .expect("request should translate");
        let body: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
        let messages = body["messages"].as_array().unwrap();

        assert_eq!(messages[0]["role"], "user");
        assert_eq!(messages[1]["role"], "assistant");
        assert_eq!(messages[2]["role"], "tool");
        assert_eq!(messages[3]["role"], "assistant");
        assert_eq!(messages[4]["role"], "user");
        assert_eq!(messages[4]["content"], "what else can be improved?");
        assert_eq!(
            messages
                .iter()
                .filter(
                    |message| message.get("role").and_then(serde_json::Value::as_str)
                        == Some("tool")
                )
                .count(),
            1
        );
        assert_eq!(messages.len(), 5);
    }

    #[test]
    fn deepseek_thinking_mode_drops_tool_choice() {
        let conversations = conversation_store();
        let request = serde_json::json!({
            "model": "deepseek-v4-pro",
            "stream": true,
            "input": "read recent commits",
            "tools": [{
                "type": "function",
                "name": "shell",
                "parameters": {"type": "object"}
            }],
            "tool_choice": {
                "type": "function",
                "name": "shell"
            },
            "reasoning": {"effort": "xhigh"}
        });
        let translated = runtime_deepseek_chat_request_body(
            &serde_json::to_vec(&request).unwrap(),
            &conversations,
        )
        .expect("request should translate");
        let body: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

        assert_eq!(body["thinking"]["type"], "enabled");
        assert!(body.get("tool_choice").is_none());
    }

    #[test]
    fn deepseek_wraps_noisy_shell_arguments_with_rtk() {
        let arguments = runtime_deepseek_rtk_wrapped_tool_arguments(
            "shell",
            r#"{"cmd":"cargo test -q login -- --test-threads=1"}"#,
        );
        let arguments: serde_json::Value = serde_json::from_str(&arguments).unwrap();

        assert_eq!(
            arguments["cmd"],
            "rtk cargo test -q login -- --test-threads=1"
        );
    }

    #[test]
    fn deepseek_wraps_noisy_shell_segment_after_cd() {
        let arguments = runtime_deepseek_rtk_wrapped_tool_arguments(
            "shell",
            r#"{"cmd":"cd /home/doxa/IdeaProjects/prodex && cargo check -q"}"#,
        );
        let arguments: serde_json::Value = serde_json::from_str(&arguments).unwrap();

        assert_eq!(
            arguments["cmd"],
            "cd /home/doxa/IdeaProjects/prodex && rtk cargo check -q"
        );
    }

    #[test]
    fn deepseek_does_not_double_wrap_rtk_shell_arguments() {
        let arguments = runtime_deepseek_rtk_wrapped_tool_arguments(
            "shell",
            r#"{"cmd":"rtk cargo test -q login"}"#,
        );
        let arguments: serde_json::Value = serde_json::from_str(&arguments).unwrap();

        assert_eq!(arguments["cmd"], "rtk cargo test -q login");
    }

    #[test]
    fn deepseek_keeps_quiet_shell_arguments_unchanged() {
        let arguments = runtime_deepseek_rtk_wrapped_tool_arguments("shell", r#"{"cmd":"pwd"}"#);
        let arguments: serde_json::Value = serde_json::from_str(&arguments).unwrap();

        assert_eq!(arguments["cmd"], "pwd");
    }
}
