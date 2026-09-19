//! DeepSeek response-format metadata and degraded JSON-mode notes.

#[cfg(feature = "mojo")]
struct MojoMetadataRequest<'a> {
    existing: serde_json::Map<String, serde_json::Value>,
    provider_label: &'a str,
    provider_key: &'a str,
    client_metadata: Option<&'a serde_json::Value>,
    prompt_cache_key: Option<&'a str>,
    prompt_cache_retention: Option<&'a str>,
    degraded_from: Option<&'a str>,
    tool_choice: Option<&'a serde_json::Value>,
    thinking_enabled: bool,
}

#[cfg(feature = "mojo")]
fn mojo_metadata_value(
    request: MojoMetadataRequest<'_>,
) -> Result<Option<serde_json::Value>, String> {
    let MojoMetadataRequest {
        existing,
        provider_label,
        provider_key,
        client_metadata,
        prompt_cache_key,
        prompt_cache_retention,
        degraded_from,
        tool_choice,
        thinking_enabled,
    } = request;
    let mut base = existing;
    let provider = base
        .remove(provider_key)
        .unwrap_or_else(|| serde_json::json!({}));
    let base = serde_json::to_string(&base)
        .map_err(|error| format!("{provider_label} metadata serialization failed: {error}"))?;
    let provider = serde_json::to_string(&provider)
        .map_err(|error| format!("{provider_label} metadata serialization failed: {error}"))?;
    let client_metadata = client_metadata
        .map(serde_json::to_string)
        .transpose()
        .map_err(|error| {
            format!("{provider_label} client_metadata serialization failed: {error}")
        })?;
    let tool_choice = tool_choice
        .map(serde_json::to_string)
        .transpose()
        .map_err(|error| format!("{provider_label} tool_choice serialization failed: {error}"))?;
    let degraded_reason = degraded_from.map(|_| {
        format!(
            "{provider_label} response_format supports json_object but not native JSON Schema enforcement"
        )
    });
    let omitted_reason = (thinking_enabled && tool_choice.is_some()).then(|| {
        format!(
            "{provider_label} thinking mode currently rejects explicit tool_choice on the OpenAI Chat route, so Prodex omits it while preserving translated function tools"
        )
    });
    let mut input =
        super::DeepSeekKernelInput::new(super::DeepSeekKernelOperation::RequestMetadata);
    input.extra = Some(&base);
    input.metadata = Some(&provider);
    input.item = client_metadata.as_deref();
    input.content = prompt_cache_key;
    input.reasoning_content = prompt_cache_retention;
    input.response = degraded_from;
    input.error_message = degraded_reason.as_deref();
    input.name = Some(provider_key);
    input.tool_choice = tool_choice.as_deref();
    input.arguments = omitted_reason.as_deref();
    input.stream = thinking_enabled;
    let mapped = super::deepseek_provider_core_mojo_value(input).map_err(|error| {
        format!("{provider_label} response metadata normalization failed: {error}")
    })?;
    let object = mapped.as_object().ok_or_else(|| {
        format!("{provider_label} response metadata normalization returned a non-object")
    })?;
    Ok((!object.is_empty()).then_some(mapped))
}

pub fn deepseek_provider_core_response_format_from_responses_request(
    value: &serde_json::Value,
    provider_label: &str,
) -> Result<Option<serde_json::Value>, String> {
    let response_format = value
        .get("response_format")
        .or_else(|| value.get("text").and_then(|text| text.get("format")));
    let Some(response_format) = response_format else {
        return Ok(None);
    };
    let format_type = response_format
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    match format_type {
        "json_object" | "json_schema" | "json" | "structured_output" => {
            #[cfg(feature = "mojo")]
            {
                let mut input =
                    super::DeepSeekKernelInput::new(super::DeepSeekKernelOperation::ResponseFormat);
                input.role = Some(format_type);
                super::deepseek_provider_core_mojo_value(input)
                    .map(Some)
                    .map_err(|error| {
                        format!("{provider_label} response_format could not be normalized: {error}")
                    })
            }
            #[cfg(not(feature = "mojo"))]
            Ok(Some(serde_json::json!({"type": "json_object"})))
        }
        "text" => Ok(None),
        "" => Err(format!(
            "{provider_label} response_format must include a type"
        )),
        other => Err(format!(
            "{provider_label} response_format type \x60{other}\x60 is not supported"
        )),
    }
}

pub fn deepseek_provider_core_response_metadata_from_responses_request(
    value: &serde_json::Value,
    provider_label: &str,
    provider_key: &str,
) -> Result<Option<serde_json::Value>, String> {
    let metadata = match value.get("metadata") {
        Some(metadata) => metadata
            .as_object()
            .cloned()
            .ok_or_else(|| format!("{provider_label} request metadata must be an object"))?,
        None => serde_json::Map::new(),
    };
    if metadata
        .get(provider_key)
        .is_some_and(|provider_metadata| !provider_metadata.is_object())
    {
        return Err(format!(
            "{provider_label} request metadata.{provider_key} must be an object"
        ));
    }
    let client_metadata = value.get("client_metadata");
    if client_metadata.is_some_and(|value| !value.is_object()) {
        return Err(format!(
            "{provider_label} client_metadata must be an object"
        ));
    }
    let prompt_cache_key = match value.get("prompt_cache_key") {
        Some(value) => Some(
            value
                .as_str()
                .ok_or_else(|| format!("{provider_label} prompt_cache_key must be a string"))?,
        ),
        None => None,
    };
    let prompt_cache_key = prompt_cache_key.filter(|value| !value.trim().is_empty());
    let prompt_cache_retention =
        match value.get("prompt_cache_retention") {
            Some(value) => Some(value.as_str().ok_or_else(|| {
                format!("{provider_label} prompt_cache_retention must be a string")
            })?),
            None => None,
        };
    let response_format = value
        .get("response_format")
        .or_else(|| value.get("text").and_then(|text| text.get("format")));
    let degraded_from = response_format
        .and_then(|response_format| response_format.get("type"))
        .and_then(serde_json::Value::as_str)
        .filter(|format_type| matches!(*format_type, "json_schema" | "structured_output"));

    #[cfg(feature = "mojo")]
    {
        mojo_metadata_value(MojoMetadataRequest {
            existing: metadata,
            provider_label,
            provider_key,
            client_metadata,
            prompt_cache_key,
            prompt_cache_retention,
            degraded_from,
            tool_choice: None,
            thinking_enabled: false,
        })
    }
    #[cfg(not(feature = "mojo"))]
    {
        let mut metadata = metadata;
        if let Some(client_metadata) = client_metadata {
            metadata.insert("client_metadata".to_string(), client_metadata.clone());
        }
        if let Some(prompt_cache_key) = prompt_cache_key {
            metadata.insert(
                "prompt_cache_key".to_string(),
                serde_json::Value::String(prompt_cache_key.to_string()),
            );
        }
        if let Some(prompt_cache_retention) = prompt_cache_retention {
            metadata.insert(
                "prompt_cache_retention".to_string(),
                serde_json::Value::String(prompt_cache_retention.to_string()),
            );
        }
        if let Some(format_type) = degraded_from {
            let provider_metadata = metadata
                .entry(provider_key.to_string())
                .or_insert_with(|| serde_json::json!({}))
                .as_object_mut()
                .ok_or_else(|| {
                    format!("{provider_label} request metadata.{provider_key} must be an object")
                })?;
            provider_metadata.insert(
                "degraded_response_format".to_string(),
                serde_json::json!({
                    "from": format_type,
                    "to": "json_object",
                    "reason": format!(
                        "{provider_label} response_format supports json_object but not native JSON Schema enforcement"
                    )
                }),
            );
        }
        Ok((!metadata.is_empty()).then_some(serde_json::Value::Object(metadata)))
    }
}

pub fn deepseek_provider_core_note_thinking_tool_choice_omission(
    value: &serde_json::Value,
    thinking_enabled: bool,
    provider_label: &str,
    provider_key: &str,
    response_metadata: &mut Option<serde_json::Value>,
) {
    if !thinking_enabled {
        return;
    }
    let Some(tool_choice) = value.get("tool_choice") else {
        return;
    };
    #[cfg(feature = "mojo")]
    {
        let existing = response_metadata
            .as_ref()
            .and_then(serde_json::Value::as_object)
            .cloned()
            .unwrap_or_default();
        *response_metadata = mojo_metadata_value(MojoMetadataRequest {
            existing,
            provider_label,
            provider_key,
            client_metadata: None,
            prompt_cache_key: None,
            prompt_cache_retention: None,
            degraded_from: None,
            tool_choice: Some(tool_choice),
            thinking_enabled: true,
        })
        .unwrap_or_else(|error| panic!("Mojo DeepSeek metadata omission failed: {error}"));
    }
    #[cfg(not(feature = "mojo"))]
    {
        let metadata = response_metadata
            .get_or_insert_with(|| serde_json::json!({}))
            .as_object_mut();
        let Some(metadata) = metadata else {
            return;
        };
        let provider_metadata = metadata
            .entry(provider_key.to_string())
            .or_insert_with(|| serde_json::json!({}))
            .as_object_mut();
        let Some(provider_metadata) = provider_metadata else {
            return;
        };
        provider_metadata.insert(
            "omitted_tool_choice".to_string(),
            serde_json::json!({
                "from": tool_choice,
                "reason": format!(
                    "{provider_label} thinking mode currently rejects explicit tool_choice on the OpenAI Chat route, so Prodex omits it while preserving translated function tools"
                )
            }),
        );
    }
}

pub fn deepseek_provider_core_ensure_json_prompt_instruction(
    messages: &mut Vec<serde_json::Value>,
) {
    if messages
        .iter()
        .any(deepseek_provider_core_message_has_json_guidance)
    {
        return;
    }
    messages.insert(
        0,
        serde_json::json!({
            "role": "system",
            "content": "Respond with valid JSON only.",
        }),
    );
}

fn deepseek_provider_core_message_has_json_guidance(message: &serde_json::Value) -> bool {
    if !matches!(
        message.get("role").and_then(serde_json::Value::as_str),
        Some("system" | "user")
    ) {
        return false;
    }
    message
        .get("content")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|content| content.to_ascii_lowercase().contains("json"))
}
