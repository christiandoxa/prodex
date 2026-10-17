//! DeepSeek Responses input-item validation.
//!
//! Mojo owns both the issue classification and the exact provider-facing
//! diagnostic. Rust only acquires JSON and forwards the typed result.

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
        let diagnostic =
            prodex_mojo_core::rich::deepseek_input_item_issue_message(1, provider_label, "")
                .map_err(|error| format!("DeepSeek input item diagnostic ABI failed: {error:?}"))?;
        return Err(diagnostic);
    };
    let (tag, detail) = deepseek_provider_core_input_item_plan(object, gemini_compat)?;
    if tag == 0 {
        return Ok(());
    }
    let diagnostic = prodex_mojo_core::rich::deepseek_input_item_issue_message(
        tag,
        provider_label,
        detail.as_deref().unwrap_or_default(),
    )
    .map_err(|error| format!("DeepSeek input item diagnostic ABI failed: {error:?}"))?;
    Err(diagnostic)
}
