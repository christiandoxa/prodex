//! DeepSeek reasoning-mode request parameter mapping.

fn reasoning_effort_from_responses_request(
    value: &serde_json::Value,
    provider_label: &str,
) -> Result<Option<String>, String> {
    let relevant = value.as_object().map(|object| {
        serde_json::Value::Object(
            ["reasoning", "reasoning_effort"]
                .into_iter()
                .filter_map(|key| {
                    object
                        .get(key)
                        .map(|field| (key.to_string(), field.clone()))
                })
                .collect(),
        )
    });
    let (source, plan) = super::super::request_policy::try_plan_value(
        relevant.as_ref().unwrap_or(value),
        prodex_mojo_core::rich::DeepSeekRequestPolicyOperation::ReasoningShape,
        false,
        provider_label,
    )?;
    match plan.tag {
        0 => Ok(None),
        1 => Err(format!("{provider_label} reasoning must be an object")),
        2 => Err(format!(
            "{provider_label} reasoning.{} is not supported by this Responses adapter",
            super::super::request_policy::detail(&source, plan).unwrap_or_default()
        )),
        3 => Err(format!(
            "{provider_label} reasoning.effort must be a string"
        )),
        4 => Err(format!(
            "{provider_label} reasoning_effort must be a string"
        )),
        10 => Ok(super::super::request_policy::detail(&source, plan)),
        tag => Err(format!(
            "{provider_label} reasoning validation returned unknown result {tag}"
        )),
    }
}

pub fn deepseek_provider_core_apply_reasoning_from_responses_request(
    value: &serde_json::Value,
    request: &mut serde_json::Map<String, serde_json::Value>,
    provider_label: &str,
    gemini_compat: bool,
) -> Result<(), String> {
    let effort = reasoning_effort_from_responses_request(value, provider_label)?;
    if let Some(effort) = effort.as_deref() {
        let mut input =
            super::DeepSeekKernelInput::new(super::DeepSeekKernelOperation::ReasoningParameters);
        input.stream = gemini_compat;
        input.reasoning_content = Some(effort);
        let mapped = super::deepseek_provider_core_mojo_value(input)
            .map_err(|_| format!("{provider_label} reasoning effort is not supported"))?;
        let Some(mapped) = mapped.as_object() else {
            return Err(format!(
                "{provider_label} reasoning effort normalization returned a non-object"
            ));
        };
        request.extend(
            mapped
                .iter()
                .map(|(key, value)| (key.clone(), value.clone())),
        );
    }
    Ok(())
}

pub fn deepseek_provider_core_thinking_enabled(value: &serde_json::Value) -> bool {
    let Ok(Some(effort)) = reasoning_effort_from_responses_request(value, "DeepSeek") else {
        return false;
    };
    let mut input =
        super::DeepSeekKernelInput::new(super::DeepSeekKernelOperation::ReasoningParameters);
    input.reasoning_content = Some(&effort);
    super::deepseek_provider_core_mojo_value(input)
        .ok()
        .is_some_and(|mapped| {
            mapped
                .get("thinking")
                .and_then(|thinking| thinking.get("type"))
                .and_then(serde_json::Value::as_str)
                == Some("enabled")
        })
}

pub fn deepseek_provider_core_validate_reasoning_shape(
    value: &serde_json::Value,
    provider_label: &str,
) -> Result<(), String> {
    reasoning_effort_from_responses_request(value, provider_label).map(|_| ())
}
