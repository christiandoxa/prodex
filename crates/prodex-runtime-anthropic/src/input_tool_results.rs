use super::*;

#[cfg(any(not(feature = "mojo"), test))]
#[path = "input_tool_results/usage_fallback.rs"]
mod usage_fallback;

#[cfg(any(not(feature = "mojo"), test))]
use usage_fallback::*;

#[cfg(all(test, feature = "mojo"))]
#[path = "input_tool_results_tests.rs"]
mod tests;

pub fn runtime_proxy_translate_anthropic_tool_result_payload(
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

pub fn runtime_proxy_translate_anthropic_error_tool_result_output(output_text: String) -> String {
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

#[cfg(feature = "mojo")]
pub fn runtime_proxy_translate_anthropic_shell_tool_result(
    block: &serde_json::Value,
    max_output_length: Option<u64>,
) -> Result<Vec<serde_json::Value>> {
    let (call_id, output_text, image_parts) =
        runtime_proxy_translate_anthropic_tool_result_payload(block)?;
    let image_parts_json = if image_parts.is_empty() {
        None
    } else {
        Some(serde_json::to_string(&image_parts).context("failed to serialize Anthropic images")?)
    };
    let mut input = prodex_mojo_core::rich::RuntimeAnthropicKernelInput::new(
        prodex_mojo_core::rich::RuntimeAnthropicKernelOperation::ShellToolResult,
    );
    input.id = Some(&call_id);
    input.text = Some(&output_text);
    input.content = image_parts_json.as_deref();
    if block
        .get("is_error")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        input.flags |= prodex_mojo_core::rich::RUNTIME_ANTHROPIC_FLAG_ERROR;
    }
    if let Some(max_output_length) = max_output_length {
        input.max_output_length = max_output_length;
        input.flags |= prodex_mojo_core::rich::RUNTIME_ANTHROPIC_FLAG_MAX_OUTPUT_LENGTH;
    }
    Ok(crate::mojo::json(input)
        .as_array()
        .cloned()
        .expect("Anthropic shell result output should be an array"))
}

#[cfg(not(feature = "mojo"))]
pub fn runtime_proxy_translate_anthropic_shell_tool_result(
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

#[cfg(feature = "mojo")]
pub fn runtime_proxy_translate_anthropic_computer_tool_result(
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
        .context("Anthropic tool_result block requires a non-empty tool_use_id")?;
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
    let mut input = prodex_mojo_core::rich::RuntimeAnthropicKernelInput::new(
        prodex_mojo_core::rich::RuntimeAnthropicKernelOperation::ComputerToolResult,
    );
    input.id = Some(call_id);
    input.text = Some(&image_url);
    Ok(Some(crate::mojo::json(input).as_array().cloned().expect(
        "Anthropic computer result output should be an array",
    )))
}

#[cfg(not(feature = "mojo"))]
pub fn runtime_proxy_translate_anthropic_computer_tool_result(
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

pub fn runtime_proxy_translate_anthropic_tool_result_content(
    items: &[serde_json::Value],
) -> (String, Vec<serde_json::Value>) {
    let mut text_parts = Vec::new();
    let mut tool_references = Vec::new();
    let mut structured_blocks = Vec::new();
    let mut image_parts = Vec::new();
    let mut content_blocks = Vec::new();

    for item in items {
        runtime_proxy_collect_anthropic_tool_result_item(
            item,
            &mut text_parts,
            &mut tool_references,
            &mut structured_blocks,
            &mut image_parts,
            &mut content_blocks,
        );
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

fn runtime_proxy_collect_anthropic_tool_result_item(
    item: &serde_json::Value,
    text_parts: &mut Vec<String>,
    tool_references: &mut Vec<String>,
    structured_blocks: &mut Vec<serde_json::Value>,
    image_parts: &mut Vec<serde_json::Value>,
    content_blocks: &mut Vec<serde_json::Value>,
) {
    let item_type = item.get("type").and_then(serde_json::Value::as_str);
    match item_type {
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
            let Some(text) = runtime_proxy_translate_anthropic_text_from_block(item) else {
                structured_blocks.push(item.clone());
                content_blocks.push(item.clone());
                return;
            };
            text_parts.push(text);
            content_blocks.push(item.clone());
            if matches!(item_type, Some("document" | "web_fetch_result")) {
                structured_blocks.push(item.clone());
            }
        }
    }
}

pub fn runtime_proxy_extract_balanced_json_array_bounds(
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

pub fn runtime_proxy_anthropic_web_search_query_from_tool_result_text(
    text: &str,
) -> Option<String> {
    runtime_proxy_anthropic_tool_result_text_plan(text, 0).map(|(query, _)| query)
}

#[cfg(any(not(feature = "mojo"), test))]
fn runtime_proxy_anthropic_web_search_query_from_tool_result_text_rust(
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

pub fn runtime_proxy_anthropic_web_search_urls_from_tool_result_text(
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

pub fn runtime_proxy_compact_web_search_tool_result_summary(summary: &str) -> String {
    #[cfg(feature = "mojo")]
    return runtime_proxy_anthropic_mojo_tool_result_text_plan(summary, 0, true)
        .map(|(_, summary)| summary)
        .unwrap_or_default();
    #[cfg(not(feature = "mojo"))]
    runtime_proxy_compact_web_search_tool_result_summary_rust(summary)
}

#[cfg(any(not(feature = "mojo"), test))]
fn runtime_proxy_compact_web_search_tool_result_summary_rust(summary: &str) -> String {
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

pub fn runtime_proxy_normalize_anthropic_tool_result_text(text: &str) -> Option<String> {
    let (urls, last_array_end) =
        runtime_proxy_anthropic_web_search_urls_from_tool_result_text(text);
    let (query, summary) = runtime_proxy_anthropic_tool_result_text_plan(text, last_array_end)?;
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

#[cfg(feature = "mojo")]
fn runtime_proxy_anthropic_tool_result_text_plan(
    text: &str,
    summary_start: usize,
) -> Option<(String, String)> {
    runtime_proxy_anthropic_mojo_tool_result_text_plan(text, summary_start, false)
}

#[cfg(feature = "mojo")]
fn runtime_proxy_anthropic_mojo_tool_result_text_plan(
    text: &str,
    summary_start: usize,
    compact_only: bool,
) -> Option<(String, String)> {
    let mut input = prodex_mojo_core::rich::RuntimeAnthropicKernelInput::new(
        prodex_mojo_core::rich::RuntimeAnthropicKernelOperation::ToolResultTextPlan,
    );
    input.text = Some(text);
    input.index = u64::try_from(summary_start).ok()?;
    input.flags = i64::from(compact_only);
    let value = crate::mojo::json(input);
    let object = value.as_object()?;
    Some((
        object.get("query")?.as_str()?.to_string(),
        object.get("summary")?.as_str()?.to_string(),
    ))
}

#[cfg(not(feature = "mojo"))]
fn runtime_proxy_anthropic_tool_result_text_plan(
    text: &str,
    summary_start: usize,
) -> Option<(String, String)> {
    runtime_proxy_anthropic_tool_result_text_plan_rust(text, summary_start)
}

#[cfg(any(not(feature = "mojo"), test))]
fn runtime_proxy_anthropic_tool_result_text_plan_rust(
    text: &str,
    summary_start: usize,
) -> Option<(String, String)> {
    let query = runtime_proxy_anthropic_web_search_query_from_tool_result_text_rust(text)?;
    let summary_source = if summary_start > 0 {
        text.get(summary_start..)?
    } else {
        text
    };
    Some((
        query,
        runtime_proxy_compact_web_search_tool_result_summary_rust(summary_source),
    ))
}

#[cfg(feature = "mojo")]
pub fn runtime_proxy_translate_anthropic_tool_result(
    block: &serde_json::Value,
) -> Result<Vec<serde_json::Value>> {
    let (call_id, output_text, image_parts) =
        runtime_proxy_translate_anthropic_tool_result_payload(block)?;
    let image_parts_json = if image_parts.is_empty() {
        None
    } else {
        Some(serde_json::to_string(&image_parts).context("failed to serialize Anthropic images")?)
    };
    let mut input = prodex_mojo_core::rich::RuntimeAnthropicKernelInput::new(
        prodex_mojo_core::rich::RuntimeAnthropicKernelOperation::FunctionCallOutput,
    );
    input.id = Some(&call_id);
    input.text = Some(&output_text);
    input.content = image_parts_json.as_deref();
    Ok(crate::mojo::json(input)
        .as_array()
        .cloned()
        .expect("Anthropic tool result output should be an array"))
}

#[cfg(not(feature = "mojo"))]
pub fn runtime_proxy_translate_anthropic_tool_result(
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

#[cfg(feature = "mojo")]
pub fn runtime_proxy_anthropic_tool_use_server_tool_usage(
    block: &serde_json::Value,
) -> RuntimeAnthropicServerToolUsage {
    runtime_proxy_anthropic_server_tool_usage_mojo(
        prodex_mojo_core::rich::RuntimeAnthropicKernelOperation::ServerToolUsage,
        &serde_json::to_string(block).expect("Anthropic tool-use block serializes"),
    )
}

#[cfg(not(feature = "mojo"))]
pub fn runtime_proxy_anthropic_tool_use_server_tool_usage(
    block: &serde_json::Value,
) -> RuntimeAnthropicServerToolUsage {
    runtime_proxy_anthropic_tool_use_server_tool_usage_rust(block)
}

#[cfg(feature = "mojo")]
pub fn runtime_proxy_anthropic_register_server_tools_from_messages(
    messages: &[serde_json::Value],
    server_tools: &mut RuntimeAnthropicServerTools,
) {
    let message = serde_json::to_string(messages).expect("Anthropic messages serialize");
    let mut input = prodex_mojo_core::rich::RuntimeAnthropicKernelInput::new(
        prodex_mojo_core::rich::RuntimeAnthropicKernelOperation::ServerToolRegistrations,
    );
    input.message = Some(&message);
    let value = crate::mojo::json(input);
    for registration in value
        .as_array()
        .expect("Anthropic server-tool registrations should be an array")
    {
        let Some(tool_name) = registration["tool_name"].as_str() else {
            continue;
        };
        let Some(response_name) = registration["response_name"].as_str() else {
            continue;
        };
        let Some(block_type) = registration["block_type"].as_str() else {
            continue;
        };
        server_tools.register_with_block_type(tool_name, response_name, block_type);
    }
}

#[cfg(not(feature = "mojo"))]
pub fn runtime_proxy_anthropic_register_server_tools_from_messages(
    messages: &[serde_json::Value],
    server_tools: &mut RuntimeAnthropicServerTools,
) {
    runtime_proxy_anthropic_register_server_tools_from_messages_rust(messages, server_tools);
}

#[cfg(feature = "mojo")]
pub fn runtime_proxy_anthropic_message_has_tool_chain_blocks(message: &serde_json::Value) -> bool {
    let message = serde_json::to_string(message).expect("Anthropic message serializes");
    let mut input = prodex_mojo_core::rich::RuntimeAnthropicKernelInput::new(
        prodex_mojo_core::rich::RuntimeAnthropicKernelOperation::MessageHasToolChain,
    );
    input.message = Some(&message);
    crate::mojo::json(input)
        .as_bool()
        .expect("Anthropic tool-chain result should be a boolean")
}

#[cfg(not(feature = "mojo"))]
pub fn runtime_proxy_anthropic_message_has_tool_chain_blocks(message: &serde_json::Value) -> bool {
    runtime_proxy_anthropic_message_has_tool_chain_blocks_rust(message)
}

#[cfg(feature = "mojo")]
fn runtime_proxy_anthropic_server_tool_usage_mojo(
    operation: prodex_mojo_core::rich::RuntimeAnthropicKernelOperation,
    message: &str,
) -> RuntimeAnthropicServerToolUsage {
    let mut input = prodex_mojo_core::rich::RuntimeAnthropicKernelInput::new(operation);
    input.message = Some(message);
    let value = crate::mojo::json(input);
    RuntimeAnthropicServerToolUsage {
        web_search_requests: value["web_search_requests"].as_u64().unwrap_or_default(),
        web_fetch_requests: value["web_fetch_requests"].as_u64().unwrap_or_default(),
        code_execution_requests: value["code_execution_requests"]
            .as_u64()
            .unwrap_or_default(),
        tool_search_requests: value["tool_search_requests"].as_u64().unwrap_or_default(),
    }
}

#[cfg(feature = "mojo")]
pub fn runtime_proxy_anthropic_carried_server_tool_usage(
    messages: &[serde_json::Value],
) -> RuntimeAnthropicServerToolUsage {
    runtime_proxy_anthropic_server_tool_usage_mojo(
        prodex_mojo_core::rich::RuntimeAnthropicKernelOperation::CarriedServerToolUsage,
        &serde_json::to_string(messages).expect("Anthropic messages serialize"),
    )
}

#[cfg(not(feature = "mojo"))]
pub fn runtime_proxy_anthropic_carried_server_tool_usage(
    messages: &[serde_json::Value],
) -> RuntimeAnthropicServerToolUsage {
    runtime_proxy_anthropic_carried_server_tool_usage_rust(messages)
}
