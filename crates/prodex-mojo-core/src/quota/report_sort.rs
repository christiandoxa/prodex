use super::{QUOTA_MODEL_POLICY_ABI_VERSION, prodex_quota_report_compare_v1};

unsafe extern "C" {
    fn prodex_quota_report_sort_next_v1(abi_version: i64, sort: i64) -> i64;
}

#[allow(clippy::too_many_arguments)]
pub fn quota_report_compare(
    sort: i64,
    left_active: bool,
    right_active: bool,
    left_status_rank: i64,
    right_status_rank: i64,
    left_reset_epoch: i64,
    right_reset_epoch: i64,
    left_profile: &str,
    right_profile: &str,
    left_auth: &str,
    right_auth: &str,
    left_account: &str,
    right_account: &str,
    left_plan: &str,
    right_plan: &str,
) -> Result<i64, crate::MojoError> {
    let length =
        |value: &str| i64::try_from(value.len()).map_err(|_| crate::MojoError::InvalidInput);
    let value = unsafe {
        prodex_quota_report_compare_v1(
            QUOTA_MODEL_POLICY_ABI_VERSION,
            sort,
            i64::from(left_active),
            i64::from(right_active),
            left_status_rank,
            right_status_rank,
            left_reset_epoch,
            right_reset_epoch,
            left_profile.as_ptr() as usize as u64,
            length(left_profile)?,
            right_profile.as_ptr() as usize as u64,
            length(right_profile)?,
            left_auth.as_ptr() as usize as u64,
            length(left_auth)?,
            right_auth.as_ptr() as usize as u64,
            length(right_auth)?,
            left_account.as_ptr() as usize as u64,
            length(left_account)?,
            right_account.as_ptr() as usize as u64,
            length(right_account)?,
            left_plan.as_ptr() as usize as u64,
            length(left_plan)?,
            right_plan.as_ptr() as usize as u64,
            length(right_plan)?,
        )
    };
    match value {
        -1..=1 => Ok(value),
        -2 => Err(crate::MojoError::InvalidInput),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}

pub fn quota_report_sort_next(sort: i64) -> Result<i64, crate::MojoError> {
    let value = unsafe { prodex_quota_report_sort_next_v1(QUOTA_MODEL_POLICY_ABI_VERSION, sort) };
    (0..=5)
        .contains(&value)
        .then_some(value)
        .ok_or(crate::MojoError::InvalidOutput)
}
