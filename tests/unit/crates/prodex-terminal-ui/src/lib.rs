use super::*;

#[test]
fn section_headers_expand_to_requested_width() {
    assert_eq!(text_width(&section_header_with_width("Doctor", 72)), 72);
}

#[test]
fn wrapping_respects_requested_width() {
    let lines = wrap_text("alpha beta-gamma-delta epsilon", 10);
    assert!(lines.iter().all(|line| text_width(line) <= 10));
    assert!(!lines.is_empty());
}

#[test]
fn field_layout_respects_requested_width() {
    let lines =
        format_field_lines_with_layout("Long Label", "some longer value that must wrap", 24, 10);
    assert!(lines.iter().all(|line| text_width(line) <= 24));
    assert!(lines.len() > 1);
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
