//! Ordered choice of ready quota capacity rows, compiled in the quota kernel.
//! Rust acquires already-classified candidate facts and applies the selected
//! original pair reference without repeating Mojo's precedence rules.

use super::{
    QUOTA_CAPACITY_LANE_MAIN, QUOTA_CAPACITY_LANE_MODEL_SPECIFIC,
    QUOTA_CAPACITY_LANE_UNKNOWN_ADDITIONAL,
};
use crate::MojoError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuotaCapacityChoiceInput {
    pub lane: i64,
    pub routing_eligible: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuotaCapacityChoice {
    pub selected_index: Option<usize>,
    pub any_routing_eligible: bool,
}

unsafe extern "C" {
    fn prodex_quota_capacity_choice_v1(
        abi_version: i64,
        fields_address: u64,
        count: i64,
        output_address: u64,
        output_count: i64,
    ) -> i64;
}

pub fn quota_capacity_choice(
    candidates: &[QuotaCapacityChoiceInput],
) -> Result<QuotaCapacityChoice, MojoError> {
    if candidates.len() > 1_048_576 {
        return Err(MojoError::InvalidInput);
    }
    let mut fields = Vec::new();
    fields
        .try_reserve_exact(candidates.len().checked_mul(2).ok_or(MojoError::Capacity)?)
        .map_err(|_| MojoError::Capacity)?;
    for candidate in candidates {
        if !matches!(
            candidate.lane,
            QUOTA_CAPACITY_LANE_MAIN
                | QUOTA_CAPACITY_LANE_MODEL_SPECIFIC
                | QUOTA_CAPACITY_LANE_UNKNOWN_ADDITIONAL
        ) {
            return Err(MojoError::InvalidInput);
        }
        fields.extend([candidate.lane, i64::from(candidate.routing_eligible)]);
    }
    let mut output = [-1_i64; 2];
    match unsafe {
        prodex_quota_capacity_choice_v1(
            1,
            fields.as_ptr() as usize as u64,
            i64::try_from(candidates.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            2,
        )
    } {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let selected_index = match output[0] {
        -1 => None,
        index => Some(usize::try_from(index).map_err(|_| MojoError::InvalidOutput)?),
    };
    if selected_index.is_some_and(|index| index >= candidates.len()) {
        return Err(MojoError::InvalidOutput);
    }
    let any_routing_eligible = match output[1] {
        0 => false,
        1 => true,
        _ => return Err(MojoError::InvalidOutput),
    };
    if any_routing_eligible && selected_index.is_none() {
        return Err(MojoError::InvalidOutput);
    }
    Ok(QuotaCapacityChoice {
        selected_index,
        any_routing_eligible,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(lane: i64, routing_eligible: bool) -> QuotaCapacityChoiceInput {
        QuotaCapacityChoiceInput {
            lane,
            routing_eligible,
        }
    }

    #[test]
    fn compiled_quota_capacity_choice_preserves_first_ready_and_main_fallback() {
        let main = QUOTA_CAPACITY_LANE_MAIN;
        let specific = QUOTA_CAPACITY_LANE_MODEL_SPECIFIC;
        let other = QUOTA_CAPACITY_LANE_UNKNOWN_ADDITIONAL;
        for (rows, index, ready) in [
            (vec![], None, false),
            (vec![row(other, false)], None, false),
            (vec![row(main, false)], Some(0), false),
            (vec![row(other, false), row(main, false)], Some(1), false),
            (
                vec![row(other, false), row(main, false), row(specific, true)],
                Some(2),
                true,
            ),
            (
                vec![row(other, true), row(main, true), row(specific, true)],
                Some(0),
                true,
            ),
            (
                vec![row(main, false), row(other, false), row(main, false)],
                Some(0),
                false,
            ),
            (vec![row(main, true), row(main, true)], Some(0), true),
        ] {
            assert_eq!(
                quota_capacity_choice(&rows),
                Ok(QuotaCapacityChoice {
                    selected_index: index,
                    any_routing_eligible: ready,
                }),
                "{rows:?}",
            );
        }
        let many = vec![row(other, false); 1025];
        assert_eq!(
            quota_capacity_choice(&many),
            Ok(QuotaCapacityChoice {
                selected_index: None,
                any_routing_eligible: false,
            }),
        );
        assert_eq!(
            quota_capacity_choice(&[row(3, false)]),
            Err(MojoError::InvalidInput)
        );
    }

    #[test]
    fn compiled_quota_capacity_choice_abi_rejects_late_invalid_flags_before_writes() {
        let mut rows = [0_i64, 1, 1, 0, 2, 0];
        let mut result = [91_i64; 2];
        let ptr = rows.as_ptr() as usize as u64;
        let output = result.as_mut_ptr() as usize as u64;
        for (version, source, count, output_count, expected) in [
            (2, ptr, 3, 2, 4),
            (1, 0, 3, 2, 1),
            (1, ptr, -1, 2, 1),
            (1, ptr, 1_048_577, 2, 1),
            (1, ptr, 3, 1, 1),
        ] {
            assert_eq!(
                unsafe {
                    prodex_quota_capacity_choice_v1(version, source, count, output, output_count)
                },
                expected
            );
            assert_eq!(result, [91; 2]);
        }
        rows[5] = 2;
        assert_eq!(
            unsafe {
                prodex_quota_capacity_choice_v1(1, rows.as_ptr() as usize as u64, 3, output, 2)
            },
            1
        );
        assert_eq!(result, [91; 2]);
        assert_eq!(
            unsafe { prodex_quota_capacity_choice_v1(1, 0, 0, output, 2) },
            0,
        );
        assert_eq!(result, [-1, 0]);
    }
}
