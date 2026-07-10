use super::deepseek_rewrite::{
    RuntimeDeepSeekRewriteOptions, runtime_deepseek_chat_request_body_with_options,
};
#[path = "local_rewrite_deepseek_send.rs"]
mod local_rewrite_deepseek_send;
pub(super) use local_rewrite_deepseek_send::send_runtime_deepseek_upstream_request;

#[cfg(test)]
mod tests {
    use super::super::deepseek_rewrite::RuntimeDeepSeekConversationStore;
    use super::{RuntimeDeepSeekRewriteOptions, runtime_deepseek_chat_request_body_with_options};
    use prodex_provider_core::{
        ProviderEndpoint, ProviderId, ProviderTransformInput, deepseek_provider_core_request_body,
        deepseek_provider_core_simple_request, provider_translator,
    };

    fn conversation_store() -> RuntimeDeepSeekConversationStore {
        RuntimeDeepSeekConversationStore::default()
    }

    fn simple_request(
        body: &[u8],
        conversations: &super::super::deepseek_rewrite::RuntimeDeepSeekConversationStore,
    ) -> bool {
        deepseek_provider_core_simple_request(body, |previous_response_id| {
            conversations
                .lock()
                .ok()
                .is_some_and(|store| store.contains_key(previous_response_id))
        })
    }

    #[test]
    fn deepseek_provider_core_simple_request_accepts_plain_text_first_turn() {
        let body = serde_json::to_vec(&serde_json::json!({
            "model": "deepseek-chat",
            "input": "hello",
            "stream": true,
            "temperature": 0.2
        }))
        .unwrap();
        assert!(simple_request(&body, &conversation_store()));
    }

    #[test]
    fn deepseek_provider_core_simple_request_accepts_message_content_arrays() {
        let body = serde_json::to_vec(&serde_json::json!({
            "model": "deepseek-chat",
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
                }
            ]
        }))
        .unwrap();
        assert!(simple_request(&body, &conversation_store()));
    }

    #[test]
    fn deepseek_provider_core_simple_request_accepts_assistant_tool_history() {
        let body = serde_json::to_vec(&serde_json::json!({
            "model": "deepseek-chat",
            "input": [
                {
                    "type": "message",
                    "role": "user",
                    "content": [{"type": "input_text", "text": "find the symbol"}]
                },
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [{"type": "output_text", "text": ""}],
                    "tool_calls": [
                        {
                            "id":"call_1",
                            "type":"function",
                            "function":{
                                "name":"grep",
                                "arguments":"{\"pattern\":\"ProviderTranslator\"}"
                            }
                        }
                    ]
                },
                {
                    "type": "message",
                    "role": "tool",
                    "tool_call_id": "call_1",
                    "content": [{"type": "output_text", "text": "{\"match_count\":1}"}]
                }
            ]
        }))
        .unwrap();
        assert!(simple_request(&body, &conversation_store()));
    }

    #[test]
    fn deepseek_provider_core_simple_request_accepts_function_call_and_output_items() {
        let body = serde_json::to_vec(&serde_json::json!({
            "model": "deepseek-chat",
            "input": [
                {
                    "type": "message",
                    "role": "user",
                    "content": [{"type": "input_text", "text": "find the symbol"}]
                },
                {
                    "type": "function_call",
                    "call_id": "call_1",
                    "name": "grep",
                    "arguments": {"pattern": "ProviderTranslator"}
                },
                {
                    "type": "function_call_output",
                    "call_id": "call_1",
                    "output": "{\"match_count\":1}"
                }
            ]
        }))
        .unwrap();
        assert!(simple_request(&body, &conversation_store()));
    }

    #[test]
    fn deepseek_provider_core_simple_request_accepts_custom_and_shell_call_items() {
        let body = serde_json::to_vec(&serde_json::json!({
            "model": "deepseek-chat",
            "input": [
                {
                    "type": "custom_tool_call",
                    "call_id": "call_patch_1",
                    "name": "apply_patch",
                    "input": "*** Begin Patch\n*** End Patch"
                },
                {
                    "type": "custom_tool_call_output",
                    "call_id": "call_patch_1",
                    "output": "applied"
                },
                {
                    "type": "local_shell_call",
                    "call_id": "call_shell_1",
                    "action": {
                        "command": ["git", "status", "--short"]
                    }
                },
                {
                    "type": "function_call_output",
                    "call_id": "call_shell_1",
                    "output": " M Cargo.toml"
                }
            ]
        }))
        .unwrap();
        assert!(simple_request(&body, &conversation_store()));
    }

    #[test]
    fn deepseek_provider_core_simple_request_accepts_mcp_call_and_result_items() {
        let body = serde_json::to_vec(&serde_json::json!({
            "model": "deepseek-chat",
            "input": [
                {
                    "type": "mcp_call",
                    "id": "call_sqz_1",
                    "name": "mcp__prodex_sqz__compress",
                    "arguments": {"text": "large repeated content"},
                    "output": "ref:abc123"
                },
                {
                    "type": "mcp_tool_result",
                    "call_id": "call_sqz_1",
                    "content": [{"type": "output_text", "text": "ref:abc123"}]
                }
            ]
        }))
        .unwrap();
        assert!(simple_request(&body, &conversation_store()));
    }

    #[test]
    fn deepseek_provider_core_simple_request_accepts_unbound_continuations_but_rejects_bound_ones_and_tools()
     {
        let continuation = serde_json::to_vec(&serde_json::json!({
            "model": "deepseek-chat",
            "input": "hello",
            "previous_response_id": "resp_1"
        }))
        .unwrap();
        let empty = conversation_store();
        assert!(simple_request(&continuation, &empty));

        let bound = conversation_store();
        bound.lock().unwrap().insert(
            "resp_1".to_string(),
            vec![serde_json::json!({
                "role": "assistant",
                "content": "stored history"
            })],
        );
        assert!(!simple_request(&continuation, &bound));

        let tools = serde_json::to_vec(&serde_json::json!({
            "model": "deepseek-chat",
            "input": "hello",
            "tools": [{"type":"web_search_preview"}]
        }))
        .unwrap();
        assert!(!simple_request(&tools, &conversation_store()));
    }

    #[test]
    fn deepseek_provider_core_simple_request_accepts_supported_response_format() {
        let body = serde_json::to_vec(&serde_json::json!({
            "model": "deepseek-chat",
            "input": "hello",
            "response_format": {
                "type": "json_schema",
                "json_schema": {
                    "name": "answer",
                    "schema": {
                        "type": "object",
                        "properties": {
                            "answer": {"type": "string"}
                        },
                        "required": ["answer"]
                    }
                }
            }
        }))
        .unwrap();
        assert!(simple_request(&body, &conversation_store()));
    }

    #[test]
    fn deepseek_provider_core_body_matches_app_translation_for_simple_message_arrays() {
        let body = serde_json::to_vec(&serde_json::json!({
            "model": "deepseek-chat",
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
                }
            ]
        }))
        .unwrap();
        assert!(simple_request(&body, &conversation_store()));

        let translated = runtime_deepseek_chat_request_body_with_options(
            &body,
            &conversation_store(),
            RuntimeDeepSeekRewriteOptions::default(),
        )
        .unwrap();
        let result = provider_translator(ProviderId::DeepSeek).transform_request(
            ProviderTransformInput::new(ProviderEndpoint::Responses, body),
        );
        let provider_core_body = deepseek_provider_core_request_body(&result).unwrap();

        let provider_core_json: serde_json::Value =
            serde_json::from_slice(&provider_core_body).unwrap();
        let app_json: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
        assert_eq!(provider_core_json, app_json);
    }

    #[test]
    fn deepseek_provider_core_body_matches_app_translation_for_unbound_previous_response_id() {
        let body = serde_json::to_vec(&serde_json::json!({
            "model": "deepseek-chat",
            "input": "hello deepseek",
            "stream": false,
            "previous_response_id": "resp_1"
        }))
        .unwrap();
        let conversations = conversation_store();
        assert!(simple_request(&body, &conversations));

        let translated = runtime_deepseek_chat_request_body_with_options(
            &body,
            &conversations,
            RuntimeDeepSeekRewriteOptions::default(),
        )
        .unwrap();
        let result = provider_translator(ProviderId::DeepSeek).transform_request(
            ProviderTransformInput::new(ProviderEndpoint::Responses, body),
        );
        let provider_core_body = deepseek_provider_core_request_body(&result).unwrap();

        let provider_core_json: serde_json::Value =
            serde_json::from_slice(&provider_core_body).unwrap();
        let app_json: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
        assert_eq!(provider_core_json, app_json);
    }

    #[test]
    fn deepseek_provider_core_body_matches_app_translation_for_plain_function_tools() {
        let body = serde_json::to_vec(&serde_json::json!({
            "model": "deepseek-chat",
            "input": "find the symbol",
            "tools": [{
                "type": "function",
                "function": {
                    "name": "grep",
                    "description": "Search source files.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "pattern": {"type": "string"}
                        },
                        "required": ["pattern"]
                    }
                }
            }],
            "tool_choice": {
                "type": "function",
                "name": "grep"
            }
        }))
        .unwrap();
        let conversations = conversation_store();
        assert!(simple_request(&body, &conversations));

        let translated = runtime_deepseek_chat_request_body_with_options(
            &body,
            &conversations,
            RuntimeDeepSeekRewriteOptions::default(),
        )
        .unwrap();
        let result = provider_translator(ProviderId::DeepSeek).transform_request(
            ProviderTransformInput::new(ProviderEndpoint::Responses, body),
        );
        let provider_core_body = deepseek_provider_core_request_body(&result).unwrap();

        let provider_core_json: serde_json::Value =
            serde_json::from_slice(&provider_core_body).unwrap();
        let app_json: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
        assert_eq!(provider_core_json, app_json);
    }

    #[test]
    fn deepseek_provider_core_body_matches_app_translation_for_instructions_without_history() {
        let body = serde_json::to_vec(&serde_json::json!({
            "model": "deepseek-chat",
            "instructions": "You are Codex. Keep answers concise.",
            "input": "hello deepseek"
        }))
        .unwrap();
        let conversations = conversation_store();
        assert!(simple_request(&body, &conversations));

        let translated = runtime_deepseek_chat_request_body_with_options(
            &body,
            &conversations,
            RuntimeDeepSeekRewriteOptions::default(),
        )
        .unwrap();
        let result = provider_translator(ProviderId::DeepSeek).transform_request(
            ProviderTransformInput::new(ProviderEndpoint::Responses, body),
        );
        let provider_core_body = deepseek_provider_core_request_body(&result).unwrap();

        let provider_core_json: serde_json::Value =
            serde_json::from_slice(&provider_core_body).unwrap();
        let app_json: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
        assert_eq!(provider_core_json, app_json);
    }

    #[test]
    fn deepseek_provider_core_body_matches_app_translation_for_json_schema_response_format() {
        let body = serde_json::to_vec(&serde_json::json!({
            "model": "deepseek-chat",
            "input": "return json",
            "response_format": {
                "type": "json_schema",
                "json_schema": {
                    "name": "answer",
                    "schema": {
                        "type": "object",
                        "properties": {
                            "answer": {"type": "string"}
                        },
                        "required": ["answer"]
                    }
                }
            }
        }))
        .unwrap();
        let conversations = conversation_store();
        assert!(simple_request(&body, &conversations));

        let translated = runtime_deepseek_chat_request_body_with_options(
            &body,
            &conversations,
            RuntimeDeepSeekRewriteOptions::default(),
        )
        .unwrap();
        let result = provider_translator(ProviderId::DeepSeek).transform_request(
            ProviderTransformInput::new(ProviderEndpoint::Responses, body),
        );
        let provider_core_body = deepseek_provider_core_request_body(&result).unwrap();

        let provider_core_json: serde_json::Value =
            serde_json::from_slice(&provider_core_body).unwrap();
        let app_json: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
        assert_eq!(provider_core_json, app_json);
    }
}
