use super::*;

pub(crate) fn runtime_anthropic_message_id() -> String {
    format!("msg_{}", runtime_random_token("claude").replace('-', ""))
}

pub(crate) fn runtime_anthropic_error_type_for_status(status: u16) -> &'static str {
    match status {
        400 => "invalid_request_error",
        401 => "authentication_error",
        403 => "permission_error",
        404 => "not_found_error",
        429 => "rate_limit_error",
        500 | 502 | 503 | 504 | 529 => "overloaded_error",
        _ => "api_error",
    }
}

pub(crate) fn runtime_anthropic_error_message_from_parts(
    parts: &RuntimeBufferedResponseParts,
) -> String {
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&parts.body) {
        if let Some(message) = value
            .get("error")
            .and_then(|error| error.get("message"))
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return message.to_string();
        }
        if let Some(message) = value
            .get("message")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return message.to_string();
        }
    }
    let body = String::from_utf8_lossy(&parts.body).trim().to_string();
    if body.is_empty() {
        "Upstream runtime proxy request failed.".to_string()
    } else {
        body
    }
}

pub(crate) fn build_runtime_anthropic_error_parts(
    status: u16,
    error_type: &str,
    message: &str,
) -> RuntimeBufferedResponseParts {
    RuntimeBufferedResponseParts {
        status,
        headers: vec![("Content-Type".to_string(), b"application/json".to_vec())],
        body: serde_json::json!({
            "type": "error",
            "error": {
                "type": error_type,
                "message": message,
            }
        })
        .to_string()
        .into_bytes(),
    }
}

pub(crate) fn runtime_anthropic_error_from_upstream_parts(
    parts: RuntimeBufferedResponseParts,
) -> RuntimeBufferedResponseParts {
    let status = parts.status;
    let message = runtime_anthropic_error_message_from_parts(&parts);
    build_runtime_anthropic_error_parts(
        status,
        runtime_anthropic_error_type_for_status(status),
        &message,
    )
}

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

pub(crate) fn runtime_anthropic_tool_input_from_arguments(arguments: &str) -> serde_json::Value {
    serde_json::from_str::<serde_json::Value>(arguments)
        .ok()
        .filter(|value| value.is_object())
        .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()))
}

pub(crate) fn runtime_anthropic_reasoning_summary_text(item: &serde_json::Value) -> String {
    item.get("summary")
        .and_then(serde_json::Value::as_array)
        .map(|summary| {
            summary
                .iter()
                .filter_map(|entry| {
                    entry
                        .get("text")
                        .and_then(serde_json::Value::as_str)
                        .or_else(|| {
                            (entry.get("type").and_then(serde_json::Value::as_str)
                                == Some("summary_text"))
                            .then(|| entry.get("text").and_then(serde_json::Value::as_str))
                            .flatten()
                        })
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

pub(crate) fn runtime_anthropic_message_annotation_titles_by_url(
    output: &[serde_json::Value],
) -> BTreeMap<String, String> {
    let mut titles = BTreeMap::new();
    for item in output {
        let Some(parts) = item.get("content").and_then(serde_json::Value::as_array) else {
            continue;
        };
        for part in parts {
            let Some(annotations) = part
                .get("annotations")
                .and_then(serde_json::Value::as_array)
            else {
                continue;
            };
            for annotation in annotations {
                let url = annotation
                    .get("url")
                    .and_then(serde_json::Value::as_str)
                    .or_else(|| {
                        annotation
                            .get("url_citation")
                            .and_then(|value| value.get("url"))
                            .and_then(serde_json::Value::as_str)
                    })
                    .map(str::trim)
                    .filter(|value| !value.is_empty());
                let title = annotation
                    .get("title")
                    .and_then(serde_json::Value::as_str)
                    .or_else(|| {
                        annotation
                            .get("url_citation")
                            .and_then(|value| value.get("title"))
                            .and_then(serde_json::Value::as_str)
                    })
                    .map(str::trim)
                    .filter(|value| !value.is_empty());
                if let (Some(url), Some(title)) = (url, title) {
                    titles
                        .entry(url.to_string())
                        .or_insert_with(|| title.to_string());
                }
            }
        }
    }
    titles
}

pub(crate) fn runtime_anthropic_web_search_blocks_from_output_item(
    item: &serde_json::Value,
    annotation_titles_by_url: &BTreeMap<String, String>,
) -> Vec<serde_json::Value> {
    let call_id = item
        .get("id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("web_search_call")
        .to_string();
    let queries = item
        .get("action")
        .and_then(|action| action.get("queries"))
        .and_then(serde_json::Value::as_array)
        .map(|queries| {
            queries
                .iter()
                .filter_map(|query| {
                    query
                        .as_str()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(|value| serde_json::Value::String(value.to_string()))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let query = item
        .get("action")
        .and_then(|action| action.get("query"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            queries
                .first()
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_default();

    let mut seen_urls = BTreeSet::new();
    let mut results = Vec::new();
    if let Some(sources) = item
        .get("action")
        .and_then(|action| action.get("sources"))
        .and_then(serde_json::Value::as_array)
    {
        for source in sources {
            let Some(url) = source
                .get("url")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            if !seen_urls.insert(url.to_string()) {
                continue;
            }
            let mut result = serde_json::Map::new();
            result.insert(
                "type".to_string(),
                serde_json::Value::String("web_search_result".to_string()),
            );
            result.insert(
                "url".to_string(),
                serde_json::Value::String(url.to_string()),
            );
            if let Some(title) = source
                .get("title")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .or_else(|| annotation_titles_by_url.get(url).map(String::as_str))
            {
                result.insert(
                    "title".to_string(),
                    serde_json::Value::String(title.to_string()),
                );
            }
            for key in [
                "encrypted_content",
                "page_age",
                "snippet",
                "summary",
                "text",
            ] {
                if let Some(value) = source
                    .get(key)
                    .and_then(serde_json::Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    result.insert(
                        key.to_string(),
                        serde_json::Value::String(value.to_string()),
                    );
                }
            }
            results.push(serde_json::Value::Object(result));
        }
    }
    if results.is_empty() {
        for (url, title) in annotation_titles_by_url {
            if !seen_urls.insert(url.clone()) {
                continue;
            }
            results.push(serde_json::json!({
                "type": "web_search_result",
                "url": url,
                "title": title,
            }));
        }
    }

    vec![
        serde_json::json!({
            "type": "server_tool_use",
            "id": call_id,
            "name": "web_search",
            "input": {
                "query": query,
                "queries": queries,
            },
        }),
        serde_json::json!({
            "type": "web_search_tool_result",
            "tool_use_id": call_id,
            "content": results,
        }),
    ]
}

pub(crate) fn runtime_anthropic_shell_tool_input_from_output_item(
    item: &serde_json::Value,
) -> serde_json::Value {
    let mut input = serde_json::Map::new();
    if let Some(action) = item.get("action").and_then(serde_json::Value::as_object) {
        let commands = action
            .get("commands")
            .and_then(serde_json::Value::as_array)
            .map(|commands| {
                commands
                    .iter()
                    .filter_map(|command| {
                        command
                            .as_str()
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                            .map(|value| serde_json::Value::String(value.to_string()))
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if !commands.is_empty() {
            let command_text = commands
                .iter()
                .filter_map(serde_json::Value::as_str)
                .collect::<Vec<_>>()
                .join("\n");
            input.insert(
                "command".to_string(),
                serde_json::Value::String(command_text),
            );
        } else if let Some(command) = action
            .get("command")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            input.insert(
                "command".to_string(),
                serde_json::Value::String(command.to_string()),
            );
        }
        if let Some(timeout_ms) = action.get("timeout_ms").and_then(serde_json::Value::as_u64) {
            input.insert(
                "timeout_ms".to_string(),
                serde_json::Value::Number(timeout_ms.into()),
            );
        }
        if let Some(max_output_length) = action
            .get("max_output_length")
            .and_then(serde_json::Value::as_u64)
        {
            input.insert(
                "max_output_length".to_string(),
                serde_json::Value::Number(max_output_length.into()),
            );
        }
    }
    serde_json::Value::Object(input)
}

pub(crate) fn runtime_anthropic_shell_tool_use_block_from_output_item(
    item: &serde_json::Value,
) -> serde_json::Value {
    let call_id = item
        .get("call_id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("shell_call");
    serde_json::json!({
        "type": "tool_use",
        "id": call_id,
        "name": "bash",
        "input": runtime_anthropic_shell_tool_input_from_output_item(item),
    })
}

pub(crate) fn runtime_anthropic_computer_key_combo_from_output_action(
    action: &serde_json::Map<String, serde_json::Value>,
) -> Option<String> {
    let keys = action
        .get("keys")
        .and_then(serde_json::Value::as_array)?
        .iter()
        .filter_map(|key| {
            key.as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| value.to_ascii_lowercase())
        })
        .collect::<Vec<_>>();
    (!keys.is_empty()).then_some(keys.join("+"))
}

pub(crate) fn runtime_anthropic_computer_tool_input_from_output_item(
    item: &serde_json::Value,
) -> Option<serde_json::Value> {
    let actions = item.get("actions").and_then(serde_json::Value::as_array)?;
    if actions.len() != 1 {
        return None;
    }
    let action = actions.first()?.as_object()?;
    let action_type = action
        .get("type")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let input = match action_type {
        "screenshot" => serde_json::json!({ "action": "screenshot" }),
        "click" => {
            let button = action
                .get("button")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("left");
            let button_action = match button {
                "left" => "left_click",
                "right" => "right_click",
                "middle" => "middle_click",
                _ => return None,
            };
            let x = runtime_proxy_anthropic_coordinate_component(action.get("x")?)?;
            let y = runtime_proxy_anthropic_coordinate_component(action.get("y")?)?;
            serde_json::json!({
                "action": button_action,
                "coordinate": [x, y],
            })
        }
        "double_click" => {
            let x = runtime_proxy_anthropic_coordinate_component(action.get("x")?)?;
            let y = runtime_proxy_anthropic_coordinate_component(action.get("y")?)?;
            serde_json::json!({
                "action": "double_click",
                "coordinate": [x, y],
            })
        }
        "move" => {
            let x = runtime_proxy_anthropic_coordinate_component(action.get("x")?)?;
            let y = runtime_proxy_anthropic_coordinate_component(action.get("y")?)?;
            serde_json::json!({
                "action": "mouse_move",
                "coordinate": [x, y],
            })
        }
        "type" => serde_json::json!({
            "action": "type",
            "text": action
                .get("text")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())?,
        }),
        "keypress" => serde_json::json!({
            "action": "key",
            "key": runtime_anthropic_computer_key_combo_from_output_action(action)?,
        }),
        "wait" => serde_json::json!({
            "action": "wait",
        }),
        _ => return None,
    };
    Some(input)
}

pub(crate) fn runtime_anthropic_raw_computer_tool_input_from_output_item(
    item: &serde_json::Value,
) -> serde_json::Value {
    item.get("actions")
        .filter(|value| value.is_array())
        .cloned()
        .map(|actions| serde_json::json!({ "actions": actions }))
        .unwrap_or_else(|| serde_json::json!({}))
}

pub(crate) fn runtime_anthropic_computer_tool_use_block_from_output_item(
    item: &serde_json::Value,
) -> serde_json::Value {
    let call_id = item
        .get("call_id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("computer_call");
    serde_json::json!({
        "type": "tool_use",
        "id": call_id,
        "name": "computer",
        "input": runtime_anthropic_computer_tool_input_from_output_item(item)
            .unwrap_or_else(|| runtime_anthropic_raw_computer_tool_input_from_output_item(item)),
    })
}

pub(crate) fn runtime_anthropic_server_tool_use_block(
    call_id: &str,
    tool_name: &str,
    input: serde_json::Value,
    server_tools: Option<&RuntimeAnthropicServerTools>,
) -> Option<serde_json::Value> {
    runtime_anthropic_server_tool_registration_for_call(tool_name, server_tools).map(
        |(response_name, block_type)| {
            if block_type == "mcp_tool_use" {
                let mut input = input;
                let server_name = input
                    .get("server_name")
                    .and_then(serde_json::Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string);
                if let Some(object) = input.as_object_mut() {
                    object.remove("server_name");
                }
                let mut block = serde_json::Map::new();
                block.insert(
                    "type".to_string(),
                    serde_json::Value::String("mcp_tool_use".to_string()),
                );
                block.insert(
                    "id".to_string(),
                    serde_json::Value::String(call_id.to_string()),
                );
                block.insert("name".to_string(), serde_json::Value::String(response_name));
                block.insert("input".to_string(), input);
                if let Some(server_name) = server_name {
                    block.insert(
                        "server_name".to_string(),
                        serde_json::Value::String(server_name),
                    );
                }
                serde_json::Value::Object(block)
            } else {
                serde_json::json!({
                    "type": "server_tool_use",
                    "id": call_id,
                    "name": response_name,
                    "input": input,
                })
            }
        },
    )
}

pub(crate) fn runtime_anthropic_mcp_call_blocks_from_output_item(
    item: &serde_json::Value,
) -> Vec<serde_json::Value> {
    let call_id = item
        .get("id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("mcp_call")
        .to_string();
    let name = item
        .get("name")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("mcp_tool")
        .to_string();
    let server_name = item
        .get("server_label")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("mcp")
        .to_string();
    let input = runtime_anthropic_tool_input_from_arguments(
        item.get("arguments")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("{}"),
    );

    let mut content = vec![serde_json::json!({
        "type": "mcp_tool_use",
        "id": call_id,
        "name": name,
        "server_name": server_name,
        "input": input,
    })];

    let mut result_content = Vec::new();
    if let Some(output) = item
        .get("output")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        result_content.push(serde_json::json!({
            "type": "text",
            "text": output,
        }));
    }
    let is_error = item
        .get("error")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|error| {
            result_content.push(serde_json::json!({
                "type": "text",
                "text": error,
            }));
            true
        })
        .unwrap_or(false);
    if !result_content.is_empty() {
        content.push(serde_json::json!({
            "type": "mcp_tool_result",
            "tool_use_id": content
                .first()
                .and_then(|block| block.get("id"))
                .cloned()
                .unwrap_or_else(|| serde_json::Value::String("mcp_call".to_string())),
            "is_error": is_error,
            "content": result_content,
        }));
    }

    content
}

pub(crate) fn runtime_anthropic_mcp_approval_request_block_from_output_item(
    item: &serde_json::Value,
) -> serde_json::Value {
    let id = item
        .get("id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("mcp_approval_request");
    let name = item
        .get("name")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("mcp_tool");
    let server_name = item
        .get("server_label")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("mcp");
    let arguments = item
        .get("arguments")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .unwrap_or("{}");
    let input = runtime_anthropic_tool_input_from_arguments(arguments);
    serde_json::json!({
        "type": "mcp_approval_request",
        "id": id,
        "name": name,
        "server_name": server_name,
        "server_label": server_name,
        "arguments": arguments,
        "input": input,
    })
}

pub(crate) fn runtime_anthropic_mcp_list_tools_block_from_output_item(
    item: &serde_json::Value,
) -> serde_json::Value {
    let id = item
        .get("id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("mcp_list_tools");
    let server_name = item
        .get("server_label")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("mcp");
    let mut block = serde_json::Map::new();
    block.insert(
        "type".to_string(),
        serde_json::Value::String("mcp_list_tools".to_string()),
    );
    block.insert("id".to_string(), serde_json::Value::String(id.to_string()));
    block.insert(
        "server_name".to_string(),
        serde_json::Value::String(server_name.to_string()),
    );
    block.insert(
        "server_label".to_string(),
        serde_json::Value::String(server_name.to_string()),
    );
    if let Some(tools) = item.get("tools").filter(|value| value.is_array()).cloned() {
        block.insert("tools".to_string(), tools);
    }
    if let Some(error) = item
        .get("error")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        block.insert(
            "error".to_string(),
            serde_json::Value::String(error.to_string()),
        );
    }
    serde_json::Value::Object(block)
}

struct RuntimeAnthropicOutputBlockTranslator<'a> {
    content: Vec<serde_json::Value>,
    has_tool_calls: bool,
    want_thinking: bool,
    server_tools: Option<&'a RuntimeAnthropicServerTools>,
    annotation_titles_by_url: BTreeMap<String, String>,
}

impl<'a> RuntimeAnthropicOutputBlockTranslator<'a> {
    fn new(
        output: &[serde_json::Value],
        want_thinking: bool,
        server_tools: Option<&'a RuntimeAnthropicServerTools>,
    ) -> Self {
        Self {
            content: Vec::new(),
            has_tool_calls: false,
            want_thinking,
            server_tools,
            annotation_titles_by_url: runtime_anthropic_message_annotation_titles_by_url(output),
        }
    }

    fn translate(mut self, output: &[serde_json::Value]) -> (Vec<serde_json::Value>, bool) {
        for item in output {
            self.push_item(item);
        }
        if self.content.is_empty() {
            self.content.push(serde_json::json!({
                "type": "text",
                "text": "",
            }));
        }
        (self.content, self.has_tool_calls)
    }

    fn push_item(&mut self, item: &serde_json::Value) {
        match item.get("type").and_then(serde_json::Value::as_str) {
            Some("reasoning") if self.want_thinking => {
                self.push_reasoning(item);
            }
            Some("message") => {
                self.push_message_text(item);
            }
            Some("web_search_call") => {
                self.content
                    .extend(runtime_anthropic_web_search_blocks_from_output_item(
                        item,
                        &self.annotation_titles_by_url,
                    ));
            }
            Some("mcp_call") => {
                self.content
                    .extend(runtime_anthropic_mcp_call_blocks_from_output_item(item));
            }
            Some("mcp_approval_request") => {
                self.has_tool_calls = true;
                self.content
                    .push(runtime_anthropic_mcp_approval_request_block_from_output_item(item));
            }
            Some("mcp_list_tools") => {
                self.content
                    .push(runtime_anthropic_mcp_list_tools_block_from_output_item(
                        item,
                    ));
            }
            Some("shell_call") => {
                self.has_tool_calls = true;
                self.content
                    .push(runtime_anthropic_shell_tool_use_block_from_output_item(
                        item,
                    ));
            }
            Some("computer_call") => {
                self.has_tool_calls = true;
                self.content
                    .push(runtime_anthropic_computer_tool_use_block_from_output_item(
                        item,
                    ));
            }
            Some("function_call") => {
                self.push_function_call(item);
            }
            _ => {}
        }
    }

    fn push_reasoning(&mut self, item: &serde_json::Value) {
        if !self.want_thinking {
            return;
        }
        let thinking = runtime_anthropic_reasoning_summary_text(item);
        if !thinking.is_empty() {
            self.content.push(serde_json::json!({
                "type": "thinking",
                "thinking": thinking,
            }));
        }
    }

    fn push_message_text(&mut self, item: &serde_json::Value) {
        let Some(parts) = item.get("content").and_then(serde_json::Value::as_array) else {
            return;
        };
        let mut text = String::new();
        for part in parts {
            if part
                .get("type")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|part_type| matches!(part_type, "output_text" | "text"))
                && let Some(part_text) = part.get("text").and_then(serde_json::Value::as_str)
            {
                text.push_str(part_text);
            }
        }
        if !text.is_empty() {
            self.content.push(serde_json::json!({
                "type": "text",
                "text": text,
            }));
        }
    }

    fn push_function_call(&mut self, item: &serde_json::Value) {
        self.has_tool_calls = true;
        let call_id = item
            .get("call_id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("tool_call");
        let name = item
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("tool");
        let input = runtime_anthropic_tool_input_from_arguments(
            item.get("arguments")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("{}"),
        );
        self.content.push(
            runtime_anthropic_server_tool_use_block(
                call_id,
                name,
                input.clone(),
                self.server_tools,
            )
            .unwrap_or_else(|| {
                serde_json::json!({
                    "type": "tool_use",
                    "id": call_id,
                    "name": name,
                    "input": input,
                })
            }),
        );
    }
}

pub(crate) fn runtime_anthropic_output_blocks_from_json(
    output: &[serde_json::Value],
    want_thinking: bool,
    server_tools: Option<&RuntimeAnthropicServerTools>,
) -> (Vec<serde_json::Value>, bool) {
    RuntimeAnthropicOutputBlockTranslator::new(output, want_thinking, server_tools)
        .translate(output)
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn runtime_anthropic_response_from_json_value(
    value: &serde_json::Value,
    requested_model: &str,
    want_thinking: bool,
) -> serde_json::Value {
    runtime_anthropic_response_from_json_value_with_carried_usage(
        value,
        requested_model,
        want_thinking,
        0,
        0,
        0,
        0,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn runtime_anthropic_response_from_json_value_with_carried_usage(
    value: &serde_json::Value,
    requested_model: &str,
    want_thinking: bool,
    carried_web_search_requests: u64,
    carried_web_fetch_requests: u64,
    carried_code_execution_requests: u64,
    carried_tool_search_requests: u64,
    server_tools: Option<&RuntimeAnthropicServerTools>,
) -> serde_json::Value {
    let (input_tokens, output_tokens, cached_tokens) = runtime_anthropic_usage_from_value(value);
    let output = value
        .get("output")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let web_search_requests = runtime_anthropic_tool_usage_web_search_requests_from_value(value)
        .max(runtime_anthropic_web_search_request_count_from_output(
            &output,
            server_tools,
        ))
        .max(carried_web_search_requests);
    let web_fetch_requests =
        runtime_anthropic_web_fetch_request_count_from_output(&output, server_tools)
            .max(carried_web_fetch_requests);
    let code_execution_requests =
        runtime_anthropic_tool_usage_code_execution_requests_from_value(value)
            .max(runtime_anthropic_code_execution_request_count_from_output(
                &output,
                server_tools,
            ))
            .max(carried_code_execution_requests);
    let tool_search_requests = runtime_anthropic_tool_usage_tool_search_requests_from_value(value)
        .max(runtime_anthropic_tool_search_request_count_from_output(
            &output,
            server_tools,
        ))
        .max(carried_tool_search_requests);
    let (content, has_tool_calls) =
        runtime_anthropic_output_blocks_from_json(&output, want_thinking, server_tools);
    let usage = runtime_anthropic_usage_json(
        input_tokens,
        output_tokens,
        cached_tokens,
        web_search_requests,
        web_fetch_requests,
        code_execution_requests,
        tool_search_requests,
    );
    serde_json::json!({
        "id": runtime_anthropic_message_id(),
        "type": "message",
        "role": "assistant",
        "content": content,
        "model": requested_model,
        "stop_reason": if has_tool_calls { "tool_use" } else { "end_turn" },
        "stop_sequence": serde_json::Value::Null,
        "usage": usage,
    })
}

#[derive(Debug, Clone, Default)]
pub(crate) struct RuntimeAnthropicCollectedToolUse {
    call_id: String,
    name: String,
    arguments: String,
    saw_delta: bool,
    server_tool_name: Option<String>,
    server_tool_block_type: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct RuntimeAnthropicCollectedResponse {
    content: Vec<serde_json::Value>,
    pending_text: String,
    pending_thinking: String,
    active_tool_use: Option<RuntimeAnthropicCollectedToolUse>,
    final_output: Option<Vec<serde_json::Value>>,
    input_tokens: u64,
    output_tokens: u64,
    cached_tokens: Option<u64>,
    web_search_requests: u64,
    web_fetch_requests: u64,
    code_execution_requests: u64,
    tool_search_requests: u64,
    has_tool_calls: bool,
    want_thinking: bool,
    server_tools: RuntimeAnthropicServerTools,
}

fn runtime_anthropic_response_event_item(value: &serde_json::Value) -> Option<&serde_json::Value> {
    value.get("item")
}

fn runtime_anthropic_output_item_type(item: &serde_json::Value) -> Option<&str> {
    item.get("type").and_then(serde_json::Value::as_str)
}

fn runtime_anthropic_output_item_call_id<'a>(
    item: &'a serde_json::Value,
    default: &'static str,
) -> &'a str {
    item.get("call_id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(default)
}

fn runtime_anthropic_output_item_name<'a>(
    item: &'a serde_json::Value,
    default: &'static str,
) -> &'a str {
    item.get("name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(default)
}

fn runtime_anthropic_response_event_error_message(value: &serde_json::Value) -> &str {
    value
        .get("error")
        .and_then(|error| error.get("message"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("Codex returned an error.")
}

impl RuntimeAnthropicCollectedResponse {
    fn flush_text(&mut self) {
        if self.pending_text.is_empty() {
            return;
        }
        self.content.push(serde_json::json!({
            "type": "text",
            "text": std::mem::take(&mut self.pending_text),
        }));
    }

    fn flush_thinking(&mut self) {
        if self.pending_thinking.is_empty() {
            return;
        }
        self.content.push(serde_json::json!({
            "type": "thinking",
            "thinking": std::mem::take(&mut self.pending_thinking),
        }));
    }

    fn flush_pending_textual_content(&mut self) {
        self.flush_thinking();
        self.flush_text();
    }

    fn close_active_tool_use(&mut self) {
        let Some(active_tool_use) = self.active_tool_use.take() else {
            return;
        };
        self.has_tool_calls = true;
        let input = runtime_anthropic_tool_input_from_arguments(&active_tool_use.arguments);
        self.content.push(
            active_tool_use
                .server_tool_name
                .as_deref()
                .and_then(|server_tool_name| {
                    runtime_anthropic_server_tool_use_block(
                        &active_tool_use.call_id,
                        server_tool_name,
                        input.clone(),
                        Some(&self.server_tools),
                    )
                })
                .unwrap_or_else(|| {
                    serde_json::json!({
                        "type": "tool_use",
                        "id": active_tool_use.call_id,
                        "name": active_tool_use.name,
                        "input": input,
                    })
                }),
        );
    }

    fn observe_event(&mut self, value: &serde_json::Value) -> Result<()> {
        match value.get("type").and_then(serde_json::Value::as_str) {
            Some("response.reasoning_summary_text.delta") if self.want_thinking => {
                self.observe_reasoning_delta(value);
            }
            Some("response.output_text.delta") => {
                self.observe_text_delta(value);
            }
            Some("response.output_item.added") => {
                self.observe_output_item_added(value);
            }
            Some("response.function_call_arguments.delta") => {
                self.observe_function_call_arguments_delta(value);
            }
            Some("response.function_call_arguments.done") => {
                self.observe_function_call_arguments_done(value);
            }
            Some("response.output_item.done") => {
                self.observe_output_item_done(value);
            }
            Some("response.completed") => {
                self.observe_completed(value);
            }
            Some("error" | "response.failed") => {
                bail!(runtime_anthropic_response_event_error_message(value).to_string());
            }
            _ => {}
        }
        Ok(())
    }

    fn observe_reasoning_delta(&mut self, value: &serde_json::Value) {
        self.flush_text();
        if let Some(delta) = value.get("delta").and_then(serde_json::Value::as_str) {
            self.pending_thinking.push_str(delta);
        }
    }

    fn observe_text_delta(&mut self, value: &serde_json::Value) {
        self.flush_thinking();
        if let Some(delta) = value.get("delta").and_then(serde_json::Value::as_str) {
            self.pending_text.push_str(delta);
        }
    }

    fn observe_output_item_added(&mut self, value: &serde_json::Value) {
        let Some(item) = runtime_anthropic_response_event_item(value) else {
            return;
        };
        if runtime_anthropic_output_item_type(item) != Some("function_call") {
            return;
        }
        self.flush_pending_textual_content();
        self.active_tool_use = Some(self.collected_tool_use_from_item(item));
    }

    fn collected_tool_use_from_item(
        &self,
        item: &serde_json::Value,
    ) -> RuntimeAnthropicCollectedToolUse {
        let name = runtime_anthropic_output_item_name(item, "tool");
        let server_tool_registration =
            runtime_anthropic_server_tool_registration_for_call(name, Some(&self.server_tools));
        RuntimeAnthropicCollectedToolUse {
            call_id: runtime_anthropic_output_item_call_id(item, "tool_call").to_string(),
            name: name.to_string(),
            server_tool_name: server_tool_registration
                .as_ref()
                .map(|(server_tool_name, _)| server_tool_name.clone()),
            server_tool_block_type: server_tool_registration.map(|(_, block_type)| block_type),
            ..RuntimeAnthropicCollectedToolUse::default()
        }
    }

    fn observe_function_call_arguments_delta(&mut self, value: &serde_json::Value) {
        if let Some(active_tool_use) = self.active_tool_use.as_mut()
            && let Some(delta) = value.get("delta").and_then(serde_json::Value::as_str)
        {
            active_tool_use.saw_delta = true;
            active_tool_use.arguments.push_str(delta);
        }
    }

    fn observe_function_call_arguments_done(&mut self, value: &serde_json::Value) {
        if let Some(active_tool_use) = self.active_tool_use.as_mut()
            && let Some(arguments) = value.get("arguments").and_then(serde_json::Value::as_str)
            && !active_tool_use.saw_delta
        {
            active_tool_use.arguments = arguments.to_string();
        }
    }

    fn observe_output_item_done(&mut self, value: &serde_json::Value) {
        let Some(item) = runtime_anthropic_response_event_item(value) else {
            return;
        };
        match runtime_anthropic_output_item_type(item) {
            Some("function_call") => self.observe_function_call_item_done(item),
            Some("web_search_call") => self.observe_web_search_item_done(item),
            Some("shell_call") => self.observe_shell_call_item_done(item),
            Some("computer_call") => self.observe_computer_call_item_done(item),
            _ => {}
        }
    }

    fn observe_function_call_item_done(&mut self, item: &serde_json::Value) {
        self.add_output_item_server_tool_usage(item);
        if let Some(active_tool_use) = self.active_tool_use.as_mut() {
            if let Some(arguments) = item.get("arguments").and_then(serde_json::Value::as_str)
                && !active_tool_use.saw_delta
            {
                active_tool_use.arguments = arguments.to_string();
            }
            if let Some(name) = item.get("name").and_then(serde_json::Value::as_str) {
                active_tool_use.name = name.to_string();
                Self::set_collected_tool_server_registration(
                    active_tool_use,
                    name,
                    &self.server_tools,
                );
            }
        }
        self.close_active_tool_use();
    }

    fn set_collected_tool_server_registration(
        active_tool_use: &mut RuntimeAnthropicCollectedToolUse,
        name: &str,
        server_tools: &RuntimeAnthropicServerTools,
    ) {
        if let Some((server_tool_name, block_type)) =
            runtime_anthropic_server_tool_registration_for_call(name, Some(server_tools))
        {
            active_tool_use.server_tool_name = Some(server_tool_name);
            active_tool_use.server_tool_block_type = Some(block_type);
        } else {
            active_tool_use.server_tool_name = None;
            active_tool_use.server_tool_block_type = None;
        }
    }

    fn observe_web_search_item_done(&mut self, item: &serde_json::Value) {
        self.flush_pending_textual_content();
        self.web_search_requests +=
            runtime_anthropic_web_search_request_count_from_output_item(item);
        self.content
            .extend(runtime_anthropic_web_search_blocks_from_output_item(
                item,
                &BTreeMap::new(),
            ));
    }

    fn observe_shell_call_item_done(&mut self, item: &serde_json::Value) {
        self.flush_pending_textual_content();
        self.has_tool_calls = true;
        self.content
            .push(runtime_anthropic_shell_tool_use_block_from_output_item(
                item,
            ));
    }

    fn observe_computer_call_item_done(&mut self, item: &serde_json::Value) {
        self.flush_pending_textual_content();
        self.has_tool_calls = true;
        self.content
            .push(runtime_anthropic_computer_tool_use_block_from_output_item(
                item,
            ));
    }

    fn observe_completed(&mut self, value: &serde_json::Value) {
        let (input_tokens, output_tokens, cached_tokens) =
            runtime_anthropic_usage_from_value(value);
        self.input_tokens = input_tokens;
        self.output_tokens = output_tokens;
        self.cached_tokens = cached_tokens;
        self.web_search_requests = self.web_search_requests.max(
            runtime_anthropic_tool_usage_web_search_requests_from_value(value),
        );
        self.code_execution_requests = self
            .code_execution_requests
            .max(runtime_anthropic_tool_usage_code_execution_requests_from_value(value));
        self.tool_search_requests = self
            .tool_search_requests
            .max(runtime_anthropic_tool_usage_tool_search_requests_from_value(value));
        let final_output = value
            .get("response")
            .and_then(|response| response.get("output"))
            .and_then(serde_json::Value::as_array)
            .cloned();
        if let Some(output) = final_output.as_ref() {
            self.merge_output_usage(output);
        }
        self.final_output = final_output;
    }

    fn add_output_item_server_tool_usage(&mut self, item: &serde_json::Value) {
        let usage = runtime_anthropic_output_item_server_tool_usage(item, Some(&self.server_tools));
        self.web_search_requests += usage.web_search_requests;
        self.web_fetch_requests += usage.web_fetch_requests;
        self.code_execution_requests += usage.code_execution_requests;
        self.tool_search_requests += usage.tool_search_requests;
    }

    fn merge_output_usage(&mut self, output: &[serde_json::Value]) {
        self.web_search_requests =
            self.web_search_requests
                .max(runtime_anthropic_web_search_request_count_from_output(
                    output,
                    Some(&self.server_tools),
                ));
        self.web_fetch_requests =
            self.web_fetch_requests
                .max(runtime_anthropic_web_fetch_request_count_from_output(
                    output,
                    Some(&self.server_tools),
                ));
        self.code_execution_requests = self.code_execution_requests.max(
            runtime_anthropic_code_execution_request_count_from_output(
                output,
                Some(&self.server_tools),
            ),
        );
        self.tool_search_requests =
            self.tool_search_requests
                .max(runtime_anthropic_tool_search_request_count_from_output(
                    output,
                    Some(&self.server_tools),
                ));
    }

    fn into_response(mut self, requested_model: &str) -> serde_json::Value {
        if let Some(output) = self.final_output.take().filter(|output| !output.is_empty()) {
            let (content, has_tool_calls) = runtime_anthropic_output_blocks_from_json(
                &output,
                self.want_thinking,
                Some(&self.server_tools),
            );
            self.content = content;
            self.has_tool_calls = has_tool_calls;
        } else {
            self.close_active_tool_use();
            self.flush_pending_textual_content();
            if self.content.is_empty() {
                self.content.push(serde_json::json!({
                    "type": "text",
                    "text": "",
                }));
            }
        }
        let usage = runtime_anthropic_usage_json(
            self.input_tokens,
            self.output_tokens,
            self.cached_tokens,
            self.web_search_requests,
            self.web_fetch_requests,
            self.code_execution_requests,
            self.tool_search_requests,
        );
        serde_json::json!({
            "id": runtime_anthropic_message_id(),
            "type": "message",
            "role": "assistant",
            "content": self.content,
            "model": requested_model,
            "stop_reason": if self.has_tool_calls { "tool_use" } else { "end_turn" },
            "stop_sequence": serde_json::Value::Null,
            "usage": usage,
        })
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn runtime_anthropic_response_from_sse_bytes(
    body: &[u8],
    requested_model: &str,
    want_thinking: bool,
) -> Result<serde_json::Value> {
    runtime_anthropic_response_from_sse_bytes_with_carried_usage(
        body,
        requested_model,
        want_thinking,
        0,
        0,
        0,
        0,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn runtime_anthropic_response_from_sse_bytes_with_carried_usage(
    body: &[u8],
    requested_model: &str,
    want_thinking: bool,
    carried_web_search_requests: u64,
    carried_web_fetch_requests: u64,
    carried_code_execution_requests: u64,
    carried_tool_search_requests: u64,
    server_tools: Option<&RuntimeAnthropicServerTools>,
) -> Result<serde_json::Value> {
    let mut collected = RuntimeAnthropicCollectedResponse {
        want_thinking,
        web_search_requests: carried_web_search_requests,
        web_fetch_requests: carried_web_fetch_requests,
        code_execution_requests: carried_code_execution_requests,
        tool_search_requests: carried_tool_search_requests,
        server_tools: server_tools.cloned().unwrap_or_default(),
        ..RuntimeAnthropicCollectedResponse::default()
    };
    let mut line = Vec::new();
    let mut data_lines = Vec::new();

    let mut process_event = |data_lines: &mut Vec<String>| -> Result<()> {
        if data_lines.is_empty() {
            return Ok(());
        }
        let payload = data_lines.join("\n");
        let value = serde_json::from_str::<serde_json::Value>(&payload)
            .context("failed to parse buffered Responses SSE payload")?;
        collected.observe_event(&value)?;
        data_lines.clear();
        Ok(())
    };

    for byte in body {
        line.push(*byte);
        if *byte != b'\n' {
            continue;
        }
        let line_text = String::from_utf8_lossy(&line);
        let trimmed = line_text.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            process_event(&mut data_lines)?;
            line.clear();
            continue;
        }
        if let Some(payload) = trimmed.strip_prefix("data:") {
            data_lines.push(payload.trim_start().to_string());
        }
        line.clear();
    }
    if !line.is_empty() {
        let line_text = String::from_utf8_lossy(&line);
        let trimmed = line_text.trim_end_matches(['\r', '\n']);
        if let Some(payload) = trimmed.strip_prefix("data:") {
            data_lines.push(payload.trim_start().to_string());
        }
    }
    process_event(&mut data_lines)?;
    Ok(collected.into_response(requested_model))
}

pub(crate) fn runtime_anthropic_json_response_parts(
    value: serde_json::Value,
) -> RuntimeBufferedResponseParts {
    RuntimeBufferedResponseParts {
        status: 200,
        headers: vec![("Content-Type".to_string(), b"application/json".to_vec())],
        body: serde_json::to_vec(&value).unwrap_or_else(|_| b"{}".to_vec()),
    }
}

pub(crate) fn runtime_anthropic_sse_event_bytes(
    event_type: &str,
    data: serde_json::Value,
) -> Vec<u8> {
    format!(
        "event: {event_type}\ndata: {}\n\n",
        serde_json::to_string(&data).unwrap_or_else(|_| "{}".to_string())
    )
    .into_bytes()
}

pub(crate) fn runtime_anthropic_sse_response_parts_from_message_value(
    value: serde_json::Value,
) -> RuntimeBufferedResponseParts {
    let mut body = Vec::new();
    let message_id = value
        .get("id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("msg_prodex")
        .to_string();
    let model = value
        .get("model")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("claude-sonnet-4-6")
        .to_string();
    let stop_reason = value
        .get("stop_reason")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let stop_sequence = value
        .get("stop_sequence")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let usage = value
        .get("usage")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    let message_start_usage = serde_json::json!({
        "input_tokens": 0,
        "output_tokens": 0,
        "server_tool_use": usage
            .get("server_tool_use")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({
                "web_search_requests": 0,
                "web_fetch_requests": 0,
                "code_execution_requests": 0,
                "tool_search_requests": 0,
            })),
    });

    body.extend(runtime_anthropic_sse_event_bytes(
        "message_start",
        serde_json::json!({
            "type": "message_start",
            "message": {
                "id": message_id,
                "type": "message",
                "role": "assistant",
                "content": [],
                "model": model,
                "stop_reason": serde_json::Value::Null,
                "stop_sequence": serde_json::Value::Null,
                "usage": message_start_usage,
            }
        }),
    ));

    for (index, block) in value
        .get("content")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        let index_value = serde_json::Value::Number((index as u64).into());
        match block.get("type").and_then(serde_json::Value::as_str) {
            Some("thinking") => {
                body.extend(runtime_anthropic_sse_event_bytes(
                    "content_block_start",
                    serde_json::json!({
                        "type": "content_block_start",
                        "index": index_value,
                        "content_block": {
                            "type": "thinking",
                            "thinking": "",
                        }
                    }),
                ));
                body.extend(runtime_anthropic_sse_event_bytes(
                    "content_block_delta",
                    serde_json::json!({
                        "type": "content_block_delta",
                        "index": index,
                        "delta": {
                            "type": "thinking_delta",
                            "thinking": block
                                .get("thinking")
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or(""),
                        }
                    }),
                ));
            }
            Some("tool_use") => {
                let input_json = block
                    .get("input")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));
                body.extend(runtime_anthropic_sse_event_bytes(
                    "content_block_start",
                    serde_json::json!({
                        "type": "content_block_start",
                        "index": index_value,
                        "content_block": {
                            "type": "tool_use",
                            "id": block.get("id").cloned().unwrap_or(serde_json::Value::String("tool_use".to_string())),
                            "name": block.get("name").cloned().unwrap_or(serde_json::Value::String("tool".to_string())),
                            "input": serde_json::json!({}),
                        }
                    }),
                ));
                body.extend(runtime_anthropic_sse_event_bytes(
                    "content_block_delta",
                    serde_json::json!({
                        "type": "content_block_delta",
                        "index": index,
                        "delta": {
                            "type": "input_json_delta",
                            "partial_json": serde_json::to_string(&input_json)
                                .unwrap_or_else(|_| "{}".to_string()),
                        }
                    }),
                ));
            }
            Some("server_tool_use") => {
                let input_json = block
                    .get("input")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));
                body.extend(runtime_anthropic_sse_event_bytes(
                    "content_block_start",
                    serde_json::json!({
                        "type": "content_block_start",
                        "index": index_value,
                        "content_block": {
                            "type": "server_tool_use",
                            "id": block.get("id").cloned().unwrap_or(serde_json::Value::String("server_tool_use".to_string())),
                            "name": block.get("name").cloned().unwrap_or(serde_json::Value::String("web_search".to_string())),
                            "input": serde_json::json!({}),
                        }
                    }),
                ));
                body.extend(runtime_anthropic_sse_event_bytes(
                    "content_block_delta",
                    serde_json::json!({
                        "type": "content_block_delta",
                        "index": index,
                        "delta": {
                            "type": "input_json_delta",
                            "partial_json": serde_json::to_string(&input_json)
                                .unwrap_or_else(|_| "{}".to_string()),
                        }
                    }),
                ));
            }
            Some("mcp_tool_use") => {
                let input_json = block
                    .get("input")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));
                body.extend(runtime_anthropic_sse_event_bytes(
                    "content_block_start",
                    serde_json::json!({
                        "type": "content_block_start",
                        "index": index_value,
                        "content_block": {
                            "type": "mcp_tool_use",
                            "id": block.get("id").cloned().unwrap_or(serde_json::Value::String("mcp_tool_use".to_string())),
                            "name": block.get("name").cloned().unwrap_or(serde_json::Value::String("mcp_tool".to_string())),
                            "server_name": block.get("server_name").cloned().unwrap_or_else(|| serde_json::Value::String("mcp".to_string())),
                            "input": serde_json::json!({}),
                        }
                    }),
                ));
                body.extend(runtime_anthropic_sse_event_bytes(
                    "content_block_delta",
                    serde_json::json!({
                        "type": "content_block_delta",
                        "index": index,
                        "delta": {
                            "type": "input_json_delta",
                            "partial_json": serde_json::to_string(&input_json)
                                .unwrap_or_else(|_| "{}".to_string()),
                        }
                    }),
                ));
            }
            Some(block_type) if block_type.ends_with("_tool_result") => {
                body.extend(runtime_anthropic_sse_event_bytes(
                    "content_block_start",
                    serde_json::json!({
                        "type": "content_block_start",
                        "index": index_value,
                        "content_block": {
                            "type": block_type,
                            "tool_use_id": block.get("tool_use_id").cloned().unwrap_or_else(|| {
                                serde_json::Value::String(format!("{block_type}_call"))
                            }),
                            "content": block.get("content").cloned().unwrap_or(serde_json::Value::Null),
                        }
                    }),
                ));
            }
            Some("mcp_approval_request" | "mcp_list_tools") => {
                body.extend(runtime_anthropic_sse_event_bytes(
                    "content_block_start",
                    serde_json::json!({
                        "type": "content_block_start",
                        "index": index_value,
                        "content_block": block.clone(),
                    }),
                ));
            }
            _ => {
                body.extend(runtime_anthropic_sse_event_bytes(
                    "content_block_start",
                    serde_json::json!({
                        "type": "content_block_start",
                        "index": index_value,
                        "content_block": {
                            "type": "text",
                            "text": "",
                        }
                    }),
                ));
                body.extend(runtime_anthropic_sse_event_bytes(
                    "content_block_delta",
                    serde_json::json!({
                        "type": "content_block_delta",
                        "index": index,
                        "delta": {
                            "type": "text_delta",
                            "text": block.get("text").and_then(serde_json::Value::as_str).unwrap_or(""),
                        }
                    }),
                ));
            }
        }
        body.extend(runtime_anthropic_sse_event_bytes(
            "content_block_stop",
            serde_json::json!({
                "type": "content_block_stop",
                "index": index,
            }),
        ));
    }

    body.extend(runtime_anthropic_sse_event_bytes(
        "message_delta",
        serde_json::json!({
            "type": "message_delta",
            "delta": {
                "stop_reason": stop_reason,
                "stop_sequence": stop_sequence,
            },
            "usage": usage,
        }),
    ));
    body.extend(runtime_anthropic_sse_event_bytes(
        "message_stop",
        serde_json::json!({
            "type": "message_stop",
        }),
    ));

    RuntimeBufferedResponseParts {
        status: 200,
        headers: vec![("Content-Type".to_string(), b"text/event-stream".to_vec())],
        body,
    }
}

pub(crate) fn runtime_response_body_looks_like_sse(body: &[u8]) -> bool {
    let trimmed = body
        .iter()
        .copied()
        .skip_while(|byte| byte.is_ascii_whitespace());
    let prefix = trimmed.take(8).collect::<Vec<_>>();
    prefix.starts_with(b"event:") || prefix.starts_with(b"data:")
}

pub(crate) fn runtime_buffered_response_ids(parts: &RuntimeBufferedResponseParts) -> Vec<String> {
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&parts.body) {
        return extract_runtime_response_ids_from_value(&value);
    }

    let mut response_ids = Vec::new();
    let mut line = Vec::new();
    let mut data_lines = Vec::new();
    let push_data_lines = |data_lines: &mut Vec<String>, response_ids: &mut Vec<String>| {
        if let Some(value) = parse_runtime_sse_payload(data_lines) {
            for response_id in extract_runtime_response_ids_from_value(&value) {
                push_runtime_response_id(response_ids, Some(&response_id));
            }
        }
        data_lines.clear();
    };

    for byte in &parts.body {
        line.push(*byte);
        if *byte != b'\n' {
            continue;
        }
        let line_text = String::from_utf8_lossy(&line);
        let trimmed = line_text.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            push_data_lines(&mut data_lines, &mut response_ids);
            line.clear();
            continue;
        }
        if let Some(payload) = trimmed.strip_prefix("data:") {
            data_lines.push(payload.trim_start().to_string());
        }
        line.clear();
    }
    if !line.is_empty() {
        let line_text = String::from_utf8_lossy(&line);
        let trimmed = line_text.trim_end_matches(['\r', '\n']);
        if let Some(payload) = trimmed.strip_prefix("data:") {
            data_lines.push(payload.trim_start().to_string());
        }
    }
    push_data_lines(&mut data_lines, &mut response_ids);
    response_ids
}

pub(crate) fn runtime_request_for_anthropic_server_tool_followup(
    request: &RuntimeProxyRequest,
    previous_response_id: &str,
) -> Result<RuntimeProxyRequest> {
    let mut value = serde_json::from_slice::<serde_json::Value>(&request.body)
        .context("failed to parse translated Anthropic request body")?;
    let object = value
        .as_object_mut()
        .context("translated Anthropic request body must be a JSON object")?;
    object.remove("input");
    object.remove("tool_choice");
    object.insert(
        "previous_response_id".to_string(),
        serde_json::Value::String(previous_response_id.to_string()),
    );
    let stream = object
        .get("stream")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);
    object.insert("stream".to_string(), serde_json::Value::Bool(stream));
    let body = serde_json::to_vec(&value)
        .context("failed to serialize Anthropic server-tool follow-up request")?;
    Ok(RuntimeProxyRequest {
        method: request.method.clone(),
        path_and_query: request.path_and_query.clone(),
        headers: request.headers.clone(),
        body,
    })
}

pub(crate) fn runtime_anthropic_message_needs_server_tool_followup(
    value: &serde_json::Value,
) -> bool {
    let Some(content) = value.get("content").and_then(serde_json::Value::as_array) else {
        return false;
    };

    let mut saw_server_tool_use = false;
    for block in content {
        match block.get("type").and_then(serde_json::Value::as_str) {
            Some("server_tool_use") => saw_server_tool_use = true,
            Some("web_search_tool_result") => {}
            _ => return false,
        }
    }

    saw_server_tool_use
}

pub(crate) fn runtime_anthropic_message_server_tool_usage(
    value: &serde_json::Value,
) -> RuntimeAnthropicServerToolUsage {
    let usage = value
        .get("usage")
        .and_then(|usage| usage.get("server_tool_use"));
    RuntimeAnthropicServerToolUsage {
        web_search_requests: usage
            .and_then(|server_tool_use| server_tool_use.get("web_search_requests"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0),
        web_fetch_requests: usage
            .and_then(|server_tool_use| server_tool_use.get("web_fetch_requests"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0),
        code_execution_requests: usage
            .and_then(|server_tool_use| server_tool_use.get("code_execution_requests"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0),
        tool_search_requests: usage
            .and_then(|server_tool_use| server_tool_use.get("tool_search_requests"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0),
    }
}

pub(crate) fn runtime_anthropic_message_from_buffered_responses_parts_with_carried_usage(
    parts: &RuntimeBufferedResponseParts,
    request: &RuntimeAnthropicMessagesRequest,
    carried_usage: RuntimeAnthropicServerToolUsage,
) -> Result<serde_json::Value> {
    let content_type = runtime_buffered_response_content_type(parts)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let looks_like_sse = content_type.contains("text/event-stream")
        || runtime_response_body_looks_like_sse(&parts.body);
    if looks_like_sse {
        return runtime_anthropic_response_from_sse_bytes_with_carried_usage(
            &parts.body,
            &request.requested_model,
            request.want_thinking,
            carried_usage.web_search_requests,
            carried_usage.web_fetch_requests,
            carried_usage.code_execution_requests,
            carried_usage.tool_search_requests,
            Some(&request.server_tools),
        );
    }

    let value = serde_json::from_slice::<serde_json::Value>(&parts.body)
        .context("failed to parse buffered Responses JSON body")?;
    Ok(
        runtime_anthropic_response_from_json_value_with_carried_usage(
            &value,
            &request.requested_model,
            request.want_thinking,
            carried_usage.web_search_requests,
            carried_usage.web_fetch_requests,
            carried_usage.code_execution_requests,
            carried_usage.tool_search_requests,
            Some(&request.server_tools),
        ),
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn runtime_anthropic_sse_response_parts_from_responses_sse_bytes(
    body: &[u8],
    requested_model: &str,
    want_thinking: bool,
    carried_web_search_requests: u64,
    carried_web_fetch_requests: u64,
    carried_code_execution_requests: u64,
    carried_tool_search_requests: u64,
    server_tools: &RuntimeAnthropicServerTools,
) -> Result<RuntimeBufferedResponseParts> {
    let response = runtime_anthropic_response_from_sse_bytes_with_carried_usage(
        body,
        requested_model,
        want_thinking,
        carried_web_search_requests,
        carried_web_fetch_requests,
        carried_code_execution_requests,
        carried_tool_search_requests,
        Some(server_tools),
    )
    .context("failed to translate buffered Responses SSE body")?;
    Ok(runtime_anthropic_sse_response_parts_from_message_value(
        response,
    ))
}

pub(crate) fn buffer_runtime_streaming_response_parts(
    response: RuntimeStreamingResponse,
) -> Result<RuntimeBufferedResponseParts> {
    let RuntimeStreamingResponse {
        status,
        headers,
        mut body,
        ..
    } = response;
    let mut buffered_body = Vec::new();
    body.read_to_end(&mut buffered_body)
        .context("failed to buffer streaming runtime response")?;
    Ok(RuntimeBufferedResponseParts {
        status,
        headers: headers
            .into_iter()
            .map(|(name, value)| (name, value.into_bytes()))
            .collect(),
        body: buffered_body,
    })
}

#[derive(Debug, Clone, Default)]
pub(crate) struct RuntimeAnthropicStreamToolUse {
    call_id: String,
    name: String,
    arguments: String,
    saw_delta: bool,
    server_tool_name: Option<String>,
    server_tool_block_type: Option<String>,
}

pub(crate) struct RuntimeAnthropicSseReader {
    inner: Box<dyn Read + Send>,
    pending: VecDeque<u8>,
    upstream_line: Vec<u8>,
    upstream_data_lines: Vec<String>,
    message_id: String,
    model: String,
    want_thinking: bool,
    content_index: usize,
    thinking_block_open: bool,
    text_block_open: bool,
    has_tool_calls: bool,
    has_content: bool,
    input_tokens: u64,
    output_tokens: u64,
    cached_tokens: Option<u64>,
    web_search_requests: u64,
    web_fetch_requests: u64,
    code_execution_requests: u64,
    tool_search_requests: u64,
    server_tools: RuntimeAnthropicServerTools,
    active_tool_use: Option<RuntimeAnthropicStreamToolUse>,
    terminal_sent: bool,
    inner_finished: bool,
}

impl RuntimeAnthropicSseReader {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        inner: Box<dyn Read + Send>,
        model: String,
        want_thinking: bool,
        carried_web_search_requests: u64,
        carried_web_fetch_requests: u64,
        carried_code_execution_requests: u64,
        carried_tool_search_requests: u64,
        server_tools: RuntimeAnthropicServerTools,
    ) -> Self {
        let mut reader = Self {
            inner,
            pending: VecDeque::new(),
            upstream_line: Vec::new(),
            upstream_data_lines: Vec::new(),
            message_id: runtime_anthropic_message_id(),
            model,
            want_thinking,
            content_index: 0,
            thinking_block_open: false,
            text_block_open: false,
            has_tool_calls: false,
            has_content: false,
            input_tokens: 0,
            output_tokens: 0,
            cached_tokens: None,
            web_search_requests: carried_web_search_requests,
            web_fetch_requests: carried_web_fetch_requests,
            code_execution_requests: carried_code_execution_requests,
            tool_search_requests: carried_tool_search_requests,
            server_tools,
            active_tool_use: None,
            terminal_sent: false,
            inner_finished: false,
        };
        reader.push_event(
            "message_start",
            serde_json::json!({
                "type": "message_start",
                "message": {
                    "id": reader.message_id.clone(),
                    "type": "message",
                    "role": "assistant",
                    "content": [],
                    "model": reader.model.clone(),
                    "stop_reason": serde_json::Value::Null,
                    "stop_sequence": serde_json::Value::Null,
                    "usage": {
                        "input_tokens": 0,
                        "output_tokens": 0,
                        "server_tool_use": {
                            "web_search_requests": carried_web_search_requests,
                            "web_fetch_requests": carried_web_fetch_requests,
                            "code_execution_requests": carried_code_execution_requests,
                            "tool_search_requests": carried_tool_search_requests,
                        }
                    }
                }
            }),
        );
        reader
    }

    fn push_event(&mut self, event_type: &str, data: serde_json::Value) {
        let frame = format!(
            "event: {event_type}\ndata: {}\n\n",
            serde_json::to_string(&data).unwrap_or_else(|_| "{}".to_string())
        );
        self.pending.extend(frame.into_bytes());
    }

    fn close_thinking_block(&mut self) {
        if !self.thinking_block_open {
            return;
        }
        self.push_event(
            "content_block_stop",
            serde_json::json!({
                "type": "content_block_stop",
                "index": self.content_index,
            }),
        );
        self.content_index += 1;
        self.thinking_block_open = false;
    }

    fn close_text_block(&mut self) {
        if !self.text_block_open {
            return;
        }
        self.push_event(
            "content_block_stop",
            serde_json::json!({
                "type": "content_block_stop",
                "index": self.content_index,
            }),
        );
        self.content_index += 1;
        self.text_block_open = false;
    }

    fn ensure_text_block(&mut self) {
        if self.text_block_open {
            return;
        }
        self.push_event(
            "content_block_start",
            serde_json::json!({
                "type": "content_block_start",
                "index": self.content_index,
                "content_block": {
                    "type": "text",
                    "text": "",
                }
            }),
        );
        self.text_block_open = true;
    }

    fn ensure_thinking_block(&mut self) {
        if self.thinking_block_open {
            return;
        }
        self.push_event(
            "content_block_start",
            serde_json::json!({
                "type": "content_block_start",
                "index": self.content_index,
                "content_block": {
                    "type": "thinking",
                    "thinking": "",
                }
            }),
        );
        self.thinking_block_open = true;
    }

    fn start_tool_use_block(&mut self, call_id: &str, name: &str) {
        let server_tool_registration =
            runtime_anthropic_server_tool_registration_for_call(name, Some(&self.server_tools));
        let block_type = server_tool_registration
            .as_ref()
            .map(|(_, block_type)| block_type.as_str())
            .unwrap_or("tool_use");
        let output_name = server_tool_registration
            .as_ref()
            .map(|(server_tool_name, _)| server_tool_name.as_str())
            .unwrap_or(name);
        let mut content_block = serde_json::Map::new();
        content_block.insert(
            "type".to_string(),
            serde_json::Value::String(block_type.to_string()),
        );
        content_block.insert(
            "id".to_string(),
            serde_json::Value::String(call_id.to_string()),
        );
        content_block.insert(
            "name".to_string(),
            serde_json::Value::String(output_name.to_string()),
        );
        content_block.insert("input".to_string(), serde_json::json!({}));
        self.close_thinking_block();
        self.close_text_block();
        self.push_event(
            "content_block_start",
            serde_json::json!({
                "type": "content_block_start",
                "index": self.content_index,
                "content_block": serde_json::Value::Object(content_block)
            }),
        );
        self.active_tool_use = Some(RuntimeAnthropicStreamToolUse {
            call_id: call_id.to_string(),
            name: name.to_string(),
            server_tool_name: server_tool_registration
                .as_ref()
                .map(|(server_tool_name, _)| server_tool_name.clone()),
            server_tool_block_type: server_tool_registration.map(|(_, block_type)| block_type),
            ..RuntimeAnthropicStreamToolUse::default()
        });
        self.has_content = true;
        self.has_tool_calls = true;
    }

    fn finish_active_tool_use(
        &mut self,
        arguments_override: Option<&str>,
        name_override: Option<&str>,
        call_id_override: Option<&str>,
    ) {
        let Some(mut active_tool_use) = self.active_tool_use.take() else {
            return;
        };
        if let Some(name) = name_override {
            active_tool_use.name = name.to_string();
            if let Some((server_tool_name, block_type)) =
                runtime_anthropic_server_tool_registration_for_call(name, Some(&self.server_tools))
            {
                active_tool_use.server_tool_name = Some(server_tool_name);
                active_tool_use.server_tool_block_type = Some(block_type);
            } else {
                active_tool_use.server_tool_name = None;
                active_tool_use.server_tool_block_type = None;
            }
        }
        if let Some(call_id) = call_id_override {
            active_tool_use.call_id = call_id.to_string();
        }
        if let Some(arguments) = arguments_override
            && !active_tool_use.saw_delta
        {
            active_tool_use.arguments = arguments.to_string();
        }
        if !active_tool_use.saw_delta && !active_tool_use.arguments.is_empty() {
            self.push_event(
                "content_block_delta",
                serde_json::json!({
                    "type": "content_block_delta",
                    "index": self.content_index,
                    "delta": {
                        "type": "input_json_delta",
                        "partial_json": active_tool_use.arguments,
                    }
                }),
            );
        }
        self.push_event(
            "content_block_stop",
            serde_json::json!({
                "type": "content_block_stop",
                "index": self.content_index,
            }),
        );
        self.content_index += 1;
    }

    fn emit_completed_content_block(&mut self, block: serde_json::Value, has_tool_calls: bool) {
        let Some(block_type) = block.get("type").and_then(serde_json::Value::as_str) else {
            return;
        };

        self.finish_active_tool_use(None, None, None);
        self.close_thinking_block();
        self.close_text_block();

        match block_type {
            "mcp_tool_use" => {
                let input_json = block
                    .get("input")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));
                let mut content_block = serde_json::Map::new();
                content_block.insert(
                    "type".to_string(),
                    serde_json::Value::String("mcp_tool_use".to_string()),
                );
                content_block.insert(
                    "id".to_string(),
                    block
                        .get("id")
                        .cloned()
                        .unwrap_or_else(|| serde_json::Value::String("mcp_tool_use".to_string())),
                );
                content_block.insert(
                    "name".to_string(),
                    block
                        .get("name")
                        .cloned()
                        .unwrap_or_else(|| serde_json::Value::String("mcp_tool".to_string())),
                );
                content_block.insert(
                    "server_name".to_string(),
                    block
                        .get("server_name")
                        .cloned()
                        .unwrap_or_else(|| serde_json::Value::String("mcp".to_string())),
                );
                content_block.insert("input".to_string(), serde_json::json!({}));
                self.push_event(
                    "content_block_start",
                    serde_json::json!({
                        "type": "content_block_start",
                        "index": self.content_index,
                        "content_block": serde_json::Value::Object(content_block),
                    }),
                );
                self.push_event(
                    "content_block_delta",
                    serde_json::json!({
                        "type": "content_block_delta",
                        "index": self.content_index,
                        "delta": {
                            "type": "input_json_delta",
                            "partial_json": serde_json::to_string(&input_json)
                                .unwrap_or_else(|_| "{}".to_string()),
                        }
                    }),
                );
            }
            block_type if block_type.ends_with("_tool_result") => {
                let mut content_block = serde_json::Map::new();
                content_block.insert(
                    "type".to_string(),
                    serde_json::Value::String(block_type.to_string()),
                );
                content_block.insert(
                    "tool_use_id".to_string(),
                    block
                        .get("tool_use_id")
                        .cloned()
                        .unwrap_or_else(|| serde_json::Value::String(format!("{block_type}_call"))),
                );
                content_block.insert(
                    "content".to_string(),
                    block
                        .get("content")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null),
                );
                if let Some(is_error) = block.get("is_error").cloned() {
                    content_block.insert("is_error".to_string(), is_error);
                }
                self.push_event(
                    "content_block_start",
                    serde_json::json!({
                        "type": "content_block_start",
                        "index": self.content_index,
                        "content_block": serde_json::Value::Object(content_block),
                    }),
                );
            }
            "mcp_approval_request" | "mcp_list_tools" => {
                self.push_event(
                    "content_block_start",
                    serde_json::json!({
                        "type": "content_block_start",
                        "index": self.content_index,
                        "content_block": block,
                    }),
                );
            }
            _ => return,
        }

        self.push_event(
            "content_block_stop",
            serde_json::json!({
                "type": "content_block_stop",
                "index": self.content_index,
            }),
        );
        self.content_index += 1;
        self.has_content = true;
        self.has_tool_calls |= has_tool_calls;
    }

    fn emit_mcp_call_blocks(&mut self, item: &serde_json::Value) {
        for block in runtime_anthropic_mcp_call_blocks_from_output_item(item) {
            self.emit_completed_content_block(block, false);
        }
    }

    fn finish_success(&mut self) {
        if self.terminal_sent {
            return;
        }
        self.finish_active_tool_use(None, None, None);
        self.close_thinking_block();
        self.close_text_block();
        if !self.has_content {
            self.ensure_text_block();
            self.close_text_block();
        }
        let usage = runtime_anthropic_usage_json(
            self.input_tokens,
            self.output_tokens,
            self.cached_tokens,
            self.web_search_requests,
            self.web_fetch_requests,
            self.code_execution_requests,
            self.tool_search_requests,
        );
        self.push_event(
            "message_delta",
            serde_json::json!({
                "type": "message_delta",
                "delta": {
                    "stop_reason": if self.has_tool_calls { "tool_use" } else { "end_turn" },
                },
                "usage": usage,
            }),
        );
        self.push_event(
            "message_stop",
            serde_json::json!({
                "type": "message_stop",
            }),
        );
        self.terminal_sent = true;
        self.inner_finished = true;
    }

    fn finish_error(&mut self, message: &str) {
        if self.terminal_sent {
            return;
        }
        self.finish_active_tool_use(None, None, None);
        self.close_thinking_block();
        self.close_text_block();
        self.ensure_text_block();
        self.push_event(
            "content_block_delta",
            serde_json::json!({
                "type": "content_block_delta",
                "index": self.content_index,
                "delta": {
                    "type": "text_delta",
                    "text": format!("[Error] {message}"),
                }
            }),
        );
        self.has_content = true;
        self.close_text_block();
        self.push_event(
            "error",
            serde_json::json!({
                "type": "error",
                "error": {
                    "type": "api_error",
                    "message": message,
                }
            }),
        );
        self.push_event(
            "message_stop",
            serde_json::json!({
                "type": "message_stop",
            }),
        );
        self.terminal_sent = true;
        self.inner_finished = true;
    }

    fn observe_upstream_event(&mut self, value: &serde_json::Value) {
        match value.get("type").and_then(serde_json::Value::as_str) {
            Some("response.reasoning_summary_text.delta") if self.want_thinking => {
                self.observe_stream_reasoning_delta(value);
            }
            Some("response.output_text.delta") => {
                self.observe_stream_text_delta(value);
            }
            Some("response.output_item.added") => {
                self.observe_stream_output_item_added(value);
            }
            Some("response.function_call_arguments.delta") => {
                self.observe_stream_function_call_arguments_delta(value);
            }
            Some("response.function_call_arguments.done") => {
                self.observe_stream_function_call_arguments_done(value);
            }
            Some("response.output_item.done") => {
                self.observe_stream_output_item_done(value);
            }
            Some("response.completed") => {
                self.observe_stream_completed(value);
            }
            Some("error" | "response.failed") => {
                self.finish_error(runtime_anthropic_response_event_error_message(value));
            }
            _ => {}
        }
    }

    fn observe_stream_reasoning_delta(&mut self, value: &serde_json::Value) {
        self.close_text_block();
        self.ensure_thinking_block();
        if let Some(delta) = value.get("delta").and_then(serde_json::Value::as_str) {
            self.push_event(
                "content_block_delta",
                serde_json::json!({
                    "type": "content_block_delta",
                    "index": self.content_index,
                    "delta": {
                        "type": "thinking_delta",
                        "thinking": delta,
                    }
                }),
            );
            self.has_content = true;
        }
    }

    fn observe_stream_text_delta(&mut self, value: &serde_json::Value) {
        self.close_thinking_block();
        self.ensure_text_block();
        if let Some(delta) = value.get("delta").and_then(serde_json::Value::as_str) {
            self.push_event(
                "content_block_delta",
                serde_json::json!({
                    "type": "content_block_delta",
                    "index": self.content_index,
                    "delta": {
                        "type": "text_delta",
                        "text": delta,
                    }
                }),
            );
            self.has_content = true;
        }
    }

    fn observe_stream_output_item_added(&mut self, value: &serde_json::Value) {
        let Some(item) = runtime_anthropic_response_event_item(value) else {
            return;
        };
        match runtime_anthropic_output_item_type(item) {
            Some("function_call") => {
                self.start_tool_use_block(
                    runtime_anthropic_output_item_call_id(item, "tool_call"),
                    runtime_anthropic_output_item_name(item, "tool"),
                );
            }
            Some("shell_call") => {
                self.start_tool_use_block(
                    runtime_anthropic_output_item_call_id(item, "shell_call"),
                    "bash",
                );
            }
            Some("computer_call") => {
                self.start_tool_use_block(
                    runtime_anthropic_output_item_call_id(item, "computer_call"),
                    "computer",
                );
            }
            _ => {}
        }
    }

    fn observe_stream_function_call_arguments_delta(&mut self, value: &serde_json::Value) {
        let Some(delta) = value.get("delta").and_then(serde_json::Value::as_str) else {
            return;
        };
        if let Some(active_tool_use) = self.active_tool_use.as_mut() {
            active_tool_use.saw_delta = true;
            active_tool_use.arguments.push_str(delta);
        }
        self.push_event(
            "content_block_delta",
            serde_json::json!({
                "type": "content_block_delta",
                "index": self.content_index,
                "delta": {
                    "type": "input_json_delta",
                    "partial_json": delta,
                }
            }),
        );
    }

    fn observe_stream_function_call_arguments_done(&mut self, value: &serde_json::Value) {
        if let Some(active_tool_use) = self.active_tool_use.as_mut()
            && let Some(arguments) = value.get("arguments").and_then(serde_json::Value::as_str)
            && !active_tool_use.saw_delta
        {
            active_tool_use.arguments = arguments.to_string();
        }
    }

    fn observe_stream_output_item_done(&mut self, value: &serde_json::Value) {
        let Some(item) = runtime_anthropic_response_event_item(value) else {
            return;
        };
        match runtime_anthropic_output_item_type(item) {
            Some("function_call") => self.observe_stream_function_call_item_done(item),
            Some("web_search_call") => self.observe_stream_web_search_item_done(item),
            Some("mcp_call") => self.emit_mcp_call_blocks(item),
            Some("shell_call") => self.observe_stream_shell_call_item_done(item),
            Some("computer_call") => self.observe_stream_computer_call_item_done(item),
            Some("mcp_approval_request") => self.emit_completed_content_block(
                runtime_anthropic_mcp_approval_request_block_from_output_item(item),
                true,
            ),
            Some("mcp_list_tools") => self.emit_completed_content_block(
                runtime_anthropic_mcp_list_tools_block_from_output_item(item),
                false,
            ),
            _ => {}
        }
    }

    fn observe_stream_function_call_item_done(&mut self, item: &serde_json::Value) {
        self.add_stream_output_item_server_tool_usage(item);
        if self.active_tool_use.is_none() {
            self.start_tool_use_block(
                runtime_anthropic_output_item_call_id(item, "tool_call"),
                runtime_anthropic_output_item_name(item, "tool"),
            );
        }
        self.finish_active_tool_use(
            item.get("arguments").and_then(serde_json::Value::as_str),
            item.get("name").and_then(serde_json::Value::as_str),
            item.get("call_id").and_then(serde_json::Value::as_str),
        );
    }

    fn observe_stream_web_search_item_done(&mut self, item: &serde_json::Value) {
        self.web_search_requests +=
            runtime_anthropic_web_search_request_count_from_output_item(item);
    }

    fn observe_stream_shell_call_item_done(&mut self, item: &serde_json::Value) {
        if self.active_tool_use.is_none() {
            self.start_tool_use_block(
                runtime_anthropic_output_item_call_id(item, "shell_call"),
                "bash",
            );
        }
        let arguments =
            serde_json::to_string(&runtime_anthropic_shell_tool_input_from_output_item(item)).ok();
        self.finish_active_tool_use(
            arguments.as_deref(),
            Some("bash"),
            item.get("call_id").and_then(serde_json::Value::as_str),
        );
    }

    fn observe_stream_computer_call_item_done(&mut self, item: &serde_json::Value) {
        if self.active_tool_use.is_none() {
            self.start_tool_use_block(
                runtime_anthropic_output_item_call_id(item, "computer_call"),
                "computer",
            );
        }
        let input = runtime_anthropic_computer_tool_input_from_output_item(item)
            .unwrap_or_else(|| runtime_anthropic_raw_computer_tool_input_from_output_item(item));
        let arguments = serde_json::to_string(&input).ok();
        self.finish_active_tool_use(
            arguments.as_deref(),
            Some("computer"),
            item.get("call_id").and_then(serde_json::Value::as_str),
        );
    }

    fn observe_stream_completed(&mut self, value: &serde_json::Value) {
        let (input_tokens, output_tokens, cached_tokens) =
            runtime_anthropic_usage_from_value(value);
        self.input_tokens = input_tokens;
        self.output_tokens = output_tokens;
        self.cached_tokens = cached_tokens;
        self.web_search_requests = self.web_search_requests.max(
            runtime_anthropic_tool_usage_web_search_requests_from_value(value),
        );
        self.code_execution_requests = self
            .code_execution_requests
            .max(runtime_anthropic_tool_usage_code_execution_requests_from_value(value));
        self.tool_search_requests = self
            .tool_search_requests
            .max(runtime_anthropic_tool_usage_tool_search_requests_from_value(value));
        if let Some(output) = value
            .get("response")
            .and_then(|response| response.get("output"))
            .and_then(serde_json::Value::as_array)
        {
            self.merge_stream_output_usage(output);
        }
        self.finish_success();
    }

    fn add_stream_output_item_server_tool_usage(&mut self, item: &serde_json::Value) {
        let usage = runtime_anthropic_output_item_server_tool_usage(item, Some(&self.server_tools));
        self.web_search_requests += usage.web_search_requests;
        self.web_fetch_requests += usage.web_fetch_requests;
        self.code_execution_requests += usage.code_execution_requests;
        self.tool_search_requests += usage.tool_search_requests;
    }

    fn merge_stream_output_usage(&mut self, output: &[serde_json::Value]) {
        self.web_search_requests =
            self.web_search_requests
                .max(runtime_anthropic_web_search_request_count_from_output(
                    output,
                    Some(&self.server_tools),
                ));
        self.web_fetch_requests =
            self.web_fetch_requests
                .max(runtime_anthropic_web_fetch_request_count_from_output(
                    output,
                    Some(&self.server_tools),
                ));
        self.code_execution_requests = self.code_execution_requests.max(
            runtime_anthropic_code_execution_request_count_from_output(
                output,
                Some(&self.server_tools),
            ),
        );
        self.tool_search_requests =
            self.tool_search_requests
                .max(runtime_anthropic_tool_search_request_count_from_output(
                    output,
                    Some(&self.server_tools),
                ));
    }

    fn process_upstream_event(&mut self) -> io::Result<()> {
        if self.upstream_data_lines.is_empty() {
            return Ok(());
        }
        let payload = self.upstream_data_lines.join("\n");
        self.upstream_data_lines.clear();
        let value = serde_json::from_str::<serde_json::Value>(&payload).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("failed to parse runtime Responses SSE payload: {err}"),
            )
        })?;
        self.observe_upstream_event(&value);
        Ok(())
    }

    fn observe_upstream_bytes(&mut self, chunk: &[u8]) -> io::Result<()> {
        for byte in chunk {
            self.upstream_line.push(*byte);
            if *byte != b'\n' {
                continue;
            }
            let line_text = String::from_utf8_lossy(&self.upstream_line);
            let trimmed = line_text.trim_end_matches(['\r', '\n']);
            if trimmed.is_empty() {
                self.process_upstream_event()?;
                self.upstream_line.clear();
                if self.inner_finished {
                    break;
                }
                continue;
            }
            if let Some(payload) = trimmed.strip_prefix("data:") {
                self.upstream_data_lines
                    .push(payload.trim_start().to_string());
            }
            self.upstream_line.clear();
        }
        Ok(())
    }
}

impl Read for RuntimeAnthropicSseReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            let read = buf.len().min(self.pending.len());
            if read > 0 {
                for (index, byte) in self.pending.drain(..read).enumerate() {
                    buf[index] = byte;
                }
                return Ok(read);
            }
            if self.inner_finished {
                return Ok(0);
            }

            let mut upstream_buffer = [0_u8; 8192];
            let read = self.inner.read(&mut upstream_buffer)?;
            if read == 0 {
                self.finish_success();
                continue;
            }
            self.observe_upstream_bytes(&upstream_buffer[..read])?;
        }
    }
}

pub(crate) fn translate_runtime_buffered_responses_reply_to_anthropic(
    parts: RuntimeBufferedResponseParts,
    request: &RuntimeAnthropicMessagesRequest,
) -> Result<RuntimeResponsesReply> {
    if parts.status >= 400 {
        return Ok(RuntimeResponsesReply::Buffered(
            runtime_anthropic_error_from_upstream_parts(parts),
        ));
    }

    let content_type = runtime_buffered_response_content_type(&parts)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let looks_like_sse = content_type.contains("text/event-stream")
        || runtime_response_body_looks_like_sse(&parts.body);
    if request.stream && looks_like_sse {
        return Ok(RuntimeResponsesReply::Buffered(
            runtime_anthropic_sse_response_parts_from_responses_sse_bytes(
                &parts.body,
                &request.requested_model,
                request.want_thinking,
                request.carried_web_search_requests,
                request.carried_web_fetch_requests,
                request.carried_code_execution_requests,
                request.carried_tool_search_requests,
                &request.server_tools,
            )?,
        ));
    }

    let response = if looks_like_sse {
        runtime_anthropic_response_from_sse_bytes_with_carried_usage(
            &parts.body,
            &request.requested_model,
            request.want_thinking,
            request.carried_web_search_requests,
            request.carried_web_fetch_requests,
            request.carried_code_execution_requests,
            request.carried_tool_search_requests,
            Some(&request.server_tools),
        )?
    } else {
        let value = serde_json::from_slice::<serde_json::Value>(&parts.body)
            .context("failed to parse buffered Responses JSON body")?;
        if value.get("error").is_some() {
            return Ok(RuntimeResponsesReply::Buffered(
                runtime_anthropic_error_from_upstream_parts(parts),
            ));
        }
        runtime_anthropic_response_from_json_value_with_carried_usage(
            &value,
            &request.requested_model,
            request.want_thinking,
            request.carried_web_search_requests,
            request.carried_web_fetch_requests,
            request.carried_code_execution_requests,
            request.carried_tool_search_requests,
            Some(&request.server_tools),
        )
    };

    if request.stream {
        return Ok(RuntimeResponsesReply::Buffered(
            runtime_anthropic_sse_response_parts_from_message_value(response),
        ));
    }

    Ok(RuntimeResponsesReply::Buffered(
        runtime_anthropic_json_response_parts(response),
    ))
}

pub(crate) fn translate_runtime_responses_reply_to_anthropic(
    response: RuntimeResponsesReply,
    request: &RuntimeAnthropicMessagesRequest,
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
) -> Result<RuntimeResponsesReply> {
    if request.server_tools.needs_buffered_translation() {
        let mut parts = match response {
            RuntimeResponsesReply::Buffered(parts) => parts,
            RuntimeResponsesReply::Streaming(response) => {
                buffer_runtime_streaming_response_parts(response)?
            }
        };
        let mut carried_usage = RuntimeAnthropicServerToolUsage {
            web_search_requests: request.carried_web_search_requests,
            web_fetch_requests: request.carried_web_fetch_requests,
            code_execution_requests: request.carried_code_execution_requests,
            tool_search_requests: request.carried_tool_search_requests,
        };

        for followup_attempt in 0..=RUNTIME_PROXY_ANTHROPIC_WEB_SEARCH_FOLLOWUP_LIMIT {
            if std::env::var_os("PRODEX_DEBUG_ANTHROPIC_COMPAT").is_some() {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http anthropic_translated_upstream status={} content_type={:?} followup_attempt={} body_snippet={}",
                        parts.status,
                        runtime_buffered_response_content_type(&parts),
                        followup_attempt,
                        runtime_proxy_body_snippet(&parts.body, 2048),
                    ),
                );
            }

            if parts.status >= 400 {
                return Ok(RuntimeResponsesReply::Buffered(
                    runtime_anthropic_error_from_upstream_parts(parts),
                ));
            }

            if !runtime_response_body_looks_like_sse(&parts.body)
                && !runtime_buffered_response_content_type(&parts)
                    .unwrap_or_default()
                    .to_ascii_lowercase()
                    .contains("text/event-stream")
                && serde_json::from_slice::<serde_json::Value>(&parts.body)
                    .ok()
                    .is_some_and(|value| value.get("error").is_some())
            {
                return Ok(RuntimeResponsesReply::Buffered(
                    runtime_anthropic_error_from_upstream_parts(parts),
                ));
            }

            let response_message =
                runtime_anthropic_message_from_buffered_responses_parts_with_carried_usage(
                    &parts,
                    request,
                    carried_usage,
                )?;
            carried_usage = runtime_anthropic_message_server_tool_usage(&response_message);

            if followup_attempt == RUNTIME_PROXY_ANTHROPIC_WEB_SEARCH_FOLLOWUP_LIMIT
                || !runtime_anthropic_message_needs_server_tool_followup(&response_message)
            {
                if request.stream {
                    return Ok(RuntimeResponsesReply::Buffered(
                        runtime_anthropic_sse_response_parts_from_message_value(response_message),
                    ));
                }

                return Ok(RuntimeResponsesReply::Buffered(
                    runtime_anthropic_json_response_parts(response_message),
                ));
            }

            let Some(previous_response_id) = runtime_buffered_response_ids(&parts).last().cloned()
            else {
                if request.stream {
                    return Ok(RuntimeResponsesReply::Buffered(
                        runtime_anthropic_sse_response_parts_from_message_value(response_message),
                    ));
                }
                return Ok(RuntimeResponsesReply::Buffered(
                    runtime_anthropic_json_response_parts(response_message),
                ));
            };

            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http anthropic_server_tool_followup previous_response_id={previous_response_id} attempt={}",
                    followup_attempt + 1,
                ),
            );
            let followup_request = runtime_request_for_anthropic_server_tool_followup(
                &request.translated_request,
                &previous_response_id,
            )?;
            parts = match proxy_runtime_responses_request(request_id, &followup_request, shared)? {
                RuntimeResponsesReply::Buffered(parts) => parts,
                RuntimeResponsesReply::Streaming(response) => {
                    buffer_runtime_streaming_response_parts(response)?
                }
            };
        }

        unreachable!("anthropic buffered server-tool translation should return inside loop");
    }

    match response {
        RuntimeResponsesReply::Buffered(parts) => {
            translate_runtime_buffered_responses_reply_to_anthropic(parts, request)
        }
        RuntimeResponsesReply::Streaming(response) => {
            if !request.stream {
                let parts = buffer_runtime_streaming_response_parts(response)?;
                return translate_runtime_buffered_responses_reply_to_anthropic(parts, request);
            }

            let mut headers = response.headers;
            headers.retain(|(name, _)| !name.eq_ignore_ascii_case("content-type"));
            headers.push(("Content-Type".to_string(), "text/event-stream".to_string()));
            Ok(RuntimeResponsesReply::Streaming(RuntimeStreamingResponse {
                status: response.status,
                headers,
                body: Box::new(RuntimeAnthropicSseReader::new(
                    response.body,
                    request.requested_model.clone(),
                    request.want_thinking,
                    request.carried_web_search_requests,
                    request.carried_web_fetch_requests,
                    request.carried_code_execution_requests,
                    request.carried_tool_search_requests,
                    request.server_tools.clone(),
                )),
                request_id: response.request_id,
                profile_name: response.profile_name,
                log_path: response.log_path,
                shared: response.shared,
                _inflight_guard: response._inflight_guard,
            }))
        }
    }
}
