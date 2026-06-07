use std::collections::BTreeSet;

pub(super) fn runtime_provider_chat_tools_from_responses_request(
    value: &serde_json::Value,
) -> Option<Vec<serde_json::Value>> {
    let tools = value.get("tools")?.as_array()?;
    let mut translated = Vec::new();
    let mut seen_names = BTreeSet::new();
    for tool in tools {
        for translated_tool in runtime_provider_tools_from_responses_tool(tool) {
            let Some(name) = runtime_provider_translated_tool_name(&translated_tool) else {
                continue;
            };
            if seen_names.insert(name) {
                translated.push(translated_tool);
            }
        }
    }
    if translated.is_empty() {
        None
    } else {
        Some(translated)
    }
}

pub(super) fn runtime_provider_chat_web_search_options_from_responses_request(
    value: &serde_json::Value,
) -> Option<serde_json::Value> {
    let tool = value
        .get("tools")?
        .as_array()?
        .iter()
        .find(|tool| runtime_provider_is_web_search_tool(tool))?;
    let object = tool.as_object()?;
    let mut options = serde_json::Map::new();
    if let Some(search_context_size) =
        runtime_provider_json_string(object, &["search_context_size"])
            .filter(|size| matches!(size.as_str(), "low" | "medium" | "high"))
    {
        options.insert(
            "search_context_size".to_string(),
            serde_json::Value::String(search_context_size),
        );
    }
    if let Some(user_location) = object
        .get("user_location")
        .filter(|value| value.is_object())
    {
        options.insert("user_location".to_string(), user_location.clone());
    }
    Some(serde_json::Value::Object(options))
}

pub(super) fn runtime_provider_chat_request_body_without_web_search_options(
    body: &[u8],
) -> Option<Vec<u8>> {
    let mut value: serde_json::Value = serde_json::from_slice(body).ok()?;
    let object = value.as_object_mut()?;
    object.remove("web_search_options")?;
    serde_json::to_vec(&value).ok()
}

pub(super) fn runtime_provider_chat_tool_choice_from_responses_request(
    value: &serde_json::Value,
    thinking_enabled: bool,
) -> Option<serde_json::Value> {
    if thinking_enabled {
        return None;
    }
    let choice = value.get("tool_choice")?;
    if let Some(choice) = choice.as_str() {
        return matches!(choice, "auto" | "none" | "required")
            .then(|| serde_json::Value::String(choice.to_string()));
    }

    let object = choice.as_object()?;
    let choice_type = object
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if choice_type != "function" && !choice_type.starts_with("mcp") {
        return None;
    }
    let name = runtime_provider_json_string(object, &["name"])
        .or_else(|| runtime_provider_json_string_at_path(object, &["function", "name"]))
        .filter(|name| !name.trim().is_empty())?;
    Some(serde_json::json!({
        "type": "function",
        "function": {
            "name": name,
        },
    }))
}

fn runtime_provider_tools_from_responses_tool(tool: &serde_json::Value) -> Vec<serde_json::Value> {
    if runtime_provider_is_web_search_tool(tool) {
        return Vec::new();
    }
    runtime_provider_tool_from_responses_tool(tool)
        .into_iter()
        .chain(runtime_provider_namespace_function_tools(tool))
        .chain(runtime_provider_tool_search_tool(tool))
        .chain(runtime_provider_mcp_toolset_function_tools(tool))
        .collect()
}

fn runtime_provider_is_web_search_tool(tool: &serde_json::Value) -> bool {
    let tool_type = tool
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    tool_type == "web_search"
        || tool_type == "web_search_preview"
        || tool_type.starts_with("web_search_preview_")
}

fn runtime_provider_translated_tool_name(tool: &serde_json::Value) -> Option<String> {
    tool.get("function")
        .and_then(|function| function.get("name"))
        .and_then(serde_json::Value::as_str)
        .filter(|name| !name.trim().is_empty())
        .map(str::to_string)
}

fn runtime_provider_tool_from_responses_tool(
    tool: &serde_json::Value,
) -> Option<serde_json::Value> {
    let object = tool.as_object()?;
    if let Some(tool) = runtime_provider_custom_tool_from_responses_tool(object) {
        return Some(tool);
    }
    let function_object = runtime_provider_function_like_tool_object(object)?;
    let name = runtime_provider_json_string(function_object, &["name"])
        .filter(|name| !name.trim().is_empty())?;
    let mut function = serde_json::Map::new();
    function.insert("name".to_string(), serde_json::Value::String(name));
    if let Some(description) = runtime_provider_json_string(function_object, &["description"])
        .filter(|description| !description.trim().is_empty())
    {
        function.insert(
            "description".to_string(),
            serde_json::Value::String(description),
        );
    }
    if let Some(parameters) = function_object
        .get("parameters")
        .or_else(|| function_object.get("parametersJsonSchema"))
        .or_else(|| function_object.get("input_schema"))
        .or_else(|| function_object.get("schema"))
    {
        function.insert("parameters".to_string(), parameters.clone());
    }

    Some(serde_json::json!({
        "type": "function",
        "function": function,
    }))
}

fn runtime_provider_custom_tool_from_responses_tool(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Option<serde_json::Value> {
    if object.get("type").and_then(serde_json::Value::as_str) != Some("custom") {
        return None;
    }
    let name =
        runtime_provider_json_string(object, &["name"]).filter(|name| !name.trim().is_empty())?;
    let description = runtime_provider_json_string(object, &["description"])
        .filter(|description| !description.trim().is_empty())
        .unwrap_or_else(|| "Freeform custom tool input.".to_string());
    let input_hint = if name == "apply_patch" {
        "Call this custom/freeform tool with the exact Codex apply_patch grammar in the `input` string field. The first line must be `*** Begin Patch`, the last line must be `*** End Patch`, and hunks must use `*** Add File:`, `*** Delete File:`, or `*** Update File:`. For `*** Add File: path`, every new file content line must start with `+`; for example `+hello`, and a blank content line is `+`. Do not pass unified diff headers such as `--- a/...` or `+++ b/...` as the top-level input."
    } else {
        "Call this custom/freeform tool with the exact raw tool input in the `input` string field."
    };
    let input_description = if name == "apply_patch" {
        "Exact raw apply_patch input. For Add File, prefix every new file content line with '+'."
    } else {
        "Exact raw input for the custom/freeform tool."
    };
    Some(serde_json::json!({
        "type": "function",
        "function": {
            "name": name,
            "description": format!("{description}\n\n{input_hint}"),
            "parameters": {
                "type": "object",
                "properties": {
                    "input": {
                        "type": "string",
                        "description": input_description
                    }
                },
                "required": ["input"],
                "additionalProperties": false
            }
        }
    }))
}

fn runtime_provider_namespace_function_tools(tool: &serde_json::Value) -> Vec<serde_json::Value> {
    let Some(object) = tool.as_object() else {
        return Vec::new();
    };
    if object.get("type").and_then(serde_json::Value::as_str) != Some("namespace") {
        return Vec::new();
    }
    let Some(namespace) =
        runtime_provider_json_string(object, &["name"]).filter(|name| !name.trim().is_empty())
    else {
        return Vec::new();
    };
    object
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|tool| {
            let tool = tool.as_object()?;
            if tool.get("type").and_then(serde_json::Value::as_str) != Some("function") {
                return None;
            }
            let tool_name = runtime_provider_json_string(tool, &["name"])
                .filter(|name| !name.trim().is_empty())?;
            let mut function = serde_json::Map::new();
            function.insert(
                "name".to_string(),
                serde_json::Value::String(runtime_provider_flatten_namespace_tool_name(
                    &namespace, &tool_name,
                )),
            );
            if let Some(description) = runtime_provider_json_string(tool, &["description"])
                .filter(|description| !description.trim().is_empty())
            {
                function.insert(
                    "description".to_string(),
                    serde_json::Value::String(description),
                );
            }
            if let Some(parameters) = tool
                .get("parameters")
                .or_else(|| tool.get("parametersJsonSchema"))
                .or_else(|| tool.get("input_schema"))
                .or_else(|| tool.get("schema"))
            {
                function.insert("parameters".to_string(), parameters.clone());
            }
            Some(serde_json::json!({
                "type": "function",
                "function": function,
            }))
        })
        .collect()
}

fn runtime_provider_tool_search_tool(tool: &serde_json::Value) -> Option<serde_json::Value> {
    let object = tool.as_object()?;
    if object.get("type").and_then(serde_json::Value::as_str) != Some("tool_search") {
        return None;
    }
    let mut function = serde_json::Map::new();
    function.insert(
        "name".to_string(),
        serde_json::Value::String("tool_search".to_string()),
    );
    if let Some(description) = runtime_provider_json_string(object, &["description"])
        .filter(|description| !description.trim().is_empty())
    {
        function.insert(
            "description".to_string(),
            serde_json::Value::String(description),
        );
    }
    if let Some(parameters) = object.get("parameters") {
        function.insert("parameters".to_string(), parameters.clone());
    }
    Some(serde_json::json!({
        "type": "function",
        "function": function,
    }))
}

pub(super) fn runtime_provider_flatten_namespace_tool_name(namespace: &str, name: &str) -> String {
    let namespace = namespace.trim();
    let name = name.trim();
    if namespace.starts_with("mcp__")
        && !namespace.ends_with('_')
        && !name.starts_with('_')
        && !name.contains("__")
    {
        format!("{namespace}__{name}")
    } else {
        format!("{namespace}--{name}")
    }
}

pub(super) fn runtime_provider_split_flat_namespace_tool_name(
    name: &str,
) -> (Option<String>, String) {
    if let Some((namespace, tool_name)) = name.rsplit_once("--")
        && !namespace.trim().is_empty()
        && !tool_name.trim().is_empty()
    {
        return (Some(namespace.to_string()), tool_name.to_string());
    }
    let Some(rest) = name.strip_prefix("mcp__") else {
        return (None, name.to_string());
    };
    let Some(index) = rest.rfind("__") else {
        return (None, name.to_string());
    };
    let namespace = format!("mcp__{}", &rest[..index]);
    let tool_name = rest[index + 2..].to_string();
    if tool_name.trim().is_empty() {
        return (None, name.to_string());
    }
    (Some(namespace), tool_name)
}

fn runtime_provider_mcp_toolset_function_tools(tool: &serde_json::Value) -> Vec<serde_json::Value> {
    let Some(object) = tool.as_object() else {
        return Vec::new();
    };
    let tool_type = object
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if !matches!(tool_type, "mcp" | "mcp_toolset") {
        return Vec::new();
    }
    let Some(server_name) = runtime_provider_json_string(
        object,
        &["mcp_server_name", "server_label", "server_name", "name"],
    )
    .filter(|server_name| !server_name.trim().is_empty()) else {
        return Vec::new();
    };
    let Some(server_label) = runtime_provider_function_name_segment(&server_name) else {
        return Vec::new();
    };
    runtime_provider_mcp_toolset_tool_names(object)
        .into_iter()
        .filter_map(|tool_name| {
            let tool_label = runtime_provider_function_name_segment(&tool_name)?;
            let name = if tool_name.trim().starts_with("mcp__") {
                tool_label
            } else {
                format!("mcp__{server_label}__{tool_label}")
            };
            let description = runtime_provider_json_string(object, &["description"])
                .filter(|description| !description.trim().is_empty())
                .unwrap_or_else(|| format!("MCP tool {tool_name} from {server_name}."));
            Some(serde_json::json!({
                "type": "function",
                "function": {
                    "name": name,
                    "description": description,
                    "parameters": {
                        "type": "object",
                        "additionalProperties": true
                    }
                }
            }))
        })
        .collect()
}

fn runtime_provider_mcp_toolset_tool_names(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Vec<String> {
    let mut names = Vec::new();
    if let Some(allowed_tools) = object
        .get("allowed_tools")
        .and_then(serde_json::Value::as_array)
    {
        names.extend(
            allowed_tools
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_string),
        );
    }
    let default_enabled = object
        .get("default_config")
        .and_then(serde_json::Value::as_object)
        .and_then(|config| config.get("enabled"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);
    if let Some(configs) = object.get("configs").and_then(serde_json::Value::as_object) {
        names.extend(configs.iter().filter_map(|(tool_name, config)| {
            let enabled = config
                .as_object()
                .and_then(|config| config.get("enabled"))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(default_enabled);
            enabled.then(|| tool_name.to_string())
        }));
    }
    names.sort();
    names.dedup();
    names
}

fn runtime_provider_function_name_segment(value: &str) -> Option<String> {
    let segment = value
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string();
    (!segment.is_empty()).then_some(segment)
}

fn runtime_provider_function_like_tool_object(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Option<&serde_json::Map<String, serde_json::Value>> {
    if object.get("type").and_then(serde_json::Value::as_str) == Some("function") {
        return Some(
            object
                .get("function")
                .and_then(serde_json::Value::as_object)
                .unwrap_or(object),
        );
    }

    let tool_type = object
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let name = runtime_provider_json_string(object, &["name"])?;
    let has_schema = object.contains_key("parameters")
        || object.contains_key("parametersJsonSchema")
        || object.contains_key("input_schema")
        || object.contains_key("schema");
    let is_mcp_tool = tool_type.starts_with("mcp") || name.starts_with("mcp__");
    (has_schema && is_mcp_tool).then_some(object)
}

fn runtime_provider_json_string(
    object: &serde_json::Map<String, serde_json::Value>,
    keys: &[&str],
) -> Option<String> {
    keys.iter()
        .find_map(|key| object.get(*key).and_then(serde_json::Value::as_str))
        .map(str::to_string)
}

fn runtime_provider_json_string_at_path(
    object: &serde_json::Map<String, serde_json::Value>,
    path: &[&str],
) -> Option<String> {
    let mut value = object.get(*path.first()?)?;
    for key in &path[1..] {
        value = value.get(*key)?;
    }
    value.as_str().map(str::to_string)
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
        assert_eq!(value["tools"][0]["function"]["name"], "shell");
    }

    #[test]
    fn provider_tools_split_flat_mcp_namespace_names() {
        let (namespace, name) =
            runtime_provider_split_flat_namespace_tool_name("mcp__prodex_sqz__sqz_read_file");

        assert_eq!(namespace.as_deref(), Some("mcp__prodex_sqz"));
        assert_eq!(name, "sqz_read_file");
        assert_eq!(
            runtime_provider_split_flat_namespace_tool_name("shell"),
            (None, "shell".to_string())
        );
    }

    #[test]
    fn provider_tools_preserve_namespace_and_tool_underscores() {
        let flat_name = runtime_provider_flatten_namespace_tool_name("mcp__calendar__", "_create");
        let (namespace, name) = runtime_provider_split_flat_namespace_tool_name(&flat_name);

        assert_eq!(flat_name, "mcp__calendar__--_create");
        assert_eq!(namespace.as_deref(), Some("mcp__calendar__"));
        assert_eq!(name, "_create");
    }

    #[test]
    fn provider_tools_round_trip_non_mcp_namespaces() {
        let flat_name = runtime_provider_flatten_namespace_tool_name("agents", "spawn_agent");
        let (namespace, name) = runtime_provider_split_flat_namespace_tool_name(&flat_name);

        assert_eq!(flat_name, "agents--spawn_agent");
        assert_eq!(namespace.as_deref(), Some("agents"));
        assert_eq!(name, "spawn_agent");
    }
}
