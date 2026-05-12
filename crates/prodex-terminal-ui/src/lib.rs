mod info;
mod panel;
mod print;
mod runtime_launch;
mod session;
mod terminal;
mod text;

pub use info::{
    InfoLoadSummaryDisplay, InfoRunwayEstimateDisplay, InfoRunwayResetDisplay,
    RuntimeTuningBudgetsDisplay, RuntimeTuningTransportDisplay, RuntimeTuningWorkersDisplay,
    TokenUsageCounts, TokenUsageProfileDisplay, format_info_load_summary_display,
    format_info_pool_remaining_display, format_info_process_summary_display,
    format_info_quota_data_summary_display, format_info_runway_display,
    format_info_token_usage_summary_display, format_relative_duration,
    format_runtime_logs_summary_display, format_runtime_policy_summary_display,
    format_runtime_tuning_budgets_display, format_runtime_tuning_transport_display,
    format_runtime_tuning_workers_display,
};
pub use panel::{
    FieldRowsBuilder, PanelBuilder, format_field_lines_with_layout, panel_label_width, print_panel,
    render_panel, section_header, section_header_with_width,
};
pub use print::{
    print_blank_line, print_stderr_line, print_stderr_prompt, print_stdout_line, print_stdout_text,
    print_wrapped_stderr,
};
pub use runtime_launch::{
    RuntimeLaunchCandidateDisplay, RuntimeLaunchScoredCandidateMessage,
    RuntimeLaunchScoredCandidateOutput, RuntimeLaunchSelectedProfileStatus,
    format_runtime_launch_quota_inspect_hint, format_runtime_launch_scored_candidate_message,
    format_runtime_provider_direct_launch_message,
};
pub use session::{
    SessionReportDisplay, render_session_reports, render_session_reports_with_width,
};
pub use terminal::{
    current_cli_width, terminal_dimensions_from_tty, terminal_height_lines,
    terminal_size_override_usize, terminal_width_chars,
};
pub use text::{chunk_token, fit_cell, text_width, wrap_text};

pub const CLI_WIDTH: usize = 110;
pub const CLI_MIN_WIDTH: usize = 60;
pub const CLI_LABEL_WIDTH: usize = 16;
pub const CLI_MIN_LABEL_WIDTH: usize = 10;
pub const CLI_MAX_LABEL_WIDTH: usize = 24;
pub const CLI_TABLE_GAP: &str = "  ";

#[cfg(test)]
#[path = "../tests/src/lib.rs"]
mod tests;
