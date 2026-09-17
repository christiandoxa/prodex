use super::Feature;

const COMBO_INPUT_I64_FIELD_COUNT: usize = 5;
const COMBO_INPUT_U64_FIELD_COUNT: usize = 5;
const COMBO_OUTPUT_I64_FIELD_COUNT: usize = 2;
const COMBO_OUTPUT_U64_FIELD_COUNT: usize = 3;
const COMBO_MAX_CANDIDATES: usize = 256;

/// Candidate inputs for cross-model output-limit normalization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComboOutputAdjustmentInput {
    pub eligible: bool,
    pub output_limit_field: Option<super::OutputLimitField>,
    pub adjustment_requested_tokens: Option<u64>,
    pub adjustment_applied_tokens: Option<u64>,
    pub explicit_output_tokens: Option<u64>,
    pub estimated_input_tokens: u64,
    pub reasoning_reserve_tokens: Option<u64>,
}

/// Normalized output-limit decision for one candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComboOutputAdjustment {
    pub field: super::OutputLimitField,
    pub requested_tokens: u64,
    pub applied_tokens: u64,
    pub total_required_tokens: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequirementResolutionInput {
    pub explicit_output_present: bool,
    pub default_output_reserve_tokens: Option<u64>,
    pub requested_reasoning_effort: Option<i64>,
    pub default_reasoning_effort: Option<i64>,
    pub reasoning_reserve_tokens: Option<u64>,
    pub reasoning_reserve_by_effort: [Option<u64>; 9],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequirementResolution {
    pub default_output_reserve_tokens: Option<u64>,
    pub reasoning_effort: Option<i64>,
    pub reasoning_reserve_tokens: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreclassificationInput {
    pub endpoint_kind: i64,
    pub provider_endpoint_supported: bool,
    pub catalog_entry_present: bool,
    pub provider_streaming_supported: bool,
    pub supported_endpoint_mask: u64,
    pub feature_mask: u64,
    pub required_features: Vec<Feature>,
    pub reasoning_effort: Option<i64>,
    pub supported_reasoning_efforts: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Preclassification {
    pub endpoint_supported: bool,
    pub missing_feature: Option<Feature>,
    pub reasoning_effort_unsupported: bool,
}

unsafe extern "C" {
    fn prodex_provider_constraints_resolve_v1(
        explicit_output_present: i64,
        default_output_present: i64,
        default_output_reserve_tokens: u64,
        requested_reasoning_effort: i64,
        default_reasoning_effort: i64,
        reasoning_reserve_present: i64,
        reasoning_reserve_tokens: u64,
        reasoning_reserve_by_effort: *const u64,
        reasoning_reserve_mask: u64,
        output_default_output_present: *mut i64,
        output_default_output_reserve_tokens: *mut u64,
        output_reasoning_effort_present: *mut i64,
        output_reasoning_effort: *mut i64,
        output_reasoning_reserve_present: *mut i64,
        output_reasoning_reserve_tokens: *mut u64,
    ) -> i64;

    fn prodex_provider_constraints_preclassify_v1(
        endpoint_kind: i64,
        provider_endpoint_supported: i64,
        catalog_entry_present: i64,
        provider_streaming_supported: i64,
        supported_endpoint_mask: u64,
        feature_mask: u64,
        required_features: *const i64,
        required_feature_count: i64,
        reasoning_effort: i64,
        supported_reasoning_efforts_present: i64,
        supported_reasoning_efforts: u64,
        endpoint_supported: *mut i64,
        missing_feature_present: *mut i64,
        missing_feature: *mut i64,
        reasoning_effort_unsupported: *mut i64,
    ) -> i64;
    fn prodex_provider_constraints_normalize_combo_v1(
        abi_version: i64,
        input_i64: *const i64,
        input_i64_count: i64,
        input_u64: *const u64,
        input_u64_count: i64,
        output_i64: *mut i64,
        output_i64_count: i64,
        output_u64: *mut u64,
        output_u64_count: i64,
        candidate_count: i64,
    ) -> i64;
}

pub fn resolve_requirement_input(
    input: RequirementResolutionInput,
) -> Result<RequirementResolution, crate::MojoError> {
    let reserves = input
        .reasoning_reserve_by_effort
        .map(|value| value.unwrap_or_default());
    let mut reasoning_reserve_mask = 0_u64;
    for (index, value) in input.reasoning_reserve_by_effort.iter().enumerate() {
        if value.is_some() {
            reasoning_reserve_mask |= 1_u64 << index;
        }
    }
    let mut output_default_present = 0_i64;
    let mut output_default = 0_u64;
    let mut output_effort_present = 0_i64;
    let mut output_effort = -1_i64;
    let mut output_reserve_present = 0_i64;
    let mut output_reserve = 0_u64;
    let status = unsafe {
        prodex_provider_constraints_resolve_v1(
            i64::from(input.explicit_output_present),
            i64::from(input.default_output_reserve_tokens.is_some()),
            input.default_output_reserve_tokens.unwrap_or_default(),
            input.requested_reasoning_effort.unwrap_or(-1),
            input.default_reasoning_effort.unwrap_or(-1),
            i64::from(input.reasoning_reserve_tokens.is_some()),
            input.reasoning_reserve_tokens.unwrap_or_default(),
            reserves.as_ptr(),
            reasoning_reserve_mask,
            &mut output_default_present,
            &mut output_default,
            &mut output_effort_present,
            &mut output_effort,
            &mut output_reserve_present,
            &mut output_reserve,
        )
    };
    if status != 0 {
        return Err(crate::MojoError::InvalidOutput);
    }
    let optional = |present: i64, value: u64| match present {
        0 if value == 0 => Ok(None),
        1 => Ok(Some(value)),
        _ => Err(crate::MojoError::InvalidOutput),
    };
    let effort = match output_effort_present {
        0 if output_effort == -1 => None,
        1 if (0..=8).contains(&output_effort) => Some(output_effort),
        _ => return Err(crate::MojoError::InvalidOutput),
    };
    Ok(RequirementResolution {
        default_output_reserve_tokens: optional(output_default_present, output_default)?,
        reasoning_effort: effort,
        reasoning_reserve_tokens: optional(output_reserve_present, output_reserve)?,
    })
}

pub fn preclassify(input: PreclassificationInput) -> Result<Preclassification, crate::MojoError> {
    if input.required_features.len() > 9 {
        return Err(crate::MojoError::InvalidInput);
    }
    let required_features = input
        .required_features
        .iter()
        .map(|feature| *feature as i64)
        .collect::<Vec<_>>();
    let mut endpoint_supported = 0_i64;
    let mut missing_feature_present = 0_i64;
    let mut missing_feature = 0_i64;
    let mut reasoning_effort_unsupported = 0_i64;
    let status = unsafe {
        prodex_provider_constraints_preclassify_v1(
            input.endpoint_kind,
            i64::from(input.provider_endpoint_supported),
            i64::from(input.catalog_entry_present),
            i64::from(input.provider_streaming_supported),
            input.supported_endpoint_mask,
            input.feature_mask,
            required_features.as_ptr(),
            i64::try_from(required_features.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            input.reasoning_effort.unwrap_or(-1),
            i64::from(input.supported_reasoning_efforts.is_some()),
            input.supported_reasoning_efforts.unwrap_or_default(),
            &mut endpoint_supported,
            &mut missing_feature_present,
            &mut missing_feature,
            &mut reasoning_effort_unsupported,
        )
    };
    if status != 0
        || !matches!(endpoint_supported, 0 | 1)
        || !matches!(missing_feature_present, 0 | 1)
        || !matches!(reasoning_effort_unsupported, 0 | 1)
    {
        return Err(crate::MojoError::InvalidOutput);
    }
    let missing_feature = if missing_feature_present == 1 {
        Some(Feature::try_from(missing_feature)?)
    } else if missing_feature == 0 {
        None
    } else {
        return Err(crate::MojoError::InvalidOutput);
    };
    Ok(Preclassification {
        endpoint_supported: endpoint_supported == 1,
        missing_feature,
        reasoning_effort_unsupported: reasoning_effort_unsupported == 1,
    })
}

/// Normalize one shared output limit across eligible fallback candidates.
pub fn normalize_combo_output_adjustment(
    inputs: &[ComboOutputAdjustmentInput],
) -> Result<Vec<Option<ComboOutputAdjustment>>, crate::MojoError> {
    if inputs.len() > COMBO_MAX_CANDIDATES {
        return Err(crate::MojoError::InvalidInput);
    }
    if inputs.iter().any(|input| {
        input.adjustment_requested_tokens.is_some() != input.adjustment_applied_tokens.is_some()
    }) {
        return Err(crate::MojoError::InvalidInput);
    }
    let input_i64_len = inputs
        .len()
        .checked_mul(COMBO_INPUT_I64_FIELD_COUNT)
        .ok_or(crate::MojoError::InvalidInput)?;
    let input_u64_len = inputs
        .len()
        .checked_mul(COMBO_INPUT_U64_FIELD_COUNT)
        .ok_or(crate::MojoError::InvalidInput)?;
    let output_i64_len = inputs
        .len()
        .checked_mul(COMBO_OUTPUT_I64_FIELD_COUNT)
        .ok_or(crate::MojoError::InvalidInput)?;
    let output_u64_len = inputs
        .len()
        .checked_mul(COMBO_OUTPUT_U64_FIELD_COUNT)
        .ok_or(crate::MojoError::InvalidInput)?;
    let mut input_i64 = vec![0_i64; input_i64_len];
    let mut input_u64 = vec![0_u64; input_u64_len];
    for (index, input) in inputs.iter().enumerate() {
        let i64_base = index * COMBO_INPUT_I64_FIELD_COUNT;
        input_i64[i64_base] = i64::from(input.eligible);
        input_i64[i64_base + 1] = input.output_limit_field.map_or(-1, |field| field as i64);
        input_i64[i64_base + 2] = i64::from(input.adjustment_applied_tokens.is_some());
        input_i64[i64_base + 3] = i64::from(input.explicit_output_tokens.is_some());
        input_i64[i64_base + 4] = i64::from(input.reasoning_reserve_tokens.is_some());

        let u64_base = index * COMBO_INPUT_U64_FIELD_COUNT;
        input_u64[u64_base] = input.adjustment_requested_tokens.unwrap_or_default();
        input_u64[u64_base + 1] = input.adjustment_applied_tokens.unwrap_or_default();
        input_u64[u64_base + 2] = input.explicit_output_tokens.unwrap_or_default();
        input_u64[u64_base + 3] = input.estimated_input_tokens;
        input_u64[u64_base + 4] = input.reasoning_reserve_tokens.unwrap_or_default();
    }
    let mut output_i64 = vec![0_i64; output_i64_len];
    let mut output_u64 = vec![0_u64; output_u64_len];
    let candidate_count =
        i64::try_from(inputs.len()).map_err(|_| crate::MojoError::InvalidInput)?;
    let status = unsafe {
        prodex_provider_constraints_normalize_combo_v1(
            super::PROVIDER_CONSTRAINT_COMBO_ABI_VERSION,
            input_i64.as_ptr(),
            i64::try_from(input_i64.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            input_u64.as_ptr(),
            i64::try_from(input_u64.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            output_i64.as_mut_ptr(),
            i64::try_from(output_i64.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            output_u64.as_mut_ptr(),
            i64::try_from(output_u64.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            candidate_count,
        )
    };
    match status {
        0 => {}
        1 => return Err(crate::MojoError::AbiMismatch),
        2 => return Err(crate::MojoError::InvalidInput),
        _ => return Err(crate::MojoError::InvalidOutput),
    }

    inputs
        .iter()
        .enumerate()
        .map(|(index, input)| {
            let i64_base = index * COMBO_OUTPUT_I64_FIELD_COUNT;
            let u64_base = index * COMBO_OUTPUT_U64_FIELD_COUNT;
            let changed = match output_i64[i64_base] {
                0 => false,
                1 => true,
                _ => return Err(crate::MojoError::InvalidOutput),
            };
            let field = output_i64[i64_base + 1];
            let requested_tokens = output_u64[u64_base];
            let applied_tokens = output_u64[u64_base + 1];
            let total_required_tokens = output_u64[u64_base + 2];
            if !changed {
                if field != -1
                    || requested_tokens != 0
                    || applied_tokens != 0
                    || total_required_tokens != 0
                {
                    return Err(crate::MojoError::InvalidOutput);
                }
                return Ok(None);
            }
            if !input.eligible || input.output_limit_field.is_none() {
                return Err(crate::MojoError::InvalidOutput);
            }
            let field = super::OutputLimitField::try_from(field)?;
            let expected_requested = input
                .adjustment_requested_tokens
                .or(input.explicit_output_tokens)
                .unwrap_or(applied_tokens);
            if requested_tokens != expected_requested
                || total_required_tokens
                    != input
                        .estimated_input_tokens
                        .saturating_add(applied_tokens)
                        .saturating_add(input.reasoning_reserve_tokens.unwrap_or_default())
            {
                return Err(crate::MojoError::InvalidOutput);
            }
            Ok(Some(ComboOutputAdjustment {
                field,
                requested_tokens,
                applied_tokens,
                total_required_tokens,
            }))
        })
        .collect()
}
