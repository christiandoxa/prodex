//! Borrowed canonical versions selected and classified by compiled Mojo.
use crate::MojoError;

const MAX_INPUT_BYTES: usize = 1_048_576;

unsafe extern "C" {
    fn prodex_optional_tool_probe_version_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        minimum_address: u64,
        minimum_length: i64,
        output_address: u64,
    ) -> i64;
}

/// Classification of the first valid SemVer candidate, irrespective of later tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionalToolProbeVersion<'a> {
    Missing,
    TooOld(&'a str),
    Supported(&'a str),
}

/// Probe text and minimum are bounded to 1 MiB; Rust only validates ABI spans.
pub fn optional_tool_probe_version<'a>(
    text: &'a str,
    minimum: &str,
) -> Result<OptionalToolProbeVersion<'a>, MojoError> {
    if text.len() > MAX_INPUT_BYTES || minimum.len() > MAX_INPUT_BYTES {
        return Err(MojoError::InvalidInput);
    }
    let mut output = [-1_i64; 3];
    match unsafe {
        prodex_optional_tool_probe_version_v1(
            1,
            text.as_ptr() as u64,
            text.len() as i64,
            minimum.as_ptr() as u64,
            minimum.len() as i64,
            output.as_mut_ptr() as u64,
        )
    } {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    match output {
        [0, 0, 0] => Ok(OptionalToolProbeVersion::Missing),
        [tag @ (1 | 2), start, end] => {
            let start = usize::try_from(start).map_err(|_| MojoError::InvalidOutput)?;
            let end = usize::try_from(end).map_err(|_| MojoError::InvalidOutput)?;
            let version = text
                .get(start..end)
                .filter(|value| !value.is_empty())
                .ok_or(MojoError::InvalidOutput)?;
            Ok(if tag == 1 {
                OptionalToolProbeVersion::TooOld(version)
            } else {
                OptionalToolProbeVersion::Supported(version)
            })
        }
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_probe_version_preserves_token_priority_and_compatibility() {
        use OptionalToolProbeVersion::*;
        for (text, minimum, expected) in [
            (
                " tool\u{2003}v1.2.3\u{a0}[vv2.3.4] ",
                "1.2.3",
                Supported("1.2.3"),
            ),
            ("[vv2.3.4]", "2.3.4", Supported("2.3.4")),
            ("0.0.1 999.0.0", "1.0.0", TooOld("0.0.1")),
            ("1.2.3-rc.1 1.2.3", "1.2.3", TooOld("1.2.3-rc.1")),
            ("invalid 01.2.3 1.2.3", "1.2.3", Supported("1.2.3")),
            ("1.2.3+build.01", "1.2.3", Supported("1.2.3+build.01")),
            ("V1.2.3", "1.2.3", Missing),
            ("18446744073709551616.0.0", "1.0.0", Missing),
            ("\u{2002} ", "1.0.0", Missing),
            ("", "1.0.0", Missing),
        ] {
            assert_eq!(
                optional_tool_probe_version(text, minimum),
                Ok(expected),
                "{text}"
            );
        }
    }

    #[test]
    fn compiled_probe_version_rejects_invalid_minimum_and_size() {
        assert_eq!(
            optional_tool_probe_version("1.2.3", "invalid"),
            Err(MojoError::InvalidInput)
        );
        assert_eq!(
            optional_tool_probe_version(&"x".repeat(MAX_INPUT_BYTES + 1), "1.0.0"),
            Err(MojoError::InvalidInput)
        );
        assert_eq!(
            optional_tool_probe_version("1.2.3", &"x".repeat(MAX_INPUT_BYTES + 1)),
            Err(MojoError::InvalidInput)
        );
    }
}
