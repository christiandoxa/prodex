#[cfg(test)]
mod tests {
    use super::super::*;
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};

    fn conversation_store() -> RuntimeDeepSeekConversationStore {
        Arc::new(Mutex::new(BTreeMap::new()))
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
    fn deepseek_request_translation_maps_web_search_options_without_tool_stub() {
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

        let translated = runtime_deepseek_chat_request_body(
            &serde_json::to_vec(&request).unwrap(),
            &conversation_store(),
        )
        .expect("request should translate");
        let body: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

        assert_eq!(body["web_search_options"]["search_context_size"], "high");
        assert_eq!(
            body["web_search_options"]["allowed_domains"][0],
            "example.com"
        );
        assert_eq!(body["web_search_options"]["user_location"]["country"], "US");
        assert_eq!(body["tools"].as_array().unwrap().len(), 1);
        assert_eq!(body["tools"][0]["function"]["name"], "shell");
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
                "requires the DeepSeek Anthropic adapter",
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

        let error = runtime_deepseek_chat_request_body(
            &serde_json::to_vec(&request).unwrap(),
            &conversation_store(),
        )
        .expect_err("invalid web search options should fail");

        assert!(
            error
                .to_string()
                .contains("allowed_domains entries must be non-empty strings")
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

        let error = runtime_deepseek_chat_request_body(
            &serde_json::to_vec(&request).unwrap(),
            &conversation_store(),
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
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "done"
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
