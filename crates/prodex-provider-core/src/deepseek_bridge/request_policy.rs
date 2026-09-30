use prodex_mojo_core::rich::{
    DEEPSEEK_KERNEL_MAX_BYTES, DeepSeekRequestPolicyOperation, DeepSeekRequestPolicyPlan,
    deepseek_request_policy,
};

pub(super) fn plan_value(
    value: &serde_json::Value,
    operation: DeepSeekRequestPolicyOperation,
    flag: bool,
) -> (String, DeepSeekRequestPolicyPlan) {
    let source = serde_json::to_string(value).expect("DeepSeek policy input serializes");
    let plan = deepseek_request_policy(operation, &source, flag, 0)
        .expect("Mojo DeepSeek request policy returned invalid output");
    (source, plan)
}

pub(super) fn try_plan_value(
    value: &serde_json::Value,
    operation: DeepSeekRequestPolicyOperation,
    flag: bool,
    provider_label: &str,
) -> Result<(String, DeepSeekRequestPolicyPlan), String> {
    let source = serde_json::to_string(value).map_err(|error| {
        format!("{provider_label} request policy serialization failed: {error}")
    })?;
    if source.len() > DEEPSEEK_KERNEL_MAX_BYTES {
        return Err(format!(
            "{provider_label} request policy input exceeds {DEEPSEEK_KERNEL_MAX_BYTES} bytes"
        ));
    }
    let plan = deepseek_request_policy(operation, &source, flag, 0)
        .map_err(|error| format!("{provider_label} request policy failed: {error:?}"))?;
    Ok((source, plan))
}

pub(super) fn detail(source: &str, plan: DeepSeekRequestPolicyPlan) -> Option<String> {
    let start = plan.detail_start?;
    let end = plan.detail_end?;
    serde_json::from_str::<String>(&source[start..end]).ok()
}

fn bool_policy(
    operation: DeepSeekRequestPolicyOperation,
    input: &str,
    flag: bool,
    scalar: i64,
) -> Result<bool, String> {
    let plan = deepseek_request_policy(operation, input, flag, scalar)
        .map_err(|error| format!("DeepSeek Mojo policy failed: {error:?}"))?;
    match plan.tag {
        0 => Ok(false),
        1 => Ok(true),
        value => Err(format!(
            "DeepSeek Mojo policy returned invalid boolean tag {value}"
        )),
    }
}

pub fn deepseek_provider_core_uses_native_web_search(
    mode: i64,
    body: &[u8],
) -> Result<bool, String> {
    let source = std::str::from_utf8(body)
        .map_err(|error| format!("DeepSeek request body is not UTF-8: {error}"))?;
    bool_policy(
        DeepSeekRequestPolicyOperation::NativeWebSearch,
        source,
        false,
        mode,
    )
}

pub fn deepseek_provider_core_native_translation_fallback_is_safe(
    result: &crate::ProviderTransformResult,
) -> bool {
    let crate::ProviderTransformLoss::Rejected { reason } = &result.loss else {
        return false;
    };
    bool_policy(
        DeepSeekRequestPolicyOperation::NativeFallbackSafe,
        reason,
        false,
        0,
    )
    .expect("Mojo DeepSeek native fallback policy returned invalid output")
}

pub fn deepseek_provider_core_auto_chat_fallback_body(mode: i64, body: &[u8]) -> Option<Vec<u8>> {
    let allowed = bool_policy(
        DeepSeekRequestPolicyOperation::AutoChatFallback,
        "",
        false,
        mode,
    )
    .expect("Mojo DeepSeek auto-chat fallback policy returned invalid output");
    allowed
        .then(|| crate::provider_core_chat_request_body_without_web_search_options(body))
        .flatten()
}

pub fn deepseek_provider_core_use_beta_binding_route(
    strict_tools: bool,
    endpoint_is_responses: bool,
) -> bool {
    bool_policy(
        DeepSeekRequestPolicyOperation::StrictBindingRoute,
        "",
        strict_tools,
        if endpoint_is_responses { 0 } else { 1 },
    )
    .expect("Mojo DeepSeek strict binding route policy returned invalid output")
}
