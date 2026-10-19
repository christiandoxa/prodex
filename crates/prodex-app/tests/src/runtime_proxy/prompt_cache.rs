use super::*;

#[test]
fn compiled_prompt_cache_caller_preserves_zero_hit_skips_and_owner_strength() {
    clear_runtime_prompt_cache_profile_bindings();
    let key = "cache\0東京";
    assert!(remember_runtime_prompt_cache_profile_at(
        "alpha",
        Some("\u{2003}cache\0東京\u{3000}"),
        100
    ));
    assert_eq!(
        observe_runtime_prompt_cache_profile_hit_at("alpha", Some(key), u64::MAX, 110),
        RuntimePromptCacheProfileObservation::OwnerUnchanged
    );
    assert_eq!(
        observe_runtime_prompt_cache_profile_hit_at("beta", Some(key), u64::MAX, 109),
        RuntimePromptCacheProfileObservation::ExistingOwnerPreserved
    );
    assert!(!remember_runtime_prompt_cache_profile_at(
        "beta",
        Some(key),
        111
    ));
    {
        let bindings = runtime_prompt_cache_profile_bindings().lock().unwrap();
        let binding = &bindings[key];
        assert_eq!(
            (
                &*binding.profile_name,
                binding.bound_at,
                binding.cached_input_tokens
            ),
            ("alpha", 111, u64::MAX)
        );
    }
    let expired_now = 112 + RUNTIME_PROMPT_CACHE_AFFINITY_RETENTION_SECONDS;
    assert_eq!(
        observe_runtime_prompt_cache_profile_hit_at("beta", None, 0, expired_now),
        RuntimePromptCacheProfileObservation::NoCachedTokens
    );
    assert_eq!(
        observe_runtime_prompt_cache_profile_hit_at("beta", Some(" \u{2003}"), 1, expired_now),
        RuntimePromptCacheProfileObservation::NoKey
    );
    assert!(!remember_runtime_prompt_cache_profile_at(
        "beta",
        None,
        expired_now
    ));
    assert!(
        runtime_prompt_cache_profile_bindings()
            .lock()
            .unwrap()
            .contains_key(key)
    );
    assert_eq!(
        runtime_prompt_cache_bound_profile_at(Some(key), expired_now),
        None
    );

    assert!(remember_runtime_prompt_cache_profile_at(
        "alpha",
        Some(key),
        100
    ));
    assert!(remember_runtime_prompt_cache_profile_at(
        "beta",
        Some(key),
        99
    ));
    let bindings = runtime_prompt_cache_profile_bindings().lock().unwrap();
    assert_eq!(
        (
            &*bindings[key].profile_name,
            bindings[key].bound_at,
            bindings[key].cached_input_tokens
        ),
        ("beta", 99, 0)
    );
}

#[test]
fn compiled_prompt_cache_caller_retains_ttl_boundary_and_stable_eviction_order() {
    clear_runtime_prompt_cache_profile_bindings();
    let mut bindings = runtime_prompt_cache_profile_bindings().lock().unwrap();
    for index in 0..RUNTIME_PROMPT_CACHE_AFFINITY_LIMIT + 2 {
        bindings.insert(
            format!("cache-{index:04}"),
            RuntimePromptCacheProfileBinding {
                profile_name: "alpha".to_string(),
                bound_at: 100,
                cached_input_tokens: 0,
            },
        );
    }
    prune_runtime_prompt_cache_profile_bindings(&mut bindings, 100);
    assert_eq!(
        bindings.keys().cloned().collect::<Vec<_>>(),
        (2..RUNTIME_PROMPT_CACHE_AFFINITY_LIMIT + 2)
            .map(|index| format!("cache-{index:04}"))
            .collect::<Vec<_>>()
    );
    prune_runtime_prompt_cache_profile_bindings(
        &mut bindings,
        100 + RUNTIME_PROMPT_CACHE_AFFINITY_RETENTION_SECONDS,
    );
    assert_eq!(bindings.len(), RUNTIME_PROMPT_CACHE_AFFINITY_LIMIT);
    prune_runtime_prompt_cache_profile_bindings(
        &mut bindings,
        101 + RUNTIME_PROMPT_CACHE_AFFINITY_RETENTION_SECONDS,
    );
    assert!(bindings.is_empty());
    bindings.insert(
        "minimum".to_string(),
        RuntimePromptCacheProfileBinding {
            profile_name: "alpha".to_string(),
            bound_at: i64::MIN,
            cached_input_tokens: 0,
        },
    );
    prune_runtime_prompt_cache_profile_bindings(&mut bindings, i64::MIN);
    assert_eq!(bindings.len(), 1);
}
