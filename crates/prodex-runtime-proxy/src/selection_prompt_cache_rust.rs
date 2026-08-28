pub fn runtime_prompt_cache_affinity_batch(
    prompt_cache_key: Option<&str>,
    prompt_cache_owner_profile: Option<&str>,
    profiles: &[&str],
) -> Result<Vec<(u8, u64)>, std::convert::Infallible> {
    Ok(profiles
        .iter()
        .map(|profile| {
            runtime_prompt_cache_affinity_sort_key_with_owner(
                prompt_cache_key,
                prompt_cache_owner_profile,
                profile,
            )
        })
        .collect())
}

pub fn runtime_prompt_cache_affinity_sort_key_with_owner(
    prompt_cache_key: Option<&str>,
    prompt_cache_owner_profile: Option<&str>,
    profile_name: &str,
) -> (u8, u64) {
    if prompt_cache_key
        .map(str::trim)
        .is_none_or(|prompt_cache_key| prompt_cache_key.is_empty())
    {
        return (0, 0);
    }
    if let Some(owner) = prompt_cache_owner_profile
        .map(str::trim)
        .filter(|owner| !owner.is_empty())
    {
        return if owner == profile_name {
            (0, 0)
        } else {
            (
                1,
                runtime_prompt_cache_affinity_sort_key(prompt_cache_key, profile_name),
            )
        };
    }
    (
        0,
        runtime_prompt_cache_affinity_sort_key(prompt_cache_key, profile_name),
    )
}

pub fn runtime_prompt_cache_affinity_sort_key(
    prompt_cache_key: Option<&str>,
    profile_name: &str,
) -> u64 {
    let Some(prompt_cache_key) = prompt_cache_key
        .map(str::trim)
        .filter(|prompt_cache_key| !prompt_cache_key.is_empty())
    else {
        return 0;
    };

    u64::MAX - runtime_prompt_cache_affinity_score(prompt_cache_key, profile_name)
}

fn runtime_prompt_cache_affinity_score(prompt_cache_key: &str, profile_name: &str) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET_BASIS;
    for bytes in [
        b"prodex-prompt-cache-affinity-v1".as_slice(),
        b"\0".as_slice(),
        prompt_cache_key.as_bytes(),
        b"\0".as_slice(),
        profile_name.as_bytes(),
    ] {
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
    }
    hash
}
