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
        && provider_retry_plan(1, 1, 0, 4, 0)
            .is_ok_and(|plan| plan.decision == 0 && plan.remaining_precommit_retries == 1)
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
        assert_eq!(
            provider_retry_plan(1, 1, 0, 4, 0).unwrap(),
            ProviderRetryScalarPlan {
                decision: 0,
                remaining_precommit_retries: 1,
            }
        );
        assert_eq!(provider_retry_plan(1, 2, 0, 4, 0).unwrap().decision, 1);
        assert_eq!(provider_retry_plan(1, 1, 0, 0, 0).unwrap().decision, 3);
        assert_eq!(provider_retry_plan(1, 1, 0, 4, 1).unwrap().decision, 2);
        assert_eq!(provider_retry_transition(4, 0, 2, 0, 2, true).unwrap(), 1);
        assert_eq!(provider_retry_transition(0, 1, 2, 0, 2, true).unwrap(), 2);
        assert_eq!(provider_retry_transition(3, 1, 2, 1, 2, true).unwrap(), 0);
        assert_eq!(provider_retry_transition(4, 0, 2, 0, 2, false).unwrap(), 0);
        assert_eq!(
            provider_precommit_buffered_fallback_class(429, 2, false).unwrap(),
            None
        );
        assert_eq!(
            provider_precommit_buffered_fallback_class(429, 2, true).unwrap(),
            Some(2)
        );
        assert_eq!(
            provider_precommit_live_fallback_class(ProviderPrecommitLiveProgress::QuotaBlocked, 5,)
                .unwrap(),
            Some(1)
        );
        assert!(provider_precommit_should_prefetch(4, false, true, 200, true, true).unwrap());
        assert!(!provider_precommit_should_prefetch(1, false, true, 200, true, true).unwrap());
        assert!(provider_precommit_native_first_should_prefetch(true, 200, true, true).unwrap());
        assert!(!provider_precommit_native_first_should_prefetch(false, 200, true, true).unwrap());
        assert!(!provider_precommit_native_first_should_prefetch(true, 400, true, true).unwrap());
        assert_eq!(
            provider_precommit_sse_action(false, false, false, false, true).unwrap(),
            ProviderPrecommitSseAction::None
        );
        assert_eq!(
            provider_precommit_sse_action(false, false, false, false, false).unwrap(),
            ProviderPrecommitSseAction::Commit
        );
        assert_eq!(
            provider_precommit_health_action(false, 0, None).unwrap(),
            ProviderPrecommitHealthAction::TransportFailure
        );
        assert_eq!(
            provider_precommit_health_action(true, 503, None).unwrap(),
            ProviderPrecommitHealthAction::Overload
        );
        assert_eq!(
            provider_precommit_metric_class(true, 429, Some(2)).unwrap(),
            ProviderPrecommitMetricClass::RateLimited
        );
        assert_eq!(
            provider_bridge_rate_limit_header_prefix(3).unwrap(),
            "deepseek"
        );
        assert_eq!(
            provider_bridge_rate_limit_header_label(4).unwrap(),
            "Google Gemini"
        );
        assert_eq!(
            provider_bridge_chat_compatible_adapter_label(4).unwrap(),
            "Gemini OpenAI-compatible"
        );
        assert_eq!(
            provider_bridge_function_tool_name_max_bytes(2).unwrap(),
            128
        );
        assert_eq!(provider_bridge_function_tool_name_max_bytes(3).unwrap(), 64);
        assert!(provider_bridge_native_passthrough(2, 0, 1).unwrap());
        assert!(!provider_bridge_native_passthrough(2, 1, 0).unwrap());
        assert!(!provider_bridge_native_passthrough(3, 5, 0).unwrap());
        assert!(provider_bridge_native_passthrough(3, 2, 0).unwrap());
        assert!(provider_bridge_native_passthrough(3, 2, 2).unwrap());
        assert!(!provider_bridge_native_passthrough(3, 2, 1).unwrap());
        assert!(provider_bridge_native_passthrough(5, -1, -1).unwrap());
    }
}

const PROVIDER_RETRY_ABI_VERSION: i64 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderRetryScalarPlan {
    /// 0 allowed, 1 committed, 2 budget exhausted, 3 not retryable.
    pub decision: i64,
    pub remaining_precommit_retries: u8,
}

#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProviderPrecommitOperation {
    BufferedFallback = 0,
    LiveFallback = 1,
    Prefetch = 2,
    SseProgress = 3,
    HealthAction = 4,
    MetricClass = 5,
    NativeFirstPrefetch = 6,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderPrecommitLiveProgress {
    QuotaBlocked,
    RateLimited,
    Overloaded,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderPrecommitSseAction {
    None,
    QuotaBlocked,
    RateLimited,
    Overloaded,
    PreviousResponseNotFound,
    Commit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderPrecommitHealthAction {
    None,
    TransportFailure,
    Overload,
    Commit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderPrecommitMetricClass {
    Success,
    ProviderError,
    RateLimited,
    Overloaded,
    TransportError,
}

unsafe extern "C" {
    fn prodex_provider_bridge_label_v1(
        abi_version: i64,
        provider: i64,
        label_kind: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_provider_bridge_function_tool_name_max_bytes_v1(
        abi_version: i64,
        provider: i64,
    ) -> i64;
    fn prodex_provider_bridge_native_passthrough_v1(
        abi_version: i64,
        provider: i64,
        route_kind: i64,
        capability_status: i64,
    ) -> i64;
    fn prodex_provider_retry_transition_v1(
        abi_version: i64,
        error_class: i64,
        model_index: i64,
        model_count: i64,
        auth_index: i64,
        auth_count: i64,
        retry_enabled: i64,
    ) -> i64;
    fn prodex_provider_retry_plan_v1(
        abi_version: i64,
        max_precommit_attempts: i64,
        stage: i64,
        cause: i64,
        error_class: i64,
        attempted_precommit_retries: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_provider_precommit_policy_v1(
        abi_version: i64,
        operation: i64,
        a: i64,
        b: i64,
        c: i64,
        d: i64,
        e: i64,
        f: i64,
        output_address: u64,
    ) -> i64;
}

const PROVIDER_BRIDGE_KIND_COUNT: usize = 6;
const PROVIDER_BRIDGE_LABEL_MAX_BYTES: usize = 64;
const PROVIDER_BRIDGE_LABEL_RATE_LIMIT_PREFIX: i64 = 0;
const PROVIDER_BRIDGE_LABEL_RATE_LIMIT_HEADER: i64 = 1;
const PROVIDER_BRIDGE_LABEL_CHAT_ADAPTER: i64 = 2;

fn load_provider_bridge_label(provider: i64, label_kind: i64) -> Result<String, crate::MojoError> {
    let mut output = [0_u8; PROVIDER_BRIDGE_LABEL_MAX_BYTES];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_provider_bridge_label_v1(
            PROVIDER_RETRY_ABI_VERSION,
            provider,
            label_kind,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => {}
        1 => return Err(crate::MojoError::InvalidInput),
        2 => return Err(crate::MojoError::Capacity),
        4 => return Err(crate::MojoError::AbiMismatch),
        _ => return Err(crate::MojoError::InvalidOutput),
    }
    let written = usize::try_from(written).map_err(|_| crate::MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(crate::MojoError::InvalidOutput);
    }
    String::from_utf8(output[..written].to_vec()).map_err(|_| crate::MojoError::InvalidOutput)
}

fn cached_provider_bridge_label(
    cache: &'static std::sync::OnceLock<Result<Vec<String>, crate::MojoError>>,
    label_kind: i64,
    provider: i64,
) -> Result<&'static str, crate::MojoError> {
    let provider = usize::try_from(provider)
        .ok()
        .filter(|provider| *provider < PROVIDER_BRIDGE_KIND_COUNT)
        .ok_or(crate::MojoError::InvalidInput)?;
    match cache.get_or_init(|| {
        (0_i64..PROVIDER_BRIDGE_KIND_COUNT as i64)
            .map(|kind| load_provider_bridge_label(kind, label_kind))
            .collect()
    }) {
        Ok(labels) => labels
            .get(provider)
            .map(String::as_str)
            .ok_or(crate::MojoError::InvalidOutput),
        Err(error) => Err(*error),
    }
}

pub fn provider_bridge_rate_limit_header_prefix(
    provider: i64,
) -> Result<&'static str, crate::MojoError> {
    static LABELS: std::sync::OnceLock<Result<Vec<String>, crate::MojoError>> =
        std::sync::OnceLock::new();
    cached_provider_bridge_label(&LABELS, PROVIDER_BRIDGE_LABEL_RATE_LIMIT_PREFIX, provider)
}

pub fn provider_bridge_rate_limit_header_label(
    provider: i64,
) -> Result<&'static str, crate::MojoError> {
    static LABELS: std::sync::OnceLock<Result<Vec<String>, crate::MojoError>> =
        std::sync::OnceLock::new();
    cached_provider_bridge_label(&LABELS, PROVIDER_BRIDGE_LABEL_RATE_LIMIT_HEADER, provider)
}

pub fn provider_bridge_chat_compatible_adapter_label(
    provider: i64,
) -> Result<&'static str, crate::MojoError> {
    static LABELS: std::sync::OnceLock<Result<Vec<String>, crate::MojoError>> =
        std::sync::OnceLock::new();
    cached_provider_bridge_label(&LABELS, PROVIDER_BRIDGE_LABEL_CHAT_ADAPTER, provider)
}

pub fn provider_bridge_function_tool_name_max_bytes(
    provider: i64,
) -> Result<usize, crate::MojoError> {
    let value = unsafe {
        prodex_provider_bridge_function_tool_name_max_bytes_v1(PROVIDER_RETRY_ABI_VERSION, provider)
    };
    match value {
        -4 => Err(crate::MojoError::AbiMismatch),
        -1 => Err(crate::MojoError::InvalidInput),
        0.. => usize::try_from(value).map_err(|_| crate::MojoError::InvalidOutput),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}

pub fn provider_bridge_native_passthrough(
    provider: i64,
    route_kind: i64,
    capability_status: i64,
) -> Result<bool, crate::MojoError> {
    let value = unsafe {
        prodex_provider_bridge_native_passthrough_v1(
            PROVIDER_RETRY_ABI_VERSION,
            provider,
            route_kind,
            capability_status,
        )
    };
    match value {
        -4 => Err(crate::MojoError::AbiMismatch),
        -1 => Err(crate::MojoError::InvalidInput),
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}

pub fn provider_retry_transition(
    error_class: i64,
    model_index: usize,
    model_count: usize,
    auth_index: usize,
    auth_count: usize,
    retry_enabled: bool,
) -> Result<i64, crate::MojoError> {
    if !(0..=5).contains(&error_class) {
        return Err(crate::MojoError::InvalidInput);
    }
    let result = unsafe {
        prodex_provider_retry_transition_v1(
            PROVIDER_RETRY_ABI_VERSION,
            error_class,
            i64::try_from(model_index).map_err(|_| crate::MojoError::InvalidInput)?,
            i64::try_from(model_count).map_err(|_| crate::MojoError::InvalidInput)?,
            i64::try_from(auth_index).map_err(|_| crate::MojoError::InvalidInput)?,
            i64::try_from(auth_count).map_err(|_| crate::MojoError::InvalidInput)?,
            i64::from(retry_enabled),
        )
    };
    match result {
        0..=2 => Ok(result),
        -1 => Err(crate::MojoError::InvalidInput),
        -4 => Err(crate::MojoError::AbiMismatch),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}

pub fn provider_retry_plan(
    max_precommit_attempts: u8,
    stage: i64,
    cause: i64,
    error_class: i64,
    attempted_precommit_retries: u8,
) -> Result<ProviderRetryScalarPlan, crate::MojoError> {
    if !(0..=3).contains(&stage) || !(0..=2).contains(&cause) || !(0..=5).contains(&error_class) {
        return Err(crate::MojoError::InvalidInput);
    }
    let mut output = [-1_i64; 2];
    let status = unsafe {
        prodex_provider_retry_plan_v1(
            PROVIDER_RETRY_ABI_VERSION,
            i64::from(max_precommit_attempts),
            stage,
            cause,
            error_class,
            i64::from(attempted_precommit_retries),
            output.as_mut_ptr() as usize as u64,
        )
    };
    match status {
        0 => {}
        1 => return Err(crate::MojoError::InvalidInput),
        4 => return Err(crate::MojoError::AbiMismatch),
        _ => return Err(crate::MojoError::InvalidOutput),
    }
    if !(0..=3).contains(&output[0]) || !(0..=255).contains(&output[1]) {
        return Err(crate::MojoError::InvalidOutput);
    }
    Ok(ProviderRetryScalarPlan {
        decision: output[0],
        remaining_precommit_retries: u8::try_from(output[1])
            .map_err(|_| crate::MojoError::InvalidOutput)?,
    })
}

fn provider_precommit_policy(
    operation: ProviderPrecommitOperation,
    values: [i64; 6],
) -> Result<i64, crate::MojoError> {
    let mut output = -1_i64;
    let status = unsafe {
        prodex_provider_precommit_policy_v1(
            PROVIDER_RETRY_ABI_VERSION,
            operation as i64,
            values[0],
            values[1],
            values[2],
            values[3],
            values[4],
            values[5],
            (&mut output as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => Ok(output),
        1 => Err(crate::MojoError::InvalidInput),
        4 => Err(crate::MojoError::AbiMismatch),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}

pub fn provider_precommit_buffered_fallback_class(
    status: u16,
    error_class: i64,
    explicit_rate_limit_marker: bool,
) -> Result<Option<i64>, crate::MojoError> {
    let value = provider_precommit_policy(
        ProviderPrecommitOperation::BufferedFallback,
        [
            i64::from(status),
            error_class,
            i64::from(explicit_rate_limit_marker),
            0,
            0,
            0,
        ],
    )?;
    match value {
        -1 => Ok(None),
        0..=5 => Ok(Some(value)),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}

pub fn provider_precommit_live_fallback_class(
    progress: ProviderPrecommitLiveProgress,
    error_class: i64,
) -> Result<Option<i64>, crate::MojoError> {
    let progress = match progress {
        ProviderPrecommitLiveProgress::QuotaBlocked => 0,
        ProviderPrecommitLiveProgress::RateLimited => 1,
        ProviderPrecommitLiveProgress::Overloaded => 2,
        ProviderPrecommitLiveProgress::Other => 3,
    };
    let value = provider_precommit_policy(
        ProviderPrecommitOperation::LiveFallback,
        [progress, error_class, 0, 0, 0, 0],
    )?;
    match value {
        -1 => Ok(None),
        0..=5 => Ok(Some(value)),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn provider_precommit_should_prefetch(
    provider: i64,
    native_anthropic_messages: bool,
    responses_route: bool,
    status: u16,
    content_type_event_stream: bool,
    prefix_empty: bool,
) -> Result<bool, crate::MojoError> {
    match provider_precommit_policy(
        ProviderPrecommitOperation::Prefetch,
        [
            provider,
            i64::from(native_anthropic_messages),
            i64::from(responses_route),
            i64::from(status),
            i64::from(content_type_event_stream),
            i64::from(prefix_empty),
        ],
    )? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}

pub fn provider_precommit_native_first_should_prefetch(
    native_anthropic_messages: bool,
    status: u16,
    content_type_event_stream: bool,
    prefix_empty: bool,
) -> Result<bool, crate::MojoError> {
    match provider_precommit_policy(
        ProviderPrecommitOperation::NativeFirstPrefetch,
        [
            i64::from(native_anthropic_messages),
            i64::from(status),
            i64::from(content_type_event_stream),
            i64::from(prefix_empty),
            0,
            0,
        ],
    )? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}

pub fn provider_precommit_sse_action(
    quota_blocked: bool,
    rate_limited: bool,
    overloaded: bool,
    previous_response_not_found: bool,
    hold_event: bool,
) -> Result<ProviderPrecommitSseAction, crate::MojoError> {
    let flags = i64::from(quota_blocked)
        | (i64::from(rate_limited) << 1)
        | (i64::from(overloaded) << 2)
        | (i64::from(previous_response_not_found) << 3)
        | (i64::from(hold_event) << 4);
    match provider_precommit_policy(
        ProviderPrecommitOperation::SseProgress,
        [flags, 0, 0, 0, 0, 0],
    )? {
        0 => Ok(ProviderPrecommitSseAction::None),
        1 => Ok(ProviderPrecommitSseAction::QuotaBlocked),
        2 => Ok(ProviderPrecommitSseAction::RateLimited),
        3 => Ok(ProviderPrecommitSseAction::Overloaded),
        4 => Ok(ProviderPrecommitSseAction::PreviousResponseNotFound),
        5 => Ok(ProviderPrecommitSseAction::Commit),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}

pub fn provider_precommit_health_action(
    result_ok: bool,
    status: u16,
    fallback_class: Option<i64>,
) -> Result<ProviderPrecommitHealthAction, crate::MojoError> {
    match provider_precommit_policy(
        ProviderPrecommitOperation::HealthAction,
        [
            i64::from(result_ok),
            i64::from(status),
            fallback_class.unwrap_or(-1),
            0,
            0,
            0,
        ],
    )? {
        0 => Ok(ProviderPrecommitHealthAction::None),
        1 => Ok(ProviderPrecommitHealthAction::TransportFailure),
        2 => Ok(ProviderPrecommitHealthAction::Overload),
        3 => Ok(ProviderPrecommitHealthAction::Commit),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}

pub fn provider_precommit_metric_class(
    result_ok: bool,
    status: u16,
    fallback_class: Option<i64>,
) -> Result<ProviderPrecommitMetricClass, crate::MojoError> {
    match provider_precommit_policy(
        ProviderPrecommitOperation::MetricClass,
        [
            i64::from(result_ok),
            i64::from(status),
            fallback_class.unwrap_or(-1),
            0,
            0,
            0,
        ],
    )? {
        0 => Ok(ProviderPrecommitMetricClass::Success),
        1 => Ok(ProviderPrecommitMetricClass::ProviderError),
        2 => Ok(ProviderPrecommitMetricClass::RateLimited),
        3 => Ok(ProviderPrecommitMetricClass::Overloaded),
        4 => Ok(ProviderPrecommitMetricClass::TransportError),
        _ => Err(crate::MojoError::InvalidOutput),
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
