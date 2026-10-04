unsafe extern "C" {
    fn prodex_quota_reset_epoch_v1(fields_address: u64, output_address: u64) -> i64;
}

/// JSON reset candidates projected by the Rust parser for Mojo precedence.
///
/// The v1 ABI orders candidates as `resets_at`, `reset_at`,
/// `error.resets_at`, `error.reset_at`, then primary/secondary header reset
/// epochs and used percentages.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct QuotaResetEpochInput {
    pub resets_at: Option<i64>,
    pub reset_at: Option<i64>,
    pub error_resets_at: Option<i64>,
    pub error_reset_at: Option<i64>,
    pub primary_reset_at: Option<i64>,
    pub secondary_reset_at: Option<i64>,
    pub primary_used_percent: Option<i64>,
    pub secondary_used_percent: Option<i64>,
}

/// Apply JSON reset-candidate precedence in the real Mojo quota kernel.
pub fn quota_reset_epoch_precedence(
    input: QuotaResetEpochInput,
) -> Result<Option<i64>, crate::MojoError> {
    let fields = [
        input.resets_at.unwrap_or_default(),
        i64::from(input.resets_at.is_some()),
        input.reset_at.unwrap_or_default(),
        i64::from(input.reset_at.is_some()),
        input.error_resets_at.unwrap_or_default(),
        i64::from(input.error_resets_at.is_some()),
        input.error_reset_at.unwrap_or_default(),
        i64::from(input.error_reset_at.is_some()),
        input.primary_reset_at.unwrap_or_default(),
        i64::from(input.primary_reset_at.is_some()),
        input.secondary_reset_at.unwrap_or_default(),
        i64::from(input.secondary_reset_at.is_some()),
        input.primary_used_percent.unwrap_or_default(),
        i64::from(input.primary_used_percent.is_some()),
        input.secondary_used_percent.unwrap_or_default(),
        i64::from(input.secondary_used_percent.is_some()),
    ];
    let mut output = [0_i64; 2];
    let status = unsafe {
        prodex_quota_reset_epoch_v1(
            fields.as_ptr() as usize as u64,
            output.as_mut_ptr() as usize as u64,
        )
    };
    if status == 1 || status == 2 {
        return Err(crate::MojoError::InvalidInput);
    }
    if status != 0 || !matches!(output[1], 0 | 1) {
        return Err(crate::MojoError::InvalidOutput);
    }
    Ok((output[1] == 1).then_some(output[0]))
}
