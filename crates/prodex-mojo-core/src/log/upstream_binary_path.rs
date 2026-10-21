//! Exact field precedence and Unicode whitespace span for upstream binary logs.
use crate::MojoError;

const ABI_VERSION: i64 = 1;
const MAX_BYTES: usize = 1_048_576;

#[repr(C)]
#[derive(Clone, Copy)]
struct PathFieldView {
    address: u64,
    length: u64,
    present: u64,
}

unsafe extern "C" {
    fn prodex_mojo_upstream_binary_path_plan_v1(
        abi_version: i64,
        field_views_address: u64,
        field_count: i64,
        output_address: u64,
    ) -> i64;
}

/// Return the selected span, preserving first-present field precedence even
/// when it is empty. No Rust search, trim, or fallback recomputes that policy.
pub fn upstream_binary_path(values: [Option<&str>; 7]) -> Result<Option<&str>, MojoError> {
    if values.iter().flatten().any(|value| value.len() > MAX_BYTES) {
        return Err(MojoError::InvalidInput);
    }
    let views = values.map(|value| match value {
        Some(value) => PathFieldView {
            address: value.as_ptr() as usize as u64,
            length: value.len() as u64,
            present: 1,
        },
        None => PathFieldView {
            address: 0,
            length: 0,
            present: 0,
        },
    });
    let mut output = [-1_i64; 3];
    let status = unsafe {
        prodex_mojo_upstream_binary_path_plan_v1(
            ABI_VERSION,
            views.as_ptr() as usize as u64,
            7,
            output.as_mut_ptr() as usize as u64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    if output == [-1; 3] {
        return Ok(None);
    }
    let index = usize::try_from(output[0]).map_err(|_| MojoError::InvalidOutput)?;
    let start = usize::try_from(output[1]).map_err(|_| MojoError::InvalidOutput)?;
    let end = usize::try_from(output[2]).map_err(|_| MojoError::InvalidOutput)?;
    let source = values
        .get(index)
        .copied()
        .flatten()
        .ok_or(MojoError::InvalidOutput)?;
    let selected = source
        .get(start..end)
        .filter(|path| !path.is_empty())
        .ok_or(MojoError::InvalidOutput)?;
    Ok(Some(selected))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_upstream_path_policy_keeps_first_present_even_when_empty() {
        assert_eq!(upstream_binary_path([None; 7]), Ok(None));
        assert_eq!(
            upstream_binary_path([
                Some(" \u{2003}\u{00a0} "),
                Some("lower"),
                None,
                None,
                None,
                None,
                None
            ]),
            Ok(None)
        );
        assert_eq!(
            upstream_binary_path([
                None,
                Some(" \u{2003}file.json\u{00a0} "),
                Some("other"),
                None,
                None,
                None,
                None
            ]),
            Ok(Some("file.json"))
        );
        assert_eq!(
            upstream_binary_path([Some("path"), Some("other"), None, None, None, None, None]),
            Ok(Some("path"))
        );
        assert_eq!(
            upstream_binary_path([
                None,
                None,
                None,
                None,
                None,
                None,
                Some(" https://example.test/one ")
            ]),
            Ok(Some("https://example.test/one"))
        );
    }

    #[test]
    fn compiled_upstream_path_policy_bounds_each_value() {
        let too_long = "x".repeat(MAX_BYTES + 1);
        assert_eq!(
            upstream_binary_path([Some(&too_long), None, None, None, None, None, None]),
            Err(MojoError::InvalidInput)
        );
    }
}
