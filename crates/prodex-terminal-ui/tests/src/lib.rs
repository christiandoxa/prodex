use super::*;

#[test]
fn section_headers_expand_to_requested_width() {
    assert_eq!(text_width(&section_header_with_width("Doctor", 72)), 72);
}

#[test]
fn section_headers_fit_tiny_width() {
    assert_eq!(text_width(&section_header_with_width("Doctor", 8)), 8);
}

#[test]
fn wrapping_respects_requested_width() {
    let lines = wrap_text("alpha beta-gamma-delta epsilon", 10);
    assert!(lines.iter().all(|line| text_width(line) <= 10));
    assert!(!lines.is_empty());
}

#[test]
fn wrapping_counts_terminal_display_width() {
    assert_eq!(text_width("表🙂"), 4);
    assert!(
        wrap_text("alpha 表🙂 beta", 8)
            .iter()
            .all(|line| text_width(line) <= 8)
    );
    assert!(text_width(&fit_cell("表🙂abcdef", 6)) <= 6);
}

#[test]
fn field_layout_respects_requested_width() {
    let lines =
        format_field_lines_with_layout("Long Label", "some longer value that must wrap", 24, 10);
    assert!(lines.iter().all(|line| text_width(line) <= 24));
    assert!(lines.len() > 1);
}

#[test]
fn panel_label_width_allows_tiny_terminals() {
    assert_eq!(
        panel_label_width(&[("Long Label".to_string(), "value".to_string())], 8),
        1
    );
}

#[test]
fn text_panel_renderer_adds_panel_header() {
    let rendered = render_text_panel("Audit", "line one\nline two");
    assert!(rendered.contains("[ Audit ]"));
    assert!(rendered.contains("line one"));
    assert!(rendered.contains("line two"));
}

#[test]
fn tui_panel_styles_use_theme_safe_ansi_colors() {
    use ratatui::style::Color;

    assert_eq!(tui_title_style().fg, Some(Color::Cyan));
    assert_eq!(tui_secondary_style().fg, Some(Color::Gray));
    assert_eq!(tui_muted_style().fg, Some(Color::Gray));
    assert_eq!(tui_detail_style().fg, Some(Color::Gray));
    assert_eq!(tui_primary_style().fg, None);
    assert_eq!(tui_border_style().fg, Some(Color::Cyan));
    assert_eq!(tui_hint_style().fg, Some(Color::Cyan));
    assert_eq!(tui_success_style().fg, Some(Color::Green));
    assert_eq!(tui_metric_style().fg, Some(Color::Green));
    assert_eq!(tui_accent_style().fg, Some(Color::LightCyan));
    assert_eq!(tui_tool_style().fg, Some(Color::LightMagenta));
    assert_eq!(tui_error_style().fg, Some(Color::Red));
}

#[test]
fn connected_tui_border_helpers_use_junctions() {
    let header = tui_connected_header_border_set();
    assert_eq!(header.bottom_left, "├");
    assert_eq!(header.bottom_right, "┤");

    let footer = tui_connected_footer_border_set();
    assert_eq!(footer.top_left, "├");
    assert_eq!(footer.top_right, "┤");

    assert_eq!(tui_connected_separator_line(0), "");
    assert_eq!(tui_connected_separator_line(1), "─");
    assert_eq!(tui_connected_separator_line(2), "├┤");
    assert_eq!(tui_connected_separator_line(5), "├───┤");
}

#[test]
fn status_panel_draws_on_test_backend() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let backend = TestBackend::new(48, 3);
    let mut terminal = Terminal::new(backend).expect("terminal");

    draw_status_panel_terminal(
        &mut terminal,
        "Prodex Launch",
        "preflight",
        "Status",
        "starting child process...",
    )
    .expect("draw status panel");
}

#[test]
fn info_process_summary_preserves_pid_limit() {
    assert_eq!(
        format_info_process_summary_display(7, 2, 10..17, 6),
        "Yes (7 total, 2 runtime; pids: 10, 11, 12, 13, 14, 15 (+1 more))"
    );
}

#[test]
fn info_token_usage_summary_formats_profile_totals() {
    let summary = format_info_token_usage_summary_display(
        2,
        3,
        TokenUsageCounts {
            input_tokens: 110,
            cached_input_tokens: 25,
            output_tokens: 44,
            reasoning_tokens: 9,
        },
        [
            TokenUsageProfileDisplay {
                profile: "backup",
                total: TokenUsageCounts {
                    input_tokens: 10,
                    cached_input_tokens: 0,
                    output_tokens: 4,
                    reasoning_tokens: 1,
                },
            },
            TokenUsageProfileDisplay {
                profile: "main",
                total: TokenUsageCounts {
                    input_tokens: 100,
                    cached_input_tokens: 25,
                    output_tokens: 40,
                    reasoning_tokens: 8,
                },
            },
        ],
    );

    assert_eq!(
        summary,
        "2 event(s), logs=3: input=110, cached_input=25, output=44, reasoning=9; by profile: backup:10 in/0 cached/4 out/1 reasoning; main:100 in/25 cached/40 out/8 reasoning"
    );
}

#[test]
fn relative_duration_formats_existing_info_style() {
    assert_eq!(format_relative_duration(0), "now");
    assert_eq!(format_relative_duration(59), "<1m");
    assert_eq!(format_relative_duration(3_660), "1h 1m");
    assert_eq!(format_relative_duration(90_000), "1d 1h");
}

#[test]
fn session_report_renderer_keeps_existing_columns_and_profile_line() {
    let rendered = render_session_reports_with_width(
        &[SessionReportDisplay {
            id: "sess-a",
            updated_at: Some("2026-04-29T12:30:00Z"),
            thread_name: Some("Issue triage"),
            cwd: Some("/repo"),
            profile: Some("main"),
            path: "/tmp/sessions/session-a.jsonl",
        }],
        80,
    );

    assert!(rendered.contains("[ Sessions ]"));
    assert!(rendered.contains("ID"));
    assert!(rendered.contains("sess-a"));
    assert!(rendered.contains("Issue triage"));
    assert!(rendered.contains("profile: main"));
}

#[test]
fn session_report_renderer_prints_full_id_when_id_column_is_truncated() {
    let long_id = "550e8400-e29b-41d4-a716-446655440000";
    let rendered = render_session_reports_with_width(
        &[SessionReportDisplay {
            id: long_id,
            updated_at: Some("2026-04-29T12:30:00Z"),
            thread_name: Some("Issue triage"),
            cwd: Some("/repo"),
            profile: None,
            path: "/tmp/sessions/550e8400-e29b-41d4-a716-446655440000.jsonl",
        }],
        80,
    );

    assert!(rendered.contains(&format!("  id: {long_id}")));
}

#[test]
fn session_report_renderer_respects_narrow_width() {
    let rendered = render_session_reports_with_width(
        &[SessionReportDisplay {
            id: "sess-wide-name",
            updated_at: Some("2026-04-29T12:30:00Z"),
            thread_name: Some("表🙂 thread"),
            cwd: Some("/repo"),
            profile: None,
            path: "/tmp/sessions/session-a.jsonl",
        }],
        40,
    );

    assert!(rendered.lines().take(3).all(|line| text_width(line) <= 40));
}

#[test]
fn runtime_launch_scored_candidate_message_formats_blocked_selected_profile() {
    let output =
        format_runtime_launch_scored_candidate_message(RuntimeLaunchScoredCandidateMessage {
            initial_profile_name: "main",
            candidate: RuntimeLaunchCandidateDisplay {
                name: "backup",
                quota_summary: "5h 80%, weekly 90%",
            },
            selected_profile_status: Some(RuntimeLaunchSelectedProfileStatus::Blocked {
                blocked_summary: "5h limit exhausted",
            }),
        });

    assert_eq!(
        output.warning.as_deref(),
        Some("Quota preflight blocked profile 'main': 5h limit exhausted")
    );
    assert_eq!(
        output.selection,
        "Auto-rotating to profile 'backup' using quota-pressure scoring (5h 80%, weekly 90%)."
    );
}

#[test]
fn runtime_launch_provider_message_preserves_output_text() {
    assert_eq!(
        format_runtime_provider_direct_launch_message("amazon-bedrock", "config.toml"),
        "Detected model_provider 'amazon-bedrock' from config.toml. Launching directly without prodex quota preflight or auto-rotate proxy."
    );
}
