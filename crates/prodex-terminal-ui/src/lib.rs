mod info;
mod panel;
mod print;
mod runtime_launch;
mod session;
mod terminal;
mod terminal_session;
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
    FieldRowsBuilder, PanelBuilder, draw_status_panel_terminal, format_field_lines_with_layout,
    panel_label_width, print_panel, print_stderr_panel, print_text_panel, render_panel,
    render_text_panel, section_header, section_header_with_width, tui_accent_style,
    tui_border_style, tui_connected_footer_block, tui_connected_footer_border_set,
    tui_connected_header_block, tui_connected_header_border_set, tui_connected_separator_line,
    tui_detail_style, tui_error_style, tui_hint_style, tui_metric_style, tui_muted_style,
    tui_primary_style, tui_secondary_style, tui_success_style, tui_title_style, tui_tool_style,
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
pub use terminal_session::AlternateScreenTerminal;
pub use text::{chunk_token, fit_cell, pad_cell, text_width, wrap_text};

pub const CLI_WIDTH: usize = 110;
pub const CLI_MIN_WIDTH: usize = 60;
pub const CLI_LABEL_WIDTH: usize = 16;
pub const CLI_MIN_LABEL_WIDTH: usize = 10;
pub const CLI_MAX_LABEL_WIDTH: usize = 24;
pub const CLI_TABLE_GAP: &str = "  ";

#[cfg(test)]
#[path = "../tests/src/lib.rs"]
mod tests;
