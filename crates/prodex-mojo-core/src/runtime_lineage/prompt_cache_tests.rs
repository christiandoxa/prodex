use super::*;

#[test]
fn compiled_prompt_cache_keys_preserve_unicode_nul_and_zero_hit_precedence() {
    for key in [None, Some(""), Some(" \u{2003}\u{3000}\n")] {
        assert_eq!(
            prepare_prompt_cache_key(key, PromptCacheEvent::Remember),
            Ok(PromptCacheKey::NoKey)
        );
        assert_eq!(
            prepare_prompt_cache_key(key, PromptCacheEvent::Hit(0)),
            Ok(PromptCacheKey::NoCachedTokens)
        );
    }
    let key = "\u{2003}cache\0東京\u{3000}";
    assert_eq!(
        prepare_prompt_cache_key(Some(key), PromptCacheEvent::Hit(u64::MAX)),
        Ok(PromptCacheKey::Value("cache\0東京"))
    );
    let large = "x".repeat(65_537);
    assert_eq!(
        prepare_prompt_cache_key(Some(&large), PromptCacheEvent::Remember),
        Ok(PromptCacheKey::Value(&large))
    );
}

#[test]
fn compiled_prompt_cache_updates_preserve_owner_timestamp_and_unsigned_token_rules() {
    let existing = |same_owner, bound_at, cached_input_tokens| {
        Some(PromptCacheBindingFacts {
            same_owner,
            bound_at,
            cached_input_tokens,
        })
    };
    for (event, facts, now, owner, bound_at, cached_input_tokens) in [
        (
            PromptCacheEvent::Remember,
            None,
            -1,
            PromptCacheOwnerUpdate::Inserted,
            -1,
            0,
        ),
        (
            PromptCacheEvent::Hit(u64::MAX),
            None,
            i64::MIN,
            PromptCacheOwnerUpdate::Inserted,
            i64::MIN,
            u64::MAX,
        ),
        (
            PromptCacheEvent::Remember,
            existing(false, 10, 0),
            -1,
            PromptCacheOwnerUpdate::Changed,
            -1,
            0,
        ),
        (
            PromptCacheEvent::Remember,
            existing(false, 10, 1),
            -1,
            PromptCacheOwnerUpdate::Preserved,
            10,
            1,
        ),
        (
            PromptCacheEvent::Hit(1),
            existing(true, i64::MAX, u64::MAX),
            -1,
            PromptCacheOwnerUpdate::Unchanged,
            i64::MAX,
            u64::MAX,
        ),
        (
            PromptCacheEvent::Hit(u64::MAX),
            existing(false, 10, u64::MAX - 1),
            -1,
            PromptCacheOwnerUpdate::Changed,
            -1,
            u64::MAX,
        ),
        (
            PromptCacheEvent::Hit(u64::MAX),
            existing(false, 10, u64::MAX),
            20,
            PromptCacheOwnerUpdate::Preserved,
            20,
            u64::MAX,
        ),
        (
            PromptCacheEvent::Hit(1),
            existing(true, i64::MIN, 0),
            i64::MIN,
            PromptCacheOwnerUpdate::Unchanged,
            i64::MIN,
            1,
        ),
    ] {
        assert_eq!(
            plan_prompt_cache_update(event, facts, now),
            Ok(PromptCacheUpdatePlan {
                owner,
                bound_at,
                cached_input_tokens
            })
        );
    }
    assert_eq!(
        plan_prompt_cache_update(PromptCacheEvent::Hit(0), None, 0),
        Err(MojoError::InvalidInput)
    );
}

#[test]
fn prompt_cache_abi_rejects_invalid_inputs_before_output_and_zero_hits_skip_key_reads() {
    let mut output = [71_u64; 3];
    let address = output.as_mut_ptr() as u64;
    let invalid_utf8 = [0xff_u8];
    unsafe {
        assert_eq!(prodex_prompt_cache_key_v1(2, 0, 0, 0, 0, address, 3), 4);
        assert_eq!(prodex_prompt_cache_key_v1(1, 2, 0, 0, 0, address, 3), 1);
        assert_eq!(prodex_prompt_cache_key_v1(1, 0, 0, 0, -1, address, 3), 1);
        assert_eq!(prodex_prompt_cache_key_v1(1, 0, 0, 0, 1, address, 3), 1);
        assert_eq!(
            prodex_prompt_cache_key_v1(1, 0, 0, invalid_utf8.as_ptr() as u64, 1, address, 3),
            1
        );
        assert_eq!(prodex_prompt_cache_key_v1(1, 0, 0, 0, 0, address, 2), 3);
        assert_eq!(prodex_prompt_cache_key_v1(1, 0, 0, 0, 0, 1, 3), 1);
        assert_eq!(
            prodex_prompt_cache_update_v1(2, 0, 0, 0, 0, 0, 0, 0, address, 3),
            4
        );
        assert_eq!(
            prodex_prompt_cache_update_v1(1, 1, 0, 0, 0, 0, 0, 0, address, 3),
            1
        );
        assert_eq!(
            prodex_prompt_cache_update_v1(1, 0, 0, 1, 0, 0, 0, 0, address, 3),
            1
        );
        assert_eq!(
            prodex_prompt_cache_update_v1(1, 0, 0, 0, 0, 0, 0, 0, address, 2),
            3
        );
    }
    assert_eq!(output, [71; 3]);
    let address = output.as_mut_ptr() as u64;
    assert_eq!(
        unsafe { prodex_prompt_cache_key_v1(1, 1, 0, 1, -1, address, 3) },
        0
    );
    assert_eq!(output, [1, 0, 0]);
}
