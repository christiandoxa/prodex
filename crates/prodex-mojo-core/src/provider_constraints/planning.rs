use super::Feature;

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
