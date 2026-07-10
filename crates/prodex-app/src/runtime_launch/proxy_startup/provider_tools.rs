use prodex_provider_core::{
    provider_core_chat_request_body_without_web_search_options,
    provider_core_chat_tool_choice_from_responses_request,
    provider_core_chat_tools_from_responses_request,
    provider_core_chat_web_search_options_from_responses_request,
    provider_core_flatten_namespace_tool_name,
};

pub(super) fn runtime_provider_chat_tools_from_responses_request(
    value: &serde_json::Value,
) -> Option<Vec<serde_json::Value>> {
    provider_core_chat_tools_from_responses_request(value)
}

pub(super) fn runtime_provider_chat_web_search_options_from_responses_request(
    value: &serde_json::Value,
) -> Option<serde_json::Value> {
    provider_core_chat_web_search_options_from_responses_request(value)
}

pub(super) fn runtime_provider_chat_request_body_without_web_search_options(
    body: &[u8],
) -> Option<Vec<u8>> {
    provider_core_chat_request_body_without_web_search_options(body)
}

pub(super) fn runtime_provider_chat_tool_choice_from_responses_request(
    value: &serde_json::Value,
    thinking_enabled: bool,
) -> Option<serde_json::Value> {
    provider_core_chat_tool_choice_from_responses_request(value, thinking_enabled)
}

pub(super) fn runtime_provider_flatten_namespace_tool_name(namespace: &str, name: &str) -> String {
    provider_core_flatten_namespace_tool_name(namespace, name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_tools_normalize_mcp_toolsets() {
        let value = serde_json::json!({
            "tools": [{
                "type": "mcp",
                "server_label": "git tools",
                "configs": {
                    "status": {"enabled": true},
                    "push": {"enabled": false}
                }
            }]
        });

        let tools = runtime_provider_chat_tools_from_responses_request(&value).unwrap();

        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["function"]["name"], "mcp__git_tools__status");
    }

    #[test]
    fn provider_tools_accept_parameters_json_schema() {
        let value = serde_json::json!({
            "tools": [{
                "type": "mcp_tool",
                "name": "mcp__prodex_sqz__compress",
                "parametersJsonSchema": {
                    "type": "object",
                    "properties": {
                        "text": {"type": "string"}
                    },
                    "required": ["text"]
                }
            }]
        });

        let tools = runtime_provider_chat_tools_from_responses_request(&value).unwrap();

        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["function"]["name"], "mcp__prodex_sqz__compress");
        assert_eq!(tools[0]["function"]["parameters"]["required"][0], "text");
    }

    #[test]
    fn provider_tools_map_custom_freeform_tool_to_function() {
        let value = serde_json::json!({
            "tools": [{
                "type": "custom",
                "name": "apply_patch",
                "description": "Use apply_patch to edit files.",
                "format": {
                    "type": "grammar",
                    "syntax": "lark",
                    "definition": "start: begin_patch hunk+ end_patch"
                }
            }]
        });

        let tools = runtime_provider_chat_tools_from_responses_request(&value).unwrap();

        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["function"]["name"], "apply_patch");
        assert_eq!(
            tools[0]["function"]["parameters"]["properties"]["input"]["type"],
            "string"
        );
        assert_eq!(tools[0]["function"]["parameters"]["required"][0], "input");
    }

    #[test]
    fn provider_tools_flatten_namespace_tools() {
        let value = serde_json::json!({
            "tools": [{
                "type": "namespace",
                "name": "mcp__prodex_sqz",
                "description": "SQZ tools",
                "tools": [{
                    "type": "function",
                    "name": "sqz_read_file",
                    "description": "Read a file through SQZ.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "path": {"type": "string"}
                        },
                        "required": ["path"]
                    }
                }]
            }]
        });

        let tools = runtime_provider_chat_tools_from_responses_request(&value).unwrap();

        assert_eq!(tools.len(), 1);
        assert_eq!(
            tools[0]["function"]["name"],
            "mcp__prodex_sqz__sqz_read_file"
        );
        assert_eq!(tools[0]["function"]["parameters"]["required"][0], "path");
    }

    #[test]
    fn provider_tools_map_tool_search_to_function() {
        let value = serde_json::json!({
            "tools": [{
                "type": "tool_search",
                "execution": "client",
                "description": "Search deferred tools.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": {"type": "string"}
                    },
                    "required": ["query"]
                }
            }]
        });

        let tools = runtime_provider_chat_tools_from_responses_request(&value).unwrap();

        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["function"]["name"], "tool_search");
        assert_eq!(tools[0]["function"]["parameters"]["required"][0], "query");
    }

    #[test]
    fn provider_tools_extract_web_search_options_without_function_tool() {
        let value = serde_json::json!({
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

        let tools = runtime_provider_chat_tools_from_responses_request(&value).unwrap();
        let options =
            runtime_provider_chat_web_search_options_from_responses_request(&value).unwrap();

        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["function"]["name"], "shell");
        assert_eq!(options["search_context_size"], "high");
        assert_eq!(options["user_location"]["country"], "US");
    }

    #[test]
    fn provider_tools_strip_chat_web_search_options_for_fallback() {
        let body = serde_json::to_vec(&serde_json::json!({
            "model": "provider-model",
            "messages": [],
            "web_search_options": {
                "search_context_size": "medium"
            },
            "metadata": {"ticket": "DS-123"},
            "tools": [{
                "type": "function",
                "function": {"name": "shell"}
            }]
        }))
        .unwrap();

        let stripped = runtime_provider_chat_request_body_without_web_search_options(&body)
            .expect("web search options should be stripped");
        let value: serde_json::Value = serde_json::from_slice(&stripped).unwrap();

        assert!(value.get("web_search_options").is_none());
        assert_eq!(value["metadata"]["ticket"], "DS-123");
        assert_eq!(value["tools"][0]["function"]["name"], "shell");
    }

    #[test]
    fn provider_tools_split_flat_mcp_namespace_names() {
        let (namespace, name) = prodex_provider_core::provider_core_split_flat_namespace_tool_name(
            "mcp__prodex_sqz__sqz_read_file",
        );

        assert_eq!(namespace.as_deref(), Some("mcp__prodex_sqz"));
        assert_eq!(name, "sqz_read_file");
        assert_eq!(
            prodex_provider_core::provider_core_split_flat_namespace_tool_name("shell"),
            (None, "shell".to_string())
        );
    }

    #[test]
    fn provider_tools_preserve_namespace_and_tool_underscores() {
        let flat_name = runtime_provider_flatten_namespace_tool_name("mcp__calendar__", "_create");
        let (namespace, name) =
            prodex_provider_core::provider_core_split_flat_namespace_tool_name(&flat_name);

        assert_eq!(flat_name, "mcp__calendar__--_create");
        assert_eq!(namespace.as_deref(), Some("mcp__calendar__"));
        assert_eq!(name, "_create");
    }

    #[test]
    fn provider_tools_round_trip_non_mcp_namespaces() {
        let flat_name = runtime_provider_flatten_namespace_tool_name("agents", "spawn_agent");
        let (namespace, name) =
            prodex_provider_core::provider_core_split_flat_namespace_tool_name(&flat_name);

        assert_eq!(flat_name, "agents--spawn_agent");
        assert_eq!(namespace.as_deref(), Some("agents"));
        assert_eq!(name, "spawn_agent");
    }

    #[test]
    fn provider_tools_flatten_namespace_tool_choice() {
        let value = serde_json::json!({
            "tool_choice": {
                "type": "function",
                "namespace": "agents",
                "name": "spawn_agent"
            }
        });

        let choice = runtime_provider_chat_tool_choice_from_responses_request(&value, false)
            .expect("tool choice should translate");

        assert_eq!(choice["function"]["name"], "agents--spawn_agent");
    }

    #[test]
    fn provider_tools_flatten_mcp_tool_choice() {
        let value = serde_json::json!({
            "tool_choice": {
                "type": "mcp",
                "server_label": "prodex_sqz",
                "name": "sqz_read_file"
            }
        });

        let choice = runtime_provider_chat_tool_choice_from_responses_request(&value, false)
            .expect("tool choice should translate");

        assert_eq!(choice["function"]["name"], "mcp__prodex_sqz__sqz_read_file");
    }
}
