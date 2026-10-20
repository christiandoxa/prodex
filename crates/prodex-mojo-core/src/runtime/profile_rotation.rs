use super::RUNTIME_PROFILE_SCHEDULE_MAX_COUNT;

unsafe extern "C" {
    fn prodex_runtime_profile_rotation_by_current_name_v2(
        abi_version: i64,
        names_address: u64,
        priorities_address: u64,
        count: i64,
        current_name_address: u64,
        current_name_length: i64,
        include_mode: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_runtime_profile_rotation_order_batch(
        priorities: *const i64,
        ordered_indices: *mut i64,
        ordered_count: *mut i64,
        count: i64,
        current_index: i64,
        include_current: i64,
    ) -> i64;
}

/// Host-chosen inclusion contract for the same compiled name matcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum ProfileCurrentNameInclusion {
    Exclude = 0,
    Include = 1,
    IncludeIfPresent = 2,
}

/// Resolve the first exact current-profile name and apply existing compiled
/// provider-priority rotation in one Mojo call. No Rust .position/label match.
pub fn profile_selection_order_by_current_name(
    names: &[&str],
    provider_priorities: &[usize],
    current_name: &str,
    inclusion: ProfileCurrentNameInclusion,
) -> Result<Vec<usize>, crate::MojoError> {
    use crate::MojoError;
    if names.len() != provider_priorities.len()
        || names.len() > RUNTIME_PROFILE_SCHEDULE_MAX_COUNT
        || current_name.len() > 65_536
        || names.iter().any(|name| name.len() > 65_536)
    {
        return Err(MojoError::InvalidInput);
    }
    let mut views = Vec::<u64>::new();
    views
        .try_reserve_exact(names.len().checked_mul(2).ok_or(MojoError::Capacity)?)
        .map_err(|_| MojoError::Capacity)?;
    for name in names {
        views.extend([
            name.as_ptr() as usize as u64,
            u64::try_from(name.len()).map_err(|_| MojoError::InvalidInput)?,
        ]);
    }
    let priorities = provider_priorities
        .iter()
        .map(|value| i64::try_from(*value).unwrap_or(i64::MAX))
        .collect::<Vec<_>>();
    // The largest valid result is one synthetic current entry beyond the
    // original profiles. This is capacity, not a duplicate Rust policy.
    let mut output = vec![-1_i64; names.len() + 1];
    let mut written = -1_i64;
    match unsafe {
        prodex_runtime_profile_rotation_by_current_name_v2(
            2,
            views.as_ptr() as usize as u64,
            priorities.as_ptr() as usize as u64,
            i64::try_from(names.len()).map_err(|_| MojoError::InvalidInput)?,
            current_name.as_ptr() as usize as u64,
            i64::try_from(current_name.len()).map_err(|_| MojoError::InvalidInput)?,
            inclusion as i64,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    } {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let count = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if count > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    let mut seen = vec![false; output.len()];
    output
        .into_iter()
        .take(count)
        .map(|value| {
            let index = usize::try_from(value).map_err(|_| MojoError::InvalidOutput)?;
            if index == names.len() && inclusion != ProfileCurrentNameInclusion::Include {
                return Err(MojoError::InvalidOutput);
            }
            let used = seen.get_mut(index).ok_or(MojoError::InvalidOutput)?;
            if *used {
                return Err(MojoError::InvalidOutput);
            }
            *used = true;
            Ok(index)
        })
        .collect()
}

/// Returns the provider-priority order for a current-relative profile rotation.
///
/// `current_index` identifies the current profile in `provider_priorities`. A missing current
/// profile is represented by `None`; when `include_current` is true, the returned index equal to
/// `provider_priorities.len()` represents that synthetic current profile.
pub fn profile_selection_order_batch(
    provider_priorities: &[usize],
    current_index: Option<usize>,
    include_current: bool,
) -> Result<Vec<usize>, crate::MojoError> {
    if provider_priorities.len() > RUNTIME_PROFILE_SCHEDULE_MAX_COUNT {
        return Err(crate::MojoError::InvalidInput);
    }
    let current_index_value = match current_index {
        Some(index) if index < provider_priorities.len() => {
            i64::try_from(index).map_err(|_| crate::MojoError::InvalidInput)?
        }
        Some(_) => return Err(crate::MojoError::InvalidInput),
        None => -1,
    };
    let synthetic_current = include_current && current_index.is_none();
    let excluded_current = !include_current && current_index.is_some();
    let output_len = provider_priorities
        .len()
        .checked_add(usize::from(synthetic_current))
        .and_then(|length| length.checked_sub(usize::from(excluded_current)))
        .ok_or(crate::MojoError::InvalidInput)?;
    if output_len > RUNTIME_PROFILE_SCHEDULE_MAX_COUNT {
        return Err(crate::MojoError::InvalidInput);
    }

    let priorities = provider_priorities
        .iter()
        .map(|priority| i64::try_from(*priority).unwrap_or(i64::MAX))
        .collect::<Vec<_>>();
    let mut ordered_indices = vec![0_i64; output_len];
    let mut ordered_count = 0_i64;
    let status = unsafe {
        prodex_runtime_profile_rotation_order_batch(
            priorities.as_ptr(),
            ordered_indices.as_mut_ptr(),
            &mut ordered_count,
            i64::try_from(provider_priorities.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            current_index_value,
            i64::from(include_current),
        )
    };
    if status != 0 || ordered_count < 0 || usize::try_from(ordered_count).ok() != Some(output_len) {
        return Err(crate::MojoError::InvalidOutput);
    }

    let mut seen = vec![false; provider_priorities.len() + 1];
    let ordered_indices = ordered_indices
        .into_iter()
        .map(|index| {
            let index = usize::try_from(index).map_err(|_| crate::MojoError::InvalidOutput)?;
            if index > provider_priorities.len() || seen[index] {
                return Err(crate::MojoError::InvalidOutput);
            }
            seen[index] = true;
            Ok(index)
        })
        .collect::<Result<Vec<_>, _>>()?;

    for (index, was_seen) in seen
        .iter()
        .copied()
        .enumerate()
        .take(provider_priorities.len())
    {
        let expected = include_current || current_index != Some(index);
        if was_seen != expected {
            return Err(crate::MojoError::InvalidOutput);
        }
    }
    if seen[provider_priorities.len()] != synthetic_current {
        return Err(crate::MojoError::InvalidOutput);
    }

    Ok(ordered_indices)
}

#[cfg(test)]
mod compiled_profile_rotation_name_tests {
    use super::*;
    use ProfileCurrentNameInclusion::{Exclude, Include, IncludeIfPresent};

    #[test]
    fn compiled_current_name_rotation_preserves_priority_and_current_relative_ties() {
        let names = ["alpha", "雪", "z", "é"];
        let priorities = [2, 0, 0, 1];
        assert_eq!(
            profile_selection_order_by_current_name(&names, &priorities, "雪", IncludeIfPresent),
            Ok(vec![1, 2, 3, 0]),
        );
        assert_eq!(
            profile_selection_order_by_current_name(&names, &priorities, "z", IncludeIfPresent),
            Ok(vec![2, 1, 3, 0]),
        );
        assert_eq!(
            profile_selection_order_by_current_name(
                &names,
                &priorities,
                "missing",
                IncludeIfPresent
            ),
            Ok(vec![1, 2, 3, 0]),
        );
        assert_eq!(
            profile_selection_order_by_current_name(&[], &[], "雪", IncludeIfPresent),
            Ok(vec![]),
        );
        assert_eq!(
            profile_selection_order_by_current_name(
                &["", "same", "same"],
                &[0, 0, 0],
                "same",
                IncludeIfPresent
            ),
            Ok(vec![1, 2, 0]),
            "first exact duplicate name is current, even when equal priorities tie",
        );
        assert_eq!(
            profile_selection_order_by_current_name(
                &["", "same", "same"],
                &[0, 0, 0],
                "",
                IncludeIfPresent
            ),
            Ok(vec![0, 1, 2]),
        );
        assert_eq!(
            profile_selection_order_by_current_name(&["alpha"], &[], "alpha", IncludeIfPresent),
            Err(crate::MojoError::InvalidInput),
        );
        assert_eq!(
            profile_selection_order_by_current_name(
                &["alpha"],
                &[0],
                &"雪".repeat(21_846),
                IncludeIfPresent
            ),
            Err(crate::MojoError::InvalidInput),
        );
    }

    #[test]
    fn compiled_current_name_rotation_handles_quota_synthetic_and_exclusion_contract() {
        let names = ["alpha", "雪", "z", "é"];
        let priorities = [2, 0, 0, 1];
        assert_eq!(
            profile_selection_order_by_current_name(&names, &priorities, "雪", Exclude),
            Ok(vec![2, 3, 0]),
        );
        assert_eq!(
            profile_selection_order_by_current_name(&names, &priorities, "雪", Include),
            Ok(vec![1, 2, 3, 0]),
        );
        assert_eq!(
            profile_selection_order_by_current_name(&names, &priorities, "missing", Exclude),
            Ok(vec![1, 2, 3, 0]),
        );
        assert_eq!(
            profile_selection_order_by_current_name(&names, &priorities, "missing", Include),
            Ok(vec![1, 2, 3, 0, 4]),
            "quota must receive the missing synthetic active profile",
        );
        assert_eq!(
            profile_selection_order_by_current_name(&[], &[], "missing", Include),
            Ok(vec![0]),
        );
        assert_eq!(
            profile_selection_order_by_current_name(&[], &[], "missing", Exclude),
            Ok(vec![]),
        );
        let full = vec!["profile"; RUNTIME_PROFILE_SCHEDULE_MAX_COUNT];
        let priorities = vec![0; full.len()];
        assert_eq!(
            profile_selection_order_by_current_name(&full, &priorities, "missing", Include),
            Err(crate::MojoError::InvalidInput),
            "the bounded compiled scheduler rejects a synthetic 257th slot",
        );
    }

    #[test]
    fn compiled_current_name_rotation_rejects_malformed_late_views_before_writing() {
        let known = "snow";
        let mut names = [known.as_ptr() as usize as u64, known.len() as u64, 0, 0];
        let mut priorities = [0_i64, 0];
        let mut output = [73_i64; 3];
        let mut written = 91_i64;
        let source = names.as_ptr() as usize as u64;
        let priority_ptr = priorities.as_ptr() as usize as u64;
        let out = output.as_mut_ptr() as usize as u64;
        let written_ptr = (&mut written as *mut i64) as usize as u64;
        let current = known.as_ptr() as usize as u64;
        for (
            version,
            names_address,
            count,
            current_addr,
            current_len,
            mode,
            cap,
            output_addr,
            expected,
        ) in [
            (3, source, 2, current, 4, 2, 3, out, 4),
            (2, 0, 2, current, 4, 2, 3, out, 1),
            (2, source, -1, current, 4, 2, 3, out, 1),
            (2, source, 257, current, 4, 2, 3, out, 1),
            (2, source, 2, 0, 4, 2, 3, out, 1),
            (2, source, 2, current, -1, 2, 3, out, 1),
            (2, source, 2, current, 4, 3, 3, out, 1),
            (2, source, 2, current, 4, -1, 3, out, 1),
            (2, source, 2, current, 4, 2, 1, out, 1),
            (2, source, 2, current, 4, 2, 3, 0, 1),
        ] {
            assert_eq!(
                unsafe {
                    prodex_runtime_profile_rotation_by_current_name_v2(
                        version,
                        names_address,
                        priority_ptr,
                        count,
                        current_addr,
                        current_len,
                        mode,
                        output_addr,
                        cap,
                        written_ptr,
                    )
                },
                expected,
            );
            assert_eq!(output, [73; 3]);
            assert_eq!(written, 91);
        }
        names[3] = u64::MAX;
        assert_eq!(
            unsafe {
                prodex_runtime_profile_rotation_by_current_name_v2(
                    2,
                    names.as_ptr() as usize as u64,
                    priority_ptr,
                    2,
                    current,
                    4,
                    2,
                    out,
                    3,
                    written_ptr,
                )
            },
            1,
        );
        assert_eq!(output, [73; 3]);
        assert_eq!(written, 91);
        priorities[1] = -1;
        assert_eq!(
            unsafe {
                prodex_runtime_profile_rotation_by_current_name_v2(
                    2,
                    source,
                    priorities.as_ptr() as usize as u64,
                    2,
                    current,
                    4,
                    2,
                    out,
                    3,
                    written_ptr,
                )
            },
            1,
        );
        assert_eq!(output, [73; 3]);
        assert_eq!(written, 91);
    }
}
