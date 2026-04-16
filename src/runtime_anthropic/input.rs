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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeAnthropicNativeClientToolKind {
    Shell,
    Computer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeAnthropicNativeClientToolCall {
    kind: RuntimeAnthropicNativeClientToolKind,
    max_output_length: Option<u64>,
}

pub(crate) fn runtime_proxy_translate_anthropic_shell_tool_call(
    block: &serde_json::Value,
) -> Option<(
    String,
    serde_json::Value,
    RuntimeAnthropicNativeClientToolCall,
)> {
    let name = block
        .get("name")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    if runtime_proxy_anthropic_client_tool_name(name) != Some("bash") {
        return None;
    }
    let call_id = block
        .get("id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?
        .to_string();
    let input = block.get("input")?.as_object()?;
    if input
        .get("restart")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        return None;
    }
    let command = input
        .get("command")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let mut action = serde_json::Map::new();
    action.insert(
        "commands".to_string(),
        serde_json::json!([command.to_string()]),
    );
    if let Some(timeout) = input.get("timeout_ms").and_then(serde_json::Value::as_u64) {
        action.insert(
            "timeout_ms".to_string(),
            serde_json::Value::Number(timeout.into()),
        );
    }
    if let Some(max_output_length) = input
        .get("max_output_length")
        .and_then(serde_json::Value::as_u64)
    {
        action.insert(
            "max_output_length".to_string(),
            serde_json::Value::Number(max_output_length.into()),
        );
    }
    Some((
        call_id.clone(),
        serde_json::json!({
            "type": "shell_call",
            "call_id": call_id,
            "action": serde_json::Value::Object(action),
            "status": "completed",
        }),
        RuntimeAnthropicNativeClientToolCall {
            kind: RuntimeAnthropicNativeClientToolKind::Shell,
            max_output_length: input
                .get("max_output_length")
                .and_then(serde_json::Value::as_u64),
        },
    ))
}

pub(crate) fn runtime_proxy_anthropic_coordinate_component(
    value: &serde_json::Value,
) -> Option<i64> {
    value.as_i64().or_else(|| {
        value
            .as_u64()
            .and_then(|component| i64::try_from(component).ok())
    })
}

pub(crate) fn runtime_proxy_anthropic_coordinate_pair(
    value: Option<&serde_json::Value>,
) -> Option<(i64, i64)> {
    let coordinates = value?.as_array()?;
    if coordinates.len() < 2 {
        return None;
    }
    let x = runtime_proxy_anthropic_coordinate_component(coordinates.first()?)?;
    let y = runtime_proxy_anthropic_coordinate_component(coordinates.get(1)?)?;
    Some((x, y))
}

pub(crate) fn runtime_proxy_anthropic_computer_keypress_keys(
    key_combo: &str,
) -> Option<Vec<String>> {
    let keys = key_combo
        .split('+')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_uppercase())
        .collect::<Vec<_>>();
    (!keys.is_empty()).then_some(keys)
}

pub(crate) fn runtime_proxy_translate_anthropic_computer_action(
    input: &serde_json::Map<String, serde_json::Value>,
) -> Option<serde_json::Value> {
    let action = input
        .get("action")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    match action {
        "screenshot" => Some(serde_json::json!({ "type": "screenshot" })),
        "left_click" | "right_click" | "middle_click" => {
            let (x, y) = runtime_proxy_anthropic_coordinate_pair(input.get("coordinate"))?;
            let button = match action {
                "left_click" => "left",
                "right_click" => "right",
                "middle_click" => "middle",
                _ => unreachable!(),
            };
            Some(serde_json::json!({
                "type": "click",
                "button": button,
                "x": x,
                "y": y,
            }))
        }
        "double_click" => {
            let (x, y) = runtime_proxy_anthropic_coordinate_pair(input.get("coordinate"))?;
            Some(serde_json::json!({
                "type": "double_click",
                "x": x,
                "y": y,
            }))
        }
        "mouse_move" => {
            let (x, y) = runtime_proxy_anthropic_coordinate_pair(input.get("coordinate"))?;
            Some(serde_json::json!({
                "type": "move",
                "x": x,
                "y": y,
            }))
        }
        "type" => input
            .get("text")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|text| {
                serde_json::json!({
                    "type": "type",
                    "text": text,
                })
            }),
        "key" => input
            .get("key")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .and_then(runtime_proxy_anthropic_computer_keypress_keys)
            .map(|keys| {
                serde_json::json!({
                    "type": "keypress",
                    "keys": keys,
                })
            }),
        "wait" => Some(serde_json::json!({ "type": "wait" })),
        _ => None,
    }
}

pub(crate) fn runtime_proxy_translate_anthropic_computer_tool_call(
    block: &serde_json::Value,
) -> Option<(
    String,
    serde_json::Value,
    RuntimeAnthropicNativeClientToolCall,
)> {
    let name = block
        .get("name")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    if runtime_proxy_anthropic_client_tool_name(name) != Some("computer") {
        return None;
    }
    let call_id = block
        .get("id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?
        .to_string();
    let input = block.get("input")?.as_object()?;
    let action = runtime_proxy_translate_anthropic_computer_action(input)?;
    Some((
        call_id.clone(),
        serde_json::json!({
            "type": "computer_call",
            "call_id": call_id,
            "actions": [action],
            "status": "completed",
        }),
        RuntimeAnthropicNativeClientToolCall {
            kind: RuntimeAnthropicNativeClientToolKind::Computer,
            max_output_length: None,
        },
    ))
}

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

pub(crate) fn translate_runtime_anthropic_messages_request(
    request: &RuntimeProxyRequest,
) -> Result<RuntimeAnthropicMessagesRequest> {
    let value = serde_json::from_slice::<serde_json::Value>(&request.body)
        .context("failed to parse Anthropic request body as JSON")?;
    let requested_model = value
        .get("model")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .context("Anthropic request requires a non-empty model")?
        .to_string();
    let messages = value
        .get("messages")
        .and_then(serde_json::Value::as_array)
        .context("Anthropic request requires a messages array")?;
    if messages.is_empty() {
        bail!("Anthropic request requires at least one message");
    }
    let carried_server_tool_usage = runtime_proxy_anthropic_carried_server_tool_usage(messages);
    let target_model = runtime_proxy_claude_target_model(&requested_model);
    let native_shell_enabled = runtime_proxy_anthropic_native_shell_enabled_for_request(&value);
    let native_computer_enabled =
        runtime_proxy_anthropic_native_computer_enabled_for_request(&value, &target_model);

    let mut input = Vec::new();
    let mut native_client_tool_calls = BTreeMap::new();
    for message in messages {
        let role = message
            .get("role")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .context("Anthropic message is missing a non-empty role")?;
        let content = message
            .get("content")
            .context("Anthropic message is missing content")?;
        input.extend(runtime_proxy_translate_anthropic_message_content(
            role,
            content,
            native_shell_enabled,
            native_computer_enabled,
            &mut native_client_tool_calls,
        )?);
    }
    if input.is_empty() {
        input.push(serde_json::json!({
            "role": "user",
            "content": "",
        }));
    }

    let mut translated_body = serde_json::Map::new();
    translated_body.insert(
        "model".to_string(),
        serde_json::Value::String(target_model.clone()),
    );
    translated_body.insert("input".to_string(), serde_json::Value::Array(input));
    translated_body.insert("stream".to_string(), serde_json::Value::Bool(true));
    translated_body.insert("store".to_string(), serde_json::Value::Bool(false));
    let base_instructions = runtime_proxy_anthropic_system_instructions(&value)?;
    let translated_tools = runtime_proxy_translate_anthropic_tools(
        &value,
        native_shell_enabled,
        native_computer_enabled,
    )?;
    let mut translated_tools = translated_tools;
    runtime_proxy_anthropic_register_server_tools_from_messages(
        messages,
        &mut translated_tools.server_tools,
    );
    if let Some(instructions) =
        runtime_proxy_anthropic_append_tool_instructions(base_instructions, translated_tools.memory)
    {
        translated_body.insert(
            "instructions".to_string(),
            serde_json::Value::String(instructions),
        );
    }
    if !translated_tools.tools.is_empty() {
        translated_body.insert(
            "tools".to_string(),
            serde_json::Value::Array(translated_tools.tools.clone()),
        );
    }
    if translated_tools.server_tools.web_search {
        translated_body.insert(
            "include".to_string(),
            serde_json::json!(["web_search_call.action.sources"]),
        );
    }
    if let Some(tool_choice) = runtime_proxy_translate_anthropic_tool_choice(
        &value,
        &translated_tools.server_tools,
        &translated_tools.tool_name_aliases,
        &translated_tools.native_tool_names,
    )?
    .or_else(|| translated_tools.implicit_tool_choice())
    {
        translated_body.insert("tool_choice".to_string(), tool_choice);
    }
    if let Some(effort) = runtime_proxy_anthropic_reasoning_effort(&value, &target_model) {
        translated_body.insert(
            "reasoning".to_string(),
            serde_json::json!({
                "summary": "auto",
                "effort": effort,
            }),
        );
    } else if runtime_proxy_anthropic_wants_thinking(&value) {
        translated_body.insert(
            "reasoning".to_string(),
            serde_json::json!({
                "summary": "auto",
            }),
        );
    }

    let mut translated_headers = vec![("Content-Type".to_string(), "application/json".to_string())];
    if let Some(user_agent) = runtime_proxy_request_header_value(&request.headers, "User-Agent") {
        translated_headers.push(("User-Agent".to_string(), user_agent.to_string()));
    }
    if let Some(session_id) = runtime_proxy_claude_session_id(request) {
        translated_headers.push(("session_id".to_string(), session_id));
    }
    translated_headers.push((
        PRODEX_INTERNAL_REQUEST_ORIGIN_HEADER.to_string(),
        PRODEX_INTERNAL_REQUEST_ORIGIN_ANTHROPIC_MESSAGES.to_string(),
    ));

    Ok(RuntimeAnthropicMessagesRequest {
        translated_request: RuntimeProxyRequest {
            method: request.method.clone(),
            path_and_query: format!("{RUNTIME_PROXY_OPENAI_UPSTREAM_PATH}/responses"),
            headers: translated_headers,
            body: serde_json::to_vec(&serde_json::Value::Object(translated_body))
                .context("failed to serialize translated Anthropic request")?,
        },
        requested_model,
        stream: value
            .get("stream")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        want_thinking: runtime_proxy_anthropic_wants_thinking(&value),
        server_tools: translated_tools.server_tools,
        carried_web_search_requests: carried_server_tool_usage.web_search_requests,
        carried_web_fetch_requests: carried_server_tool_usage.web_fetch_requests,
        carried_code_execution_requests: carried_server_tool_usage.code_execution_requests,
        carried_tool_search_requests: carried_server_tool_usage.tool_search_requests,
    })
}
