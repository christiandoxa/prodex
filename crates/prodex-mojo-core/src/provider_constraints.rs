pub const PROVIDER_CONSTRAINT_COMPATIBLE: i64 = 0;
pub const PROVIDER_CONSTRAINT_ENDPOINT_UNSUPPORTED: i64 = 1;
pub const PROVIDER_CONSTRAINT_REQUIRED_CAPABILITY_MISSING: i64 = 2;
pub const PROVIDER_CONSTRAINT_CATALOG_UNAVAILABLE: i64 = 3;
pub const PROVIDER_CONSTRAINT_CONTEXT_UNKNOWN: i64 = 4;
pub const PROVIDER_CONSTRAINT_CONTEXT_EXCEEDED: i64 = 5;
pub const PROVIDER_CONSTRAINT_OUTPUT_UNKNOWN: i64 = 6;
pub const PROVIDER_CONSTRAINT_OUTPUT_EXCEEDS_LIMIT: i64 = 7;
pub const PROVIDER_CONSTRAINT_REASONING_UNSUPPORTED: i64 = 8;
pub const PROVIDER_CONSTRAINT_REASONING_EXCESSIVE: i64 = 9;
pub const PROVIDER_CONSTRAINT_MALFORMED_LIMITS: i64 = 10;
pub const PROVIDER_CONSTRAINT_OUTPUT_CLAMPED: i64 = 11;

pub const PROVIDER_CONSTRAINT_FEATURE_REASONING: i64 = 5;
pub const PROVIDER_CONSTRAINT_OUTPUT_FIELD_MAX_OUTPUT: i64 = 0;
pub const PROVIDER_CONSTRAINT_OUTPUT_FIELD_MAX_COMPLETION: i64 = 1;
pub const PROVIDER_CONSTRAINT_OUTPUT_FIELD_MAX_TOKENS: i64 = 2;
pub const PROVIDER_CONSTRAINT_UNKNOWN_ALLOW: i64 = 0;
pub const PROVIDER_CONSTRAINT_UNKNOWN_SAFE_WINDOW: i64 = 1;
pub const PROVIDER_CONSTRAINT_UNKNOWN_REJECT: i64 = 2;
pub const PROVIDER_CONSTRAINT_OVERSIZED_PASSTHROUGH: i64 = 0;
pub const PROVIDER_CONSTRAINT_OVERSIZED_REJECT: i64 = 1;
pub const PROVIDER_CONSTRAINT_OVERSIZED_CLAMP: i64 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Input {
    pub policy_enabled: bool,
    pub endpoint_supported: bool,
    pub catalog_entry_present: bool,
    pub embeddings_endpoint: bool,
    pub missing_feature: Option<i64>,
    pub reasoning_effort_unsupported: bool,
    pub estimated_input_tokens: u64,
    pub explicit_output_tokens: Option<u64>,
    pub default_output_reserve_tokens: Option<u64>,
    pub reasoning_reserve_tokens: Option<u64>,
    pub max_output_tokens: Option<u64>,
    pub context_window_tokens: Option<u64>,
    pub unknown_context_policy: i64,
    pub safe_window_tokens: u64,
    pub oversized_output_policy: i64,
    pub output_limit_field: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Evaluation {
    pub decision: i64,
    pub eligible: bool,
    pub missing_feature: Option<i64>,
    pub adjusted_output_tokens: Option<u64>,
    pub total_required_tokens: u64,
    pub available_context_tokens: Option<u64>,
    pub max_output_tokens: Option<u64>,
    pub warnings: u64,
    pub adjustment_field: Option<i64>,
    pub adjustment_reason: Option<i64>,
}

unsafe extern "C" {
    fn prodex_provider_constraints_evaluate(
        policy_enabled: i64,
        endpoint_supported: i64,
        catalog_entry_present: i64,
        embeddings_endpoint: i64,
        missing_feature_present: i64,
        missing_feature: i64,
        reasoning_effort_unsupported: i64,
        estimated_input_tokens: u64,
        explicit_output_tokens: u64,
        explicit_output_present: i64,
        default_output_reserve_tokens: u64,
        default_output_present: i64,
        reasoning_reserve_tokens: u64,
        reasoning_reserve_present: i64,
        max_output_tokens: u64,
        max_output_present: i64,
        context_window_tokens: u64,
        context_window_present: i64,
        unknown_context_policy: i64,
        safe_window_tokens: u64,
        oversized_output_policy: i64,
        output_limit_field: i64,
        output_limit_field_present: i64,
        decision: *mut i64,
        eligible: *mut i64,
        missing_feature_present: *mut i64,
        missing_feature: *mut i64,
        adjusted_output_tokens: *mut u64,
        adjusted_output_present: *mut i64,
        total_required_tokens: *mut u64,
        available_context_tokens: *mut u64,
        available_context_present: *mut i64,
        result_max_output_tokens: *mut u64,
        result_max_output_present: *mut i64,
        warnings: *mut u64,
        adjustment_field: *mut i64,
        adjustment_field_present: *mut i64,
        adjustment_reason: *mut i64,
        adjustment_reason_present: *mut i64,
    ) -> i64;
}

pub fn evaluate(input: Input) -> std::result::Result<Evaluation, crate::MojoError> {
    let mut decision = 0_i64;
    let mut eligible = 0_i64;
    let mut missing_feature_present = 0_i64;
    let mut missing_feature = 0_i64;
    let mut adjusted_output_tokens = 0_u64;
    let mut adjusted_output_present = 0_i64;
    let mut total_required_tokens = 0_u64;
    let mut available_context_tokens = 0_u64;
    let mut available_context_present = 0_i64;
    let mut max_output_tokens = 0_u64;
    let mut max_output_present = 0_i64;
    let mut warnings = 0_u64;
    let mut adjustment_field = 0_i64;
    let mut adjustment_field_present = 0_i64;
    let mut adjustment_reason = 0_i64;
    let mut adjustment_reason_present = 0_i64;
    let status = unsafe {
        prodex_provider_constraints_evaluate(
            i64::from(input.policy_enabled),
            i64::from(input.endpoint_supported),
            i64::from(input.catalog_entry_present),
            i64::from(input.embeddings_endpoint),
            i64::from(input.missing_feature.is_some()),
            input.missing_feature.unwrap_or(0),
            i64::from(input.reasoning_effort_unsupported),
            input.estimated_input_tokens,
            input.explicit_output_tokens.unwrap_or(0),
            i64::from(input.explicit_output_tokens.is_some()),
            input.default_output_reserve_tokens.unwrap_or(0),
            i64::from(input.default_output_reserve_tokens.is_some()),
            input.reasoning_reserve_tokens.unwrap_or(0),
            i64::from(input.reasoning_reserve_tokens.is_some()),
            input.max_output_tokens.unwrap_or(0),
            i64::from(input.max_output_tokens.is_some()),
            input.context_window_tokens.unwrap_or(0),
            i64::from(input.context_window_tokens.is_some()),
            input.unknown_context_policy,
            input.safe_window_tokens,
            input.oversized_output_policy,
            input.output_limit_field.unwrap_or(0),
            i64::from(input.output_limit_field.is_some()),
            &mut decision,
            &mut eligible,
            &mut missing_feature_present,
            &mut missing_feature,
            &mut adjusted_output_tokens,
            &mut adjusted_output_present,
            &mut total_required_tokens,
            &mut available_context_tokens,
            &mut available_context_present,
            &mut max_output_tokens,
            &mut max_output_present,
            &mut warnings,
            &mut adjustment_field,
            &mut adjustment_field_present,
            &mut adjustment_reason,
            &mut adjustment_reason_present,
        )
    };
    if status != 0
        || !matches!(eligible, 0 | 1)
        || !matches!(missing_feature_present, 0 | 1)
        || !matches!(adjusted_output_present, 0 | 1)
        || !matches!(available_context_present, 0 | 1)
        || !matches!(max_output_present, 0 | 1)
        || !matches!(adjustment_field_present, 0 | 1)
        || !matches!(adjustment_reason_present, 0 | 1)
        || !(PROVIDER_CONSTRAINT_COMPATIBLE..=PROVIDER_CONSTRAINT_OUTPUT_CLAMPED)
            .contains(&decision)
        || (missing_feature_present == 1 && !(0..=8).contains(&missing_feature))
        || warnings & !0x1fff != 0
    {
        return Err(crate::MojoError::InvalidOutput);
    }
    if (adjustment_field_present == 1 && !(0..=2).contains(&adjustment_field))
        || (adjustment_reason_present == 1
            && !(PROVIDER_CONSTRAINT_COMPATIBLE..=PROVIDER_CONSTRAINT_OUTPUT_CLAMPED)
                .contains(&adjustment_reason))
    {
        return Err(crate::MojoError::InvalidOutput);
    }
    Ok(Evaluation {
        decision,
        eligible: eligible == 1,
        missing_feature: (missing_feature_present == 1).then_some(missing_feature),
        adjusted_output_tokens: (adjusted_output_present == 1).then_some(adjusted_output_tokens),
        total_required_tokens,
        available_context_tokens: (available_context_present == 1)
            .then_some(available_context_tokens),
        max_output_tokens: (max_output_present == 1).then_some(max_output_tokens),
        warnings,
        adjustment_field: (adjustment_field_present == 1).then_some(adjustment_field),
        adjustment_reason: (adjustment_reason_present == 1).then_some(adjustment_reason),
    })
}

pub fn self_test() -> bool {
    evaluate(Input {
        policy_enabled: true,
        endpoint_supported: true,
        catalog_entry_present: true,
        embeddings_endpoint: false,
        missing_feature: None,
        reasoning_effort_unsupported: false,
        estimated_input_tokens: 10,
        explicit_output_tokens: Some(20),
        default_output_reserve_tokens: None,
        reasoning_reserve_tokens: None,
        max_output_tokens: Some(100),
        context_window_tokens: Some(100),
        unknown_context_policy: PROVIDER_CONSTRAINT_UNKNOWN_ALLOW,
        safe_window_tokens: 50,
        oversized_output_policy: PROVIDER_CONSTRAINT_OVERSIZED_PASSTHROUGH,
        output_limit_field: Some(PROVIDER_CONSTRAINT_OUTPUT_FIELD_MAX_OUTPUT),
    })
    .is_ok_and(|result| {
        result.decision == PROVIDER_CONSTRAINT_COMPATIBLE
            && result.eligible
            && result.total_required_tokens == 30
    })
}
