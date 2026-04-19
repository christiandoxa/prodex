use super::*;

pub(crate) fn runtime_proxy_translate_anthropic_tool_result_payload(
    block: &serde_json::Value,
) -> Result<(String, String, Vec<serde_json::Value>)> {
    let call_id = block
        .get("tool_use_id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .context("Anthropic tool_result block requires a non-empty tool_use_id")?
        .to_string();
    let mut output_text = String::new();
    let mut image_parts = Vec::new();

    match block.get("content") {
        Some(serde_json::Value::String(text)) => {
            output_text = runtime_proxy_normalize_anthropic_tool_result_text(text)
                .unwrap_or_else(|| text.clone());
        }
        Some(serde_json::Value::Array(items)) => {
            let (translated_output, translated_images) =
                runtime_proxy_translate_anthropic_tool_result_content(items);
            output_text = translated_output;
            image_parts.extend(translated_images);
        }
        Some(serde_json::Value::Object(object)) => {
            let mut normalized = object.clone();
            if let Some(text) = runtime_proxy_translate_anthropic_text_from_block(
                &serde_json::Value::Object(object.clone()),
            ) && !normalized.contains_key("text")
            {
                normalized.insert("text".to_string(), serde_json::Value::String(text));
            }
            output_text = serde_json::Value::Object(normalized).to_string();
        }
        Some(other) => {
            output_text = serde_json::to_string(other)
                .context("failed to serialize Anthropic tool_result content")?;
        }
        None => {}
    }

    if block
        .get("is_error")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        output_text = runtime_proxy_translate_anthropic_error_tool_result_output(output_text);
    }

    Ok((call_id, output_text, image_parts))
}

pub(crate) fn runtime_proxy_translate_anthropic_error_tool_result_output(
    output_text: String,
) -> String {
    if output_text.is_empty() {
        return "Error".to_string();
    }
    if let Ok(mut structured) = serde_json::from_str::<serde_json::Value>(&output_text)
        && let Some(object) = structured.as_object_mut()
    {
        object.insert("is_error".to_string(), serde_json::Value::Bool(true));
        return structured.to_string();
    }
    format!("Error: {output_text}")
}

pub(crate) fn runtime_proxy_translate_anthropic_shell_tool_result(
    block: &serde_json::Value,
    max_output_length: Option<u64>,
) -> Result<Vec<serde_json::Value>> {
    let (call_id, output_text, image_parts) =
        runtime_proxy_translate_anthropic_tool_result_payload(block)?;
    let is_error = block
        .get("is_error")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let command_output = serde_json::json!({
        "stdout": if is_error { "" } else { output_text.as_str() },
        "stderr": if is_error { output_text.as_str() } else { "" },
        "outcome": {
            "type": "exit",
            "exit_code": if is_error { 1 } else { 0 },
        },
    });
    let mut shell_output = serde_json::Map::new();
    shell_output.insert(
        "type".to_string(),
        serde_json::Value::String("shell_call_output".to_string()),
    );
    shell_output.insert("call_id".to_string(), serde_json::Value::String(call_id));
    shell_output.insert(
        "output".to_string(),
        serde_json::Value::Array(vec![command_output]),
    );
    if let Some(max_output_length) = max_output_length {
        shell_output.insert(
            "max_output_length".to_string(),
            serde_json::Value::Number(max_output_length.into()),
        );
    }
    let mut translated = vec![serde_json::Value::Object(shell_output)];
    if !image_parts.is_empty() {
        translated.push(serde_json::json!({
            "role": "user",
            "content": image_parts,
        }));
    }
    Ok(translated)
}

pub(crate) fn runtime_proxy_translate_anthropic_computer_tool_result(
    block: &serde_json::Value,
) -> Result<Option<Vec<serde_json::Value>>> {
    if block
        .get("is_error")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        return Ok(None);
    }
    let call_id = block
        .get("tool_use_id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .context("Anthropic tool_result block requires a non-empty tool_use_id")?
        .to_string();
    let image_url = match block.get("content") {
        Some(serde_json::Value::Array(items)) if items.len() == 1 => items
            .first()
            .and_then(runtime_proxy_anthropic_image_data_url),
        Some(serde_json::Value::Object(object)) => {
            runtime_proxy_anthropic_image_data_url(&serde_json::Value::Object(object.clone()))
        }
        _ => None,
    };
    let Some(image_url) = image_url else {
        return Ok(None);
    };
    Ok(Some(vec![serde_json::json!({
        "type": "computer_call_output",
        "call_id": call_id,
        "output": {
            "type": "computer_screenshot",
            "image_url": image_url,
            "detail": "original",
        },
    })]))
}

pub(crate) fn runtime_proxy_translate_anthropic_tool_result_content(
    items: &[serde_json::Value],
) -> (String, Vec<serde_json::Value>) {
    let mut text_parts = Vec::new();
    let mut tool_references = Vec::new();
    let mut structured_blocks = Vec::new();
    let mut image_parts = Vec::new();
    let mut content_blocks = Vec::new();

    for item in items {
        match item.get("type").and_then(serde_json::Value::as_str) {
            Some("tool_reference") => {
                if let Some(tool_name) = item
                    .get("tool_name")
                    .and_then(serde_json::Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    tool_references.push(tool_name.to_string());
                } else {
                    structured_blocks.push(item.clone());
                }
                content_blocks.push(item.clone());
            }
            Some("image") => {
                if let Some(part) = runtime_proxy_translate_anthropic_image_part(item) {
                    image_parts.push(part);
                }
            }
            _ => {
                if let Some(text) = runtime_proxy_translate_anthropic_text_from_block(item) {
                    text_parts.push(text);
                    content_blocks.push(item.clone());
                    if item.get("type").and_then(serde_json::Value::as_str) == Some("document")
                        || item.get("type").and_then(serde_json::Value::as_str)
                            == Some("web_fetch_result")
                    {
                        structured_blocks.push(item.clone());
                    }
                } else {
                    structured_blocks.push(item.clone());
                    content_blocks.push(item.clone());
                }
            }
        }
    }

    let text = text_parts.join("\n");
    if tool_references.is_empty() && structured_blocks.is_empty() {
        return (text, image_parts);
    }

    let mut output = serde_json::Map::new();
    if !text.is_empty() {
        output.insert("text".to_string(), serde_json::Value::String(text));
    }
    let has_tool_references = !tool_references.is_empty();
    if has_tool_references {
        output.insert(
            "tool_references".to_string(),
            serde_json::Value::Array(
                tool_references
                    .into_iter()
                    .map(serde_json::Value::String)
                    .collect(),
            ),
        );
    }
    if !content_blocks.is_empty() && (has_tool_references || !structured_blocks.is_empty()) {
        output.insert(
            "content_blocks".to_string(),
            serde_json::Value::Array(content_blocks),
        );
    }

    (serde_json::Value::Object(output).to_string(), image_parts)
}

pub(crate) fn runtime_proxy_extract_balanced_json_array_bounds(
    text: &str,
    start: usize,
) -> Option<(usize, usize)> {
    if text.as_bytes().get(start).copied() != Some(b'[') {
        return None;
    }

    let mut depth = 0usize;
    let mut in_string = false;
    let mut escape = false;

    for (offset, ch) in text[start..].char_indices() {
        if in_string {
            if escape {
                escape = false;
                continue;
            }
            match ch {
                '\\' => escape = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '[' => depth += 1,
            ']' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    let end = start + offset + ch.len_utf8();
                    return Some((start, end));
                }
            }
            _ => {}
        }
    }

    None
}

pub(crate) fn runtime_proxy_anthropic_web_search_query_from_tool_result_text(
    text: &str,
) -> Option<String> {
    let prefix = "Web search results for query:";
    let remainder = text.trim().strip_prefix(prefix)?.trim_start();
    let first_line = remainder.lines().next()?.trim();
    if first_line.is_empty() {
        return None;
    }
    if let Some(stripped) = first_line.strip_prefix('"')
        && let Some(end_quote) = stripped.find('"')
    {
        let query = stripped[..end_quote].trim();
        if !query.is_empty() {
            return Some(query.to_string());
        }
    }

    let query = first_line.trim_matches('"').trim();
    (!query.is_empty()).then(|| query.to_string())
}

pub(crate) fn runtime_proxy_anthropic_web_search_urls_from_tool_result_text(
    text: &str,
) -> (Vec<String>, usize) {
    let mut urls = Vec::new();
    let mut seen = BTreeSet::new();
    let mut search_from = 0usize;
    let mut last_array_end = 0usize;

    while let Some(links_offset) = text[search_from..].find("Links:") {
        let links_start = search_from + links_offset;
        let Some(array_offset) = text[links_start..].find('[') else {
            search_from = links_start.saturating_add("Links:".len());
            continue;
        };
        let array_start = links_start + array_offset;
        let Some((_, array_end)) =
            runtime_proxy_extract_balanced_json_array_bounds(text, array_start)
        else {
            break;
        };
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text[array_start..array_end])
            && let Some(items) = value.as_array()
        {
            for item in items {
                let Some(url) = item
                    .get("url")
                    .and_then(serde_json::Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                else {
                    continue;
                };
                if seen.insert(url.to_string()) {
                    urls.push(url.to_string());
                }
            }
        }
        last_array_end = array_end;
        search_from = array_end;
    }

    (urls, last_array_end)
}

pub(crate) fn runtime_proxy_compact_web_search_tool_result_summary(summary: &str) -> String {
    let mut compact_lines = Vec::new();
    let mut saw_content = false;

    for raw_line in summary.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            if saw_content
                && compact_lines
                    .last()
                    .is_some_and(|line: &String| !line.is_empty())
            {
                compact_lines.push(String::new());
            }
            continue;
        }
        if trimmed == "No links found." || trimmed.starts_with("Link:") {
            continue;
        }
        if trimmed == "Sources:"
            || trimmed.starts_with("REMINDER:")
            || trimmed.starts_with("Kalau mau, saya bisa lanjutkan")
            || trimmed.starts_with("If you'd like")
            || trimmed.starts_with("If you want,")
        {
            break;
        }
        compact_lines.push(trimmed.to_string());
        saw_content = true;
    }

    while compact_lines.last().is_some_and(|line| line.is_empty()) {
        compact_lines.pop();
    }

    compact_lines.join("\n")
}

pub(crate) fn runtime_proxy_normalize_anthropic_tool_result_text(text: &str) -> Option<String> {
    let query = runtime_proxy_anthropic_web_search_query_from_tool_result_text(text)?;
    let (urls, last_array_end) =
        runtime_proxy_anthropic_web_search_urls_from_tool_result_text(text);
    let summary_source = if last_array_end > 0 {
        &text[last_array_end..]
    } else {
        text
    };
    let summary = runtime_proxy_compact_web_search_tool_result_summary(summary_source);
    if urls.is_empty() && summary.is_empty() {
        return None;
    }

    let mut output = serde_json::Map::new();
    output.insert("query".to_string(), serde_json::Value::String(query));
    if !summary.is_empty() {
        output.insert("text".to_string(), serde_json::Value::String(summary));
    }
    if !urls.is_empty() {
        output.insert(
            "content_blocks".to_string(),
            serde_json::Value::Array(
                urls.into_iter()
                    .map(|url| {
                        serde_json::json!({
                            "type": "web_search_result",
                            "url": url,
                        })
                    })
                    .collect(),
            ),
        );
    }

    Some(serde_json::Value::Object(output).to_string())
}

pub(crate) fn runtime_proxy_translate_anthropic_tool_result(
    block: &serde_json::Value,
) -> Result<Vec<serde_json::Value>> {
    let (call_id, output_text, image_parts) =
        runtime_proxy_translate_anthropic_tool_result_payload(block)?;
    let mut translated = vec![serde_json::json!({
        "type": "function_call_output",
        "call_id": call_id,
        "output": output_text,
    })];
    if !image_parts.is_empty() {
        translated.push(serde_json::json!({
            "role": "user",
            "content": image_parts,
        }));
    }
    Ok(translated)
}

pub(crate) fn runtime_proxy_anthropic_tool_use_server_tool_usage(
    block: &serde_json::Value,
) -> RuntimeAnthropicServerToolUsage {
    let tool_name = match block.get("type").and_then(serde_json::Value::as_str) {
        Some(block_type) if runtime_proxy_anthropic_is_tool_use_block_type(block_type) => block
            .get("name")
            .and_then(serde_json::Value::as_str)
            .map(str::trim),
        _ => None,
    };
    match tool_name.and_then(runtime_proxy_anthropic_builtin_server_tool_name) {
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
    }
}

pub(crate) fn runtime_proxy_anthropic_register_server_tools_from_messages(
    messages: &[serde_json::Value],
    server_tools: &mut RuntimeAnthropicServerTools,
) {
    for message in messages {
        let Some(blocks) = message.get("content").and_then(serde_json::Value::as_array) else {
            continue;
        };
        for block in blocks {
            let block_type = block
                .get("type")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            if !matches!(block_type, "server_tool_use" | "mcp_tool_use") {
                continue;
            }
            let Some(tool_name) = block
                .get("name")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            let response_name =
                runtime_proxy_anthropic_builtin_server_tool_name(tool_name).unwrap_or(tool_name);
            server_tools.register_with_block_type(tool_name, response_name, block_type);
        }
    }
}

pub(crate) fn runtime_proxy_anthropic_message_has_tool_chain_blocks(
    message: &serde_json::Value,
) -> bool {
    let Some(content) = message.get("content") else {
        return false;
    };
    let Some(blocks) = content.as_array() else {
        return false;
    };
    blocks.iter().any(|block| {
        block
            .get("type")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|block_type| {
                runtime_proxy_anthropic_is_tool_use_block_type(block_type)
                    || runtime_proxy_anthropic_is_tool_result_block_type(block_type)
            })
    })
}

pub(crate) fn runtime_proxy_anthropic_carried_server_tool_usage(
    messages: &[serde_json::Value],
) -> RuntimeAnthropicServerToolUsage {
    let mut usage = RuntimeAnthropicServerToolUsage::default();
    let mut collecting_suffix = false;

    for message in messages.iter().rev() {
        let Some(blocks) = message.get("content").and_then(serde_json::Value::as_array) else {
            if collecting_suffix {
                break;
            }
            continue;
        };
        let mut saw_tool_chain_block = false;
        for block in blocks {
            let block_usage = runtime_proxy_anthropic_tool_use_server_tool_usage(block);
            if block_usage != RuntimeAnthropicServerToolUsage::default() {
                usage.add_assign(block_usage);
                saw_tool_chain_block = true;
                continue;
            }
            if block
                .get("type")
                .and_then(serde_json::Value::as_str)
                .is_some_and(runtime_proxy_anthropic_is_tool_result_block_type)
            {
                saw_tool_chain_block = true;
            }
        }
        if saw_tool_chain_block {
            collecting_suffix = true;
        } else if collecting_suffix
            || runtime_proxy_anthropic_message_has_tool_chain_blocks(message)
        {
            break;
        }
    }

    usage
}
