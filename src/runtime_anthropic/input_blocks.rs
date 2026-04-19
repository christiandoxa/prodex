use super::*;

pub(crate) fn runtime_proxy_anthropic_image_data_url(block: &serde_json::Value) -> Option<String> {
    let source = block.get("source")?;
    if source.get("type").and_then(serde_json::Value::as_str) != Some("base64") {
        return None;
    }
    let media_type = source
        .get("media_type")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let data = source
        .get("data")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    Some(format!("data:{media_type};base64,{data}"))
}

pub(crate) fn runtime_proxy_translate_anthropic_image_part(
    block: &serde_json::Value,
) -> Option<serde_json::Value> {
    let image_url = runtime_proxy_anthropic_image_data_url(block)?;
    Some(serde_json::json!({
        "type": "input_image",
        "image_url": image_url,
    }))
}

pub(crate) fn runtime_proxy_translate_anthropic_document_text(
    block: &serde_json::Value,
) -> Option<String> {
    let source = block.get("source")?;
    match source.get("type").and_then(serde_json::Value::as_str) {
        Some("text") => source
            .get("text")
            .or_else(|| source.get("data"))
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        Some("base64") => {
            let media_type = source
                .get("media_type")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .unwrap_or_default()
                .to_ascii_lowercase();
            if !(media_type.starts_with("text/") || media_type == "application/json") {
                return None;
            }
            let data = source
                .get("data")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())?;
            let decoded = base64::engine::general_purpose::STANDARD
                .decode(data.as_bytes())
                .ok()?;
            String::from_utf8(decoded)
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        }
        _ => None,
    }
}

pub(crate) fn runtime_proxy_translate_anthropic_text_from_block(
    block: &serde_json::Value,
) -> Option<String> {
    match block.get("type").and_then(serde_json::Value::as_str) {
        Some("text") => block
            .get("text")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        Some("document") => runtime_proxy_translate_anthropic_document_text(block),
        Some("web_fetch_result") => block
            .get("content")
            .and_then(runtime_proxy_translate_anthropic_text_from_block),
        Some("code_execution_result" | "bash_code_execution_result") => {
            let stdout = block
                .get("stdout")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty());
            let stderr = block
                .get("stderr")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty());
            match (stdout, stderr) {
                (Some(stdout), Some(stderr)) => Some(format!("{stdout}\n{stderr}")),
                (Some(stdout), None) => Some(stdout.to_string()),
                (None, Some(stderr)) => Some(stderr.to_string()),
                (None, None) => None,
            }
        }
        Some("text_editor_code_execution_result") => block
            .get("content")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .or_else(|| {
                block
                    .get("lines")
                    .and_then(serde_json::Value::as_array)
                    .map(|lines| {
                        lines
                            .iter()
                            .filter_map(serde_json::Value::as_str)
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                            .collect::<Vec<_>>()
                            .join("\n")
                    })
            })
            .filter(|value| !value.is_empty()),
        Some(result_type) if result_type.ends_with("_tool_result_error") => block
            .get("error_code")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| format!("Error: {value}")),
        _ => None,
    }
}

pub(crate) fn runtime_proxy_translate_anthropic_block_fallback_text(
    block: &serde_json::Value,
) -> Option<String> {
    let block_type = block
        .get("type")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    if block_type == "image"
        || runtime_proxy_anthropic_is_tool_use_block_type(block_type)
        || runtime_proxy_anthropic_is_tool_result_block_type(block_type)
    {
        return None;
    }
    let serialized = serde_json::to_string(block).ok()?;
    Some(format!("[anthropic:{block_type}] {serialized}"))
}

pub(crate) fn runtime_proxy_translate_anthropic_mcp_approval_response(
    block: &serde_json::Value,
) -> Result<serde_json::Value> {
    let approval_request_id = block
        .get("approval_request_id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .context(
            "Anthropic mcp_approval_response block requires a non-empty approval_request_id",
        )?;
    let approve = block
        .get("approve")
        .and_then(serde_json::Value::as_bool)
        .context("Anthropic mcp_approval_response block requires approve=true/false")?;

    let mut translated = serde_json::Map::new();
    translated.insert(
        "type".to_string(),
        serde_json::Value::String("mcp_approval_response".to_string()),
    );
    translated.insert(
        "approval_request_id".to_string(),
        serde_json::Value::String(approval_request_id.to_string()),
    );
    translated.insert("approve".to_string(), serde_json::Value::Bool(approve));
    if let Some(id) = block
        .get("id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        translated.insert("id".to_string(), serde_json::Value::String(id.to_string()));
    }
    if let Some(reason) = block
        .get("reason")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        translated.insert(
            "reason".to_string(),
            serde_json::Value::String(reason.to_string()),
        );
    }
    Ok(serde_json::Value::Object(translated))
}

pub(crate) fn runtime_proxy_translate_anthropic_message_content(
    role: &str,
    content: &serde_json::Value,
    native_shell_enabled: bool,
    native_computer_enabled: bool,
    native_client_tool_calls: &mut BTreeMap<String, RuntimeAnthropicNativeClientToolCall>,
) -> Result<Vec<serde_json::Value>> {
    if let Some(text) = content.as_str() {
        return Ok(vec![serde_json::json!({
            "role": role,
            "content": text,
        })]);
    }

    let blocks = content
        .as_array()
        .context("Anthropic message content must be a string or an array of content blocks")?;

    RuntimeAnthropicInputContentTranslator {
        role,
        blocks,
        native_shell_enabled,
        native_computer_enabled,
        native_client_tool_calls,
        input_items: Vec::new(),
    }
    .translate()
}

struct RuntimeAnthropicInputContentTranslator<'a> {
    role: &'a str,
    blocks: &'a [serde_json::Value],
    native_shell_enabled: bool,
    native_computer_enabled: bool,
    native_client_tool_calls: &'a mut BTreeMap<String, RuntimeAnthropicNativeClientToolCall>,
    input_items: Vec<serde_json::Value>,
}

impl RuntimeAnthropicInputContentTranslator<'_> {
    fn translate(mut self) -> Result<Vec<serde_json::Value>> {
        self.push_role_content()?;
        self.push_special_input_items()?;
        Ok(self.input_items)
    }

    fn has_tool_blocks(&self) -> bool {
        self.blocks.iter().any(|block| {
            block
                .get("type")
                .and_then(serde_json::Value::as_str)
                .is_some_and(runtime_proxy_anthropic_is_special_input_item_block_type)
        })
    }

    fn push_role_content(&mut self) -> Result<()> {
        let has_tool_blocks = self.has_tool_blocks();
        match self.role {
            "user" => {
                if let Some(message_content) =
                    runtime_proxy_translate_anthropic_user_content_blocks(self.blocks)
                {
                    self.input_items.push(serde_json::json!({
                        "role": "user",
                        "content": message_content,
                    }));
                } else if !has_tool_blocks {
                    self.input_items.push(serde_json::json!({
                        "role": "user",
                        "content": "",
                    }));
                }
            }
            "assistant" => {
                let text = runtime_proxy_translate_anthropic_text_blocks(self.blocks);
                if !text.is_empty() || !has_tool_blocks {
                    self.input_items.push(serde_json::json!({
                        "role": "assistant",
                        "content": text,
                    }));
                }
            }
            other => bail!("Unsupported Anthropic role '{other}'"),
        }
        Ok(())
    }

    fn push_special_input_items(&mut self) -> Result<()> {
        for block in self.blocks {
            self.push_special_input_item(block)?;
        }
        Ok(())
    }

    fn push_special_input_item(&mut self, block: &serde_json::Value) -> Result<()> {
        match block.get("type").and_then(serde_json::Value::as_str) {
            Some(block_type) if runtime_proxy_anthropic_is_tool_use_block_type(block_type) => {
                self.push_tool_use_block(block)?;
            }
            Some(block_type) if runtime_proxy_anthropic_is_tool_result_block_type(block_type) => {
                self.push_tool_result_block(block)?;
            }
            Some("mcp_approval_response") => {
                self.input_items
                    .push(runtime_proxy_translate_anthropic_mcp_approval_response(
                        block,
                    )?);
            }
            _ => {}
        }
        Ok(())
    }

    fn push_tool_use_block(&mut self, block: &serde_json::Value) -> Result<()> {
        if self.native_shell_enabled
            && let Some((call_id, translated, native_shell_call)) =
                runtime_proxy_translate_anthropic_shell_tool_call(block)
        {
            self.native_client_tool_calls
                .insert(call_id, native_shell_call);
            self.input_items.push(translated);
            return Ok(());
        }

        if self.native_computer_enabled
            && let Some((call_id, translated, native_computer_call)) =
                runtime_proxy_translate_anthropic_computer_tool_call(block)
        {
            self.native_client_tool_calls
                .insert(call_id, native_computer_call);
            self.input_items.push(translated);
            return Ok(());
        }

        self.input_items
            .push(runtime_proxy_translate_anthropic_tool_call(block)?);
        Ok(())
    }

    fn push_tool_result_block(&mut self, block: &serde_json::Value) -> Result<()> {
        let native_client_tool_call = block
            .get("tool_use_id")
            .and_then(serde_json::Value::as_str)
            .and_then(|call_id| self.native_client_tool_calls.get(call_id))
            .copied();
        match native_client_tool_call.map(|call| call.kind) {
            Some(RuntimeAnthropicNativeClientToolKind::Shell) if self.native_shell_enabled => {
                self.input_items
                    .extend(runtime_proxy_translate_anthropic_shell_tool_result(
                        block,
                        native_client_tool_call.and_then(|call| call.max_output_length),
                    )?);
            }
            Some(RuntimeAnthropicNativeClientToolKind::Computer)
                if self.native_computer_enabled =>
            {
                if let Some(translated) =
                    runtime_proxy_translate_anthropic_computer_tool_result(block)?
                {
                    self.input_items.extend(translated);
                } else {
                    self.input_items
                        .extend(runtime_proxy_translate_anthropic_tool_result(block)?);
                }
            }
            _ => {
                self.input_items
                    .extend(runtime_proxy_translate_anthropic_tool_result(block)?);
            }
        }
        Ok(())
    }
}

pub(crate) fn runtime_proxy_translate_anthropic_user_content_blocks(
    blocks: &[serde_json::Value],
) -> Option<serde_json::Value> {
    let mut text_blocks = Vec::new();
    let mut parts = Vec::new();
    let mut saw_image = false;

    for block in blocks {
        if block.get("type").and_then(serde_json::Value::as_str) == Some("mcp_approval_response") {
            continue;
        }
        if block.get("type").and_then(serde_json::Value::as_str) == Some("image") {
            if !saw_image {
                for text in text_blocks.drain(..) {
                    parts.push(serde_json::json!({
                        "type": "input_text",
                        "text": text,
                    }));
                }
            }
            saw_image = true;
            if let Some(part) = runtime_proxy_translate_anthropic_image_part(block) {
                parts.push(part);
            }
            continue;
        }

        if let Some(text) = runtime_proxy_translate_anthropic_text_from_block(block) {
            if saw_image {
                parts.push(serde_json::json!({
                    "type": "input_text",
                    "text": text,
                }));
            } else {
                text_blocks.push(text);
            }
        } else if let Some(text) = runtime_proxy_translate_anthropic_block_fallback_text(block) {
            if saw_image {
                parts.push(serde_json::json!({
                    "type": "input_text",
                    "text": text,
                }));
            } else {
                text_blocks.push(text);
            }
        }
    }

    if saw_image {
        (!parts.is_empty()).then_some(serde_json::Value::Array(parts))
    } else {
        let text = text_blocks.join("\n");
        (!text.is_empty()).then_some(serde_json::Value::String(text))
    }
}

pub(crate) fn runtime_proxy_translate_anthropic_text_blocks(
    blocks: &[serde_json::Value],
) -> String {
    blocks
        .iter()
        .filter_map(runtime_proxy_translate_anthropic_text_from_block)
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn runtime_proxy_translate_anthropic_tool_call(
    block: &serde_json::Value,
) -> Result<serde_json::Value> {
    let block_type = block
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("tool_use");
    let name = block
        .get("name")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            if block_type == "server_tool_use" {
                runtime_proxy_anthropic_builtin_server_tool_name(value).unwrap_or(value)
            } else {
                value
            }
        })
        .with_context(|| format!("Anthropic {block_type} block requires a non-empty name"))?;
    let call_id = block
        .get("id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .with_context(|| format!("Anthropic {block_type} block requires a non-empty id"))?;
    let mut input = block
        .get("input")
        .cloned()
        .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()));
    if block_type == "mcp_tool_use"
        && let Some(server_name) = block
            .get("server_name")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        && let Some(object) = input.as_object_mut()
    {
        object
            .entry("server_name".to_string())
            .or_insert_with(|| serde_json::Value::String(server_name.to_string()));
    }
    let arguments = serde_json::to_string(&input)
        .with_context(|| format!("failed to serialize Anthropic {block_type} input"))?;
    Ok(serde_json::json!({
        "type": "function_call",
        "call_id": call_id,
        "name": name,
        "arguments": arguments,
    }))
}
