use super::*;

#[path = "watch_tui/frame.rs"]
mod frame;
#[path = "watch_tui/frame_data.rs"]
mod frame_data;
#[path = "watch_tui/runtime.rs"]
mod runtime;
#[path = "watch_tui/runtime_all.rs"]
mod runtime_all;
#[path = "watch_tui/runtime_profile.rs"]
mod runtime_profile;

#[cfg(test)]
pub(super) use frame::{
    AllQuotaWatchTuiRow, AllQuotaWatchTuiTable, quota_human_tui_spans, quota_watch_overview_height,
    quota_watch_separator_line, quota_watch_table_text,
};
pub(super) use frame::{
    build_all_quota_watch_tui_frame, build_profile_quota_watch_tui_frame,
    quota_watch_snapshot_overview_field_count, quota_watch_tui_max_scroll_offset_for_snapshot,
    quota_watch_tui_table_lines, render_all_quota_watch_tui,
};
pub(crate) use runtime::{
    render_all_quota_reports_once_tui, render_profile_quota_once_tui, watch_all_quotas,
};
#[cfg(test)]
pub(super) use runtime_profile::quota_watch_quit_key;
pub(crate) use runtime_profile::{quota_watch_enabled, watch_quota};
