//! Compiled streaming search over the virtual space-joined log event fields.
//! The Rust host formats scalar metadata but never materializes the huge join.
use super::LogStringView;
use crate::MojoError;

const ABI_VERSION: i64 = 1;
const MAX_FIELD_COUNT: usize = 9;
const MAX_FIELD_BYTES: usize = 1_048_576;

unsafe extern "C" {
    fn prodex_mojo_log_virtual_fields_contains_v1(
        abi_version: i64,
        fields_address: u64,
        field_count: i64,
        needle_address: u64,
        needle_length: i64,
        scratch_address: u64,
        scratch_capacity: i64,
        result_address: u64,
    ) -> i64;
}

/// Exactly the ASCII case-insensitive `fields.join(" ").contains(needle)`
/// semantics, without copying the virtual concatenation.
pub fn log_virtual_fields_contains(fields: &[&str], needle: &str) -> Result<bool, MojoError> {
    if fields.is_empty()
        || fields.len() > MAX_FIELD_COUNT
        || needle.len() > MAX_FIELD_BYTES
        || fields.iter().any(|f| f.len() > MAX_FIELD_BYTES)
    {
        return Err(MojoError::InvalidInput);
    }
    let mut views = [LogStringView::default(); MAX_FIELD_COUNT];
    for (view, field) in views.iter_mut().zip(fields) {
        *view = LogStringView {
            ptr: field.as_ptr() as usize as u64,
            len: field.len() as u64,
        };
    }
    let mut scratch = vec![0_i64; needle.len()];
    let mut answer = -1_i64;
    let status = unsafe {
        prodex_mojo_log_virtual_fields_contains_v1(
            ABI_VERSION,
            views.as_ptr() as usize as u64,
            i64::try_from(fields.len()).map_err(|_| MojoError::InvalidInput)?,
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
    fn compiled_virtual_log_search_preserves_separator_and_empty_fields() {
        let fields = [
            "2026-10-21",
            "main",
            "",
            "http",
            "responses",
            "123",
            "50",
            "false",
            "🧭 hello World",
        ];
        for (query, expected) in [
            ("", true),
            ("MAIN", true),
            ("main  http", true),
            ("123 50 FALSE", true),
            ("responses 123", true),
            ("🧭 hello world", true),
            ("main http", false),
            ("missing", false),
            ("🧭 HELLO WORLD ", false),
        ] {
            assert_eq!(
                log_virtual_fields_contains(&fields, query),
                Ok(expected),
                "{query:?}"
            );
        }
    }

    #[test]
    fn compiled_virtual_log_search_supports_three_field_aggregates() {
        let fields = ["2026-10-21", "main", "payload"];
        assert_eq!(
            log_virtual_fields_contains(&fields, "main PAYLOAD"),
            Ok(true)
        );
        assert_eq!(
            log_virtual_fields_contains(&fields, "main  payload"),
            Ok(false)
        );
        assert_eq!(
            log_virtual_fields_contains(&fields, "2026-10-21 MAIN"),
            Ok(true)
        );
        assert_eq!(
            log_virtual_fields_contains(&[], ""),
            Err(MojoError::InvalidInput)
        );
        assert_eq!(
            log_virtual_fields_contains(&[""; MAX_FIELD_COUNT + 1], ""),
            Err(MojoError::InvalidInput)
        );
    }

    #[test]
    fn compiled_virtual_log_search_handles_long_repetitions_linearly() {
        let payload = format!("{}b", "a".repeat(128 * 1024));
        let fields = ["", "", "", "", "", "", "", "", payload.as_str()];
        assert_eq!(
            log_virtual_fields_contains(&fields, &format!("{}B", "a".repeat(500))),
            Ok(true)
        );
        assert_eq!(
            log_virtual_fields_contains(&fields, &"x".repeat(400)),
            Ok(false)
        );
        assert_eq!(
            log_virtual_fields_contains(&fields, &"x".repeat(MAX_FIELD_BYTES + 1)),
            Err(MojoError::InvalidInput)
        );
    }
}
