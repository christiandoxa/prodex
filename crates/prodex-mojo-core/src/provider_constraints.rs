pub const PROVIDER_CONSTRAINT_ABI_VERSION: i64 = 2;
pub const PROVIDER_CONSTRAINT_INPUT_I64_FIELD_COUNT: usize = 17;
pub const PROVIDER_CONSTRAINT_INPUT_U64_FIELD_COUNT: usize = 7;
pub const PROVIDER_CONSTRAINT_OUTPUT_I64_FIELD_COUNT: usize = 12;
pub const PROVIDER_CONSTRAINT_OUTPUT_U64_FIELD_COUNT: usize = 5;

pub const PROVIDER_CONSTRAINT_WARNING_CATALOG_UNAVAILABLE: u64 = 1 << 3;
pub const PROVIDER_CONSTRAINT_WARNING_CONTEXT_UNKNOWN: u64 = 1 << 4;
pub const PROVIDER_CONSTRAINT_WARNING_OUTPUT_UNKNOWN: u64 = 1 << 6;
pub const PROVIDER_CONSTRAINT_WARNING_OUTPUT_EXCEEDS_LIMIT: u64 = 1 << 7;
const PROVIDER_CONSTRAINT_WARNING_MASK: u64 = PROVIDER_CONSTRAINT_WARNING_CATALOG_UNAVAILABLE
    | PROVIDER_CONSTRAINT_WARNING_CONTEXT_UNKNOWN
    | PROVIDER_CONSTRAINT_WARNING_OUTPUT_UNKNOWN
    | PROVIDER_CONSTRAINT_WARNING_OUTPUT_EXCEEDS_LIMIT;

const ABI_STATUS_MISMATCH: i64 = 1;
const ABI_STATUS_INVALID_INPUT: i64 = 2;

const INPUT_I64_ABI_VERSION: usize = 0;
const INPUT_I64_POLICY_ENABLED: usize = 1;
const INPUT_I64_ENDPOINT_SUPPORTED: usize = 2;
const INPUT_I64_CATALOG_ENTRY_PRESENT: usize = 3;
const INPUT_I64_EMBEDDINGS_ENDPOINT: usize = 4;
const INPUT_I64_MISSING_FEATURE_PRESENT: usize = 5;
const INPUT_I64_MISSING_FEATURE: usize = 6;
const INPUT_I64_REASONING_UNSUPPORTED: usize = 7;
const INPUT_I64_EXPLICIT_OUTPUT_PRESENT: usize = 8;
const INPUT_I64_DEFAULT_OUTPUT_PRESENT: usize = 9;
const INPUT_I64_REASONING_RESERVE_PRESENT: usize = 10;
const INPUT_I64_MAX_OUTPUT_PRESENT: usize = 11;
const INPUT_I64_CONTEXT_WINDOW_PRESENT: usize = 12;
const INPUT_I64_UNKNOWN_CONTEXT_POLICY: usize = 13;
const INPUT_I64_OVERSIZED_OUTPUT_POLICY: usize = 14;
const INPUT_I64_OUTPUT_LIMIT_FIELD: usize = 15;
const INPUT_I64_OUTPUT_LIMIT_FIELD_PRESENT: usize = 16;

const INPUT_U64_ESTIMATED_INPUT_TOKENS: usize = 0;
const INPUT_U64_EXPLICIT_OUTPUT_TOKENS: usize = 1;
const INPUT_U64_DEFAULT_OUTPUT_RESERVE_TOKENS: usize = 2;
const INPUT_U64_REASONING_RESERVE_TOKENS: usize = 3;
const INPUT_U64_MAX_OUTPUT_TOKENS: usize = 4;
const INPUT_U64_CONTEXT_WINDOW_TOKENS: usize = 5;
const INPUT_U64_SAFE_WINDOW_TOKENS: usize = 6;

const OUTPUT_I64_ABI_VERSION: usize = 0;
const OUTPUT_I64_DECISION: usize = 1;
const OUTPUT_I64_ELIGIBLE: usize = 2;
const OUTPUT_I64_MISSING_FEATURE_PRESENT: usize = 3;
const OUTPUT_I64_MISSING_FEATURE: usize = 4;
const OUTPUT_I64_ADJUSTED_OUTPUT_PRESENT: usize = 5;
const OUTPUT_I64_AVAILABLE_CONTEXT_PRESENT: usize = 6;
const OUTPUT_I64_MAX_OUTPUT_PRESENT: usize = 7;
const OUTPUT_I64_ADJUSTMENT_FIELD: usize = 8;
const OUTPUT_I64_ADJUSTMENT_FIELD_PRESENT: usize = 9;
const OUTPUT_I64_ADJUSTMENT_REASON: usize = 10;
const OUTPUT_I64_ADJUSTMENT_REASON_PRESENT: usize = 11;

const OUTPUT_U64_ADJUSTED_OUTPUT_TOKENS: usize = 0;
const OUTPUT_U64_TOTAL_REQUIRED_TOKENS: usize = 1;
const OUTPUT_U64_AVAILABLE_CONTEXT_TOKENS: usize = 2;
const OUTPUT_U64_MAX_OUTPUT_TOKENS: usize = 3;
const OUTPUT_U64_WARNINGS: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum Decision {
    Compatible = 0,
    EndpointUnsupported = 1,
    RequiredCapabilityMissing = 2,
    CatalogUnavailable = 3,
    ContextUnknown = 4,
    ContextExceeded = 5,
    OutputUnknown = 6,
    OutputExceedsLimit = 7,
    ReasoningUnsupported = 8,
    ReasoningExcessive = 9,
    MalformedLimits = 10,
    OutputClamped = 11,
}

impl TryFrom<i64> for Decision {
    type Error = crate::MojoError;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => Self::Compatible,
            1 => Self::EndpointUnsupported,
            2 => Self::RequiredCapabilityMissing,
            3 => Self::CatalogUnavailable,
            4 => Self::ContextUnknown,
            5 => Self::ContextExceeded,
            6 => Self::OutputUnknown,
            7 => Self::OutputExceedsLimit,
            8 => Self::ReasoningUnsupported,
            9 => Self::ReasoningExcessive,
            10 => Self::MalformedLimits,
            11 => Self::OutputClamped,
            _ => return Err(crate::MojoError::InvalidOutput),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum Feature {
    Tools = 0,
    JsonSchema = 1,
    Vision = 2,
    Audio = 3,
    WebSearch = 4,
    Reasoning = 5,
    Streaming = 6,
    Compact = 7,
    Websocket = 8,
}

impl TryFrom<i64> for Feature {
    type Error = crate::MojoError;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => Self::Tools,
            1 => Self::JsonSchema,
            2 => Self::Vision,
            3 => Self::Audio,
            4 => Self::WebSearch,
            5 => Self::Reasoning,
            6 => Self::Streaming,
            7 => Self::Compact,
            8 => Self::Websocket,
            _ => return Err(crate::MojoError::InvalidOutput),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum UnknownContextPolicy {
    Allow = 0,
    SafeWindow = 1,
    Reject = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum OversizedOutputPolicy {
    Passthrough = 0,
    Reject = 1,
    Clamp = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum OutputLimitField {
    MaxOutput = 0,
    MaxCompletion = 1,
    MaxTokens = 2,
}

impl TryFrom<i64> for OutputLimitField {
    type Error = crate::MojoError;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => Self::MaxOutput,
            1 => Self::MaxCompletion,
            2 => Self::MaxTokens,
            _ => return Err(crate::MojoError::InvalidOutput),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Input {
    pub policy_enabled: bool,
    pub endpoint_supported: bool,
    pub catalog_entry_present: bool,
    pub embeddings_endpoint: bool,
    pub missing_feature: Option<Feature>,
    pub reasoning_effort_unsupported: bool,
    pub estimated_input_tokens: u64,
    pub explicit_output_tokens: Option<u64>,
    pub default_output_reserve_tokens: Option<u64>,
    pub reasoning_reserve_tokens: Option<u64>,
    pub max_output_tokens: Option<u64>,
    pub context_window_tokens: Option<u64>,
    pub unknown_context_policy: UnknownContextPolicy,
    pub safe_window_tokens: u64,
    pub oversized_output_policy: OversizedOutputPolicy,
    pub output_limit_field: Option<OutputLimitField>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Evaluation {
    pub decision: Decision,
    pub eligible: bool,
    pub missing_feature: Option<Feature>,
    pub adjusted_output_tokens: Option<u64>,
    pub total_required_tokens: u64,
    pub available_context_tokens: Option<u64>,
    pub max_output_tokens: Option<u64>,
    pub warnings: u64,
    pub adjustment_field: Option<OutputLimitField>,
    pub adjustment_reason: Option<Decision>,
}

unsafe extern "C" {
    fn prodex_provider_constraints_evaluate_v2(
        input_i64: *const i64,
        input_i64_count: i64,
        input_u64: *const u64,
        input_u64_count: i64,
        output_i64: *mut i64,
        output_i64_count: i64,
        output_u64: *mut u64,
        output_u64_count: i64,
    ) -> i64;
}

fn encode_input(
    input: Input,
) -> (
    [i64; PROVIDER_CONSTRAINT_INPUT_I64_FIELD_COUNT],
    [u64; PROVIDER_CONSTRAINT_INPUT_U64_FIELD_COUNT],
) {
    let mut input_i64 = [0_i64; PROVIDER_CONSTRAINT_INPUT_I64_FIELD_COUNT];
    input_i64[INPUT_I64_ABI_VERSION] = PROVIDER_CONSTRAINT_ABI_VERSION;
    input_i64[INPUT_I64_POLICY_ENABLED] = i64::from(input.policy_enabled);
    input_i64[INPUT_I64_ENDPOINT_SUPPORTED] = i64::from(input.endpoint_supported);
    input_i64[INPUT_I64_CATALOG_ENTRY_PRESENT] = i64::from(input.catalog_entry_present);
    input_i64[INPUT_I64_EMBEDDINGS_ENDPOINT] = i64::from(input.embeddings_endpoint);
    input_i64[INPUT_I64_MISSING_FEATURE_PRESENT] = i64::from(input.missing_feature.is_some());
    input_i64[INPUT_I64_MISSING_FEATURE] =
        input.missing_feature.map_or(0, |feature| feature as i64);
    input_i64[INPUT_I64_REASONING_UNSUPPORTED] = i64::from(input.reasoning_effort_unsupported);
    input_i64[INPUT_I64_EXPLICIT_OUTPUT_PRESENT] =
        i64::from(input.explicit_output_tokens.is_some());
    input_i64[INPUT_I64_DEFAULT_OUTPUT_PRESENT] =
        i64::from(input.default_output_reserve_tokens.is_some());
    input_i64[INPUT_I64_REASONING_RESERVE_PRESENT] =
        i64::from(input.reasoning_reserve_tokens.is_some());
    input_i64[INPUT_I64_MAX_OUTPUT_PRESENT] = i64::from(input.max_output_tokens.is_some());
    input_i64[INPUT_I64_CONTEXT_WINDOW_PRESENT] = i64::from(input.context_window_tokens.is_some());
    input_i64[INPUT_I64_UNKNOWN_CONTEXT_POLICY] = input.unknown_context_policy as i64;
    input_i64[INPUT_I64_OVERSIZED_OUTPUT_POLICY] = input.oversized_output_policy as i64;
    input_i64[INPUT_I64_OUTPUT_LIMIT_FIELD_PRESENT] = i64::from(input.output_limit_field.is_some());
    input_i64[INPUT_I64_OUTPUT_LIMIT_FIELD] =
        input.output_limit_field.map_or(0, |field| field as i64);

    let mut input_u64 = [0_u64; PROVIDER_CONSTRAINT_INPUT_U64_FIELD_COUNT];
    input_u64[INPUT_U64_ESTIMATED_INPUT_TOKENS] = input.estimated_input_tokens;
    input_u64[INPUT_U64_EXPLICIT_OUTPUT_TOKENS] = input.explicit_output_tokens.unwrap_or(0);
    input_u64[INPUT_U64_DEFAULT_OUTPUT_RESERVE_TOKENS] =
        input.default_output_reserve_tokens.unwrap_or(0);
    input_u64[INPUT_U64_REASONING_RESERVE_TOKENS] = input.reasoning_reserve_tokens.unwrap_or(0);
    input_u64[INPUT_U64_MAX_OUTPUT_TOKENS] = input.max_output_tokens.unwrap_or(0);
    input_u64[INPUT_U64_CONTEXT_WINDOW_TOKENS] = input.context_window_tokens.unwrap_or(0);
    input_u64[INPUT_U64_SAFE_WINDOW_TOKENS] = input.safe_window_tokens;
    (input_i64, input_u64)
}

fn decode_flag(value: i64) -> Result<bool, crate::MojoError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}

fn decode_optional_u64(present: i64, value: u64) -> Result<Option<u64>, crate::MojoError> {
    if decode_flag(present)? {
        Ok(Some(value))
    } else if value == 0 {
        Ok(None)
    } else {
        Err(crate::MojoError::InvalidOutput)
    }
}

fn decode_output(
    output_i64: &[i64; PROVIDER_CONSTRAINT_OUTPUT_I64_FIELD_COUNT],
    output_u64: &[u64; PROVIDER_CONSTRAINT_OUTPUT_U64_FIELD_COUNT],
) -> Result<Evaluation, crate::MojoError> {
    if output_i64[OUTPUT_I64_ABI_VERSION] != PROVIDER_CONSTRAINT_ABI_VERSION {
        return Err(crate::MojoError::AbiMismatch);
    }

    let decision = Decision::try_from(output_i64[OUTPUT_I64_DECISION])?;
    if decision == Decision::MalformedLimits {
        return Err(crate::MojoError::InvalidOutput);
    }
    let eligible = decode_flag(output_i64[OUTPUT_I64_ELIGIBLE])?;

    let missing_feature = if decode_flag(output_i64[OUTPUT_I64_MISSING_FEATURE_PRESENT])? {
        Some(Feature::try_from(output_i64[OUTPUT_I64_MISSING_FEATURE])?)
    } else if output_i64[OUTPUT_I64_MISSING_FEATURE] == 0 {
        None
    } else {
        return Err(crate::MojoError::InvalidOutput);
    };
    let adjusted_output_tokens = decode_optional_u64(
        output_i64[OUTPUT_I64_ADJUSTED_OUTPUT_PRESENT],
        output_u64[OUTPUT_U64_ADJUSTED_OUTPUT_TOKENS],
    )?;
    let available_context_tokens = decode_optional_u64(
        output_i64[OUTPUT_I64_AVAILABLE_CONTEXT_PRESENT],
        output_u64[OUTPUT_U64_AVAILABLE_CONTEXT_TOKENS],
    )?;
    let max_output_tokens = decode_optional_u64(
        output_i64[OUTPUT_I64_MAX_OUTPUT_PRESENT],
        output_u64[OUTPUT_U64_MAX_OUTPUT_TOKENS],
    )?;

    let adjustment_field = if decode_flag(output_i64[OUTPUT_I64_ADJUSTMENT_FIELD_PRESENT])? {
        Some(OutputLimitField::try_from(
            output_i64[OUTPUT_I64_ADJUSTMENT_FIELD],
        )?)
    } else if output_i64[OUTPUT_I64_ADJUSTMENT_FIELD] == 0 {
        None
    } else {
        return Err(crate::MojoError::InvalidOutput);
    };
    let adjustment_reason = if decode_flag(output_i64[OUTPUT_I64_ADJUSTMENT_REASON_PRESENT])? {
        Some(Decision::try_from(
            output_i64[OUTPUT_I64_ADJUSTMENT_REASON],
        )?)
    } else if output_i64[OUTPUT_I64_ADJUSTMENT_REASON] == 0 {
        None
    } else {
        return Err(crate::MojoError::InvalidOutput);
    };

    let adjusted = adjusted_output_tokens.is_some();
    if adjusted != adjustment_field.is_some()
        || adjusted != adjustment_reason.is_some()
        || adjusted && adjustment_reason != Some(Decision::OutputClamped)
        || !adjusted && decision == Decision::OutputClamped
    {
        return Err(crate::MojoError::InvalidOutput);
    }

    let eligibility_is_valid = match decision {
        Decision::Compatible | Decision::OutputUnknown | Decision::OutputClamped => eligible,
        Decision::EndpointUnsupported
        | Decision::RequiredCapabilityMissing
        | Decision::ContextExceeded
        | Decision::ReasoningUnsupported
        | Decision::ReasoningExcessive => !eligible,
        Decision::CatalogUnavailable | Decision::ContextUnknown | Decision::OutputExceedsLimit => {
            true
        }
        Decision::MalformedLimits => false,
    };
    let warnings = output_u64[OUTPUT_U64_WARNINGS];
    if !eligibility_is_valid || warnings & !PROVIDER_CONSTRAINT_WARNING_MASK != 0 {
        return Err(crate::MojoError::InvalidOutput);
    }

    Ok(Evaluation {
        decision,
        eligible,
        missing_feature,
        adjusted_output_tokens,
        total_required_tokens: output_u64[OUTPUT_U64_TOTAL_REQUIRED_TOKENS],
        available_context_tokens,
        max_output_tokens,
        warnings,
        adjustment_field,
        adjustment_reason,
    })
}

pub fn evaluate(input: Input) -> Result<Evaluation, crate::MojoError> {
    let (input_i64, input_u64) = encode_input(input);
    let mut output_i64 = [0_i64; PROVIDER_CONSTRAINT_OUTPUT_I64_FIELD_COUNT];
    let mut output_u64 = [0_u64; PROVIDER_CONSTRAINT_OUTPUT_U64_FIELD_COUNT];
    let status = unsafe {
        prodex_provider_constraints_evaluate_v2(
            input_i64.as_ptr(),
            i64::try_from(input_i64.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            input_u64.as_ptr(),
            i64::try_from(input_u64.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            output_i64.as_mut_ptr(),
            i64::try_from(output_i64.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            output_u64.as_mut_ptr(),
            i64::try_from(output_u64.len()).map_err(|_| crate::MojoError::InvalidInput)?,
        )
    };
    match status {
        0 => decode_output(&output_i64, &output_u64),
        ABI_STATUS_MISMATCH => Err(crate::MojoError::AbiMismatch),
        ABI_STATUS_INVALID_INPUT => Err(crate::MojoError::InvalidInput),
        _ => Err(crate::MojoError::InvalidOutput),
    }
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
        unknown_context_policy: UnknownContextPolicy::Allow,
        safe_window_tokens: 50,
        oversized_output_policy: OversizedOutputPolicy::Passthrough,
        output_limit_field: Some(OutputLimitField::MaxOutput),
    })
    .is_ok_and(|result| {
        result.decision == Decision::Compatible
            && result.eligible
            && result.total_required_tokens == 30
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> Input {
        Input {
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
            unknown_context_policy: UnknownContextPolicy::Allow,
            safe_window_tokens: 50,
            oversized_output_policy: OversizedOutputPolicy::Passthrough,
            output_limit_field: Some(OutputLimitField::MaxOutput),
        }
    }

    fn raw_call(
        input_i64: &[i64; PROVIDER_CONSTRAINT_INPUT_I64_FIELD_COUNT],
        input_i64_count: i64,
        input_u64: &[u64; PROVIDER_CONSTRAINT_INPUT_U64_FIELD_COUNT],
        input_u64_count: i64,
        output_i64_count: i64,
        output_u64_count: i64,
    ) -> i64 {
        let mut output_i64 = [0_i64; PROVIDER_CONSTRAINT_OUTPUT_I64_FIELD_COUNT];
        let mut output_u64 = [0_u64; PROVIDER_CONSTRAINT_OUTPUT_U64_FIELD_COUNT];
        unsafe {
            prodex_provider_constraints_evaluate_v2(
                input_i64.as_ptr(),
                input_i64_count,
                input_u64.as_ptr(),
                input_u64_count,
                output_i64.as_mut_ptr(),
                output_i64_count,
                output_u64.as_mut_ptr(),
                output_u64_count,
            )
        }
    }

    #[test]
    fn abi_schema_indices_are_contiguous() {
        assert_eq!(
            [
                INPUT_I64_ABI_VERSION,
                INPUT_I64_POLICY_ENABLED,
                INPUT_I64_ENDPOINT_SUPPORTED,
                INPUT_I64_CATALOG_ENTRY_PRESENT,
                INPUT_I64_EMBEDDINGS_ENDPOINT,
                INPUT_I64_MISSING_FEATURE_PRESENT,
                INPUT_I64_MISSING_FEATURE,
                INPUT_I64_REASONING_UNSUPPORTED,
                INPUT_I64_EXPLICIT_OUTPUT_PRESENT,
                INPUT_I64_DEFAULT_OUTPUT_PRESENT,
                INPUT_I64_REASONING_RESERVE_PRESENT,
                INPUT_I64_MAX_OUTPUT_PRESENT,
                INPUT_I64_CONTEXT_WINDOW_PRESENT,
                INPUT_I64_UNKNOWN_CONTEXT_POLICY,
                INPUT_I64_OVERSIZED_OUTPUT_POLICY,
                INPUT_I64_OUTPUT_LIMIT_FIELD,
                INPUT_I64_OUTPUT_LIMIT_FIELD_PRESENT,
            ],
            std::array::from_fn::<_, PROVIDER_CONSTRAINT_INPUT_I64_FIELD_COUNT, _>(|index| index)
        );
        assert_eq!(
            [
                INPUT_U64_ESTIMATED_INPUT_TOKENS,
                INPUT_U64_EXPLICIT_OUTPUT_TOKENS,
                INPUT_U64_DEFAULT_OUTPUT_RESERVE_TOKENS,
                INPUT_U64_REASONING_RESERVE_TOKENS,
                INPUT_U64_MAX_OUTPUT_TOKENS,
                INPUT_U64_CONTEXT_WINDOW_TOKENS,
                INPUT_U64_SAFE_WINDOW_TOKENS,
            ],
            std::array::from_fn::<_, PROVIDER_CONSTRAINT_INPUT_U64_FIELD_COUNT, _>(|index| index)
        );
        assert_eq!(
            [
                OUTPUT_I64_ABI_VERSION,
                OUTPUT_I64_DECISION,
                OUTPUT_I64_ELIGIBLE,
                OUTPUT_I64_MISSING_FEATURE_PRESENT,
                OUTPUT_I64_MISSING_FEATURE,
                OUTPUT_I64_ADJUSTED_OUTPUT_PRESENT,
                OUTPUT_I64_AVAILABLE_CONTEXT_PRESENT,
                OUTPUT_I64_MAX_OUTPUT_PRESENT,
                OUTPUT_I64_ADJUSTMENT_FIELD,
                OUTPUT_I64_ADJUSTMENT_FIELD_PRESENT,
                OUTPUT_I64_ADJUSTMENT_REASON,
                OUTPUT_I64_ADJUSTMENT_REASON_PRESENT,
            ],
            std::array::from_fn::<_, PROVIDER_CONSTRAINT_OUTPUT_I64_FIELD_COUNT, _>(|index| index)
        );
        assert_eq!(
            [
                OUTPUT_U64_ADJUSTED_OUTPUT_TOKENS,
                OUTPUT_U64_TOTAL_REQUIRED_TOKENS,
                OUTPUT_U64_AVAILABLE_CONTEXT_TOKENS,
                OUTPUT_U64_MAX_OUTPUT_TOKENS,
                OUTPUT_U64_WARNINGS,
            ],
            std::array::from_fn::<_, PROVIDER_CONSTRAINT_OUTPUT_U64_FIELD_COUNT, _>(|index| index)
        );
    }

    #[test]
    fn raw_abi_rejects_schema_and_semantic_mismatches() {
        let (mut input_i64, input_u64) = encode_input(input());
        let input_i64_count = i64::try_from(input_i64.len()).unwrap();
        let input_u64_count = i64::try_from(input_u64.len()).unwrap();
        let output_i64_count = i64::try_from(PROVIDER_CONSTRAINT_OUTPUT_I64_FIELD_COUNT).unwrap();
        let output_u64_count = i64::try_from(PROVIDER_CONSTRAINT_OUTPUT_U64_FIELD_COUNT).unwrap();

        assert_eq!(
            raw_call(
                &input_i64,
                input_i64_count - 1,
                &input_u64,
                input_u64_count,
                output_i64_count,
                output_u64_count,
            ),
            ABI_STATUS_MISMATCH
        );
        assert_eq!(
            raw_call(
                &input_i64,
                input_i64_count,
                &input_u64,
                input_u64_count - 1,
                output_i64_count,
                output_u64_count,
            ),
            ABI_STATUS_MISMATCH
        );
        assert_eq!(
            raw_call(
                &input_i64,
                input_i64_count,
                &input_u64,
                input_u64_count,
                output_i64_count - 1,
                output_u64_count,
            ),
            ABI_STATUS_MISMATCH
        );
        assert_eq!(
            raw_call(
                &input_i64,
                input_i64_count,
                &input_u64,
                input_u64_count,
                output_i64_count,
                output_u64_count - 1,
            ),
            ABI_STATUS_MISMATCH
        );

        input_i64[INPUT_I64_ABI_VERSION] += 1;
        assert_eq!(
            raw_call(
                &input_i64,
                input_i64_count,
                &input_u64,
                input_u64_count,
                output_i64_count,
                output_u64_count,
            ),
            ABI_STATUS_MISMATCH
        );
        input_i64[INPUT_I64_ABI_VERSION] = PROVIDER_CONSTRAINT_ABI_VERSION;

        input_i64[INPUT_I64_UNKNOWN_CONTEXT_POLICY] = 99;
        assert_eq!(
            raw_call(
                &input_i64,
                input_i64_count,
                &input_u64,
                input_u64_count,
                output_i64_count,
                output_u64_count,
            ),
            ABI_STATUS_INVALID_INPUT
        );
        input_i64[INPUT_I64_UNKNOWN_CONTEXT_POLICY] = UnknownContextPolicy::Allow as i64;

        input_i64[INPUT_I64_MISSING_FEATURE_PRESENT] = 1;
        input_i64[INPUT_I64_MISSING_FEATURE] = 99;
        assert_eq!(
            raw_call(
                &input_i64,
                input_i64_count,
                &input_u64,
                input_u64_count,
                output_i64_count,
                output_u64_count,
            ),
            ABI_STATUS_INVALID_INPUT
        );
        input_i64[INPUT_I64_MISSING_FEATURE_PRESENT] = 0;
        input_i64[INPUT_I64_MISSING_FEATURE] = 0;

        input_i64[INPUT_I64_EXPLICIT_OUTPUT_PRESENT] = 0;
        assert_ne!(input_u64[INPUT_U64_EXPLICIT_OUTPUT_TOKENS], 0);
        assert_eq!(
            raw_call(
                &input_i64,
                input_i64_count,
                &input_u64,
                input_u64_count,
                output_i64_count,
                output_u64_count,
            ),
            ABI_STATUS_INVALID_INPUT
        );
    }

    #[test]
    fn decoder_rejects_unknown_and_impossible_outputs() {
        let mut output_i64 = [0_i64; PROVIDER_CONSTRAINT_OUTPUT_I64_FIELD_COUNT];
        let mut output_u64 = [0_u64; PROVIDER_CONSTRAINT_OUTPUT_U64_FIELD_COUNT];
        output_i64[OUTPUT_I64_ABI_VERSION] = PROVIDER_CONSTRAINT_ABI_VERSION;
        output_i64[OUTPUT_I64_DECISION] = Decision::Compatible as i64;
        output_i64[OUTPUT_I64_ELIGIBLE] = 1;
        assert!(decode_output(&output_i64, &output_u64).is_ok());

        output_i64[OUTPUT_I64_ABI_VERSION] += 1;
        assert_eq!(
            decode_output(&output_i64, &output_u64),
            Err(crate::MojoError::AbiMismatch)
        );
        output_i64[OUTPUT_I64_ABI_VERSION] = PROVIDER_CONSTRAINT_ABI_VERSION;

        output_i64[OUTPUT_I64_DECISION] = 99;
        assert_eq!(
            decode_output(&output_i64, &output_u64),
            Err(crate::MojoError::InvalidOutput)
        );
        output_i64[OUTPUT_I64_DECISION] = Decision::Compatible as i64;

        output_u64[OUTPUT_U64_WARNINGS] = 1;
        assert_eq!(
            decode_output(&output_i64, &output_u64),
            Err(crate::MojoError::InvalidOutput)
        );
        output_u64[OUTPUT_U64_WARNINGS] = 0;

        output_u64[OUTPUT_U64_ADJUSTED_OUTPUT_TOKENS] = 10;
        assert_eq!(
            decode_output(&output_i64, &output_u64),
            Err(crate::MojoError::InvalidOutput)
        );
        output_u64[OUTPUT_U64_ADJUSTED_OUTPUT_TOKENS] = 0;

        output_i64[OUTPUT_I64_DECISION] = Decision::OutputClamped as i64;
        assert_eq!(
            decode_output(&output_i64, &output_u64),
            Err(crate::MojoError::InvalidOutput)
        );
    }
}
