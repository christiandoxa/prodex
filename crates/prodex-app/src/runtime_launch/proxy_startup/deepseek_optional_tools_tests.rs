#[cfg(test)]
mod tests {
    use super::super::*;
    use prodex_cli::SUPER_DEEPSEEK_DEFAULT_MODEL;
    use prodex_provider_core::{
        deepseek_provider_core_rtk_wrapped_tool_arguments,
        provider_core_chat_compatible_responses_value_from_chat_value,
    };

    fn conversation_store() -> RuntimeDeepSeekConversationStore {
        RuntimeDeepSeekConversationStore::default()
    }

    fn runtime_deepseek_responses_value_from_chat_value(
        value: &serde_json::Value,
        request_id: u64,
    ) -> serde_json::Value {
        provider_core_chat_compatible_responses_value_from_chat_value(
            value,
            request_id,
            "deepseek",
            RuntimeProviderBridgeKind::DeepSeek.chat_compatible_adapter_label(),
            SUPER_DEEPSEEK_DEFAULT_MODEL,
            "resp_deepseek",
        )
    }

    #[test]
    fn deepseek_maps_mcp_function_like_optional_tools_to_function_tools() {
        let conversations = conversation_store();
        let request = serde_json::json!({
            "model": "deepseek-v4-pro",
            "stream": true,
            "input": "compress and inspect the workspace",
            "tools": [
                {
                    "type": "mcp_tool",
                    "name": "mcp__prodex_sqz__sqz_read_file",
                    "description": "Read a file through SQZ.",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "path": {"type": "string"}
                        },
                        "required": ["path"]
                    }
                },
                {
                    "type": "mcp_tool",
                    "name": "mcp__prodex_token_savior__find_symbol",
                    "description": "Find a symbol through token-savior.",
                    "schema": {
                        "type": "object",
                        "properties": {
                            "name": {"type": "string"}
                        }
                    }
                },
                {
                    "type": "mcp_toolset",
                    "mcp_server_name": "prodex-sqz",
                    "name": "prodex-sqz"
                },
                {
                    "type": "mcp_toolset",
                    "mcp_server_name": "prodex-sqz",
                    "description": "SQZ optimizer MCP server.",
                    "default_config": {"enabled": false},
                    "configs": {
                        "compress": {"enabled": true},
                        "sqz_read_file": {"enabled": true}
                    }
                }
            ],
            "tool_choice": {
                "type": "mcp_tool",
                "name": "mcp__prodex_sqz__sqz_read_file"
            }
        });
        let translated = runtime_deepseek_chat_request_body(
            &serde_json::to_vec(&request).unwrap(),
            &conversations,
        )
        .expect("request should translate");
        let body: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
        let tools = body["tools"].as_array().unwrap();

        assert_eq!(tools.len(), 3);
        assert_eq!(tools[0]["type"], "function");
        assert_eq!(
            tools[0]["function"]["name"],
            "mcp__prodex_sqz__sqz_read_file"
        );
        assert_eq!(tools[0]["function"]["parameters"]["required"][0], "path");
        assert_eq!(
            tools[1]["function"]["name"],
            "mcp__prodex_token_savior__find_symbol"
        );
        assert_eq!(
            tools[1]["function"]["parameters"]["properties"]["name"]["type"],
            "string"
        );
        assert_eq!(tools[2]["function"]["name"], "mcp__prodex_sqz__compress");
        assert_eq!(
            tools[2]["function"]["parameters"]["additionalProperties"],
            true
        );
        assert_eq!(
            body["tool_choice"]["function"]["name"],
            "mcp__prodex_sqz__sqz_read_file"
        );
    }

    #[test]
    fn deepseek_replays_mcp_call_with_result_as_chat_tool_turn() {
        let conversations = conversation_store();
        let request = serde_json::json!({
            "model": "deepseek-v4-pro",
            "stream": true,
            "input": [
                {
                    "type": "message",
                    "role": "user",
                    "content": [{"type": "input_text", "text": "compress this text"}]
                },
                {
                    "type": "mcp_call",
                    "id": "call_sqz_1",
                    "name": "mcp__prodex_sqz__compress",
                    "arguments": {"text": "large repeated content"},
                    "output": "ref:abc123"
                }
            ]
        });
        let translated = runtime_deepseek_chat_request_body(
            &serde_json::to_vec(&request).unwrap(),
            &conversations,
        )
        .expect("request should translate");
        let body: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
        let messages = body["messages"].as_array().unwrap();

        assert_eq!(messages[0]["role"], "user");
        assert_eq!(messages[1]["role"], "assistant");
        assert_eq!(messages[1]["tool_calls"][0]["id"], "call_sqz_1");
        assert_eq!(
            messages[1]["tool_calls"][0]["function"]["name"],
            "mcp__prodex_sqz__compress"
        );
        assert_eq!(
            messages[1]["tool_calls"][0]["function"]["arguments"],
            r#"{"text":"large repeated content"}"#
        );
        assert_eq!(messages[2]["role"], "tool");
        assert_eq!(messages[2]["tool_call_id"], "call_sqz_1");
        assert_eq!(messages[2]["content"], "ref:abc123");
        assert_eq!(messages.len(), 3);
    }

    #[test]
    fn deepseek_mcp_tool_result_without_previous_response_id_replays_history_by_call_id() {
        let conversations = conversation_store();
        runtime_deepseek_store_conversation(
            &conversations,
            "chatcmpl_mcp_1",
            vec![serde_json::json!({
                "role": "user",
                "content": "find symbol context",
            })],
            vec![serde_json::json!({
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": "call_token_1",
                    "type": "function",
                    "function": {
                        "name": "mcp__prodex_token_savior__find_symbol",
                        "arguments": "{\"name\":\"runtime_deepseek_chat_request_body\"}"
                    }
                }]
            })],
        );

        let next_request = serde_json::json!({
            "model": "deepseek-v4-pro",
            "stream": true,
            "input": [{
                "type": "mcp_tool_result",
                "call_id": "call_token_1",
                "content": [{"type": "output_text", "text": "deepseek_rewrite.rs:23"}]
            }]
        });
        let translated = runtime_deepseek_chat_request_body(
            &serde_json::to_vec(&next_request).unwrap(),
            &conversations,
        )
        .expect("request should translate");
        let body: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
        let messages = body["messages"].as_array().unwrap();

        assert_eq!(messages[0]["content"], "find symbol context");
        assert_eq!(messages[1]["role"], "assistant");
        assert_eq!(messages[1]["tool_calls"][0]["id"], "call_token_1");
        assert_eq!(messages[2]["role"], "tool");
        assert_eq!(messages[2]["tool_call_id"], "call_token_1");
        assert_eq!(messages[2]["content"], "deepseek_rewrite.rs:23");
    }

    #[test]
    fn deepseek_response_restores_optional_mcp_function_call_names_for_codex() {
        let response = serde_json::json!({
            "id": "chatcmpl_sqz_1",
            "model": "deepseek-v4-pro",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_sqz_1",
                        "type": "function",
                        "function": {
                            "name": "mcp__prodex_sqz__compress",
                            "arguments": "{\"text\":\"large content\"}"
                        }
                    }]
                }
            }]
        });

        let responses = runtime_deepseek_responses_value_from_chat_value(&response, 7);

        assert_eq!(responses["output"][0]["type"], "function_call");
        assert_eq!(responses["output"][0]["call_id"], "call_sqz_1");
        assert_eq!(responses["output"][0]["namespace"], "mcp__prodex_sqz");
        assert_eq!(responses["output"][0]["name"], "compress");
        assert_eq!(
            responses["output"][0]["arguments"],
            "{\"text\":\"large content\"}"
        );
    }

    #[test]
    fn deepseek_wraps_claw_compactor_shell_benchmarks_with_rtk() {
        let arguments = deepseek_provider_core_rtk_wrapped_tool_arguments(
            "exec",
            r#"{"cmd":"claw-compactor benchmark /workspace --json"}"#,
        );
        let arguments: serde_json::Value = serde_json::from_str(&arguments).unwrap();

        assert_eq!(
            arguments["cmd"],
            "rtk claw-compactor benchmark /workspace --json"
        );
    }
}
