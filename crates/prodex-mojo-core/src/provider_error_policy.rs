//! Typed adapter to canonical Mojo provider-error body policy.

use crate::MojoError;

const PROVIDER_ERROR_BODY_POLICY_ABI_VERSION: i64 = 2;
const OP_INCLUDE_UNSTRUCTURED_STATUS: i64 = 1;
const OP_SELECT_CANDIDATE: i64 = 2;

unsafe extern "C" {
    fn prodex_provider_error_body_policy_v2(
        abi_version: i64,
        operation: i64,
        http_status: i64,
        candidate_classes_address: u64,
        candidate_count: i64,
        result_address: u64,
    ) -> i64;
}

fn policy(operation: i64, status: i64, candidate_classes: &[i64]) -> Result<i64, MojoError> {
    let mut decision = -1_i64;
    let result = unsafe {
        prodex_provider_error_body_policy_v2(
            PROVIDER_ERROR_BODY_POLICY_ABI_VERSION,
            operation,
            status,
            candidate_classes.as_ptr() as usize as u64,
            i64::try_from(candidate_classes.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut decision as *mut i64) as usize as u64,
        )
    };
    match result {
        0 => Ok(decision),
        4 => Err(MojoError::AbiMismatch),
        1 => Err(MojoError::InvalidInput),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Whether a bare HTTP status and free-form text are eligible to classify an
/// error. HTTP 429 explicitly requires structured provider error codes.
pub fn provider_error_body_include_unstructured_status(status: u16) -> Result<bool, MojoError> {
    match policy(OP_INCLUDE_UNSTRUCTURED_STATUS, i64::from(status), &[])? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Selects the highest-precedence classification, preserving the first tie.
pub fn provider_error_body_select_candidate(classes: &[i64]) -> Result<Option<usize>, MojoError> {
    let index = policy(OP_SELECT_CANDIDATE, 0, classes)?;
    if index == -1 {
        return Ok(None);
    }
    usize::try_from(index)
        .ok()
        .filter(|index| *index < classes.len())
        .map(Some)
        .ok_or(MojoError::InvalidOutput)
}

#[cfg(test)]
mod tests {
    use super::{
        provider_error_body_include_unstructured_status, provider_error_body_select_candidate,
    };
    use crate::MojoError;

    #[test]
    fn body_policy_selects_most_specific_candidate_and_keeps_first_tie() {
        assert_eq!(
            provider_error_body_select_candidate(&[3, 4, 2, 2]).unwrap(),
            Some(2)
        );
        assert_eq!(
            provider_error_body_select_candidate(&[1, 0]).unwrap(),
            Some(1)
        );
        assert_eq!(provider_error_body_select_candidate(&[]).unwrap(), None);
    }

    #[test]
    fn body_policy_rejects_invalid_candidates_and_requires_code_for_429() {
        assert_eq!(
            provider_error_body_select_candidate(&[5, 6]),
            Err(MojoError::InvalidInput)
        );
        assert_eq!(
            provider_error_body_select_candidate(&[-1]),
            Err(MojoError::InvalidInput)
        );
        assert!(!provider_error_body_include_unstructured_status(429).unwrap());
        assert!(provider_error_body_include_unstructured_status(400).unwrap());
        assert!(provider_error_body_include_unstructured_status(u16::MAX).unwrap());
    }
}
