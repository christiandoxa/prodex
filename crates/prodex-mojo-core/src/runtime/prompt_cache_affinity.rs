use super::{
    RUNTIME_CANDIDATE_PLAN_MAX_COUNT, RuntimeStringView,
    prodex_runtime_prompt_cache_affinity_batch_v1,
};

pub fn prompt_cache_affinity_batch(
    prompt_cache_key: Option<&str>,
    prompt_cache_owner_profile: Option<&str>,
    profiles: &[&str],
) -> Result<Vec<(u8, u64)>, crate::MojoError> {
    if profiles.len() > RUNTIME_CANDIDATE_PLAN_MAX_COUNT {
        return Err(crate::MojoError::InvalidInput);
    }
    if profiles.is_empty() {
        return Ok(Vec::new());
    }
    let profile_views = profiles
        .iter()
        .map(|profile| RuntimeStringView {
            ptr: profile.as_ptr() as usize as u64,
            len: profile.len() as u64,
        })
        .collect::<Vec<_>>();
    let key_view = prompt_cache_key.map(|value| RuntimeStringView {
        ptr: value.as_ptr() as usize as u64,
        len: u64::try_from(value.len()).unwrap_or(u64::MAX),
    });
    let owner_view = prompt_cache_owner_profile.map(|value| RuntimeStringView {
        ptr: value.as_ptr() as usize as u64,
        len: u64::try_from(value.len()).unwrap_or(u64::MAX),
    });
    let mut priorities = vec![0_i64; profiles.len()];
    let mut scores = vec![0_u64; profiles.len()];
    let status = unsafe {
        prodex_runtime_prompt_cache_affinity_batch_v1(
            profile_views.as_ptr() as usize as u64,
            key_view
                .as_ref()
                .map_or(0, |view| view as *const RuntimeStringView as usize as u64),
            i64::from(key_view.is_some()),
            owner_view
                .as_ref()
                .map_or(0, |view| view as *const RuntimeStringView as usize as u64),
            i64::from(owner_view.is_some()),
            priorities.as_mut_ptr() as usize as u64,
            scores.as_mut_ptr() as usize as u64,
            i64::try_from(profiles.len()).map_err(|_| crate::MojoError::InvalidInput)?,
        )
    };
    if status != 0 || priorities.iter().any(|priority| !matches!(priority, 0 | 1)) {
        return Err(crate::MojoError::InvalidOutput);
    }
    Ok(priorities
        .into_iter()
        .zip(scores)
        .map(|(priority, score)| (u8::try_from(priority).unwrap_or(u8::MAX), score))
        .collect())
}
