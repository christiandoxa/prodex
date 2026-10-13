use super::{QUOTA_MODEL_POLICY_ABI_VERSION, quota_model_policy_status};

unsafe extern "C" {
    fn prodex_quota_retry_time_plan_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
        kind_address: u64,
    ) -> i64;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuotaRetryTimeCandidate {
    LocalClock(String),
    LocalDateTime(String),
}

/// Decode the retry-time shape with the compiled Mojo quota policy. Chrono
/// timezones are resolved by the caller, not by the deterministic Mojo kernel.
pub fn quota_retry_time_candidate(
    candidate: &str,
) -> Result<Option<QuotaRetryTimeCandidate>, crate::MojoError> {
    let mut output = [0_u8; 1024];
    let mut written = -1_i64;
    let mut kind = -1_i64;
    let status = unsafe {
        prodex_quota_retry_time_plan_v1(
            QUOTA_MODEL_POLICY_ABI_VERSION,
            candidate.as_ptr() as usize as u64,
            i64::try_from(candidate.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
            (&mut kind as *mut i64) as usize as u64,
        )
    };
    quota_model_policy_status(status)?;
    let written = usize::try_from(written).map_err(|_| crate::MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(crate::MojoError::InvalidOutput);
    }
    let normalized = String::from_utf8(output[..written].to_vec())
        .map_err(|_| crate::MojoError::InvalidOutput)?;
    match kind {
        0 if written == 0 => Ok(None),
        1 => Ok(Some(QuotaRetryTimeCandidate::LocalClock(normalized))),
        2 => Ok(Some(QuotaRetryTimeCandidate::LocalDateTime(normalized))),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}
