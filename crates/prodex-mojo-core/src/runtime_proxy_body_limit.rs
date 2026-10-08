use crate::MojoError;

const ABI_VERSION: i64 = 1;

unsafe extern "C" {
    fn prodex_runtime_proxy_body_limit_exceeds_v1(
        abi_version: i64,
        limit_bytes: u64,
        observed_bytes: u64,
    ) -> i64;
}

/// Return whether an observed runtime proxy request body exceeds its byte limit.
pub fn runtime_proxy_body_size_exceeds_limit(
    limit_bytes: u64,
    observed_bytes: u64,
) -> Result<bool, MojoError> {
    match unsafe {
        prodex_runtime_proxy_body_limit_exceeds_v1(ABI_VERSION, limit_bytes, observed_bytes)
    } {
        0 => Ok(false),
        1 => Ok(true),
        -1 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}
