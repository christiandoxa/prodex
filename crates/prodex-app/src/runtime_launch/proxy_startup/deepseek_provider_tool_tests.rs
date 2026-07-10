#[cfg(test)]
mod tests {
    use super::super::*;
    use prodex_cli::SUPER_DEEPSEEK_DEFAULT_MODEL;
    use prodex_provider_core::provider_core_chat_compatible_responses_value_from_chat_value;

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
    fn deepseek_response_translation_restores_namespace_tool_calls() {
        let response = serde_json::json!({
            "id": "chatcmpl_ns",
            "model": "deepseek-v4-pro",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "tool_calls": [{
                        "id": "call_sqz_1",
                        "type": "function",
                        "function": {
                            "name": "mcp__prodex_sqz__sqz_read_file",
                            "arguments": "{\"path\":\"README.md\"}"
                        }
                    }]
                }
            }]
        });

        let translated = runtime_deepseek_responses_value_from_chat_value(&response, 46);

        assert_eq!(translated["output"][0]["type"], "function_call");
        assert_eq!(translated["output"][0]["namespace"], "mcp__prodex_sqz");
        assert_eq!(translated["output"][0]["name"], "sqz_read_file");
        assert_eq!(translated["output"][0]["call_id"], "call_sqz_1");
    }

    #[test]
    fn deepseek_response_translation_maps_tool_search_function_calls() {
        let response = serde_json::json!({
            "id": "chatcmpl_search",
            "model": "deepseek-v4-pro",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "tool_calls": [{
                        "id": "call_search_1",
                        "type": "function",
                        "function": {
                            "name": "tool_search",
                            "arguments": "{\"query\":\"sqz read file\",\"limit\":2}"
                        }
                    }]
                }
            }]
        });

        let translated = runtime_deepseek_responses_value_from_chat_value(&response, 47);

        assert_eq!(translated["output"][0]["type"], "tool_search_call");
        assert_eq!(translated["output"][0]["execution"], "client");
        assert_eq!(translated["output"][0]["call_id"], "call_search_1");
        assert_eq!(
            translated["output"][0]["arguments"]["query"],
            "sqz read file"
        );
    }

    #[test]
    fn deepseek_response_translation_fails_malformed_tool_call_arguments() {
        let response = serde_json::json!({
            "id": "chatcmpl_bad_args",
            "model": "deepseek-v4-pro",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "tool_calls": [{
                        "id": "call_shell_1",
                        "type": "function",
                        "function": {
                            "name": "shell",
                            "arguments": "{\"cmd\":"
                        }
                    }]
                }
            }]
        });

        let translated = runtime_deepseek_responses_value_from_chat_value(&response, 48);

        assert_eq!(translated["status"], "failed");
        assert_eq!(translated["error"]["code"], "invalid_tool_call_arguments");
        assert!(
            translated["error"]["message"]
                .as_str()
                .unwrap()
                .contains("malformed JSON arguments")
        );
        assert!(translated["output"].as_array().unwrap().is_empty());
    }

    #[test]
    fn deepseek_response_translation_fails_tool_call_without_function_object() {
        let response = serde_json::json!({
            "id": "chatcmpl_bad_tool",
            "model": "deepseek-v4-pro",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "tool_calls": [{
                        "id": "call_missing_function",
                        "type": "function"
                    }]
                }
            }]
        });

        let translated = runtime_deepseek_responses_value_from_chat_value(&response, 48);

        assert_eq!(translated["status"], "failed");
        assert_eq!(translated["error"]["code"], "invalid_tool_call_arguments");
        assert!(
            translated["error"]["message"]
                .as_str()
                .unwrap()
                .contains("without a function object")
        );
        assert!(translated["output"].as_array().unwrap().is_empty());
    }

    #[test]
    fn deepseek_response_translation_fails_tool_call_without_function_name() {
        let response = serde_json::json!({
            "id": "chatcmpl_bad_name",
            "model": "deepseek-v4-pro",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "tool_calls": [{
                        "id": "call_missing_name",
                        "type": "function",
                        "function": {
                            "arguments": "{}"
                        }
                    }]
                }
            }]
        });

        let translated = runtime_deepseek_responses_value_from_chat_value(&response, 48);

        assert_eq!(translated["status"], "failed");
        assert_eq!(translated["error"]["code"], "invalid_tool_call_arguments");
        assert!(
            translated["error"]["message"]
                .as_str()
                .unwrap()
                .contains("without a function name")
        );
        assert!(translated["output"].as_array().unwrap().is_empty());
    }

    #[test]
    fn deepseek_response_translation_preserves_reasoning_content_metadata() {
        let response = serde_json::json!({
            "id": "chatcmpl_reasoning",
            "model": "deepseek-v4-pro",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "reasoning_content": "Need to inspect the tool output.",
                    "content": "Done."
                },
                "finish_reason": "stop"
            }]
        });

        let translated = runtime_deepseek_responses_value_from_chat_value(&response, 48);

        assert_eq!(
            translated["metadata"]["deepseek"]["reasoning_content"],
            "Need to inspect the tool output."
        );
        assert_eq!(translated["metadata"]["deepseek"]["finish_reason"], "stop");
        assert_eq!(translated["output"][0]["content"][0]["text"], "Done.");
    }

    #[test]
    fn deepseek_request_translation_maps_custom_apply_patch_tool() {
        let conversations = conversation_store();
        let request = serde_json::json!({
            "model": "deepseek-v4-pro",
            "input": "edit the file",
            "tools": [{
                "type": "custom",
                "name": "apply_patch",
                "description": "Use apply_patch to edit files.",
                "format": {
                    "type": "grammar",
                    "syntax": "lark",
                    "definition": "start: begin_patch hunk+ end_patch"
                }
            }],
            "tool_choice": {
                "type": "function",
                "name": "apply_patch"
            }
        });

        let translated = runtime_deepseek_chat_request_body(
            &serde_json::to_vec(&request).unwrap(),
            &conversations,
        )
        .expect("request should translate");
        let body: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

        assert_eq!(body["tools"][0]["function"]["name"], "apply_patch");
        assert!(
            body["tools"][0]["function"]["description"]
                .as_str()
                .unwrap()
                .contains("exact Codex apply_patch grammar")
        );
        assert_eq!(
            body["tools"][0]["function"]["parameters"]["properties"]["input"]["type"],
            "string"
        );
        assert_eq!(body["tool_choice"]["function"]["name"], "apply_patch");
    }

    #[test]
    fn deepseek_response_translation_maps_apply_patch_to_custom_tool_call() {
        let patch = "*** Begin Patch\n*** Add File: note.txt\n+hello\n*** End Patch";
        let response = serde_json::json!({
            "id": "chatcmpl_patch",
            "model": "deepseek-v4-pro",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "tool_calls": [{
                        "id": "call_patch_1",
                        "type": "function",
                        "function": {
                            "name": "apply_patch",
                            "arguments": serde_json::json!({"input": patch}).to_string()
                        }
                    }]
                }
            }]
        });

        let translated = runtime_deepseek_responses_value_from_chat_value(&response, 49);

        assert_eq!(translated["output"][0]["type"], "custom_tool_call");
        assert_eq!(translated["output"][0]["name"], "apply_patch");
        assert_eq!(translated["output"][0]["call_id"], "call_patch_1");
        assert_eq!(translated["output"][0]["input"], patch);
    }

    #[test]
    fn deepseek_request_translation_auto_web_search_rejects_without_documented_route() {
        let request = serde_json::json!({
            "model": "deepseek-v4-pro",
            "stream": true,
            "input": "find current Linux performance articles",
            "tools": [
                {
                    "type": "web_search_preview",
                    "context_size": "high",
                    "allowed_domains": ["example.com", "docs.example.com"],
                    "location": {
                        "type": "approximate",
                        "country": "US"
                    }
                },
                {
                    "type": "function",
                    "name": "shell",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "cmd": {"type": "string"}
                        },
                        "required": ["cmd"]
                    }
                }
            ]
        });

        let error = runtime_deepseek_chat_request_body(
            &serde_json::to_vec(&request).unwrap(),
            &conversation_store(),
        )
        .expect_err("auto web search should reject until a documented route exists");

        assert!(
            error
                .to_string()
                .contains("no documented native OpenAI Chat route")
        );
    }

    #[test]
    fn deepseek_request_translation_respects_web_search_mode_off() {
        let request = serde_json::json!({
            "model": "deepseek-v4-pro",
            "input": "search",
            "tools": [{"type": "web_search_preview"}]
        });

        let error = runtime_deepseek_chat_request_body_with_options(
            &serde_json::to_vec(&request).unwrap(),
            &conversation_store(),
            RuntimeDeepSeekRewriteOptions {
                strict_tools: false,
                web_search_mode: RuntimeDeepSeekWebSearchMode::Off,
            },
        )
        .expect_err("off mode should reject web search tools");

        assert!(error.to_string().contains("web search mode is off"));
    }

    #[test]
    fn deepseek_request_translation_rejects_unimplemented_web_search_modes() {
        let request = serde_json::json!({
            "model": "deepseek-v4-pro",
            "input": "search",
            "tools": [{"type": "web_search_preview"}]
        });

        for (mode, expected) in [
            (
                RuntimeDeepSeekWebSearchMode::Anthropic,
                "requires an Anthropic-compatible adapter",
            ),
            (
                RuntimeDeepSeekWebSearchMode::FunctionProxy,
                "requires a local search backend",
            ),
        ] {
            let error = runtime_deepseek_chat_request_body_with_options(
                &serde_json::to_vec(&request).unwrap(),
                &conversation_store(),
                RuntimeDeepSeekRewriteOptions {
                    strict_tools: false,
                    web_search_mode: mode,
                },
            )
            .expect_err("unimplemented web search mode should fail");

            assert!(error.to_string().contains(expected));
        }
    }

    #[test]
    fn deepseek_request_translation_openai_chat_web_search_mode_preserves_options() {
        let request = serde_json::json!({
            "model": "deepseek-v4-pro",
            "input": "search",
            "tools": [{
                "type": "web_search_preview",
                "context_size": "low"
            }]
        });

        let translated = runtime_deepseek_chat_request_body_with_options(
            &serde_json::to_vec(&request).unwrap(),
            &conversation_store(),
            RuntimeDeepSeekRewriteOptions {
                strict_tools: false,
                web_search_mode: RuntimeDeepSeekWebSearchMode::OpenAiChat,
            },
        )
        .expect("openai_chat mode should preserve best-effort options");
        let body: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

        assert_eq!(body["web_search_options"]["search_context_size"], "low");
    }

    #[test]
    fn deepseek_request_translation_auto_rejects_top_level_web_search_options() {
        let request = serde_json::json!({
            "model": "deepseek-v4-pro",
            "input": "search",
            "web_search_options": {
                "search_context_size": "medium"
            }
        });

        let error = runtime_deepseek_chat_request_body(
            &serde_json::to_vec(&request).unwrap(),
            &conversation_store(),
        )
        .expect_err("auto mode should reject top-level web search options");

        assert!(
            error
                .to_string()
                .contains("no documented native OpenAI Chat route")
        );
    }

    #[test]
    fn deepseek_request_translation_openai_chat_preserves_top_level_web_search_options() {
        let request = serde_json::json!({
            "model": "deepseek-v4-pro",
            "input": "search",
            "web_search_options": {
                "search_context_size": "medium"
            }
        });

        let translated = runtime_deepseek_chat_request_body_with_options(
            &serde_json::to_vec(&request).unwrap(),
            &conversation_store(),
            RuntimeDeepSeekRewriteOptions {
                strict_tools: false,
                web_search_mode: RuntimeDeepSeekWebSearchMode::OpenAiChat,
            },
        )
        .expect("openai_chat mode should preserve top-level web search options");
        let body: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

        assert_eq!(body["web_search_options"]["search_context_size"], "medium");
    }

    #[test]
    fn deepseek_request_translation_rejects_invalid_web_search_options() {
        let request = serde_json::json!({
            "model": "deepseek-v4-pro",
            "input": "search",
            "tools": [{
                "type": "web_search_preview",
                "allowed_domains": ["example.com", 42],
                "location": "US"
            }]
        });

        let error = runtime_deepseek_chat_request_body_with_options(
            &serde_json::to_vec(&request).unwrap(),
            &conversation_store(),
            RuntimeDeepSeekRewriteOptions {
                strict_tools: false,
                web_search_mode: RuntimeDeepSeekWebSearchMode::OpenAiChat,
            },
        )
        .expect_err("invalid web search options should fail");

        assert!(
            error
                .to_string()
                .contains("allowed_domains entries must be non-empty strings")
        );
    }

    #[test]
    fn deepseek_request_translation_rejects_non_object_web_search_options() {
        let request = serde_json::json!({
            "model": "deepseek-v4-pro",
            "input": "search",
            "web_search_options": "enabled"
        });

        let error = runtime_deepseek_chat_request_body_with_options(
            &serde_json::to_vec(&request).unwrap(),
            &conversation_store(),
            RuntimeDeepSeekRewriteOptions {
                strict_tools: false,
                web_search_mode: RuntimeDeepSeekWebSearchMode::OpenAiChat,
            },
        )
        .expect_err("non-object web_search_options should fail");

        assert!(
            error
                .to_string()
                .contains("web_search_options must be an object")
        );
    }

    #[test]
    fn deepseek_request_translation_rejects_invalid_web_search_context_size() {
        let request = serde_json::json!({
            "model": "deepseek-v4-pro",
            "input": "search",
            "tools": [{
                "type": "web_search_preview",
                "context_size": "huge"
            }]
        });

        let error = runtime_deepseek_chat_request_body_with_options(
            &serde_json::to_vec(&request).unwrap(),
            &conversation_store(),
            RuntimeDeepSeekRewriteOptions {
                strict_tools: false,
                web_search_mode: RuntimeDeepSeekWebSearchMode::OpenAiChat,
            },
        )
        .expect_err("invalid web search context size should fail");

        assert!(
            error
                .to_string()
                .contains("web_search context_size must be low, medium, or high")
        );
    }

    #[test]
    fn deepseek_request_translation_rejects_invalid_top_level_web_search_context_size() {
        let request = serde_json::json!({
            "model": "deepseek-v4-pro",
            "input": "search",
            "web_search_options": {
                "search_context_size": "huge"
            }
        });

        let error = runtime_deepseek_chat_request_body_with_options(
            &serde_json::to_vec(&request).unwrap(),
            &conversation_store(),
            RuntimeDeepSeekRewriteOptions {
                strict_tools: false,
                web_search_mode: RuntimeDeepSeekWebSearchMode::OpenAiChat,
            },
        )
        .expect_err("invalid top-level web search context size should fail");

        assert!(
            error
                .to_string()
                .contains("web_search search_context_size must be low, medium, or high")
        );
    }

    #[test]
    fn deepseek_request_translation_rejects_invalid_web_search_location() {
        let request = serde_json::json!({
            "model": "deepseek-v4-pro",
            "input": "search",
            "tools": [{
                "type": "web_search_preview",
                "location": "US"
            }]
        });

        let error = runtime_deepseek_chat_request_body_with_options(
            &serde_json::to_vec(&request).unwrap(),
            &conversation_store(),
            RuntimeDeepSeekRewriteOptions {
                strict_tools: false,
                web_search_mode: RuntimeDeepSeekWebSearchMode::OpenAiChat,
            },
        )
        .expect_err("invalid web search location should fail");

        assert!(
            error
                .to_string()
                .contains("web_search user_location must be an object")
        );
    }

    #[test]
    fn deepseek_response_translation_preserves_cache_usage_details() {
        let response = serde_json::json!({
            "id": "chatcmpl_usage",
            "model": "deepseek-v4-pro",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "done"
                }
            }],
            "usage": {
                "prompt_tokens": 20,
                "completion_tokens": 5,
                "total_tokens": 25,
                "completion_tokens_details": {
                    "reasoning_tokens": 3
                },
                "prompt_cache_hit_tokens": 12,
                "prompt_cache_miss_tokens": 8
            }
        });

        let translated = runtime_deepseek_responses_value_from_chat_value(&response, 48);

        assert_eq!(translated["usage"]["input_tokens"], 20);
        assert_eq!(translated["usage"]["output_tokens"], 5);
        assert_eq!(
            translated["usage"]["input_tokens_details"]["cached_tokens"],
            12
        );
        assert_eq!(
            translated["usage"]["output_tokens_details"]["reasoning_tokens"],
            3
        );
        assert_eq!(
            translated["usage"]["metadata"]["deepseek"]["prompt_cache_miss_tokens"],
            8
        );
    }

    #[test]
    fn deepseek_response_translation_preserves_logprobs_metadata() {
        let response = serde_json::json!({
            "id": "chatcmpl_logprobs",
            "model": "deepseek-v4-pro",
            "created": 1782740700u64,
            "system_fingerprint": "fp_deepseek_test",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "done",
                    "refusal": "I cannot help with that.",
                    "annotations": [{
                        "type": "url_citation",
                        "url": "https://example.com/ref"
                    }]
                },
                "finish_reason": "stop",
                "logprobs": {
                    "content": [{
                        "token": "done",
                        "logprob": -0.2,
                        "bytes": [100, 111, 110, 101],
                        "top_logprobs": []
                    }]
                }
            }]
        });

        let translated = runtime_deepseek_responses_value_from_chat_value(&response, 49);

        assert_eq!(
            translated["metadata"]["deepseek"]["logprobs"]["content"][0]["token"],
            "done"
        );
        assert_eq!(translated["metadata"]["deepseek"]["finish_reason"], "stop");
        assert_eq!(
            translated["metadata"]["deepseek"]["refusal"],
            "I cannot help with that."
        );
        assert_eq!(
            translated["metadata"]["deepseek"]["annotations"][0]["url"],
            "https://example.com/ref"
        );
        assert_eq!(translated["created_at"], 1782740700u64);
        assert_eq!(
            translated["metadata"]["deepseek"]["system_fingerprint"],
            "fp_deepseek_test"
        );
    }

    #[test]
    fn deepseek_response_translation_merges_request_degradation_metadata() {
        let response = serde_json::json!({
            "id": "chatcmpl_json_schema",
            "model": "deepseek-v4-pro",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "{\"answer\":\"ok\"}"
                },
                "finish_reason": "stop"
            }]
        });

        let mut translated = runtime_deepseek_responses_value_from_chat_value(&response, 11);
        runtime_deepseek_merge_response_metadata(
            &mut translated,
            Some(serde_json::json!({
                "deepseek": {
                    "degraded_response_format": {
                        "from": "json_schema",
                        "to": "json_object"
                    }
                }
            })),
        );

        assert_eq!(
            translated["metadata"]["deepseek"]["degraded_response_format"]["from"],
            "json_schema"
        );
        assert_eq!(translated["metadata"]["deepseek"]["finish_reason"], "stop");
    }
}
