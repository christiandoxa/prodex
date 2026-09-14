use super::{
    GovernanceMatchSelector, GovernanceMatchValue, GovernanceOptionalSelectorPair,
    GovernanceOptionalValuePair, GovernancePolicyRuleShape, policy_string_view,
    prodex_mojo_governance_effect_v1, prodex_mojo_governance_policy_shape_v1,
    prodex_mojo_governance_predicates_v1, prodex_mojo_governance_required_attributes_v1,
    prodex_mojo_governance_rule_matches_v1,
};

pub fn governance_policy_shape(
    valid_until_unix_ms: u64,
    rules: &[(&str, i64, usize)],
) -> Result<i64, crate::MojoError> {
    let rule_count = i64::try_from(rules.len()).map_err(|_| crate::MojoError::InvalidInput)?;
    let rules = rules
        .iter()
        .map(|(id, effect, obligation_count)| {
            Ok(GovernancePolicyRuleShape {
                id: policy_string_view(Some(id)),
                effect: *effect,
                obligation_count: i64::try_from(*obligation_count)
                    .map_err(|_| crate::MojoError::InvalidInput)?,
            })
        })
        .collect::<Result<Vec<_>, crate::MojoError>>()?;
    let mut output = -1_i64;
    let status = unsafe {
        prodex_mojo_governance_policy_shape_v1(
            6,
            valid_until_unix_ms,
            rules.as_ptr() as u64,
            rule_count,
            (&mut output as *mut i64) as u64,
        )
    };
    if status != 0 {
        return Err(match status {
            1 | 2 => crate::MojoError::InvalidInput,
            4 => crate::MojoError::AbiMismatch,
            _ => crate::MojoError::InvalidOutput,
        });
    }
    (0..=5)
        .contains(&output)
        .then_some(output)
        .ok_or(crate::MojoError::InvalidOutput)
}

fn governance_predicates(
    mode: i64,
    values: &[(Option<i64>, Option<i64>)],
    selectors: &[(Option<&str>, Option<&str>, bool)],
) -> Result<bool, crate::MojoError> {
    let values = values
        .iter()
        .map(|(left, right)| GovernanceOptionalValuePair {
            left: left.unwrap_or_default(),
            left_present: i64::from(left.is_some()),
            right: right.unwrap_or_default(),
            right_present: i64::from(right.is_some()),
        })
        .collect::<Vec<_>>();
    let selectors = selectors
        .iter()
        .map(|(left, right, wildcard)| GovernanceOptionalSelectorPair {
            left: policy_string_view(*left),
            left_present: i64::from(left.is_some()),
            right: policy_string_view(*right),
            right_present: i64::from(right.is_some()),
            wildcard: i64::from(*wildcard),
        })
        .collect::<Vec<_>>();
    let mut output = -1_i64;
    let status = unsafe {
        prodex_mojo_governance_predicates_v1(
            6,
            mode,
            values.as_ptr() as u64,
            i64::try_from(values.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            selectors.as_ptr() as u64,
            i64::try_from(selectors.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            (&mut output as *mut i64) as u64,
        )
    };
    if status != 0 {
        return Err(match status {
            1 | 2 => crate::MojoError::InvalidInput,
            4 => crate::MojoError::AbiMismatch,
            _ => crate::MojoError::InvalidOutput,
        });
    }
    match output {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}

pub fn governance_selector_matches(selector: &str, value: &str) -> Result<bool, crate::MojoError> {
    governance_predicates(0, &[], &[(Some(selector), Some(value), true)])
}

pub fn governance_policy_conditions_overlap(
    values: &[(Option<i64>, Option<i64>)],
    exact_selectors: &[(Option<&str>, Option<&str>)],
    wildcard_selectors: &[(Option<&str>, Option<&str>)],
) -> Result<bool, crate::MojoError> {
    let selectors = exact_selectors
        .iter()
        .map(|(left, right)| (*left, *right, false))
        .chain(
            wildcard_selectors
                .iter()
                .map(|(left, right)| (*left, *right, true)),
        )
        .collect::<Vec<_>>();
    governance_predicates(1, values, &selectors)
}

pub fn governance_policy_rule_matches(
    values: &[(Option<u64>, Option<u64>, i64)],
    selectors: &[(Option<&str>, &[&str], bool)],
) -> Result<bool, crate::MojoError> {
    let values = values
        .iter()
        .map(|(condition, input, comparison)| GovernanceMatchValue {
            condition: condition.unwrap_or_default(),
            condition_present: i64::from(condition.is_some()),
            input: input.unwrap_or_default(),
            input_present: i64::from(input.is_some()),
            comparison: *comparison,
        })
        .collect::<Vec<_>>();
    let selector_values = selectors
        .iter()
        .map(|(_, values, _)| {
            values
                .iter()
                .map(|value| policy_string_view(Some(value)))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let selectors = selectors
        .iter()
        .zip(&selector_values)
        .map(|((selector, _, wildcard), values)| {
            Ok(GovernanceMatchSelector {
                selector: policy_string_view(*selector),
                selector_present: i64::from(selector.is_some()),
                values: values.as_ptr() as u64,
                value_count: i64::try_from(values.len())
                    .map_err(|_| crate::MojoError::InvalidInput)?,
                wildcard: i64::from(*wildcard),
            })
        })
        .collect::<Result<Vec<_>, crate::MojoError>>()?;
    let mut output = -1_i64;
    let status = unsafe {
        prodex_mojo_governance_rule_matches_v1(
            6,
            values.as_ptr() as u64,
            i64::try_from(values.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            selectors.as_ptr() as u64,
            i64::try_from(selectors.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            (&mut output as *mut i64) as u64,
        )
    };
    if status != 0 {
        return Err(match status {
            1 | 2 => crate::MojoError::InvalidInput,
            4 => crate::MojoError::AbiMismatch,
            _ => crate::MojoError::InvalidOutput,
        });
    }
    match output {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}

pub fn governance_required_attributes_present(
    required_masks: &[u64],
    available_mask: u64,
) -> Result<bool, crate::MojoError> {
    let mut output = -1_i64;
    let status = unsafe {
        prodex_mojo_governance_required_attributes_v1(
            6,
            required_masks.as_ptr() as u64,
            i64::try_from(required_masks.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            available_mask,
            (&mut output as *mut i64) as u64,
        )
    };
    if status != 0 {
        return Err(match status {
            1 | 2 => crate::MojoError::InvalidInput,
            4 => crate::MojoError::AbiMismatch,
            _ => crate::MojoError::InvalidOutput,
        });
    }
    match output {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}

pub fn governance_policy_effect(
    effects: &[i64],
    default_effect: i64,
) -> Result<i64, crate::MojoError> {
    let mut output = -1_i64;
    let status = unsafe {
        prodex_mojo_governance_effect_v1(
            6,
            effects.as_ptr() as u64,
            i64::try_from(effects.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            default_effect,
            (&mut output as *mut i64) as u64,
        )
    };
    if status != 0 {
        return Err(match status {
            1 | 2 => crate::MojoError::InvalidInput,
            4 => crate::MojoError::AbiMismatch,
            _ => crate::MojoError::InvalidOutput,
        });
    }
    (0..=2)
        .contains(&output)
        .then_some(output)
        .ok_or(crate::MojoError::InvalidOutput)
}
