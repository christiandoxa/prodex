//! OpenAI Responses tool-shape helpers for chat-compatible providers.
//!
//! Pure provider translation only: no runtime state, auth, or transport.

mod tool_choice;
mod tools;
mod util;
mod web_search;

pub use self::tool_choice::provider_core_chat_tool_choice_from_responses_request;
pub use self::tools::provider_core_chat_tools_from_responses_request;
pub use self::util::provider_core_flatten_namespace_tool_name;
pub use self::web_search::{
    provider_core_chat_request_body_without_web_search_options,
    provider_core_chat_web_search_options_from_responses_request,
};

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
    }

    #[test]
    fn provider_core_chat_tools_extract_web_search_and_tool_choice() {
        let value = serde_json::json!({
            "tools": [{
                "type": "web_search_preview",
                "context_size": "medium",
                "allowed_domains": ["example.com"],
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
        assert_eq!(options["allowed_domains"][0], "example.com");
        assert_eq!(options["user_location"]["country"], "US");

        let choice = provider_core_chat_tool_choice_from_responses_request(&value, false).unwrap();
        assert_eq!(choice["function"]["name"], "agents--spawn_agent");
        assert!(provider_core_chat_tool_choice_from_responses_request(&value, true).is_none());
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
