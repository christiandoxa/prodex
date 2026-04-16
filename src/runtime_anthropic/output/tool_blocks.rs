use super::*;

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
