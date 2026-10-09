//! DeepSeek Responses input-item validation.
//!
//! Mojo classifies the item and returns only a stable issue tag/detail. Rust
//! keeps JSON acquisition and the provider-facing error text.

fn deepseek_provider_core_input_item_plan(
    object: &serde_json::Map<String, serde_json::Value>,
    gemini_compat: bool,
) -> Result<(u64, Option<String>), String> {
    let value = serde_json::Value::Object(object.clone());
    let source = serde_json::to_string(&value)
        .map_err(|error| format!("DeepSeek input item serialization failed: {error}"))?;
    let mut input = prodex_mojo_core::rich::DeepSeekKernelInput::new(
        prodex_mojo_core::rich::DeepSeekKernelOperation::InputItemValidation,
    );
    input.item = Some(&source);
    input.stream = gemini_compat;
    let body = prodex_mojo_core::rich::deepseek_kernel(input)
        .map_err(|error| format!("DeepSeek input item validation failed: {error:?}"))?;
    let plan: serde_json::Value = serde_json::from_slice(&body).map_err(|error| {
        format!("DeepSeek input item validation returned invalid JSON: {error}")
    })?;
    let tag = plan
        .get("tag")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| "DeepSeek input item validation omitted issue tag".to_string())?;
    let detail = plan
        .get("detail")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    Ok((tag, detail))
}

pub(super) fn deepseek_provider_core_validate_supported_input_item(
    item: &serde_json::Value,
    gemini_compat: bool,
    provider_label: &str,
) -> Result<(), String> {
    let Some(object) = item.as_object() else {
        return Err(format!("{provider_label} input items must be objects"));
    };
    let (tag, detail) = deepseek_provider_core_input_item_plan(object, gemini_compat)?;
    let detail = detail.unwrap_or_default();
    match tag {
        0 => Ok(()),
        1 => Err(format!("{provider_label} input items must be objects")),
        2 => Err(format!(
            "{provider_label} chat prefix completion requires the beta chat endpoint, which is outside this Responses adapter"
        )),
        3 => Err(format!("{provider_label} message role must be a string")),
        4 => Err(format!(
            "{provider_label} message role `{detail}` is not supported by this Responses adapter"
        )),
        5 => Err(format!(
            "{provider_label} message content must be a string, object, or array"
        )),
        6 => Err(format!(
            "{provider_label} per-message cache_control is not supported by this Responses adapter because DeepSeek context caching is automatic"
        )),
        7 => Err(format!(
            "{provider_label} text-only adapter does not support object message content parts without text or type"
        )),
        8 => Err(format!(
            "{provider_label} text-only adapter does not support message content part type `{detail}`"
        )),
        10 => Err(format!(
            "{provider_label} input tool items require a call_id"
        )),
        11 => Err(format!(
            "{provider_label} input tool call items require a function name"
        )),
        12 => Err(format!(
            "{provider_label} input tool output items require output content"
        )),
        13 => Err(format!(
            "{provider_label} local_shell_call action.command must be an array of strings"
        )),
        14 => Err(format!(
            "{provider_label} local_shell_call requires a command"
        )),
        15 => Err(format!(
            "{provider_label} input item type `{detail}` is not supported by this Responses adapter"
        )),
        16 | 21 => Err(format!(
            "{provider_label} text content parts require a text field"
        )),
        17 | 19 => Err(format!(
            "{provider_label} input_text content parts require a text field"
        )),
        18 | 20 => Err(format!(
            "{provider_label} output_text content parts require a text field"
        )),
        _ => Err(format!(
            "{provider_label} input item validation returned an unknown result"
        )),
    }
}
