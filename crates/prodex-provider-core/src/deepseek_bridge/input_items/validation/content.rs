//! DeepSeek Responses message content validation.

pub(in crate::deepseek_bridge::input_items) fn deepseek_provider_core_validate_supported_message_content(
    content: Option<&serde_json::Value>,
    gemini_compat: bool,
    provider_label: &str,
) -> Result<(), String> {
    match content {
        Some(serde_json::Value::Array(parts)) => {
            for part in parts {
                deepseek_provider_core_validate_supported_content_part(
                    part,
                    gemini_compat,
                    provider_label,
                )?;
            }
        }
        Some(content @ serde_json::Value::Object(_)) => {
            deepseek_provider_core_validate_supported_content_part(
                content,
                gemini_compat,
                provider_label,
            )?;
        }
        Some(serde_json::Value::String(_)) | None => {}
        Some(_) => {
            return Err(format!(
                "{provider_label} message content must be a string, object, or array"
            ));
        }
    }
    Ok(())
}

fn deepseek_provider_core_validate_supported_content_part(
    part: &serde_json::Value,
    gemini_compat: bool,
    provider_label: &str,
) -> Result<(), String> {
    let Some(object) = part.as_object() else {
        return Ok(());
    };
    deepseek_provider_core_reject_chat_prefix_marker(object, provider_label)?;
    if object.get("cache_control").is_some() {
        return Err(format!(
            "{provider_label} per-message cache_control is not supported by this Responses adapter because DeepSeek context caching is automatic"
        ));
    }
    let has_text = object
        .get("text")
        .or_else(|| object.get("input_text"))
        .or_else(|| object.get("output_text"))
        .and_then(serde_json::Value::as_str)
        .is_some();
    if has_text {
        return Ok(());
    }
    let Some(part_type) = object.get("type").and_then(serde_json::Value::as_str) else {
        return Err(format!(
            "{provider_label} text-only adapter does not support object message content parts without text or type"
        ));
    };
    if gemini_compat
        && matches!(
            part_type,
            "input_image"
                | "image_url"
                | "input_file"
                | "file"
                | "media"
                | "input_audio"
                | "input_video"
        )
    {
        return Ok(());
    }
    if matches!(part_type, "input_text" | "output_text" | "text") {
        return Err(format!(
            "{provider_label} {part_type} content parts require a text field"
        ));
    }
    Err(format!(
        "{provider_label} text-only adapter does not support message content part type `{part_type}`"
    ))
}

pub(in crate::deepseek_bridge::input_items) fn deepseek_provider_core_reject_chat_prefix_marker(
    object: &serde_json::Map<String, serde_json::Value>,
    provider_label: &str,
) -> Result<(), String> {
    if object.get("prefix").is_some() {
        return Err(format!(
            "{provider_label} chat prefix completion requires the beta chat endpoint, which this Responses adapter does not enable yet"
        ));
    }
    Ok(())
}
