//! OpenAI Responses tool-shape helpers for chat-compatible providers.
//!
//! Pure provider translation only: no runtime state, auth, or transport.

mod entry;
mod mojo;

pub use entry::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_core_chat_tools_translate_responses_tool_shapes() {
        let value = serde_json::json!({
            "tools": [
                {
                    "type": "custom",
                    "name": "apply_patch",
                    "description": "Edit files.",
                    "format": {"type": "grammar"}
                },
                {
                    "type": "namespace",
                    "name": "mcp__prodex_sqz",
                    "tools": [{
                        "type": "function",
                        "name": "sqz_read_file",
                        "parameters": {"type": "object", "required": ["path"]}
                    }]
                },
                {
                    "type": "tool_search",
                    "parameters": {"type": "object"}
                },
                {
                    "type": "mcp",
                    "server_label": "git tools",
                    "configs": {"status": {"enabled": true}, "push": {"enabled": false}}
                },
                {
                    "type": "web_search_preview",
                    "search_context_size": "high"
                }
            ]
        });

        let tools = provider_core_chat_tools_from_responses_request(&value).unwrap();
        let names = tools
            .iter()
            .map(|tool| tool["function"]["name"].as_str().unwrap())
            .collect::<Vec<_>>();

        assert_eq!(
            names,
            vec![
                "apply_patch",
                "mcp__prodex_sqz__sqz_read_file",
                "tool_search",
                "mcp__git_tools__status"
            ]
        );
        assert_eq!(tools[0]["function"]["parameters"]["required"][0], "input");
        assert!(
            tools[0]["function"]["description"]
                .as_str()
                .unwrap()
                .contains("Original custom tool format JSON:")
        );
        assert_eq!(tools[2]["function"]["parameters"]["type"], "object");
    }

    #[test]
    fn provider_core_chat_tools_extract_web_search_and_tool_choice() {
        let value = serde_json::json!({
            "tools": [{
                "type": "web_search_preview",
                "context_size": "medium",
                "blocked_domains": ["blocked.example"],
                "max_uses": 2,
                "location": {"country": "US"}
            }],
            "tool_choice": {
                "type": "function",
                "namespace": "agents",
                "name": "spawn_agent"
            }
        });

        let options = provider_core_chat_web_search_options_from_responses_request(&value).unwrap();
        assert_eq!(options["search_context_size"], "medium");
        assert_eq!(options["blocked_domains"][0], "blocked.example");
        assert_eq!(options["max_uses"], 2);
        assert_eq!(options["user_location"]["country"], "US");

        let choice = provider_core_chat_tool_choice_from_responses_request(&value, false).unwrap();
        assert_eq!(choice["function"]["name"], "agents--spawn_agent");
        assert!(provider_core_chat_tool_choice_from_responses_request(&value, true).is_none());
    }

    #[test]
    fn provider_core_chat_tool_choice_keeps_field_precedence_and_unicode() {
        let value = serde_json::json!({
            "tool_choice": {
                "type": "mcp",
                "namespace": "東京",
                "server_label": "ignored",
                "mcp_server_name": "also_ignored",
                "function": {"namespace": "nested", "name": "nested_name"},
                "name": "道具"
            }
        });

        assert_eq!(
            provider_core_chat_tool_choice_from_responses_request(&value, false).unwrap()["function"]
                ["name"],
            "mcp__東京__道具"
        );
    }

    #[test]
    fn provider_core_chat_tools_sort_and_deduplicate_mcp_expansion() {
        let value = serde_json::json!({"tools": [{
            "type": "mcp",
            "server_label": "git tools",
            "allowed_tools": ["z", "a", "a", "mcp__ready"],
            "configs": {
                "b": {"enabled": true},
                "a": {"enabled": true},
                "off": {"enabled": false},
                "disabled_by_default": {}
            },
            "default_config": {"enabled": false}
        }]});

        let tools = provider_core_chat_tools_from_responses_request(&value).unwrap();
        let names = tools
            .iter()
            .map(|tool| tool["function"]["name"].as_str().unwrap())
            .collect::<Vec<_>>();

        assert_eq!(
            names,
            [
                "mcp__git_tools__a",
                "mcp__git_tools__b",
                "mcp__ready",
                "mcp__git_tools__z"
            ]
        );
        assert!(
            tools
                .iter()
                .all(|tool| { tool["function"]["parameters"]["additionalProperties"] == true })
        );
    }

    #[test]
    fn provider_core_chat_tool_shapes_reject_malformed_and_preserve_web_search_precedence() {
        for value in [
            serde_json::Value::Null,
            serde_json::json!(true),
            serde_json::json!({}),
            serde_json::json!({"tools": null}),
            serde_json::json!({"tools": [null, 7, {}]}),
        ] {
            assert!(provider_core_chat_tools_from_responses_request(&value).is_none());
            assert!(provider_core_chat_web_search_options_from_responses_request(&value).is_none());
        }
        assert!(
            provider_core_chat_tool_choice_from_responses_request(
                &serde_json::json!({"tool_choice": {"type": "function", "name": 7}}),
                false
            )
            .is_none()
        );
        assert!(provider_core_chat_request_body_without_web_search_options(b"not json").is_none());

        let value = serde_json::json!({"tools": [
            {"type": "web_search", "context_size": "invalid", "search_context_size": "medium",
             "user_location": null, "location": {"country": "JP"}, "max_uses": 1},
            {"type": "web_search_preview_v2", "search_context_size": "high", "max_uses": 9}
        ]});
        let options = provider_core_chat_web_search_options_from_responses_request(&value).unwrap();
        assert_eq!(options["search_context_size"], "medium");
        assert_eq!(options["user_location"], serde_json::Value::Null);
        assert_eq!(options["max_uses"], 1);
        assert!(provider_core_chat_tools_from_responses_request(&value).is_none());
    }

    #[test]
    fn provider_core_chat_tools_expand_large_bounded_input() {
        let tools = (0..1024)
            .map(|index| serde_json::json!({"type": "custom", "name": format!("tool_{index}")}))
            .collect::<Vec<_>>();
        let translated =
            provider_core_chat_tools_from_responses_request(&serde_json::json!({"tools": tools}))
                .unwrap();

        assert_eq!(translated.len(), 1024);
        assert_eq!(translated[0]["function"]["name"], "tool_0");
        assert_eq!(translated[1023]["function"]["name"], "tool_1023");
    }

    #[test]
    fn provider_core_chat_tools_strip_web_search_options() {
        let body = serde_json::to_vec(&serde_json::json!({
            "model": "provider-model",
            "web_search_options": {"search_context_size": "low"},
            "metadata": {"ticket": "DS-123"}
        }))
        .unwrap();

        let stripped = provider_core_chat_request_body_without_web_search_options(&body).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&stripped).unwrap();

        assert!(value.get("web_search_options").is_none());
        assert_eq!(value["metadata"]["ticket"], "DS-123");
    }

    #[test]
    fn provider_core_flatten_namespace_tool_names() {
        assert_eq!(
            provider_core_flatten_namespace_tool_name("mcp__prodex_sqz", "sqz_read_file"),
            "mcp__prodex_sqz__sqz_read_file"
        );
        assert_eq!(
            provider_core_flatten_namespace_tool_name("mcp__calendar__", "_create"),
            "mcp__calendar__--_create"
        );
        assert_eq!(
            provider_core_flatten_namespace_tool_name("agents", "spawn_agent"),
            "agents--spawn_agent"
        );
    }
}
