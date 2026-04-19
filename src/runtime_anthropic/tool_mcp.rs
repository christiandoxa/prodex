use super::*;

pub(crate) fn runtime_proxy_translate_anthropic_mcp_servers(
    value: &serde_json::Value,
) -> Result<BTreeMap<String, RuntimeAnthropicMcpServer>> {
    let Some(mcp_servers) = value.get("mcp_servers") else {
        return Ok(BTreeMap::new());
    };
    let mcp_servers = mcp_servers
        .as_array()
        .context("Anthropic mcp_servers must be an array when present")?;
    let mut translated = BTreeMap::new();
    for server in mcp_servers {
        let name = server
            .get("name")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .context("Anthropic mcp_server requires a non-empty name")?;
        let mut headers = serde_json::Map::new();
        if let Some(server_headers) = server.get("headers").and_then(serde_json::Value::as_object) {
            for (header_name, header_value) in server_headers {
                let Some(header_value) = header_value
                    .as_str()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                else {
                    continue;
                };
                headers.insert(
                    header_name.clone(),
                    serde_json::Value::String(header_value.to_string()),
                );
            }
        }
        translated.insert(
            name.to_string(),
            RuntimeAnthropicMcpServer {
                name: name.to_string(),
                url: server
                    .get("url")
                    .and_then(serde_json::Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string),
                authorization_token: server
                    .get("authorization_token")
                    .and_then(serde_json::Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string),
                headers,
                description: server
                    .get("description")
                    .and_then(serde_json::Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string),
            },
        );
    }
    Ok(translated)
}

pub(crate) fn runtime_proxy_translate_anthropic_mcp_tool(
    tool: &serde_json::Value,
    mcp_servers: &BTreeMap<String, RuntimeAnthropicMcpServer>,
) -> Result<Option<serde_json::Value>> {
    let Some(server_name) = tool
        .get("mcp_server_name")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    let Some(server) = mcp_servers.get(server_name) else {
        return Ok(None);
    };
    let Some(server_url) = server.url.as_deref() else {
        return Ok(None);
    };

    let default_config = tool
        .get("default_config")
        .and_then(serde_json::Value::as_object);
    let default_enabled = default_config
        .and_then(|config| config.get("enabled"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);
    let default_defer_loading = default_config
        .and_then(|config| config.get("defer_loading"))
        .and_then(serde_json::Value::as_bool);

    let mut allowlisted_tools = Vec::new();
    let mut has_unrepresentable_denylist = false;
    if let Some(configs) = tool.get("configs") {
        let Some(configs) = configs.as_object() else {
            return Ok(None);
        };
        for (tool_name, config) in configs {
            let Some(config) = config.as_object() else {
                return Ok(None);
            };
            let enabled = config
                .get("enabled")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(default_enabled);
            if default_enabled {
                if !enabled {
                    has_unrepresentable_denylist = true;
                    break;
                }
            } else if enabled {
                allowlisted_tools.push(serde_json::Value::String(tool_name.clone()));
            }
        }
    }
    if has_unrepresentable_denylist {
        return Ok(None);
    }

    let mut translated = serde_json::Map::new();
    translated.insert(
        "type".to_string(),
        serde_json::Value::String("mcp".to_string()),
    );
    translated.insert(
        "server_label".to_string(),
        serde_json::Value::String(server.name.clone()),
    );
    translated.insert(
        "server_url".to_string(),
        serde_json::Value::String(server_url.to_string()),
    );
    translated.insert(
        "require_approval".to_string(),
        serde_json::Value::String("never".to_string()),
    );
    if !default_enabled {
        translated.insert(
            "allowed_tools".to_string(),
            serde_json::Value::Array(allowlisted_tools),
        );
    }
    if let Some(defer_loading) = default_defer_loading {
        translated.insert(
            "defer_loading".to_string(),
            serde_json::Value::Bool(defer_loading),
        );
    }
    if let Some(authorization) = server.authorization_token.as_deref() {
        translated.insert(
            "authorization".to_string(),
            serde_json::Value::String(authorization.to_string()),
        );
    }
    if !server.headers.is_empty() {
        translated.insert(
            "headers".to_string(),
            serde_json::Value::Object(server.headers.clone()),
        );
    }
    if let Some(server_description) = tool
        .get("description")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or(server.description.as_deref())
    {
        translated.insert(
            "server_description".to_string(),
            serde_json::Value::String(server_description.to_string()),
        );
    }
    Ok(Some(serde_json::Value::Object(translated)))
}
