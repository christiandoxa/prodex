pub const CONTINUATION_TOUCH_SHOULD_PERSIST: i64 = 2;
pub const CONTINUATION_TERMINAL_STATUS: i64 = 3;
pub const CONTINUATION_TOUCH_PLAN: i64 = 5;
pub const CONTINUATION_SHOULD_REFRESH_VERIFIED: i64 = 6;
pub const CONTINUATION_SHOULD_PERSIST_TOUCH: i64 = 7;
pub const CONTINUATION_VERIFY_PLAN: i64 = 8;
pub const CONTINUATION_SUSPECT_PLAN: i64 = 9;
pub const CONTINUATION_DEAD_PLAN: i64 = 10;
pub const CONTINUATION_RECENTLY_SUSPECT: i64 = 11;
pub const CONTINUATION_STALE_VERIFIED: i64 = 12;
pub const CONTINUATION_RETAIN_WITH_BINDING: i64 = 13;
pub const CONTINUATION_RETAIN_WITHOUT_BINDING: i64 = 14;
pub const CONTINUATION_DEAD_SHADOWED: i64 = 16;
pub const CONTINUATION_SHOULD_REPLACE: i64 = 18;
pub const CONTINUATION_RETENTION_KEY: i64 = 21;
pub const CONTINUATION_BINDING_SHOULD_RETAIN: i64 = 22;
pub const CONTINUATION_BINDING_RETENTION_KEY: i64 = 23;

unsafe extern "C" {
    fn prodex_runtime_continuation_status_transition_v1(
        abi_version: i64,
        operation: i64,
        fields_address: u64,
        field_count: i64,
        output_address: u64,
        output_count: i64,
    ) -> i64;
}

pub fn continuation_status_transition<const N: usize>(
    operation: i64,
    fields: &[i64],
) -> Result<[i64; N], crate::MojoError> {
    if fields.is_empty() || fields.len() > 32 || N == 0 || N > 12 {
        return Err(crate::MojoError::InvalidInput);
    }
    let mut output = [0_i64; N];
    let status = unsafe {
        prodex_runtime_continuation_status_transition_v1(
            1,
            operation,
            fields.as_ptr() as usize as u64,
            i64::try_from(fields.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(N).map_err(|_| crate::MojoError::InvalidInput)?,
        )
    };
    match status {
        0 => Ok(output),
        1 | 2 => Err(crate::MojoError::InvalidInput),
        4 => Err(crate::MojoError::AbiMismatch),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}
