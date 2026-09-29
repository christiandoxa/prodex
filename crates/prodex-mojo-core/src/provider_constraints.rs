mod gemini_bridge_request;
mod gemini_request;
mod gemini_request_content;
mod gemini_sse_tool_call_index;

pub use gemini_bridge_request::{
    GeminiBridgeRequestKernelInput, GeminiBridgeRequestOperation, gemini_bridge_request_kernel,
};
pub use gemini_request::{
    GEMINI_REQUEST_FIELD_PLAN_MAX_FIELDS, GeminiRequestField, GeminiRequestFieldTarget,
    gemini_request_field_plan,
};
pub use gemini_request_content::{
    GeminiRequestContentKernelInput, GeminiRequestContentOperation, gemini_request_content_kernel,
};
pub use gemini_sse_tool_call_index::{
    GeminiToolCallIndexBinding, GeminiToolCallIndexRecord, gemini_tool_call_index,
};

pub fn self_test() -> bool {
    gemini_request_field_plan(0, 0, 0).is_ok_and(|fields| fields.is_empty())
        && provider_reasoning_effort_class(" XHIGH ")
            .is_ok_and(|value| value == ProviderReasoningEffortClass::XHigh)
        && provider_copilot_prompt_token_limit(" GPT-5.4 ")
            .is_ok_and(|value| value == Some(922_000))
}

#[cfg(test)]
mod scalar_policy_tests {
    use super::*;

    #[test]
    fn provider_scalar_policy_preserves_reasoning_and_copilot_limits() {
        assert_eq!(
            provider_reasoning_effort_class("\u{2003}ULTRA\u{2003}").unwrap(),
            ProviderReasoningEffortClass::Ultra
        );
        assert_eq!(
            provider_reasoning_effort_class("unknown-effort").unwrap(),
            ProviderReasoningEffortClass::Unknown
        );
        assert_eq!(
            provider_copilot_prompt_token_limit("GPT-5.3-CODEX").unwrap(),
            Some(272_000)
        );
        assert_eq!(
            provider_copilot_prompt_token_limit(" claude-opus-4.8 ").unwrap(),
            Some(936_000)
        );
        assert_eq!(
            provider_copilot_prompt_token_limit("gpt-5.4-nano").unwrap(),
            Some(128_000)
        );
        assert_eq!(
            provider_copilot_prompt_token_limit("unknown-model").unwrap(),
            None
        );
        assert_eq!(provider_boolean_token(" YES ").unwrap(), Some(true));
        assert_eq!(
            provider_boolean_token("\u{2003}off\u{2003}").unwrap(),
            Some(false)
        );
        assert_eq!(provider_boolean_token("maybe").unwrap(), None);
    }
}

const PROVIDER_SCALAR_POLICY_ABI_VERSION: i64 = 1;
const PROVIDER_SCALAR_POLICY_REASONING_EFFORT: i64 = 0;
const PROVIDER_SCALAR_POLICY_COPILOT_PROMPT_LIMIT: i64 = 1;
const PROVIDER_SCALAR_POLICY_BOOLEAN_TOKEN: i64 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderReasoningEffortClass {
    None,
    Minimal,
    Low,
    Medium,
    High,
    XHigh,
    Max,
    Ultra,
    Unknown,
}

unsafe extern "C" {
    fn prodex_provider_scalar_policy_v1(
        abi_version: i64,
        operation: i64,
        address: u64,
        length: i64,
    ) -> i64;
}

fn provider_scalar_policy(operation: i64, value: &str) -> Result<i64, crate::MojoError> {
    let length = i64::try_from(value.len()).map_err(|_| crate::MojoError::InvalidInput)?;
    let result = unsafe {
        prodex_provider_scalar_policy_v1(
            PROVIDER_SCALAR_POLICY_ABI_VERSION,
            operation,
            value.as_ptr() as usize as u64,
            length,
        )
    };
    match result {
        -3 => Err(crate::MojoError::AbiMismatch),
        -2 => Err(crate::MojoError::InvalidInput),
        value => Ok(value),
    }
}

pub fn provider_reasoning_effort_class(
    value: &str,
) -> Result<ProviderReasoningEffortClass, crate::MojoError> {
    Ok(
        match provider_scalar_policy(PROVIDER_SCALAR_POLICY_REASONING_EFFORT, value)? {
            0 => ProviderReasoningEffortClass::None,
            1 => ProviderReasoningEffortClass::Minimal,
            2 => ProviderReasoningEffortClass::Low,
            3 => ProviderReasoningEffortClass::Medium,
            4 => ProviderReasoningEffortClass::High,
            5 => ProviderReasoningEffortClass::XHigh,
            6 => ProviderReasoningEffortClass::Max,
            7 => ProviderReasoningEffortClass::Ultra,
            8 => ProviderReasoningEffortClass::Unknown,
            _ => return Err(crate::MojoError::InvalidOutput),
        },
    )
}

pub fn provider_boolean_token(value: &str) -> Result<Option<bool>, crate::MojoError> {
    match provider_scalar_policy(PROVIDER_SCALAR_POLICY_BOOLEAN_TOKEN, value)? {
        -1 => Ok(None),
        0 => Ok(Some(false)),
        1 => Ok(Some(true)),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}

pub fn provider_copilot_prompt_token_limit(model: &str) -> Result<Option<usize>, crate::MojoError> {
    match provider_scalar_policy(PROVIDER_SCALAR_POLICY_COPILOT_PROMPT_LIMIT, model)? {
        -1 => Ok(None),
        value if value > 0 => usize::try_from(value)
            .map(Some)
            .map_err(|_| crate::MojoError::InvalidOutput),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}
