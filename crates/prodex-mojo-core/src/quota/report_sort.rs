use super::{QUOTA_DISPLAY_LABEL_SORT, load_quota_display_label};
use super::{QUOTA_MODEL_POLICY_ABI_VERSION, prodex_quota_report_compare_v1};

unsafe extern "C" {
    fn prodex_quota_report_sort_next_v1(abi_version: i64, sort: i64) -> i64;
    fn prodex_quota_report_page_plan_v1(
        abi_version: i64,
        total_profiles: u64,
        requested_start: u64,
        shown_profiles: u64,
        existing_lines: u64,
        section_lines: u64,
        line_limit: u64,
        limit_present: i64,
        output: *mut u64,
    ) -> i64;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuotaReportPagePlan {
    pub start_profile: usize,
    pub hidden_before: usize,
    pub hidden_after: usize,
    pub show_notice: bool,
    pub append_section: bool,
}

/// Obtain the sole viewport/pagination decision from the compiled Mojo policy.
/// Rust only supplies rendered section line counts and applies the returned plan.
pub fn quota_report_page_plan(
    total_profiles: usize,
    requested_start: usize,
    shown_profiles: usize,
    existing_lines: usize,
    section_lines: usize,
    max_lines: Option<usize>,
) -> Result<QuotaReportPagePlan, crate::MojoError> {
    let as_u64 = |value: usize| u64::try_from(value).map_err(|_| crate::MojoError::InvalidInput);
    let mut output = [u64::MAX; 5];
    let status = unsafe {
        prodex_quota_report_page_plan_v1(
            1,
            as_u64(total_profiles)?,
            as_u64(requested_start)?,
            as_u64(shown_profiles)?,
            as_u64(existing_lines)?,
            as_u64(section_lines)?,
            max_lines.map(as_u64).transpose()?.unwrap_or_default(),
            i64::from(max_lines.is_some()),
            output.as_mut_ptr(),
        )
    };
    match status {
        0 => {}
        1 => return Err(crate::MojoError::InvalidInput),
        4 => return Err(crate::MojoError::AbiMismatch),
        _ => return Err(crate::MojoError::InvalidOutput),
    }
    let start = usize::try_from(output[0]).map_err(|_| crate::MojoError::InvalidOutput)?;
    let before = usize::try_from(output[1]).map_err(|_| crate::MojoError::InvalidOutput)?;
    let after = usize::try_from(output[2]).map_err(|_| crate::MojoError::InvalidOutput)?;
    if before != start
        || (total_profiles == 0 && start != 0)
        || (total_profiles > 0 && start >= total_profiles)
        || after > total_profiles
        || !matches!((output[3], output[4]), (0 | 1, 0 | 1))
    {
        return Err(crate::MojoError::InvalidOutput);
    }
    Ok(QuotaReportPagePlan {
        start_profile: start,
        hidden_before: before,
        hidden_after: after,
        show_notice: output[3] == 1,
        append_section: output[4] == 1,
    })
}

#[cfg(all(test, feature = "mojo-quota"))]
#[test]
fn quota_report_page_plan_handles_pagination_budget_and_scroll_edges() {
    assert_eq!(
        quota_report_page_plan(0, usize::MAX, 0, 0, 0, None).unwrap(),
        QuotaReportPagePlan {
            start_profile: 0,
            hidden_before: 0,
            hidden_after: 0,
            show_notice: false,
            append_section: false,
        }
    );
    let first = quota_report_page_plan(4, 1, 0, 5, 3, Some(12)).unwrap();
    assert_eq!(first.start_profile, 1);
    assert_eq!(first.hidden_before, 1);
    assert_eq!(first.hidden_after, 3);
    assert!(first.show_notice);
    assert!(first.append_section);
    let tight = quota_report_page_plan(4, 1, 1, 10, 2, Some(12)).unwrap();
    assert!(
        !tight.append_section,
        "reserve a line for hidden-profile notice"
    );
    assert!(tight.show_notice);
    let enough = quota_report_page_plan(4, 1, 2, 10, 1, Some(12)).unwrap();
    assert!(
        enough.append_section,
        "final section still reserves notice for above"
    );
    let max_start = quota_report_page_plan(4, usize::MAX, 0, 0, 1, None).unwrap();
    assert_eq!(max_start.start_profile, 3);
    assert_eq!(max_start.hidden_before, 3);
    assert_eq!(max_start.hidden_after, 1);
    let no_room = quota_report_page_plan(4, 1, 0, 12, 1, Some(12)).unwrap();
    assert!(!no_room.show_notice);
    assert!(!no_room.append_section);
    let unlimited = quota_report_page_plan(4, 2, 0, usize::MAX, usize::MAX, None).unwrap();
    assert!(unlimited.append_section);
    assert_eq!(
        quota_report_page_plan(4, 2, 3, 0, 1, None),
        Err(crate::MojoError::InvalidInput)
    );
}

pub fn quota_report_sort_label(sort: i64) -> Result<&'static str, crate::MojoError> {
    use std::sync::OnceLock;

    static LABELS: OnceLock<Result<Vec<String>, crate::MojoError>> = OnceLock::new();
    let sort = usize::try_from(sort)
        .ok()
        .filter(|sort| *sort < 6)
        .ok_or(crate::MojoError::InvalidInput)?;
    match LABELS.get_or_init(|| {
        (0_i64..6)
            .map(|value| load_quota_display_label(QUOTA_DISPLAY_LABEL_SORT, value))
            .collect()
    }) {
        Ok(labels) => labels
            .get(sort)
            .map(String::as_str)
            .ok_or(crate::MojoError::InvalidOutput),
        Err(error) => Err(*error),
    }
}
