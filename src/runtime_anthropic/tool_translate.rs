use super::*;

pub(crate) fn runtime_proxy_translate_anthropic_tools(
    value: &serde_json::Value,
    native_shell_enabled: bool,
    native_computer_enabled: bool,
) -> Result<RuntimeAnthropicTranslatedTools> {
    let Some(tools) = value.get("tools") else {
        return Ok(RuntimeAnthropicTranslatedTools::default());
    };
    let tools = tools
        .as_array()
        .context("Anthropic tools must be an array when present")?;
    let mcp_servers = runtime_proxy_translate_anthropic_mcp_servers(value)?;
    let mut translated = RuntimeAnthropicTranslatedTools::default();
    for tool in tools {
        let tool_type = tool
            .get("type")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let (tool_value, tool_state) = runtime_proxy_translate_anthropic_tool(
            tool,
            &mcp_servers,
            native_shell_enabled,
            native_computer_enabled,
        )?;
        let translated_name = tool_value
            .get("name")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .or_else(|| {
                (tool_value.get("type").and_then(serde_json::Value::as_str) == Some("shell"))
                    .then_some("bash")
                    .or_else(|| {
                        (tool_value.get("type").and_then(serde_json::Value::as_str)
                            == Some("computer"))
                        .then_some("computer")
                    })
            });
        if let Some(translated_name) = translated_name {
            if let Some(original_name) = tool
                .get("name")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                translated
                    .tool_name_aliases
                    .insert(original_name.to_string(), translated_name.to_string());
            }
            if let Some(tool_type) = tool
                .get("type")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                translated
                    .tool_name_aliases
                    .insert(tool_type.to_string(), translated_name.to_string());
            }
            if matches!(
                tool_value.get("type").and_then(serde_json::Value::as_str),
                Some("shell" | "computer")
            ) {
                translated
                    .native_tool_names
                    .insert(translated_name.to_string());
            }
        }
        translated.memory |= tool_type
            .is_some_and(|value| runtime_proxy_anthropic_unversioned_tool_type(value) == "memory");
        let RuntimeAnthropicServerTools {
            aliases,
            web_search,
            mcp,
            tool_search,
        } = tool_state;
        translated.tools.push(tool_value);
        translated.server_tools.aliases.extend(aliases);
        translated.server_tools.web_search |= web_search;
        translated.server_tools.mcp |= mcp;
        translated.server_tools.tool_search |= tool_search;
    }
    Ok(translated)
}

pub(crate) fn runtime_proxy_anthropic_append_tool_instructions(
    instructions: Option<String>,
    has_memory_tool: bool,
) -> Option<String> {
    if !has_memory_tool {
        return instructions;
    }

    Some(match instructions {
        Some(instructions) if !instructions.trim().is_empty() => {
            format!("{instructions}\n\n{RUNTIME_PROXY_ANTHROPIC_MEMORY_TOOL_INSTRUCTIONS}")
        }
        _ => RUNTIME_PROXY_ANTHROPIC_MEMORY_TOOL_INSTRUCTIONS.to_string(),
    })
}

pub(crate) fn runtime_proxy_translate_anthropic_tool(
    tool: &serde_json::Value,
    mcp_servers: &BTreeMap<String, RuntimeAnthropicMcpServer>,
    native_shell_enabled: bool,
    native_computer_enabled: bool,
) -> Result<(serde_json::Value, RuntimeAnthropicServerTools)> {
    let mut server_tools = RuntimeAnthropicServerTools::default();
    let tool_type = tool
        .get("type")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let tool_name = tool
        .get("name")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let is_claude_named_generic_builtin_tool = tool_type.is_none()
        && tool.get("input_schema").is_some()
        && matches!(
            tool_name.and_then(runtime_proxy_anthropic_builtin_server_tool_name),
            Some("web_search" | "web_fetch")
        );
    if tool_type
        .is_some_and(|value| runtime_proxy_anthropic_unversioned_tool_type(value) == "mcp_toolset")
        && let Some(translated) = runtime_proxy_translate_anthropic_mcp_tool(tool, mcp_servers)?
    {
        server_tools.mcp = true;
        return Ok((translated, server_tools));
    }
    if let Some(canonical_name) = tool_type
        .and_then(runtime_proxy_anthropic_server_tool_name_from_type)
        .or_else(|| {
            (!is_claude_named_generic_builtin_tool)
                .then_some(())
                .and(tool_name.and_then(runtime_proxy_anthropic_builtin_server_tool_name))
        })
    {
        let tool_name = tool_name.unwrap_or(canonical_name);
        server_tools.register(tool_name, canonical_name);
        if canonical_name == "web_search" {
            let mut translated = serde_json::Map::new();
            translated.insert(
                "type".to_string(),
                serde_json::Value::String("web_search".to_string()),
            );
            let allowed_domains = tool
                .get("allowed_domains")
                .and_then(serde_json::Value::as_array)
                .map(|domains| {
                    domains
                        .iter()
                        .filter_map(|domain| {
                            domain
                                .as_str()
                                .map(str::trim)
                                .filter(|value| !value.is_empty())
                                .map(|value| serde_json::Value::String(value.to_string()))
                        })
                        .collect::<Vec<_>>()
                })
                .filter(|domains| !domains.is_empty());
            if let Some(allowed_domains) = allowed_domains {
                translated.insert(
                    "filters".to_string(),
                    serde_json::json!({
                        "allowed_domains": allowed_domains,
                    }),
                );
            }
            if let Some(user_location) = tool
                .get("user_location")
                .filter(|value| value.is_object())
                .cloned()
            {
                translated.insert("user_location".to_string(), user_location);
            }
            return Ok((serde_json::Value::Object(translated), server_tools));
        }

        let mut translated = serde_json::Map::new();
        translated.insert(
            "type".to_string(),
            serde_json::Value::String("function".to_string()),
        );
        translated.insert(
            "name".to_string(),
            serde_json::Value::String(tool_name.to_string()),
        );
        if let Some(description) = tool
            .get("description")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            translated.insert(
                "description".to_string(),
                serde_json::Value::String(description.to_string()),
            );
        }
        if let Some(schema) = tool.get("input_schema") {
            translated.insert(
                "parameters".to_string(),
                runtime_proxy_anthropic_normalize_tool_schema(schema),
            );
        } else {
            translated.insert(
                "parameters".to_string(),
                runtime_proxy_anthropic_default_tool_schema(),
            );
        }
        return Ok((serde_json::Value::Object(translated), server_tools));
    }

    if native_shell_enabled
        && tool_type
            .is_some_and(|value| runtime_proxy_anthropic_unversioned_tool_type(value) == "bash")
    {
        return Ok((serde_json::json!({ "type": "shell" }), server_tools));
    }
    if native_computer_enabled
        && tool_type
            .is_some_and(|value| runtime_proxy_anthropic_unversioned_tool_type(value) == "computer")
    {
        return Ok((serde_json::json!({ "type": "computer" }), server_tools));
    }

    let name = tool
        .get("name")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| tool_type.and_then(runtime_proxy_anthropic_builtin_client_tool_name_from_type))
        .or(tool_type)
        .context("Anthropic tool definition is missing a non-empty name")?;
    let mut translated = serde_json::Map::new();
    translated.insert(
        "type".to_string(),
        serde_json::Value::String("function".to_string()),
    );
    translated.insert(
        "name".to_string(),
        serde_json::Value::String(name.to_string()),
    );
    if let Some(description) = runtime_proxy_anthropic_tool_description(tool, tool_type, name) {
        translated.insert(
            "description".to_string(),
            serde_json::Value::String(description),
        );
    }
    if let Some(schema) = tool.get("input_schema") {
        translated.insert(
            "parameters".to_string(),
            runtime_proxy_anthropic_normalize_tool_schema(schema),
        );
    } else if let Some(schema) =
        runtime_proxy_anthropic_builtin_client_tool_schema(tool, tool_type, name)
    {
        translated.insert("parameters".to_string(), schema);
    } else if tool_type.is_some() {
        translated.insert(
            "parameters".to_string(),
            runtime_proxy_anthropic_default_tool_schema(),
        );
    }
    Ok((serde_json::Value::Object(translated), server_tools))
}

pub(crate) fn runtime_proxy_translate_anthropic_tool_choice(
    value: &serde_json::Value,
    server_tools: &RuntimeAnthropicServerTools,
    tool_name_aliases: &BTreeMap<String, String>,
    native_tool_names: &BTreeSet<String>,
) -> Result<Option<serde_json::Value>> {
    let Some(choice) = value.get("tool_choice") else {
        return Ok(None);
    };
    let choice_type = choice
        .get("type")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .context("Anthropic tool_choice requires a non-empty type")?;
    Ok(match choice_type {
        "auto" => Some(serde_json::Value::String("auto".to_string())),
        "any" => Some(serde_json::Value::String("required".to_string())),
        "none" => Some(serde_json::Value::String("none".to_string())),
        "tool" => {
            let name = choice
                .get("name")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .context("Anthropic tool_choice type=tool requires a non-empty name")?;
            let name = tool_name_aliases
                .get(name)
                .map(String::as_str)
                .unwrap_or(name);
            if native_tool_names.contains(name) {
                return Ok(Some(serde_json::Value::String("required".to_string())));
            }
            if server_tools.web_search
                && server_tools.canonical_name_for_call(name) == Some("web_search")
            {
                return Ok(Some(serde_json::Value::String("required".to_string())));
            }
            Some(serde_json::json!({
                "type": "function",
                "name": name,
            }))
        }
        other => bail!("Unsupported Anthropic tool_choice type '{other}'"),
    })
}
