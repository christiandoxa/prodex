//! DeepSeek request-field rejection checks.

pub fn deepseek_provider_core_reject_unsupported_request_fields(
    value: &serde_json::Value,
    provider_label: &str,
) -> Result<(), String> {
    deepseek_provider_core_reject_deprecated_fields(value, provider_label)?;
    deepseek_provider_core_reject_unsupported_fields(value, provider_label)?;
    deepseek_provider_core_validate_optional_request_values(value, provider_label)?;
    deepseek_provider_core_validate_background(value, provider_label)?;
    deepseek_provider_core_validate_truncation(value, provider_label)?;
    deepseek_provider_core_reject_max_tool_calls(value, provider_label)?;
    deepseek_provider_core_validate_text(value, provider_label)?;
    deepseek_provider_core_validate_parallel_tool_calls(value, provider_label)?;
    deepseek_provider_core_validate_stream_options(value, provider_label)?;
    deepseek_provider_core_validate_modalities(value, provider_label)?;
    if value.get("audio").is_some() {
        return Err(format!(
            "{provider_label} Responses adapter does not support audio output"
        ));
    }
    Ok(())
}

fn deepseek_provider_core_reject_deprecated_fields(
    value: &serde_json::Value,
    provider_label: &str,
) -> Result<(), String> {
    for field in ["frequency_penalty", "presence_penalty"] {
        if value.get(field).is_some() {
            return Err(format!(
                "{provider_label} {field} is deprecated and is not forwarded by Prodex"
            ));
        }
    }
    Ok(())
}

fn deepseek_provider_core_reject_unsupported_fields(
    value: &serde_json::Value,
    provider_label: &str,
) -> Result<(), String> {
    for field in [
        "n",
        "seed",
        "service_tier",
        "prediction",
        "logit_bias",
        "functions",
        "function_call",
    ] {
        if value.get(field).is_some() {
            return Err(format!(
                "{provider_label} {field} is not supported by this Responses adapter"
            ));
        }
    }
    Ok(())
}

fn deepseek_provider_core_validate_optional_request_values(
    value: &serde_json::Value,
    provider_label: &str,
) -> Result<(), String> {
    if let Some(include) = value.get("include")
        && !include.is_array()
    {
        return Err(format!("{provider_label} include must be an array"));
    }
    if let Some(store) = value.get("store")
        && !store.is_boolean()
    {
        return Err(format!("{provider_label} store must be a boolean"));
    }
    Ok(())
}

fn deepseek_provider_core_validate_background(
    value: &serde_json::Value,
    provider_label: &str,
) -> Result<(), String> {
    let Some(background) = value.get("background") else {
        return Ok(());
    };
    match background.as_bool() {
        Some(false) => Ok(()),
        Some(true) => Err(format!(
            "{provider_label} background responses are not supported by this Responses adapter"
        )),
        None => Err(format!("{provider_label} background must be a boolean")),
    }
}

fn deepseek_provider_core_validate_truncation(
    value: &serde_json::Value,
    provider_label: &str,
) -> Result<(), String> {
    let Some(truncation) = value.get("truncation") else {
        return Ok(());
    };
    match truncation.as_str() {
        Some("disabled") => Ok(()),
        Some("auto") => Err(format!(
            "{provider_label} truncation=auto is not supported by this Responses adapter"
        )),
        Some(other) => Err(format!(
            "{provider_label} truncation `{other}` is not supported"
        )),
        None => Err(format!("{provider_label} truncation must be a string")),
    }
}

fn deepseek_provider_core_reject_max_tool_calls(
    value: &serde_json::Value,
    provider_label: &str,
) -> Result<(), String> {
    if value.get("max_tool_calls").is_some() {
        return Err(format!(
            "{provider_label} max_tool_calls is not supported by this Responses adapter"
        ));
    }
    Ok(())
}

fn deepseek_provider_core_validate_text(
    value: &serde_json::Value,
    provider_label: &str,
) -> Result<(), String> {
    let Some(text) = value.get("text") else {
        return Ok(());
    };
    let Some(text) = text.as_object() else {
        return Err(format!("{provider_label} text must be an object"));
    };
    for key in text.keys() {
        if key != "format" {
            return Err(format!(
                "{provider_label} text.{key} is not supported by this Responses adapter"
            ));
        }
    }
    Ok(())
}

fn deepseek_provider_core_validate_parallel_tool_calls(
    value: &serde_json::Value,
    provider_label: &str,
) -> Result<(), String> {
    let Some(parallel_tool_calls) = value.get("parallel_tool_calls") else {
        return Ok(());
    };
    match parallel_tool_calls.as_bool() {
        Some(true) => Ok(()),
        Some(false) => Err(format!(
            "{provider_label} does not expose a compatible parallel_tool_calls=false control"
        )),
        None => Err(format!(
            "{provider_label} parallel_tool_calls must be a boolean"
        )),
    }
}

fn deepseek_provider_core_validate_stream_options(
    value: &serde_json::Value,
    provider_label: &str,
) -> Result<(), String> {
    let Some(stream_options) = value.get("stream_options") else {
        return Ok(());
    };
    let Some(stream_options) = stream_options.as_object() else {
        return Err(format!("{provider_label} stream_options must be an object"));
    };
    if value.get("stream").and_then(serde_json::Value::as_bool) != Some(true) {
        return Err(format!(
            "{provider_label} stream_options requires stream=true"
        ));
    }
    for key in stream_options.keys() {
        if key != "include_usage" {
            return Err(format!(
                "{provider_label} stream_options.{key} is not supported"
            ));
        }
    }
    let Some(include_usage) = stream_options.get("include_usage") else {
        return Ok(());
    };
    match include_usage.as_bool() {
        Some(true) => Ok(()),
        Some(false) => Err(format!(
            "{provider_label} streaming adapter requires stream_options.include_usage=true"
        )),
        None => Err(format!(
            "{provider_label} stream_options.include_usage must be a boolean"
        )),
    }
}

fn deepseek_provider_core_validate_modalities(
    value: &serde_json::Value,
    provider_label: &str,
) -> Result<(), String> {
    let Some(modalities) = value.get("modalities") else {
        return Ok(());
    };
    let Some(modalities) = modalities.as_array() else {
        return Err(format!("{provider_label} modalities must be an array"));
    };
    if modalities
        .iter()
        .any(|modality| modality.as_str() != Some("text"))
    {
        return Err(format!(
            "{provider_label} Responses adapter only supports text modality; audio/image/video modalities are not supported"
        ));
    }
    Ok(())
}

pub fn deepseek_provider_core_reject_beta_completion_fields(
    value: &serde_json::Value,
    provider_label: &str,
) -> Result<(), String> {
    if value.get("prefix").is_some() {
        return Err(format!(
            "{provider_label} chat prefix completion requires the beta chat endpoint, which is outside this Responses adapter"
        ));
    }
    if value.get("suffix").is_some() {
        return Err(format!(
            "{provider_label} FIM suffix completion requires the beta /completions endpoint, which is outside this Responses adapter"
        ));
    }
    if value.get("prompt").is_some() {
        return Err(format!(
            "{provider_label} prompt completions require the beta /completions endpoint, which is outside this Responses adapter"
        ));
    }
    Ok(())
}
