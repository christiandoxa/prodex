use super::RUNTIME_PROFILE_SCHEDULE_MAX_COUNT;

unsafe extern "C" {
    fn prodex_runtime_profile_rotation_order_batch(
        priorities: *const i64,
        ordered_indices: *mut i64,
        ordered_count: *mut i64,
        count: i64,
        current_index: i64,
        include_current: i64,
    ) -> i64;
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
