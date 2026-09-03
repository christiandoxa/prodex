use super::*;

#[test]
fn completed_rate_reaches_tui_header_and_survives_log_flood() {
    let path = std::path::Path::new("/tmp/runtime-throughput-tui.log");
    let mut throughput = crate::app_commands::log_tui::OutputThroughput::default();
    let initial = crate::app_commands::log_tui::render_log_header(
        crate::app_commands::log_tui::LOG_TUI_TITLE,
        "",
        None,
        throughput.display_rate_for_profile(std::time::Instant::now(), Some("main")),
        80,
    );
    assert!(initial.ends_with("— t/s"));

    let completed = super::log_stream::collect_runtime_log_line(
        path,
        "[2026-07-01 21:52:36.729 +07:00] token_usage request=30 route=responses transport=http profile=main source=responses_sse input_tokens=10 cached_input_tokens=0 output_tokens=100 reasoning_tokens=0 generation_ms=1000 output_tokens_per_second=100.0",
        false,
        Some(&mut throughput),
        true,
    )
    .unwrap();
    assert!(matches!(
        completed.as_slice(),
        [LogStreamItem::TokenUsage(_)]
    ));

    for _ in 0..1_000 {
        super::log_stream::collect_runtime_log_line(
            path,
            "[2026-07-01 21:52:36.730 +07:00] runtime_proxy_active_limit_reached route=responses profile=main active=8 limit=8",
            true,
            Some(&mut throughput),
            true,
        )
        .unwrap();
    }

    let rate = throughput.display_rate_for_profile(std::time::Instant::now(), Some("main"));
    let rendered = crate::app_commands::log_tui::render_log_header(
        crate::app_commands::log_tui::LOG_TUI_TITLE,
        "",
        None,
        rate,
        80,
    );
    assert_eq!(rate, Some(100.0));
    assert!(rendered.ends_with("100 t/s"));
}
