//! Compiled Codex version-probe selection and printable User-Agent projection.
use crate::MojoError;

const ABI_VERSION: i64 = 1;
const MAX_BYTES: usize = 1_048_576;

unsafe extern "C" {
    fn prodex_codex_auth_probe_version_span_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        result_address: u64,
    ) -> i64;
    fn prodex_codex_auth_user_agent_ascii_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
}

fn status(value: i64) -> Result<(), MojoError> {
    match value {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        3 => Err(MojoError::Capacity),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Select the rightmost whitespace-delimited version-like token and trim
/// trailing punctuation. The host copies the Mojo-selected source span.
pub fn codex_probe_version(output: &str) -> Result<Option<&str>, MojoError> {
    if output.len() > MAX_BYTES {
        return Err(MojoError::InvalidInput);
    }
    let mut result = [-2_i64; 2];
    status(unsafe {
        prodex_codex_auth_probe_version_span_v1(
            ABI_VERSION,
            output.as_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            result.as_mut_ptr() as usize as u64,
        )
    })?;
    if result == [-1, -1] {
        return Ok(None);
    }
    let start = usize::try_from(result[0]).map_err(|_| MojoError::InvalidOutput)?;
    let end = usize::try_from(result[1]).map_err(|_| MojoError::InvalidOutput)?;
    if start >= end {
        return Err(MojoError::InvalidOutput);
    }
    Ok(Some(
        output.get(start..end).ok_or(MojoError::InvalidOutput)?,
    ))
}

/// Replace every non-printable/non-ASCII scalar with exactly one underscore.
/// Host HTTP HeaderValue validity checks remain Rust-owned.
pub fn codex_user_agent_printable(candidate: &str) -> Result<String, MojoError> {
    if candidate.len() > MAX_BYTES {
        return Err(MojoError::InvalidInput);
    }
    let mut output = vec![0_u8; candidate.len()];
    let mut written = -1_i64;
    status(unsafe {
        prodex_codex_auth_user_agent_ascii_v1(
            ABI_VERSION,
            candidate.as_ptr() as usize as u64,
            i64::try_from(candidate.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    })?;
    let size = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if size > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    output.truncate(size);
    String::from_utf8(output).map_err(|_| MojoError::InvalidOutput)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_codex_version_probe_retains_rightmost_candidate() {
        for (value, expected) in [
            ("codex-cli 0.142.5\n", Some("0.142.5")),
            ("codex-cli 1.2.3 5.6.7,", Some("5.6.7")),
            ("v1.2.3 2.3.4!", Some("2.3.4")),
            (" 0.142.5\u{2003}2.0.0-BETA❄ ", Some("2.0.0-BETA")),
            ("foo\t2.3.4-alpha.1", Some("2.3.4-alpha.1")),
            ("v1.2.3", None),
            ("unknown", None),
            ("", None),
        ] {
            assert_eq!(codex_probe_version(value), Ok(expected), "{value:?}");
        }
        assert_eq!(
            codex_probe_version(&"x".repeat(MAX_BYTES + 1)),
            Err(MojoError::InvalidInput)
        );
    }

    #[test]
    fn compiled_codex_agent_projection_is_one_char_per_unicode_scalar() {
        assert_eq!(
            codex_user_agent_printable("hello world"),
            Ok("hello world".into())
        );
        assert_eq!(
            codex_user_agent_printable("Codex/1.0 🚀\tα\n"),
            Ok("Codex/1.0 ____".into())
        );
        assert_eq!(codex_user_agent_printable(""), Ok(String::new()));
        assert_eq!(
            codex_user_agent_printable(&"x".repeat(MAX_BYTES + 1)),
            Err(MojoError::InvalidInput)
        );
    }
}
