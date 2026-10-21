//! Bounded compiled profile-journal filename and lifecycle operation policy.
use crate::MojoError;

const ABI_VERSION: i64 = 1;
const MAX_BYTES: usize = 1_048_576;

unsafe extern "C" {
    fn prodex_profile_export_journal_token_policy_v1(
        abi_version: i64,
        operation: i64,
        value_address: u64,
        value_length: i64,
        output_address: u64,
    ) -> i64;
}

fn classify(operation: i64, value: &str) -> Result<bool, MojoError> {
    if value.len() > MAX_BYTES {
        return Err(MojoError::InvalidInput);
    }
    let mut selected = -1_i64;
    let status = unsafe {
        prodex_profile_export_journal_token_policy_v1(
            ABI_VERSION,
            operation,
            value.as_ptr() as usize as u64,
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut selected as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    match selected {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Accept exactly the old ASCII identifier alphabet, excluding "." and "..".
pub fn journal_filename_component_valid(value: &str) -> Result<bool, MojoError> {
    classify(0, value)
}

/// Accept only import/login/manage/remove operation strings, exact case.
pub fn journal_lifecycle_operation_valid(value: &str) -> Result<bool, MojoError> {
    classify(1, value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_journal_component_policy_preserves_strict_ascii() {
        for (value, valid) in [
            ("a", true),
            ("a_B-7.json", true),
            ("a..", true),
            ("-a", true),
            ("", false),
            (".", false),
            ("..", false),
            ("../token", false),
            ("a\\b", false),
            ("a/b", false),
            ("unsafe space", false),
            ("é", false),
            ("日本語", false),
            ("a\0b", false),
        ] {
            assert_eq!(
                journal_filename_component_valid(value),
                Ok(valid),
                "{value:?}"
            );
        }
        assert_eq!(
            journal_filename_component_valid(&"x".repeat(MAX_BYTES + 1)),
            Err(MojoError::InvalidInput)
        );
    }

    #[test]
    fn compiled_lifecycle_operation_policy_preserves_exact_verbs() {
        for (value, valid) in [
            ("import", true),
            ("login", true),
            ("manage", true),
            ("remove", true),
            ("Import", false),
            ("logout", false),
            ("", false),
            ("...", false),
        ] {
            assert_eq!(
                journal_lifecycle_operation_valid(value),
                Ok(valid),
                "{value:?}"
            );
        }
    }
}
