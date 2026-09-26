//! DeepSeek request-field rejection checks.

fn request_policy_error(
    source: &str,
    plan: prodex_mojo_core::rich::DeepSeekRequestPolicyPlan,
    provider_label: &str,
) -> Option<String> {
    let detail = super::super::request_policy::detail(source, plan);
    Some(match plan.tag {
        0 => return None,
        1 => format!(
            "{provider_label} frequency_penalty is deprecated and is not forwarded by Prodex"
        ),
        2 => format!(
            "{provider_label} presence_penalty is deprecated and is not forwarded by Prodex"
        ),
        10..=16 => {
            let field = [
                "n",
                "seed",
                "service_tier",
                "prediction",
                "logit_bias",
                "functions",
                "function_call",
            ][(plan.tag - 10) as usize];
            format!("{provider_label} {field} is not supported by this Responses adapter")
        }
        20 => format!("{provider_label} include must be an array"),
        21 => format!("{provider_label} store must be a boolean"),
        22 => format!(
            "{provider_label} background responses are not supported by this Responses adapter"
        ),
        23 => format!("{provider_label} background must be a boolean"),
        24 => {
            format!("{provider_label} truncation=auto is not supported by this Responses adapter")
        }
        25 => format!(
            "{provider_label} truncation `{}` is not supported",
            detail.as_deref().unwrap_or("")
        ),
        26 => format!("{provider_label} truncation must be a string"),
        27 => format!("{provider_label} max_tool_calls is not supported by this Responses adapter"),
        28 => format!("{provider_label} text must be an object"),
        29 => format!(
            "{provider_label} text.{} is not supported by this Responses adapter",
            detail.as_deref().unwrap_or("")
        ),
        30 => format!(
            "{provider_label} does not expose a compatible parallel_tool_calls=false control"
        ),
        31 => format!("{provider_label} parallel_tool_calls must be a boolean"),
        32 => format!("{provider_label} stream_options must be an object"),
        33 => format!("{provider_label} stream_options requires stream=true"),
        34 => format!(
            "{provider_label} stream_options.{} is not supported",
            detail.as_deref().unwrap_or("")
        ),
        35 => {
            format!("{provider_label} streaming adapter requires stream_options.include_usage=true")
        }
        36 => format!("{provider_label} stream_options.include_usage must be a boolean"),
        37 => format!("{provider_label} modalities must be an array"),
        38 => format!(
            "{provider_label} Responses adapter only supports text modality; audio/image/video modalities are not supported"
        ),
        39 => format!("{provider_label} Responses adapter does not support audio output"),
        _ => format!("{provider_label} request-field policy returned an unknown result"),
    })
}

pub fn deepseek_provider_core_reject_unsupported_request_fields(
    value: &serde_json::Value,
    provider_label: &str,
) -> Result<(), String> {
    if !value.is_object() {
        return Ok(());
    }
    let (source, plan) = super::super::request_policy::try_plan_value(
        value,
        prodex_mojo_core::rich::DeepSeekRequestPolicyOperation::RequestFields,
        false,
        provider_label,
    )?;
    request_policy_error(&source, plan, provider_label).map_or(Ok(()), Err)
}

pub fn deepseek_provider_core_reject_beta_completion_fields(
    value: &serde_json::Value,
    provider_label: &str,
) -> Result<(), String> {
    if !value.is_object() {
        return Ok(());
    }
    let (_source, plan) = super::super::request_policy::try_plan_value(
        value,
        prodex_mojo_core::rich::DeepSeekRequestPolicyOperation::BetaFields,
        false,
        provider_label,
    )?;
    match plan.tag {
        0 => Ok(()),
        1 => Err(format!(
            "{provider_label} chat prefix completion requires the beta chat endpoint, which is outside this Responses adapter"
        )),
        2 => Err(format!(
            "{provider_label} FIM suffix completion requires the beta /completions endpoint, which is outside this Responses adapter"
        )),
        3 => Err(format!(
            "{provider_label} prompt completions require the beta /completions endpoint, which is outside this Responses adapter"
        )),
        _ => Err(format!(
            "{provider_label} beta-field policy returned an unknown result"
        )),
    }
}
