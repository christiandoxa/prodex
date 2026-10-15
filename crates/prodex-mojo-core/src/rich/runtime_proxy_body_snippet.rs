//! Typed Rust FFI adapter: Mojo owns runtime proxy snippet semantics.

use super::{ensure_rich_abi, mojo_mut_pointer_address, mojo_pointer_address, view};
use crate::MojoError;

unsafe extern "C" {
    fn prodex_mojo_rich_runtime_proxy_body_snippet_v1(
        abi_version: i64,
        input_view: u64,
        maximum_chars: u64,
        output: u64,
        output_capacity: i64,
        written: u64,
    ) -> i64;
}

/// Return a bounded display snippet for an already UTF-8 validated body.
/// No Rust Unicode splitting, whitespace joining, or fallback truncation.
pub fn runtime_proxy_body_snippet(text: &str, maximum_chars: usize) -> Result<String, MojoError> {
    ensure_rich_abi()?;
    let capacity = text.len().checked_add(4).ok_or(MojoError::InvalidInput)?;
    let mut output = vec![0_u8; capacity];
    let input = view(text);
    let mut written = -1_i64;
    let status = unsafe {
        prodex_mojo_rich_runtime_proxy_body_snippet_v1(
            super::RICH_ABI_VERSION,
            mojo_pointer_address(&input),
            u64::try_from(maximum_chars).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(output.as_mut_ptr()),
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut written),
        )
    };
    if status != 0 {
        return Err(match status {
            1 => MojoError::InvalidInput,
            2 => MojoError::InvalidOutput,
            3 => MojoError::Capacity,
            4 => MojoError::AbiMismatch,
            _ => MojoError::InvalidOutput,
        });
    }
    let count = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if count > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    output.truncate(count);
    String::from_utf8(output).map_err(|_| MojoError::InvalidOutput)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mojo_proxy_snippet_unicode_normalization_and_codepoint_boundaries() {
        for (input, maximum, expected) in [
            (" one\n two\tthree ", 7, "one two..."),
            ("  \n\t  ", 7, "-"),
            ("", 0, "-"),
            ("alpha", 0, "..."),
            ("alpha", 5, "alpha"),
            ("alpha  ", 5, "alpha"),
            ("alpha beta", 6, "alpha ..."),
            ("🧭東京", 1, "🧭..."),
            ("\u{2003}alpha\u{00a0}🧭\u{2029}東京 ", 20, "alpha 🧭 東京"),
            ("x\u{200b}y", 3, "x\u{200b}y"),
        ] {
            assert_eq!(
                runtime_proxy_body_snippet(input, maximum).unwrap(),
                expected,
                "{input:?} maximum={maximum}",
            );
        }
    }

    #[test]
    fn mojo_proxy_snippet_rejects_invalid_abi_and_capacity() {
        let input = view("hi");
        let mut output = [99_u8; 4];
        let mut written = 77_i64;
        unsafe {
            assert_eq!(
                prodex_mojo_rich_runtime_proxy_body_snippet_v1(
                    super::super::RICH_ABI_VERSION + 1,
                    mojo_pointer_address(&input),
                    4,
                    mojo_mut_pointer_address(output.as_mut_ptr()),
                    4,
                    mojo_mut_pointer_address(&mut written),
                ),
                4,
            );
            assert_eq!(
                prodex_mojo_rich_runtime_proxy_body_snippet_v1(
                    super::super::RICH_ABI_VERSION,
                    mojo_pointer_address(&input),
                    4,
                    mojo_mut_pointer_address(output.as_mut_ptr()),
                    0,
                    mojo_mut_pointer_address(&mut written),
                ),
                1,
            );
        }
        assert_eq!(output, [99; 4]);
        assert_eq!(written, 77);
    }
}
