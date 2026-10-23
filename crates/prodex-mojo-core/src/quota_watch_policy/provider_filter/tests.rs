use super::*;
use crate::quota_watch_policy::*;

#[test]
fn compiled_provider_filter_preserves_all_aliases_and_normalized_diagnostics() {
    for (aliases, expected) in [
        (&["", "all"][..], FILTER_ALL),
        (
            &["openai", "chatgpt", "codex", "openai-codex"][..],
            FILTER_OPENAI,
        ),
        (&["gemini", "google", "google-gemini"][..], FILTER_GEMINI),
        (
            &["anthropic", "claude", "anthropic-claude"][..],
            FILTER_ANTHROPIC,
        ),
        (&["copilot", "github-copilot", "github"][..], FILTER_COPILOT),
        (&["kiro", "kiro-cli"][..], FILTER_KIRO),
        (&["deepseek"][..], FILTER_DEEPSEEK),
        (
            &["local", "openai-compatible", "openai_compatible"][..],
            FILTER_LOCAL,
        ),
        (&["agy", "anti-gravity"][..], FILTER_AGY),
    ] {
        for alias in aliases {
            for input in [
                alias.to_string(),
                format!(
                    "\u{2003}{}\u{0085}",
                    alias.to_ascii_uppercase().replace('-', "_")
                ),
            ] {
                assert_eq!(
                    parse_provider_filter(&input),
                    Ok(ProviderFilterParse::Known(expected)),
                    "{input:?}"
                );
            }
        }
    }
    for raw in [
        " UNKNOWN_PROVIDER ",
        "ANTHROPİC",
        "🙂\0_É",
        "\u{200b}all",
        "x ' quoted",
        &"X_".repeat(20_000),
    ] {
        let normalized = raw.trim().to_ascii_lowercase().replace('_', "-");
        let expected = format!(
            "invalid quota provider filter '{normalized}'; supported values are all, openai, gemini, anthropic, claude, copilot, kiro, deepseek, local, agy"
        );
        assert_eq!(
            parse_provider_filter(raw),
            Ok(ProviderFilterParse::Invalid(expected))
        );
    }
}

#[test]
fn compiled_profile_membership_covers_every_filter_and_native_provider() {
    for filter in FILTER_ALL..=FILTER_AGY {
        for profile in [
            FILTER_OPENAI,
            FILTER_GEMINI,
            FILTER_ANTHROPIC,
            FILTER_COPILOT,
            FILTER_KIRO,
            FILTER_AGY,
        ] {
            assert_eq!(
                filter_matches_profile(filter, profile),
                Ok(filter == FILTER_ALL || filter == profile)
            );
        }
    }
    for profile in [FILTER_ALL, FILTER_DEEPSEEK, FILTER_LOCAL, -1, 9] {
        assert_eq!(
            filter_matches_profile(FILTER_ALL, profile),
            Err(MojoError::InvalidInput)
        );
    }
    assert_eq!(
        filter_matches_profile(9, FILTER_OPENAI),
        Err(MojoError::InvalidInput)
    );
}

#[test]
fn provider_filter_abi_rejects_invalid_input_without_partial_diagnostics() {
    let value = "unknown";
    let mut output = [0x5a_u8; 256];
    let mut metadata = [-7_i64; 2];
    for (abi, address, length, phase, capacity, expected) in [
        (2, value.as_ptr() as u64, value.len() as i64, 0, 256, 4),
        (1, 0, 1, 0, 256, 1),
        (1, value.as_ptr() as u64, -1, 0, 256, 1),
        (1, value.as_ptr() as u64, value.len() as i64, 2, 256, 1),
        (1, value.as_ptr() as u64, value.len() as i64, 1, 256, 1),
        (1, value.as_ptr() as u64, value.len() as i64, 0, 0, 3),
        (1, b"\xff".as_ptr() as u64, 1, 0, 256, 1),
    ] {
        assert_eq!(
            unsafe {
                prodex_quota_watch_provider_filter_parse_v1(
                    abi,
                    address,
                    length,
                    phase,
                    output.as_mut_ptr() as u64,
                    capacity,
                    metadata.as_mut_ptr() as u64,
                )
            },
            expected
        );
        assert_eq!(output, [0x5a; 256]);
        assert_eq!(metadata, [-7; 2]);
    }
    let mut membership = -9_i64;
    assert_eq!(
        unsafe {
            prodex_quota_watch_profile_filter_matches_v1(
                2,
                0,
                1,
                (&mut membership as *mut i64) as u64,
            )
        },
        4
    );
    assert_eq!(membership, -9);
}
