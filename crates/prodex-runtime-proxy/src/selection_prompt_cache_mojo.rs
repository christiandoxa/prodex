pub fn runtime_prompt_cache_affinity_batch(
    prompt_cache_key: Option<&str>,
    prompt_cache_owner_profile: Option<&str>,
    profiles: &[&str],
) -> Result<Vec<(u8, u64)>, prodex_mojo_core::MojoError> {
    prodex_mojo_core::runtime::prompt_cache_affinity_batch(
        prompt_cache_key,
        prompt_cache_owner_profile,
        profiles,
    )
}

pub fn runtime_prompt_cache_affinity_sort_key_with_owner(
    prompt_cache_key: Option<&str>,
    prompt_cache_owner_profile: Option<&str>,
    profile_name: &str,
) -> (u8, u64) {
    runtime_prompt_cache_affinity_batch(
        prompt_cache_key,
        prompt_cache_owner_profile,
        &[profile_name],
    )
    .expect("Mojo prompt-cache affinity returned invalid output")
    .into_iter()
    .next()
    .expect("Mojo prompt-cache affinity returned no row")
}

pub fn runtime_prompt_cache_affinity_sort_key(
    prompt_cache_key: Option<&str>,
    profile_name: &str,
) -> u64 {
    runtime_prompt_cache_sort_key_with_no_owner(prompt_cache_key, profile_name)
}

fn runtime_prompt_cache_sort_key_with_no_owner(
    prompt_cache_key: Option<&str>,
    profile_name: &str,
) -> u64 {
    runtime_prompt_cache_affinity_sort_key_with_owner(prompt_cache_key, None, profile_name).1
}
