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

#[cfg(feature = "mojo")]
pub(super) fn plan_bytes(
    input: &[u8],
    operation: DeepSeekRequestPolicyOperation,
    flag: bool,
) -> Option<DeepSeekRequestPolicyPlan> {
    let source = std::str::from_utf8(input).ok()?;
    deepseek_request_policy(operation, source, flag, 0).ok()
}

pub(super) fn detail(source: &str, plan: DeepSeekRequestPolicyPlan) -> Option<String> {
    let start = plan.detail_start?;
    let end = plan.detail_end?;
    serde_json::from_str::<String>(&source[start..end]).ok()
}
