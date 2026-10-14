//! Mojo-owned blocked-window selection for quota status rendering.

pub const QUOTA_BLOCKED_WINDOW_KIND_MAIN: i64 = 0;
pub const QUOTA_BLOCKED_WINDOW_KIND_ADDITIONAL: i64 = 1;
pub const QUOTA_BLOCKED_WINDOW_KIND_CODE_REVIEW: i64 = 2;
pub const QUOTA_BLOCKED_WINDOW_BATCH_MAX_COUNT: usize = 1_024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuotaBlockedWindowInput {
    pub kind: i64,
    pub used_percent: Option<i64>,
}

/// Selects the quota windows that should become blocked status rows.
///
/// Rust supplies normalized provider observations and keeps ownership of the
/// resulting text. Mojo owns the deterministic readiness and exhaustion policy.
pub fn quota_blocked_window_plan(
    runtime_ready: bool,
    include_code_review: bool,
    inputs: &[QuotaBlockedWindowInput],
) -> Result<Vec<bool>, crate::MojoError> {
    inputs
        .chunks(QUOTA_BLOCKED_WINDOW_BATCH_MAX_COUNT)
        .try_fold(Vec::with_capacity(inputs.len()), |mut planned, chunk| {
            planned.extend(quota_blocked_window_batch(
                runtime_ready,
                include_code_review,
                chunk,
            )?);
            Ok(planned)
        })
}

fn quota_blocked_window_batch(
    runtime_ready: bool,
    include_code_review: bool,
    inputs: &[QuotaBlockedWindowInput],
) -> Result<Vec<bool>, crate::MojoError> {
    let kinds = inputs.iter().map(|input| input.kind).collect::<Vec<_>>();
    let used_percent = inputs
        .iter()
        .map(|input| input.used_percent.unwrap_or_default())
        .collect::<Vec<_>>();
    let used_present = inputs
        .iter()
        .map(|input| i64::from(input.used_percent.is_some()))
        .collect::<Vec<_>>();
    let mut blocked = vec![0_i64; inputs.len()];
    let status = unsafe {
        prodex_quota_blocked_window_plan_v1(
            1,
            i64::from(runtime_ready),
            i64::from(include_code_review),
            kinds.as_ptr(),
            used_percent.as_ptr(),
            used_present.as_ptr(),
            blocked.as_mut_ptr(),
            i64::try_from(inputs.len()).map_err(|_| crate::MojoError::InvalidInput)?,
        )
    };
    match status {
        0 => {}
        1 => return Err(crate::MojoError::InvalidInput),
        4 => return Err(crate::MojoError::AbiMismatch),
        _ => return Err(crate::MojoError::InvalidOutput),
    }
    blocked
        .into_iter()
        .map(|value| match value {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(crate::MojoError::InvalidOutput),
        })
        .collect()
}

unsafe extern "C" {
    fn prodex_quota_blocked_window_plan_v1(
        abi_version: i64,
        runtime_ready: i64,
        include_code_review: i64,
        kind_address: *const i64,
        used_percent_address: *const i64,
        used_present_address: *const i64,
        blocked_address: *mut i64,
        count: i64,
    ) -> i64;
}
