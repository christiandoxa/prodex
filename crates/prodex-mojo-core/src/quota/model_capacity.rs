//! OpenAI capacity-window projection into the existing Mojo planner.
use super::{
    QUOTA_MODEL_CAPACITY_OBSERVATION_ABI_VERSION, QUOTA_MODEL_KIND_LUNA, QUOTA_MODEL_KIND_NONE,
    QUOTA_MODEL_KIND_OTHER, QUOTA_MODEL_KIND_RETIRED_SPARK, QUOTA_MODEL_PAIR_DEFAULT,
    QUOTA_MODEL_PAIR_NONE, QUOTA_MODEL_PAIR_REGULAR, QUOTA_MODEL_PAIR_RESERVE,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenAiModelCapacityInput {
    pub model_kind: i64,
    pub regular_present: bool,
    pub regular_ready: bool,
    pub generic_ready: bool,
    pub reserve_ready: bool,
    pub regular_blocked: bool,
    pub any_unknown_window: bool,
    pub any_exhausted_window: bool,
    pub include_code_review: bool,
    pub code_review_ready: bool,
}

/// Raw window observations used by the Mojo model-capacity planner.
///
/// Rust owns provider-window acquisition; Mojo owns the deterministic
/// unknown/exhausted and code-review readiness classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OpenAiModelCapacityObservation {
    pub regular_primary_window_present: bool,
    pub regular_primary_used_percent: Option<i64>,
    pub regular_secondary_window_present: bool,
    pub regular_secondary_used_percent: Option<i64>,
    pub code_review_primary_used_percent: Option<i64>,
    pub code_review_secondary_used_percent: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenAiModelCapacityObservedInput {
    pub model_kind: i64,
    pub regular_present: bool,
    pub regular_ready: bool,
    pub generic_ready: bool,
    pub reserve_ready: bool,
    pub regular_blocked: bool,
    pub include_code_review: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenAiModelCapacityPlan {
    pub selected_pair: i64,
    pub ready: bool,
    pub supports: bool,
    pub unknown_luna_capacity: bool,
}

unsafe extern "C" {
    fn prodex_quota_openai_model_capacity_plan(fields_address: u64, output_address: u64) -> i64;
    fn prodex_quota_openai_model_capacity_plan_v2(
        abi_version: i64,
        fields_address: u64,
        output_address: u64,
    ) -> i64;
}

pub fn openai_model_capacity_plan(
    input: OpenAiModelCapacityInput,
) -> Result<OpenAiModelCapacityPlan, crate::MojoError> {
    if !matches!(
        input.model_kind,
        QUOTA_MODEL_KIND_NONE
            | QUOTA_MODEL_KIND_LUNA
            | QUOTA_MODEL_KIND_RETIRED_SPARK
            | QUOTA_MODEL_KIND_OTHER
    ) {
        return Err(crate::MojoError::InvalidInput);
    }
    let fields = [
        input.model_kind,
        i64::from(input.regular_present),
        i64::from(input.regular_ready),
        i64::from(input.generic_ready),
        i64::from(input.reserve_ready),
        i64::from(input.regular_blocked),
        i64::from(input.any_unknown_window),
        i64::from(input.any_exhausted_window),
        i64::from(input.include_code_review),
        i64::from(input.code_review_ready),
    ];
    let mut output = [0_i64; 4];
    let status = unsafe {
        prodex_quota_openai_model_capacity_plan(
            fields.as_ptr() as usize as u64,
            output.as_mut_ptr() as usize as u64,
        )
    };
    if status != 0
        || !matches!(
            output[0],
            QUOTA_MODEL_PAIR_NONE
                | QUOTA_MODEL_PAIR_REGULAR
                | QUOTA_MODEL_PAIR_RESERVE
                | QUOTA_MODEL_PAIR_DEFAULT
        )
        || !matches!((output[1], output[2], output[3]), (0 | 1, 0 | 1, 0 | 1))
    {
        return Err(if status == 1 {
            crate::MojoError::InvalidInput
        } else {
            crate::MojoError::InvalidOutput
        });
    }
    Ok(OpenAiModelCapacityPlan {
        selected_pair: output[0],
        ready: output[1] == 1,
        supports: output[2] == 1,
        unknown_luna_capacity: output[3] == 1,
    })
}

/// Plan OpenAI model capacity from provider observations in Mojo.
pub fn openai_model_capacity_plan_with_observations(
    input: OpenAiModelCapacityObservedInput,
    observation: OpenAiModelCapacityObservation,
) -> Result<OpenAiModelCapacityPlan, crate::MojoError> {
    if !matches!(
        input.model_kind,
        QUOTA_MODEL_KIND_NONE
            | QUOTA_MODEL_KIND_LUNA
            | QUOTA_MODEL_KIND_RETIRED_SPARK
            | QUOTA_MODEL_KIND_OTHER
    ) {
        return Err(crate::MojoError::InvalidInput);
    }
    let option_parts = |value: Option<i64>| (i64::from(value.is_some()), value.unwrap_or_default());
    let (regular_primary_used_present, regular_primary_used) =
        option_parts(observation.regular_primary_used_percent);
    let (regular_secondary_used_present, regular_secondary_used) =
        option_parts(observation.regular_secondary_used_percent);
    let (code_review_primary_used_present, code_review_primary_used) =
        option_parts(observation.code_review_primary_used_percent);
    let (code_review_secondary_used_present, code_review_secondary_used) =
        option_parts(observation.code_review_secondary_used_percent);
    let fields = [
        input.model_kind,
        i64::from(input.regular_present),
        i64::from(input.regular_ready),
        i64::from(input.generic_ready),
        i64::from(input.reserve_ready),
        i64::from(input.regular_blocked),
        i64::from(observation.regular_primary_window_present),
        regular_primary_used_present,
        regular_primary_used,
        i64::from(observation.regular_secondary_window_present),
        regular_secondary_used_present,
        regular_secondary_used,
        code_review_primary_used_present,
        code_review_primary_used,
        code_review_secondary_used_present,
        code_review_secondary_used,
        i64::from(input.include_code_review),
    ];
    let mut output = [0_i64; 4];
    let status = unsafe {
        prodex_quota_openai_model_capacity_plan_v2(
            QUOTA_MODEL_CAPACITY_OBSERVATION_ABI_VERSION,
            fields.as_ptr() as usize as u64,
            output.as_mut_ptr() as usize as u64,
        )
    };
    if status != 0
        || !matches!(
            output[0],
            QUOTA_MODEL_PAIR_NONE
                | QUOTA_MODEL_PAIR_REGULAR
                | QUOTA_MODEL_PAIR_RESERVE
                | QUOTA_MODEL_PAIR_DEFAULT
        )
        || !matches!((output[1], output[2], output[3]), (0 | 1, 0 | 1, 0 | 1))
    {
        return Err(match status {
            1 => crate::MojoError::InvalidInput,
            4 => crate::MojoError::AbiMismatch,
            _ => crate::MojoError::InvalidOutput,
        });
    }
    Ok(OpenAiModelCapacityPlan {
        selected_pair: output[0],
        ready: output[1] == 1,
        supports: output[2] == 1,
        unknown_luna_capacity: output[3] == 1,
    })
}
