use super::{
    ALL_QUOTA_WATCH_AUTH_BACKOFF_POLL_SECONDS, AllQuotaWatchLayout, AllQuotaWatchRefresh,
    AllQuotaWatchSnapshot, AppPaths, DEFAULT_WATCH_INTERVAL_SECONDS, ProfileProvider,
    ProviderQuotaSnapshot, QUOTA_WATCH_INPUT_POLL_MS, QuotaArgs, QuotaAuthFilter,
    QuotaProviderFilter, QuotaReport, QuotaReportSort, QuotaWatchCommand, QuotaWatchCommandOutcome,
    UsageResponse, all_quota_watch_next_refresh_at, apply_quota_watch_command, fetch_profile_quota,
    filter_quota_reports_by_provider, format_copilot_main_quota, format_copilot_quota_status,
    format_copilot_reset_summary, format_gemini_main_quota, format_gemini_quota_status,
    format_gemini_reset_summary, format_main_windows, format_main_windows_compact,
    format_openai_quota_status, format_quota_error_status, merge_all_quota_watch_snapshot,
    quota_pool_summary_fields_for_reports, quota_runtime_auth_backoff_profiles,
    quota_watch_max_scroll_offset, quota_watch_next_refresh_at,
    quota_watch_snapshot_with_auth_backoff, quota_watch_tui_fallback_message,
    quota_watch_updated_at, render_all_quota_watch_snapshot, render_profile_quota_watch_output,
    sorted_quota_report_indexes_by_sort, start_all_quota_watch_refresh,
};
use anyhow::{Context, Result};
use std::io::{self, Write};
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

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
