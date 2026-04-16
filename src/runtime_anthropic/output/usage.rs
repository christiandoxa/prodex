use super::*;

pub(crate) fn runtime_anthropic_usage_from_value(
    value: &serde_json::Value,
) -> (u64, u64, Option<u64>) {
    let usage = value.get("usage").or_else(|| {
        value
            .get("response")
            .and_then(|response| response.get("usage"))
    });
    let input_tokens = usage
        .and_then(|usage| usage.get("input_tokens"))
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let output_tokens = usage
        .and_then(|usage| usage.get("output_tokens"))
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let cached_tokens = usage
        .and_then(|usage| usage.get("input_tokens_details"))
        .and_then(|details| details.get("cached_tokens"))
        .and_then(serde_json::Value::as_u64);
    (input_tokens, output_tokens, cached_tokens)
}

pub(crate) fn runtime_anthropic_tool_usage_web_search_requests_from_value(
    value: &serde_json::Value,
) -> u64 {
    value
        .get("tool_usage")
        .or_else(|| {
            value
                .get("response")
                .and_then(|response| response.get("tool_usage"))
        })
        .and_then(|tool_usage| tool_usage.get("web_search"))
        .and_then(|web_search| web_search.get("num_requests"))
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0)
}

pub(crate) fn runtime_anthropic_tool_usage_tool_search_requests_from_value(
    value: &serde_json::Value,
) -> u64 {
    value
        .get("tool_usage")
        .or_else(|| {
            value
                .get("response")
                .and_then(|response| response.get("tool_usage"))
        })
        .and_then(|tool_usage| tool_usage.get("tool_search"))
        .and_then(|tool_search| tool_search.get("num_requests"))
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0)
}

pub(crate) fn runtime_anthropic_tool_usage_code_execution_requests_from_value(
    value: &serde_json::Value,
) -> u64 {
    value
        .get("tool_usage")
        .or_else(|| {
            value
                .get("response")
                .and_then(|response| response.get("tool_usage"))
        })
        .and_then(|tool_usage| tool_usage.get("code_execution"))
        .and_then(|code_execution| code_execution.get("num_requests"))
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0)
}

pub(crate) fn runtime_anthropic_server_tool_registration_for_call(
    tool_name: &str,
    server_tools: Option<&RuntimeAnthropicServerTools>,
) -> Option<(String, String)> {
    if let Some(server_tools) = server_tools
        && let Some(registration) = server_tools.registration_for_call(tool_name)
    {
        return Some((
            registration.response_name.clone(),
            registration.block_type.clone(),
        ));
    }
    let tool_name = tool_name.trim();
    runtime_proxy_anthropic_builtin_server_tool_name(tool_name)
        .filter(|canonical_name| *canonical_name == tool_name)
        .map(|name| (name.to_string(), "server_tool_use".to_string()))
}

pub(crate) fn runtime_anthropic_server_tool_name_for_call(
    tool_name: &str,
    server_tools: Option<&RuntimeAnthropicServerTools>,
) -> Option<String> {
    runtime_anthropic_server_tool_registration_for_call(tool_name, server_tools)
        .map(|(response_name, _)| response_name)
}

pub(crate) fn runtime_anthropic_output_item_server_tool_usage(
    item: &serde_json::Value,
    server_tools: Option<&RuntimeAnthropicServerTools>,
) -> RuntimeAnthropicServerToolUsage {
    match item.get("type").and_then(serde_json::Value::as_str) {
        Some("web_search_call") => RuntimeAnthropicServerToolUsage {
            web_search_requests: runtime_anthropic_web_search_request_count_from_output_item(item),
            ..RuntimeAnthropicServerToolUsage::default()
        },
        Some("function_call") => match item
            .get("name")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .and_then(|name| {
                runtime_anthropic_server_tool_name_for_call(name, server_tools).or_else(|| {
                    runtime_proxy_anthropic_builtin_server_tool_name(name).map(str::to_string)
                })
            })
            .as_deref()
        {
            Some("web_search") => RuntimeAnthropicServerToolUsage {
                web_search_requests: 1,
                ..RuntimeAnthropicServerToolUsage::default()
            },
            Some("web_fetch") => RuntimeAnthropicServerToolUsage {
                web_fetch_requests: 1,
                ..RuntimeAnthropicServerToolUsage::default()
            },
            Some("code_execution" | "bash_code_execution" | "text_editor_code_execution") => {
                RuntimeAnthropicServerToolUsage {
                    code_execution_requests: 1,
                    ..RuntimeAnthropicServerToolUsage::default()
                }
            }
            Some("tool_search_tool_regex" | "tool_search_tool_bm25") => {
                RuntimeAnthropicServerToolUsage {
                    tool_search_requests: 1,
                    ..RuntimeAnthropicServerToolUsage::default()
                }
            }
            _ => RuntimeAnthropicServerToolUsage::default(),
        },
        _ => RuntimeAnthropicServerToolUsage::default(),
    }
}

pub(crate) fn runtime_anthropic_web_search_request_count_from_output_item(
    item: &serde_json::Value,
) -> u64 {
    let Some(action) = item.get("action") else {
        return 1;
    };
    action
        .get("queries")
        .and_then(serde_json::Value::as_array)
        .map(|queries| queries.len() as u64)
        .filter(|count| *count > 0)
        .unwrap_or(1)
}

pub(crate) fn runtime_anthropic_web_search_request_count_from_output(
    output: &[serde_json::Value],
    server_tools: Option<&RuntimeAnthropicServerTools>,
) -> u64 {
    output
        .iter()
        .map(|item| {
            runtime_anthropic_output_item_server_tool_usage(item, server_tools).web_search_requests
        })
        .sum()
}

pub(crate) fn runtime_anthropic_web_fetch_request_count_from_output(
    output: &[serde_json::Value],
    server_tools: Option<&RuntimeAnthropicServerTools>,
) -> u64 {
    output
        .iter()
        .map(|item| {
            runtime_anthropic_output_item_server_tool_usage(item, server_tools).web_fetch_requests
        })
        .sum()
}

pub(crate) fn runtime_anthropic_tool_search_request_count_from_output(
    output: &[serde_json::Value],
    server_tools: Option<&RuntimeAnthropicServerTools>,
) -> u64 {
    output
        .iter()
        .map(|item| {
            runtime_anthropic_output_item_server_tool_usage(item, server_tools).tool_search_requests
        })
        .sum()
}

pub(crate) fn runtime_anthropic_code_execution_request_count_from_output(
    output: &[serde_json::Value],
    server_tools: Option<&RuntimeAnthropicServerTools>,
) -> u64 {
    output
        .iter()
        .map(|item| {
            runtime_anthropic_output_item_server_tool_usage(item, server_tools)
                .code_execution_requests
        })
        .sum()
}

pub(crate) fn runtime_anthropic_usage_json(
    input_tokens: u64,
    output_tokens: u64,
    cached_tokens: Option<u64>,
    web_search_requests: u64,
    web_fetch_requests: u64,
    code_execution_requests: u64,
    tool_search_requests: u64,
) -> serde_json::Map<String, serde_json::Value> {
    let mut usage = serde_json::Map::new();
    usage.insert(
        "input_tokens".to_string(),
        serde_json::Value::Number(input_tokens.into()),
    );
    usage.insert(
        "output_tokens".to_string(),
        serde_json::Value::Number(output_tokens.into()),
    );
    if let Some(cached_tokens) = cached_tokens {
        usage.insert(
            "cache_read_input_tokens".to_string(),
            serde_json::Value::Number(cached_tokens.into()),
        );
    }
    usage.insert("server_tool_use".to_string(), {
        let mut server_tool_use = serde_json::Map::new();
        server_tool_use.insert(
            "web_search_requests".to_string(),
            serde_json::Value::Number(web_search_requests.into()),
        );
        server_tool_use.insert(
            "web_fetch_requests".to_string(),
            serde_json::Value::Number(web_fetch_requests.into()),
        );
        if code_execution_requests > 0 {
            server_tool_use.insert(
                "code_execution_requests".to_string(),
                serde_json::Value::Number(code_execution_requests.into()),
            );
        }
        if tool_search_requests > 0 {
            server_tool_use.insert(
                "tool_search_requests".to_string(),
                serde_json::Value::Number(tool_search_requests.into()),
            );
        }
        serde_json::Value::Object(server_tool_use)
    });
    usage
}
