use super::*;

#[test]
fn human_log_modes_share_the_compact_title() {
    assert_eq!(LOG_TUI_TITLE, "Prodex Log");
}

#[test]
fn compiled_log_tui_header_layout_reaches_live_unicode_and_atomic_quota_consumer() {
    let snapshot = crate::RuntimeProfileUsageSnapshot {
        checked_at: 0,
        plan_type: None,
        five_hour_status: prodex_quota::RuntimeQuotaWindowStatus::Ready,
        five_hour_remaining_percent: 72,
        five_hour_reset_at: i64::MAX,
        weekly_status: prodex_quota::RuntimeQuotaWindowStatus::Ready,
        weekly_remaining_percent: 80,
        weekly_reset_at: i64::MAX,
    };
    let detail = LogTuiHeaderDetail::quota("漢字🚀profile".to_string(), &snapshot, Duration::ZERO);
    assert_eq!(detail.render(0), "");
    assert!(!detail.render(10).contains("weekly 80%"));
    assert!(!detail.render(22).contains("weekly 80%"));
    let rendered = detail.render(40);
    assert!(
        rendered.ends_with("5h 72% reset -  weekly 80% reset -"),
        "{rendered}"
    );
    assert!(terminal_ui::text_width(&rendered) <= 40, "{rendered}");
    let header = render_log_header(LOG_TUI_TITLE, "ignored", Some(&detail), Some(7.0), 80);
    assert_eq!(terminal_ui::text_width(&header), 78);
    assert!(header.ends_with("7 t/s"));
    assert!(header.contains("weekly 80%"));
    assert_eq!(
        render_log_header(LOG_TUI_TITLE, "ignored", None, None, 0),
        "",
    );
}
