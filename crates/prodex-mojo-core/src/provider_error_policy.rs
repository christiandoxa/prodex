//! Typed adapter to the canonical Mojo provider-error body precedence policy.

use crate::MojoError;

const PROVIDER_ERROR_BODY_POLICY_ABI_VERSION: i64 = 1;
const OP_INCLUDE_UNSTRUCTURED_STATUS: i64 = 1;
const OP_PREFER_CLASSIFICATION: i64 = 2;

unsafe extern "C" {
    fn prodex_provider_error_body_policy_v1(
        abi_version: i64,
        operation: i64,
        http_status: i64,
        previous_class: i64,
        incoming_class: i64,
        result_address: u64,
    ) -> i64;
}

fn policy(operation: i64, status: i64, previous: i64, incoming: i64) -> Result<bool, MojoError> {
    let mut decision = -1_i64;
    let result = unsafe {
        prodex_provider_error_body_policy_v1(
            PROVIDER_ERROR_BODY_POLICY_ABI_VERSION,
            operation,
            status,
            previous,
            incoming,
            (&mut decision as *mut i64) as usize as u64,
        )
    };
    if result == 4 {
        return Err(MojoError::AbiMismatch);
    }
    if result != 0 {
        return Err(MojoError::InvalidInput);
    }
    match decision {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Whether a bare HTTP status and free-form text are eligible to classify an
/// error. HTTP 429 explicitly requires structured provider error codes.
pub fn provider_error_body_include_unstructured_status(status: u16) -> Result<bool, MojoError> {
    policy(OP_INCLUDE_UNSTRUCTURED_STATUS, i64::from(status), 0, 0)
}

/// Returns true only when the incoming provider classification outranks the
/// previous one. Equal ranks must preserve the earliest cooldown and class.
pub fn provider_error_body_prefer_classification(
    previous: i64,
    incoming: i64,
) -> Result<bool, MojoError> {
    policy(OP_PREFER_CLASSIFICATION, 0, previous, incoming)
}
