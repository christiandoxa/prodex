//! Checked, borrowed source ranges for compiled NO_PROXY normalization.
//! Original order and casing are retained; Rust only materializes Mojo's ranges.
use crate::MojoError;

const ABI_VERSION: i64 = 1;
const MAX_SOURCES: usize = 4096;
const MAX_BYTES: usize = 1_048_576;
const MAX_TOKENS: usize = 16_384;

unsafe extern "C" {
    fn prodex_launch_proxy_bypass_plan_v1(
        abi_version: i64,
        sources_address: u64,
        source_count: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
}

/// Select normalized bypass tokens in first-seen order. Commas, Unicode
/// whitespace, and ASCII case-insensitive equality are Mojo-owned semantics.
pub fn normalize_proxy_bypass_candidates(sources: &[&str]) -> Result<Vec<String>, MojoError> {
    if sources.len() > MAX_SOURCES {
        return Err(MojoError::InvalidInput);
    }
    let mut byte_count = 0usize;
    let mut capacity = 0usize;
    let mut views = Vec::<u64>::with_capacity(sources.len().saturating_mul(2));
    for source in sources {
        byte_count = byte_count
            .checked_add(source.len())
            .ok_or(MojoError::InvalidInput)?;
        if byte_count > MAX_BYTES {
            return Err(MojoError::InvalidInput);
        }
        // Capacity counts possible token slots only, never ownership or dedup.
        capacity = capacity
            .checked_add(source.bytes().filter(|byte| *byte == b',').count() + 1)
            .ok_or(MojoError::InvalidInput)?;
        if capacity > MAX_TOKENS {
            return Err(MojoError::InvalidInput);
        }
        views.extend([
            source.as_ptr() as usize as u64,
            u64::try_from(source.len()).map_err(|_| MojoError::InvalidInput)?,
        ]);
    }
    let mut triples = vec![[0_i64; 3]; capacity];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_launch_proxy_bypass_plan_v1(
            ABI_VERSION,
            views.as_ptr() as usize as u64,
            i64::try_from(sources.len()).map_err(|_| MojoError::InvalidInput)?,
            triples.as_mut_ptr() as usize as u64,
            i64::try_from(triples.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        3 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let count = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if count > triples.len() {
        return Err(MojoError::InvalidOutput);
    }
    let mut tokens = Vec::with_capacity(count);
    for &[raw_source, raw_start, raw_length] in triples.iter().take(count) {
        let source_index = usize::try_from(raw_source).map_err(|_| MojoError::InvalidOutput)?;
        let source = sources.get(source_index).ok_or(MojoError::InvalidOutput)?;
        let start = usize::try_from(raw_start).map_err(|_| MojoError::InvalidOutput)?;
        let length = usize::try_from(raw_length).map_err(|_| MojoError::InvalidOutput)?;
        let end = start.checked_add(length).ok_or(MojoError::InvalidOutput)?;
        let token = source
            .get(start..end)
            .filter(|s| !s.is_empty())
            .ok_or(MojoError::InvalidOutput)?;
        tokens.push(token.to_string());
    }
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_proxy_normalization_preserves_case_and_unicode_whitespace() {
        assert_eq!(
            normalize_proxy_bypass_candidates(&[
                "  Example.COM,127.0.0.1,",
                " example.com,\u{2003}LocalHost\u{00a0}, ::1 ",
                "localhost,internal.local"
            ]),
            Ok(vec![
                "Example.COM".to_string(),
                "127.0.0.1".to_string(),
                "LocalHost".to_string(),
                "::1".to_string(),
                "internal.local".to_string()
            ])
        );
        assert_eq!(normalize_proxy_bypass_candidates(&[]), Ok(vec![]));
        assert_eq!(normalize_proxy_bypass_candidates(&[", , "]), Ok(vec![]));
        assert_eq!(
            normalize_proxy_bypass_candidates(&["a", "A", "a,b", "B"]),
            Ok(vec!["a".to_string(), "b".to_string()])
        );
    }

    #[test]
    fn compiled_proxy_normalization_rejects_unbounded_input() {
        let long = "a".repeat(MAX_BYTES + 1);
        assert_eq!(
            normalize_proxy_bypass_candidates(&[&long]),
            Err(MojoError::InvalidInput)
        );
    }
}
