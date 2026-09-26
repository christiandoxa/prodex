//! DeepSeek response-format metadata and degraded JSON-mode notes.

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
    let provider = base.remove(provider_key);
    let base = serde_json::to_string(&base)
        .map_err(|error| format!("{provider_label} metadata serialization failed: {error}"))?;
    let provider = provider
        .map(|value| serde_json::to_string(&value))
        .transpose()
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
    input.metadata = provider.as_deref();
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
            let mut input =
                super::DeepSeekKernelInput::new(super::DeepSeekKernelOperation::ResponseFormat);
            input.role = Some(format_type);
            super::deepseek_provider_core_mojo_value(input)
                .map(Some)
                .map_err(|error| {
                    format!("{provider_label} response_format could not be normalized: {error}")
                })
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
    let existing = match response_metadata.as_ref() {
        Some(serde_json::Value::Object(object)) => object.clone(),
        Some(_) => return,
        None => serde_json::Map::new(),
    };
    let mapped = mojo_metadata_value(MojoMetadataRequest {
        existing,
        provider_label,
        provider_key,
        client_metadata: None,
        prompt_cache_key: None,
        prompt_cache_retention: None,
        degraded_from: None,
        tool_choice: Some(tool_choice),
        thinking_enabled: true,
    });
    // ponytail: retain metadata on bounded ABI failure; report errors if this API gains a Result.
    if let Ok(mapped) = mapped {
        *response_metadata = mapped;
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

#[cfg(test)]
mod tests {
    use super::{
        deepseek_provider_core_note_thinking_tool_choice_omission,
        deepseek_provider_core_response_metadata_from_responses_request,
    };
    use serde_json::json;

    #[test]
    fn empty_provider_metadata_survives_mojo_normalization() {
        let request = json!({"metadata": {"deepseek": {}}});
        assert_eq!(
            deepseek_provider_core_response_metadata_from_responses_request(
                &request, "DeepSeek", "deepseek"
            )
            .unwrap(),
            Some(json!({"deepseek": {}}))
        );

        let request = json!({
            "metadata": {"deepseek": {}},
            "response_format": {"type": "json_schema"}
        });
        let metadata = deepseek_provider_core_response_metadata_from_responses_request(
            &request, "DeepSeek", "deepseek",
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            metadata["deepseek"]["degraded_response_format"]["from"],
            "json_schema"
        );
    }

    #[test]
    fn thinking_note_preserves_non_object_metadata() {
        let mut metadata = Some(json!(["unrelated"]));
        deepseek_provider_core_note_thinking_tool_choice_omission(
            &json!({"tool_choice": "required"}),
            true,
            "DeepSeek",
            "deepseek",
            &mut metadata,
        );
        assert_eq!(metadata, Some(json!(["unrelated"])));
    }

    #[test]
    fn oversized_thinking_note_keeps_existing_metadata() {
        let original = Some(json!({"keep": true}));
        let mut metadata = original.clone();
        deepseek_provider_core_note_thinking_tool_choice_omission(
            &json!({"tool_choice": {"name": "x".repeat(4 * 1024 * 1024)}}),
            true,
            "DeepSeek",
            "deepseek",
            &mut metadata,
        );
        assert_eq!(metadata, original);
    }
}
