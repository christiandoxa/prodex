//! Compiled O(n + m) ASCII casefolded log substring search.
//! The host borrows UTF-8 bytes and provides KMP scratch, never a Rust
//! fallback lowercase copy or alternative production matching algorithm.

use crate::MojoError;

unsafe extern "C" {
    fn prodex_mojo_log_ascii_casefold_contains_v1(
        abi_version: i64,
        haystack_address: u64,
        haystack_length: i64,
        needle_address: u64,
        needle_length: i64,
        scratch_address: u64,
        scratch_capacity: i64,
        result_address: u64,
    ) -> i64;
}

/// Same byte-preserving contract as ASCII-only lowercase substring search:
/// non-ASCII UTF-8 bytes remain exact and uppercase ASCII folds to lowercase.
pub fn log_contains_ignore_ascii_case(haystack: &str, needle: &str) -> Result<bool, MojoError> {
    let mut scratch = vec![0_i64; needle.len()];
    let mut answer = -1_i64;
    let status = unsafe {
        prodex_mojo_log_ascii_casefold_contains_v1(
            1,
            haystack.as_ptr() as usize as u64,
            i64::try_from(haystack.len()).map_err(|_| MojoError::InvalidInput)?,
            needle.as_ptr() as usize as u64,
            i64::try_from(needle.len()).map_err(|_| MojoError::InvalidInput)?,
            scratch.as_mut_ptr() as usize as u64,
            i64::try_from(scratch.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut answer as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    match answer {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_log_ascii_casefold_search_preserves_unicode_and_empty_needle() {
        for (haystack, needle, expected) in [
            ("", "", true),
            ("", "a", false),
            ("A", "", true),
            ("ERROR: Quota exhausted", "quota", true),
            ("Reasoning TOKENS", "tokens", true),
            ("abc", "ABCD", false),
            ("ababababac", "ABABAC", true),
            ("ababababac", "ababad", false),
            ("🧭abc東京🔥", "ABC東京🔥", true),
            ("東京", "东京", false),
            ("Straße", "STRASSE", false),
            ("éÉ", "éé", false),
            ("🌍a🚀b", "A🚀B", true),
            ("tool-call:Git", "TOOL-CALL:git", true),
            ("a\\nb\\nc", "B\\nC", true),
        ] {
            assert_eq!(
                log_contains_ignore_ascii_case(haystack, needle),
                Ok(expected),
                "{haystack:?}/{needle:?}",
            );
        }
        let repeated = "a".repeat(256 * 1024);
        let haystack = format!("{repeated}b");
        let needle = format!("{}B", "A".repeat(1024));
        assert_eq!(log_contains_ignore_ascii_case(&haystack, &needle), Ok(true));
        assert_eq!(
            log_contains_ignore_ascii_case(&repeated, &needle),
            Ok(false),
        );
    }

    #[test]
    fn compiled_log_ascii_search_matches_test_only_rust_oracle() {
        // The old expression is test-only, never a production fallback.
        for haystack in [
            "",
            "Zz",
            "ABabababABCABC",
            "📦 Unicode 雪 xYz",
            "QUOTA blocked Weekly",
            "❤💙éÉ aA",
            "𝒜lice",
            "🧭".repeat(80).as_str(),
        ] {
            for needle in [
                "",
                "a",
                "AB",
                "xyz",
                "雪",
                "É",
                "é",
                "QUOTA",
                "quota blocked",
                "not present",
                "🧭🧭",
            ] {
                let expected = haystack
                    .to_ascii_lowercase()
                    .contains(&needle.to_ascii_lowercase());
                assert_eq!(
                    log_contains_ignore_ascii_case(haystack, needle),
                    Ok(expected),
                    "{haystack:?}/{needle:?}",
                );
            }
        }
    }

    #[test]
    fn compiled_log_ascii_search_abi_rejects_malformed_shapes_before_writing() {
        let text = b"example";
        let src = text.as_ptr() as usize as u64;
        let mut scratch = [72_i64; 7];
        let buffer = scratch.as_mut_ptr() as usize as u64;
        let mut result = 91_i64;
        let out = (&mut result as *mut i64) as usize as u64;
        let call = |version, text, text_len, pattern, pattern_len, scratch, cap, dst| unsafe {
            prodex_mojo_log_ascii_casefold_contains_v1(
                version,
                text,
                text_len,
                pattern,
                pattern_len,
                scratch,
                cap,
                dst,
            )
        };
        assert_eq!(call(2, src, 7, src, 7, buffer, 7, out), 4);
        assert_eq!(call(1, 0, 7, src, 7, buffer, 7, out), 1);
        assert_eq!(call(1, src, -1, src, 7, buffer, 7, out), 1);
        assert_eq!(call(1, src, 7, 0, 7, buffer, 7, out), 1);
        assert_eq!(call(1, src, 7, src, -1, buffer, 7, out), 1);
        assert_eq!(call(1, src, 7, src, 7, 0, 7, out), 1);
        assert_eq!(call(1, src, 7, src, 7, buffer, 6, out), 1);
        assert_eq!(call(1, src, 7, src, 7, buffer, 7, 0), 1);
        let invalid = [0xff_u8];
        assert_eq!(
            call(
                1,
                invalid.as_ptr() as usize as u64,
                1,
                src,
                7,
                buffer,
                7,
                out
            ),
            1
        );
        assert_eq!(result, 91);
        assert_eq!(scratch, [72; 7]);
    }
}
