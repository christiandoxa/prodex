//! Bounded typed ABI for canonical Mojo quota-watch cadence and cache lifetime.

use crate::MojoError;

const ABI_VERSION: i64 = 1;

pub const ACTION_UP: i64 = 0;
pub const ACTION_DOWN: i64 = 1;
pub const ACTION_SORT: i64 = 2;
pub const ACTION_FILTER: i64 = 3;
pub const ACTION_UPDATE: i64 = 4;
pub const ACTION_QUIT: i64 = 5;

pub const OUTCOME_CONTINUE: i64 = 0;
pub const OUTCOME_SORT: i64 = 1;
pub const OUTCOME_FILTER: i64 = 2;
pub const OUTCOME_UPDATE: i64 = 3;
pub const OUTCOME_QUIT: i64 = 4;

pub const FILTER_ALL: i64 = 0;
pub const FILTER_OPENAI: i64 = 1;
pub const FILTER_GEMINI: i64 = 2;
pub const FILTER_ANTHROPIC: i64 = 3;
pub const FILTER_COPILOT: i64 = 4;
pub const FILTER_KIRO: i64 = 5;
pub const FILTER_DEEPSEEK: i64 = 6;
pub const FILTER_LOCAL: i64 = 7;
pub const FILTER_AGY: i64 = 8;

pub const SNAPSHOT_OPENAI: i64 = 0;
pub const SNAPSHOT_GEMINI: i64 = 1;
pub const SNAPSHOT_COPILOT: i64 = 2;
pub const SNAPSHOT_EXTERNAL: i64 = 3;
pub const SNAPSHOT_NONE: i64 = 4;

pub const RESET_NORMAL: i64 = 0;
pub const RESET_ERROR: i64 = 1;
pub const RESET_CREDITS: i64 = 2;

pub const SNAPSHOT_USE_NEXT: i64 = 0;
pub const SNAPSHOT_KEEP_PREVIOUS: i64 = 1;
pub const SNAPSHOT_MERGE_REPORTS: i64 = 2;

unsafe extern "C" {
    fn prodex_quota_watch_refresh_v1(
        abi_version: i64,
        windows_address: u64,
        windows_count: i64,
        watch: i64,
        profile_count: u64,
        now: i64,
        result_address: u64,
    ) -> i64;
    fn prodex_quota_watch_cache_alive_until_v1(
        abi_version: i64,
        now: i64,
        refresh_seconds: u64,
        result_address: u64,
    ) -> i64;
    fn prodex_quota_watch_cache_remaining_v1(
        abi_version: i64,
        alive_until: i64,
        now: i64,
        result_address: u64,
    ) -> i64;
    fn prodex_quota_watch_cache_live_v1(
        abi_version: i64,
        alive_until: i64,
        now: i64,
        result_address: u64,
    ) -> i64;
    fn prodex_quota_watch_live_refresh_v1(abi_version: i64, result_address: u64) -> i64;
    fn prodex_quota_watch_cache_eligible_v1(
        abi_version: i64,
        detail: i64,
        auth_filter: i64,
        provider_filter: i64,
        result_address: u64,
    ) -> i64;
    fn prodex_quota_watch_filter_next_v1(
        abi_version: i64,
        filter_kind: i64,
        result_address: u64,
    ) -> i64;
    fn prodex_quota_watch_window_v1(
        abi_version: i64,
        rows_address: u64,
        rows_count: i64,
        max_lines: i64,
        requested_start: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_quota_watch_viewport_v1(
        abi_version: i64,
        terminal_height: i64,
        overview_fields: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_quota_watch_available_lines_v1(
        abi_version: i64,
        terminal_height: i64,
        reserved_lines: i64,
        result_address: u64,
    ) -> i64;
    fn prodex_quota_watch_scroll_kind_v1(
        abi_version: i64,
        total_profiles: i64,
        shown_profiles: i64,
        hidden_before: i64,
        hidden_after: i64,
        result_address: u64,
    ) -> i64;
    fn prodex_quota_watch_filter_matches_v1(
        abi_version: i64,
        filter_kind: i64,
        snapshot_kind: i64,
        auth_address: u64,
        auth_length: i64,
        provider_address: u64,
        provider_length: i64,
        result_address: u64,
    ) -> i64;
    fn prodex_quota_watch_key_action_v1(
        abi_version: i64,
        key_kind: i64,
        char_code: i64,
        control: i64,
        result_address: u64,
    ) -> i64;
    fn prodex_quota_watch_action_v1(
        abi_version: i64,
        action: i64,
        scroll_offset: i64,
        max_scroll_offset: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_quota_watch_reset_kind_v1(
        abi_version: i64,
        address: u64,
        length: i64,
        result_address: u64,
    ) -> i64;
    fn prodex_quota_watch_merge_v1(
        abi_version: i64,
        previous_kind: i64,
        next_kind: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_quota_watch_preserve_v1(
        abi_version: i64,
        previous_success: i64,
        current_success: i64,
        current_auth_error: i64,
        same_profile: i64,
        same_auth: i64,
        result_address: u64,
    ) -> i64;
}

fn check(status: i64) -> Result<(), MojoError> {
    match status {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn refresh_seconds(
    reset_windows: &[i64],
    watch: bool,
    profile_count: usize,
    now: i64,
) -> Result<u64, MojoError> {
    let mut result = 0_u64;
    check(unsafe {
        prodex_quota_watch_refresh_v1(
            ABI_VERSION,
            reset_windows.as_ptr() as usize as u64,
            i64::try_from(reset_windows.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::from(watch),
            u64::try_from(profile_count).map_err(|_| MojoError::InvalidInput)?,
            now,
            (&mut result as *mut u64) as usize as u64,
        )
    })?;
    Ok(result)
}

pub fn cache_alive_until(now: i64, refresh_seconds: u64) -> Result<i64, MojoError> {
    let mut result = 0_i64;
    check(unsafe {
        prodex_quota_watch_cache_alive_until_v1(
            ABI_VERSION,
            now,
            refresh_seconds,
            (&mut result as *mut i64) as usize as u64,
        )
    })?;
    Ok(result)
}

pub fn cache_remaining_seconds(alive_until: i64, now: i64) -> Result<u64, MojoError> {
    let mut result = 0_u64;
    check(unsafe {
        prodex_quota_watch_cache_remaining_v1(
            ABI_VERSION,
            alive_until,
            now,
            (&mut result as *mut u64) as usize as u64,
        )
    })?;
    Ok(result)
}

pub fn cache_is_live(alive_until: i64, now: i64) -> Result<bool, MojoError> {
    let mut result = -1_i64;
    check(unsafe {
        prodex_quota_watch_cache_live_v1(
            ABI_VERSION,
            alive_until,
            now,
            (&mut result as *mut i64) as usize as u64,
        )
    })?;
    match result {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowPlan {
    pub start: usize,
    pub shown: usize,
    pub hidden_before: usize,
    pub hidden_after: usize,
    pub max_scroll: usize,
}

fn text_address(value: Option<&str>) -> Result<(u64, i64), MojoError> {
    let Some(value) = value else {
        return Ok((0, 0));
    };
    let length = i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?;
    Ok((value.as_ptr() as usize as u64, length))
}

fn output_bool(value: i64) -> Result<bool, MojoError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn live_refresh_seconds() -> Result<u64, MojoError> {
    let mut result = -1_i64;
    check(unsafe {
        prodex_quota_watch_live_refresh_v1(ABI_VERSION, (&mut result as *mut i64) as usize as u64)
    })?;
    let result = u64::try_from(result).map_err(|_| MojoError::InvalidOutput)?;
    (result > 0)
        .then_some(result)
        .ok_or(MojoError::InvalidOutput)
}

pub fn cache_is_eligible(
    detail: bool,
    auth_filter: i64,
    provider_filter: i64,
) -> Result<bool, MojoError> {
    let mut result = -1_i64;
    check(unsafe {
        prodex_quota_watch_cache_eligible_v1(
            ABI_VERSION,
            i64::from(detail),
            auth_filter,
            provider_filter,
            (&mut result as *mut i64) as usize as u64,
        )
    })?;
    output_bool(result)
}

pub fn filter_next(filter_kind: i64) -> Result<i64, MojoError> {
    let mut result = -1_i64;
    check(unsafe {
        prodex_quota_watch_filter_next_v1(
            ABI_VERSION,
            filter_kind,
            (&mut result as *mut i64) as usize as u64,
        )
    })?;
    if (FILTER_ALL..=FILTER_AGY).contains(&result) {
        Ok(result)
    } else {
        Err(MojoError::InvalidOutput)
    }
}

pub fn window_plan(
    row_line_counts: &[usize],
    max_lines: Option<usize>,
    requested_start: usize,
) -> Result<WindowPlan, MojoError> {
    let rows = row_line_counts
        .iter()
        .map(|value| i64::try_from(*value).map_err(|_| MojoError::InvalidInput))
        .collect::<Result<Vec<_>, _>>()?;
    let rows_count = i64::try_from(rows.len()).map_err(|_| MojoError::InvalidInput)?;
    let max_lines = max_lines
        .map(|value| i64::try_from(value).map_err(|_| MojoError::InvalidInput))
        .transpose()?
        .unwrap_or(-1);
    let requested_start = i64::try_from(requested_start).map_err(|_| MojoError::InvalidInput)?;
    let mut output = [0_i64; 5];
    check(unsafe {
        prodex_quota_watch_window_v1(
            ABI_VERSION,
            rows.as_ptr() as usize as u64,
            rows_count,
            max_lines,
            requested_start,
            output.as_mut_ptr() as usize as u64,
        )
    })?;
    if output.iter().any(|value| *value < 0) {
        return Err(MojoError::InvalidOutput);
    }
    let count = row_line_counts.len();
    let start = usize::try_from(output[0]).map_err(|_| MojoError::InvalidOutput)?;
    let shown = usize::try_from(output[1]).map_err(|_| MojoError::InvalidOutput)?;
    let hidden_before = usize::try_from(output[2]).map_err(|_| MojoError::InvalidOutput)?;
    let hidden_after = usize::try_from(output[3]).map_err(|_| MojoError::InvalidOutput)?;
    let max_scroll = usize::try_from(output[4]).map_err(|_| MojoError::InvalidOutput)?;
    if start > count
        || shown > count.saturating_sub(start)
        || hidden_before != start
        || hidden_after != count.saturating_sub(start.saturating_add(shown))
        || max_scroll > count.saturating_sub(1)
    {
        return Err(MojoError::InvalidOutput);
    }
    Ok(WindowPlan {
        start,
        shown,
        hidden_before,
        hidden_after,
        max_scroll,
    })
}

pub fn viewport_lines(
    terminal_height: usize,
    overview_fields: usize,
) -> Result<(usize, usize), MojoError> {
    let terminal_height = i64::try_from(terminal_height).map_err(|_| MojoError::InvalidInput)?;
    let overview_fields = i64::try_from(overview_fields).map_err(|_| MojoError::InvalidInput)?;
    let mut output = [0_i64; 2];
    check(unsafe {
        prodex_quota_watch_viewport_v1(
            ABI_VERSION,
            terminal_height,
            overview_fields,
            output.as_mut_ptr() as usize as u64,
        )
    })?;
    if output.iter().any(|value| *value < 0) {
        return Err(MojoError::InvalidOutput);
    }
    Ok((
        usize::try_from(output[0]).map_err(|_| MojoError::InvalidOutput)?,
        usize::try_from(output[1]).map_err(|_| MojoError::InvalidOutput)?,
    ))
}

pub fn available_lines(terminal_height: usize, reserved_lines: usize) -> Result<usize, MojoError> {
    let terminal_height = i64::try_from(terminal_height).map_err(|_| MojoError::InvalidInput)?;
    let reserved_lines = i64::try_from(reserved_lines).map_err(|_| MojoError::InvalidInput)?;
    let mut result = -1_i64;
    check(unsafe {
        prodex_quota_watch_available_lines_v1(
            ABI_VERSION,
            terminal_height,
            reserved_lines,
            (&mut result as *mut i64) as usize as u64,
        )
    })?;
    usize::try_from(result).map_err(|_| MojoError::InvalidOutput)
}

pub fn scroll_kind(
    total_profiles: usize,
    shown_profiles: usize,
    hidden_before: usize,
    hidden_after: usize,
) -> Result<i64, MojoError> {
    let mut result = -1_i64;
    let values = [total_profiles, shown_profiles, hidden_before, hidden_after]
        .into_iter()
        .map(|value| i64::try_from(value).map_err(|_| MojoError::InvalidInput))
        .collect::<Result<Vec<_>, _>>()?;
    check(unsafe {
        prodex_quota_watch_scroll_kind_v1(
            ABI_VERSION,
            values[0],
            values[1],
            values[2],
            values[3],
            (&mut result as *mut i64) as usize as u64,
        )
    })?;
    if (0..=2).contains(&result) {
        Ok(result)
    } else {
        Err(MojoError::InvalidOutput)
    }
}

pub fn filter_matches(
    filter_kind: i64,
    snapshot_kind: i64,
    auth_label: &str,
    provider: Option<&str>,
) -> Result<bool, MojoError> {
    let (auth_address, auth_length) = text_address(Some(auth_label))?;
    let (provider_address, provider_length) = text_address(provider)?;
    let mut result = -1_i64;
    check(unsafe {
        prodex_quota_watch_filter_matches_v1(
            ABI_VERSION,
            filter_kind,
            snapshot_kind,
            auth_address,
            auth_length,
            provider_address,
            provider_length,
            (&mut result as *mut i64) as usize as u64,
        )
    })?;
    output_bool(result)
}

pub fn key_action(key_kind: i64, char_code: Option<char>, control: bool) -> Result<i64, MojoError> {
    let mut result = -1_i64;
    check(unsafe {
        prodex_quota_watch_key_action_v1(
            ABI_VERSION,
            key_kind,
            char_code.map_or(0, |value| i64::from(u32::from(value))),
            i64::from(control),
            (&mut result as *mut i64) as usize as u64,
        )
    })?;
    if (-1..=ACTION_QUIT).contains(&result) {
        Ok(result)
    } else {
        Err(MojoError::InvalidOutput)
    }
}

pub fn action(
    action: i64,
    scroll_offset: usize,
    max_scroll_offset: usize,
) -> Result<(i64, usize), MojoError> {
    let scroll_offset = i64::try_from(scroll_offset).map_err(|_| MojoError::InvalidInput)?;
    let max_scroll_offset =
        i64::try_from(max_scroll_offset).map_err(|_| MojoError::InvalidInput)?;
    let mut output = [0_i64; 2];
    check(unsafe {
        prodex_quota_watch_action_v1(
            ABI_VERSION,
            action,
            scroll_offset,
            max_scroll_offset,
            output.as_mut_ptr() as usize as u64,
        )
    })?;
    if !(OUTCOME_CONTINUE..=OUTCOME_QUIT).contains(&output[0]) || output[1] < 0 {
        return Err(MojoError::InvalidOutput);
    }
    Ok((
        output[0],
        usize::try_from(output[1]).map_err(|_| MojoError::InvalidOutput)?,
    ))
}

pub fn reset_kind(value: &str) -> Result<i64, MojoError> {
    let (address, length) = text_address(Some(value))?;
    let mut result = -1_i64;
    check(unsafe {
        prodex_quota_watch_reset_kind_v1(
            ABI_VERSION,
            address,
            length,
            (&mut result as *mut i64) as usize as u64,
        )
    })?;
    if matches!(result, RESET_NORMAL | RESET_ERROR | RESET_CREDITS) {
        Ok(result)
    } else {
        Err(MojoError::InvalidOutput)
    }
}

pub fn merge_plan(previous_kind: i64, next_kind: i64) -> Result<i64, MojoError> {
    let mut result = -1_i64;
    check(unsafe {
        prodex_quota_watch_merge_v1(
            ABI_VERSION,
            previous_kind,
            next_kind,
            (&mut result as *mut i64) as usize as u64,
        )
    })?;
    if matches!(
        result,
        SNAPSHOT_USE_NEXT | SNAPSHOT_KEEP_PREVIOUS | SNAPSHOT_MERGE_REPORTS
    ) {
        Ok(result)
    } else {
        Err(MojoError::InvalidOutput)
    }
}

pub fn preserve_report(
    previous_success: bool,
    current_success: bool,
    current_auth_error: bool,
    same_profile: bool,
    same_auth: bool,
) -> Result<bool, MojoError> {
    let mut result = -1_i64;
    check(unsafe {
        prodex_quota_watch_preserve_v1(
            ABI_VERSION,
            i64::from(previous_success),
            i64::from(current_success),
            i64::from(current_auth_error),
            i64::from(same_profile),
            i64::from(same_auth),
            (&mut result as *mut i64) as usize as u64,
        )
    })?;
    output_bool(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quota_watch_ffi_rejects_invalid_version_and_output_pointers() {
        let mut result = -1_i64;
        let result_address = (&mut result as *mut i64) as usize as u64;
        assert_eq!(
            unsafe { prodex_quota_watch_cache_live_v1(0, 1, 1, result_address) },
            4,
        );
        assert_eq!(
            unsafe { prodex_quota_watch_cache_live_v1(ABI_VERSION, 1, 1, 0) },
            1
        );
        assert_eq!(
            unsafe { prodex_quota_watch_refresh_v1(ABI_VERSION, 0, 1, 0, 1, 0, result_address) },
            1,
        );
    }

    #[test]
    fn cadence_and_window_boundaries_are_mojo_owned() {
        assert_eq!(refresh_seconds(&[], false, 1, 100).unwrap(), 45);
        assert_eq!(refresh_seconds(&[], true, 1, 100).unwrap(), 10);
        assert_eq!(refresh_seconds(&[220], false, 1, 100).unwrap(), 5);
        assert_eq!(refresh_seconds(&[221], false, 1, 100).unwrap(), 10);
        assert_eq!(refresh_seconds(&[1001], false, 1, 100).unwrap(), 45);
        assert_eq!(refresh_seconds(&[220], true, 100, 100).unwrap(), 200);
        assert_eq!(refresh_seconds(&[i64::MAX], false, 0, i64::MAX).unwrap(), 5);
        assert_eq!(
            refresh_seconds(&[], false, usize::MAX, 100).unwrap(),
            u64::MAX
        );
    }

    #[test]
    fn lifetime_overflow_and_expiry_are_mojo_owned() {
        assert_eq!(cache_alive_until(100, 0).unwrap(), 111);
        assert_eq!(cache_alive_until(i64::MAX - 2, 1).unwrap(), i64::MAX);
        assert_eq!(
            cache_alive_until(100, u64::MAX).unwrap(),
            100 + i64::MAX / 2 + 10
        );
        assert_eq!(cache_remaining_seconds(50, 50).unwrap(), 1);
        assert_eq!(cache_remaining_seconds(70, 50).unwrap(), 20);
        assert_eq!(
            cache_remaining_seconds(i64::MAX, i64::MIN).unwrap(),
            i64::MAX as u64
        );
        assert!(cache_is_live(50, 50).unwrap());
        assert!(!cache_is_live(49, 50).unwrap());
    }

    #[test]
    fn quota_watch_viewport_and_window_edges_are_mojo_owned() {
        assert_eq!(live_refresh_seconds().unwrap(), 5);
        assert_eq!(viewport_lines(0, 0).unwrap(), (0, 1));
        assert_eq!(viewport_lines(30, 5).unwrap(), (5, 20));
        assert_eq!(available_lines(3, 9).unwrap(), 0);
        assert_eq!(
            available_lines(i64::MAX as usize, 0).unwrap(),
            i64::MAX as usize
        );

        let empty = window_plan(&[], Some(0), usize::MAX).unwrap_err();
        assert_eq!(empty, MojoError::InvalidInput);
        let plan = window_plan(&[1, 1, 1], Some(3), 0).unwrap();
        assert_eq!(
            plan,
            WindowPlan {
                start: 0,
                shown: 1,
                hidden_before: 0,
                hidden_after: 2,
                max_scroll: 2,
            }
        );
        assert_eq!(scroll_kind(0, 0, 0, 0).unwrap(), 0);
        assert_eq!(scroll_kind(4, 0, 0, 4).unwrap(), 1);
        assert_eq!(scroll_kind(4, 2, 2, 0).unwrap(), 2);
        let saturated = window_plan(&[usize::MAX], None, 0).unwrap_err();
        assert_eq!(saturated, MojoError::InvalidInput);
    }

    #[test]
    fn quota_watch_filter_action_and_cache_matrix_is_mojo_owned() {
        assert!(cache_is_eligible(true, 0, FILTER_ALL).unwrap());
        assert!(cache_is_eligible(true, 0, FILTER_OPENAI).unwrap());
        assert!(!cache_is_eligible(false, 0, FILTER_OPENAI).unwrap());
        assert!(!cache_is_eligible(true, 1, FILTER_OPENAI).unwrap());
        for (filter, next) in [
            (FILTER_ALL, FILTER_OPENAI),
            (FILTER_OPENAI, FILTER_GEMINI),
            (FILTER_GEMINI, FILTER_ANTHROPIC),
            (FILTER_ANTHROPIC, FILTER_COPILOT),
            (FILTER_COPILOT, FILTER_KIRO),
            (FILTER_KIRO, FILTER_DEEPSEEK),
            (FILTER_DEEPSEEK, FILTER_LOCAL),
            (FILTER_LOCAL, FILTER_AGY),
            (FILTER_AGY, FILTER_ALL),
        ] {
            assert_eq!(filter_next(filter).unwrap(), next);
        }
        assert!(filter_matches(FILTER_OPENAI, SNAPSHOT_OPENAI, "other", None).unwrap());
        assert!(filter_matches(FILTER_OPENAI, SNAPSHOT_NONE, "CHATGPT", None).unwrap());
        assert!(!filter_matches(FILTER_OPENAI, SNAPSHOT_NONE, " chatgpt ", None).unwrap());
        assert!(
            filter_matches(
                FILTER_LOCAL,
                SNAPSHOT_EXTERNAL,
                "other",
                Some("LOCAL OPENAI-COMPATIBLE"),
            )
            .unwrap()
        );

        assert_eq!(key_action(2, Some('q'), false).unwrap(), ACTION_QUIT);
        assert_eq!(key_action(2, Some('c'), true).unwrap(), ACTION_QUIT);
        assert_eq!(key_action(3, None, false).unwrap(), ACTION_DOWN);
        assert_eq!(key_action(0, None, false).unwrap(), -1);
        assert_eq!(action(ACTION_UP, 0, 0).unwrap(), (OUTCOME_CONTINUE, 0));
        assert_eq!(action(ACTION_DOWN, 0, 2).unwrap(), (OUTCOME_CONTINUE, 1));
        assert_eq!(action(ACTION_SORT, 2, 2).unwrap().0, OUTCOME_SORT);
        assert_eq!(action(ACTION_QUIT, 2, 2).unwrap().0, OUTCOME_QUIT);
    }

    #[test]
    fn quota_watch_error_reset_and_merge_matrix_is_mojo_owned() {
        assert_eq!(reset_kind("error: unauthorized").unwrap(), RESET_ERROR);
        assert_eq!(
            reset_kind("resets: 5h unavailable; reset credits: 1 available").unwrap(),
            RESET_CREDITS
        );
        assert_eq!(reset_kind("resets: 5h unknown").unwrap(), RESET_NORMAL);
        for previous in 0..=3 {
            for next in 0..=3 {
                let expected = match (previous, next) {
                    (1, 1) => SNAPSHOT_MERGE_REPORTS,
                    (1, 3) => SNAPSHOT_KEEP_PREVIOUS,
                    _ => SNAPSHOT_USE_NEXT,
                };
                assert_eq!(merge_plan(previous, next).unwrap(), expected);
            }
        }
        assert!(preserve_report(true, false, false, true, true).unwrap());
        assert!(!preserve_report(true, false, true, true, true).unwrap());
        assert!(!preserve_report(true, true, false, true, true).unwrap());
        assert!(!preserve_report(true, false, false, true, false).unwrap());
    }
}
