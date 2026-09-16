//! DeepSeek reasoning-mode request parameter mapping.

#[cfg(feature = "mojo")]
fn reasoning_effort_from_responses_request(
    value: &serde_json::Value,
    provider_label: &str,
) -> Result<Option<String>, String> {
    let (source, plan) = super::super::request_policy::plan_value(
        value,
        prodex_mojo_core::rich::DeepSeekRequestPolicyOperation::ReasoningShape,
        false,
    );
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
        tag => panic!("Mojo DeepSeek reasoning-shape policy returned unknown tag {tag}"),
    }
}

pub fn deepseek_provider_core_apply_reasoning_from_responses_request(
    value: &serde_json::Value,
    request: &mut serde_json::Map<String, serde_json::Value>,
    provider_label: &str,
    gemini_compat: bool,
) -> Result<(), String> {
    #[cfg(feature = "mojo")]
    {
        let effort = reasoning_effort_from_responses_request(value, provider_label)?;
        if let Some(effort) = effort.as_deref() {
            let mut input = super::DeepSeekKernelInput::new(
                super::DeepSeekKernelOperation::ReasoningParameters,
            );
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
    #[cfg(not(feature = "mojo"))]
    rust_compat::apply_reasoning_rust(value, request, provider_label, gemini_compat)
}

pub fn deepseek_provider_core_thinking_enabled(value: &serde_json::Value) -> bool {
    #[cfg(feature = "mojo")]
    {
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
    #[cfg(not(feature = "mojo"))]
    rust_compat::thinking_enabled_rust(value)
}

pub fn deepseek_provider_core_validate_reasoning_shape(
    value: &serde_json::Value,
    provider_label: &str,
) -> Result<(), String> {
    #[cfg(feature = "mojo")]
    {
        reasoning_effort_from_responses_request(value, provider_label).map(|_| ())
    }
    #[cfg(not(feature = "mojo"))]
    {
        rust_compat::validate_reasoning_shape_rust(value, provider_label)
    }
}

#[cfg(not(feature = "mojo"))]
mod rust_compat {
    use super::*;

    pub(super) fn apply_reasoning_rust(
        value: &serde_json::Value,
        request: &mut serde_json::Map<String, serde_json::Value>,
        provider_label: &str,
        gemini_compat: bool,
    ) -> Result<(), String> {
        let effort =
            deepseek_provider_core_reasoning_effort_from_responses_request(value, provider_label)?;
        #[cfg(feature = "mojo")]
        {
            if let Some(effort) = effort {
                let mut input = super::DeepSeekKernelInput::new(
                    super::DeepSeekKernelOperation::ReasoningParameters,
                );
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
        #[cfg(not(feature = "mojo"))]
        {
            if gemini_compat {
                if let Some(effort) = effort {
                    let Some(effort) =
                        deepseek_provider_core_gemini_openai_reasoning_effort(effort)
                    else {
                        return Err(format!(
                            "{provider_label} reasoning effort is not supported"
                        ));
                    };
                    request.insert(
                        "reasoning_effort".to_string(),
                        serde_json::Value::String(effort.to_string()),
                    );
                }
                return Ok(());
            }
            match effort.and_then(deepseek_provider_core_reasoning_effort) {
                Some(Some(effort)) => {
                    request.insert(
                        "thinking".to_string(),
                        serde_json::json!({"type": "enabled"}),
                    );
                    request.insert(
                        "reasoning_effort".to_string(),
                        serde_json::Value::String(effort.to_string()),
                    );
                }
                Some(None) => {
                    request.insert(
                        "thinking".to_string(),
                        serde_json::json!({"type": "disabled"}),
                    );
                }
                None => {}
            }
            if effort.is_some()
                && effort
                    .and_then(deepseek_provider_core_reasoning_effort)
                    .is_none()
            {
                return Err(format!(
                    "{provider_label} reasoning effort is not supported"
                ));
            }
            Ok(())
        }
    }

    pub(super) fn thinking_enabled_rust(value: &serde_json::Value) -> bool {
        #[cfg(feature = "mojo")]
        {
            let Ok(Some(effort)) =
                deepseek_provider_core_reasoning_effort_from_responses_request(value, "DeepSeek")
            else {
                return false;
            };
            let mut input = super::DeepSeekKernelInput::new(
                super::DeepSeekKernelOperation::ReasoningParameters,
            );
            input.reasoning_content = Some(effort);
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
        #[cfg(not(feature = "mojo"))]
        deepseek_provider_core_reasoning_effort_from_responses_request(value, "DeepSeek")
            .ok()
            .flatten()
            .and_then(deepseek_provider_core_reasoning_effort)
            .is_some_and(|effort| effort.is_some())
    }

    pub(super) fn validate_reasoning_shape_rust(
        value: &serde_json::Value,
        provider_label: &str,
    ) -> Result<(), String> {
        deepseek_provider_core_reasoning_effort_from_responses_request(value, provider_label)
            .map(|_| ())
    }

    fn deepseek_provider_core_reasoning_effort_from_responses_request<'a>(
        value: &'a serde_json::Value,
        provider_label: &str,
    ) -> Result<Option<&'a str>, String> {
        if let Some(reasoning) = value.get("reasoning") {
            let Some(reasoning) = reasoning.as_object() else {
                return Err(format!("{provider_label} reasoning must be an object"));
            };
            for key in reasoning.keys() {
                if key != "effort" {
                    return Err(format!(
                        "{provider_label} reasoning.{key} is not supported by this Responses adapter"
                    ));
                }
            }
            if let Some(effort) = reasoning.get("effort") {
                return effort
                    .as_str()
                    .map(Some)
                    .ok_or_else(|| format!("{provider_label} reasoning.effort must be a string"));
            }
        }
        if let Some(effort) = value.get("reasoning_effort") {
            return effort
                .as_str()
                .map(Some)
                .ok_or_else(|| format!("{provider_label} reasoning_effort must be a string"));
        }
        Ok(None)
    }

    #[cfg(not(feature = "mojo"))]
    fn deepseek_provider_core_gemini_openai_reasoning_effort(effort: &str) -> Option<&'static str> {
        match effort.trim().to_ascii_lowercase().as_str() {
            "xhigh" | "max" | "high" => Some("high"),
            "medium" => Some("medium"),
            "low" => Some("low"),
            "minimal" => Some("minimal"),
            "none" => Some("none"),
            _ => None,
        }
    }

    #[cfg(not(feature = "mojo"))]
    fn deepseek_provider_core_reasoning_effort(effort: &str) -> Option<Option<&'static str>> {
        match effort.trim().to_ascii_lowercase().as_str() {
            "xhigh" | "max" => Some(Some("max")),
            "high" | "medium" | "low" => Some(Some("high")),
            "minimal" | "none" => Some(None),
            _ => None,
        }
    }
}
