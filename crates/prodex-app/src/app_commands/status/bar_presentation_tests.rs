use super::token_presentation_tests::token_overview;
use super::{StatusResourceHistory, StatusResourceSnapshot, render_status_dashboard};
use prodex_mojo_core::info_render::TokenUsageCounts;
use ratatui::{Terminal, backend::TestBackend};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

#[test]
fn status_bars_preserve_original_complete_dashboard_buffers() {
    let ready = || token_overview(TokenUsageCounts::default());
    let mut empty = ready();
    empty.runtime_profile = String::new();
    empty.updated_at = String::new();
    let mut unicode = ready();
    unicode.runtime_profile = " \t雪\"\\\n ".to_string();
    unicode.updated_at = "updated\0雪".to_string();
    let mut long = ready();
    long.runtime_profile = "p".repeat(4097);
    long.updated_at = "t".repeat(4097);
    let cases = vec![
        ("loading-live", None, None, false),
        ("loading-refreshing", None, None, true),
        (
            "loading-error",
            None,
            Some("synthetic error".to_string()),
            false,
        ),
        ("ready-live", Some(ready()), None, false),
        ("ready-refreshing", Some(ready()), None, true),
        ("empty-ready", Some(empty), Some(String::new()), true),
        (
            "unicode",
            Some(unicode),
            Some("error\n雪\"\\".to_string()),
            false,
        ),
        ("long", Some(long), Some("e".repeat(4097)), true),
        (
            "ready-error",
            Some(ready()),
            Some("synthetic error".to_string()),
            false,
        ),
    ];
    let resources = StatusResourceSnapshot::default();
    let history = StatusResourceHistory::default();
    let mut snapshots = Vec::new();
    for (name, overview, error, refreshing) in cases {
        for (width, height) in [(120, 40), (80, 24), (60, 12)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal
                .draw(|frame| {
                    render_status_dashboard(
                        frame,
                        overview.as_ref(),
                        &resources,
                        &history,
                        error.as_deref(),
                        refreshing,
                    )
                    .unwrap();
                })
                .unwrap();
            let text = format!("{:?}", terminal.backend().buffer());
            let digest = Sha256::digest(text.as_bytes())
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            snapshots.push(json!({"name":name,"width":width,"height":height,"bytes":text.len(),"sha256":digest}));
        }
    }
    let expected: Value =
        serde_json::from_str(include_str!("bar_presentation_baseline.json")).unwrap();
    assert_eq!(Value::Array(snapshots), expected);
}
