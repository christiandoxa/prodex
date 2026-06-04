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
                    "search_context_size": "high",
                    "user_location": {
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
        assert_eq!(body["web_search_options"]["user_location"]["country"], "US");
        assert_eq!(body["tools"].as_array().unwrap().len(), 1);
        assert_eq!(body["tools"][0]["function"]["name"], "shell");
    }
}
