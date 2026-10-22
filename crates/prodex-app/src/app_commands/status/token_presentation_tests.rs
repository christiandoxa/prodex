use super::super::{InfoTokenUsageSummary, StatusQuotaSummary};
use super::{StatusOverview, render_token_panel};
use prodex_mojo_core::info_render::TokenUsageCounts;
use ratatui::{Terminal, backend::TestBackend};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

fn token_overview(total: TokenUsageCounts) -> StatusOverview {
    StatusOverview {
        updated_at: "synthetic snapshot".to_string(),
        active_profile: "active-a".to_string(),
        runtime_profile: "runtime-a".to_string(),
        profile_count: 2,
        quota: StatusQuotaSummary::default(),
        five_hour_runway: None,
        weekly_runway: None,
        token_summary: InfoTokenUsageSummary {
            log_count: 2,
            event_count: 3,
            total,
            ..InfoTokenUsageSummary::default()
        },
        token_history: vec![0, 1, 3, 2, 1],
        token_first_at: Some("first".to_string()),
        token_last_at: Some("last".to_string()),
        runtime_load: crate::InfoRuntimeLoadSummary::default(),
        runtime_process_count: 0,
    }
}

fn token_cases() -> Vec<(&'static str, Option<StatusOverview>)> {
    let normal = || {
        token_overview(TokenUsageCounts {
            input_tokens: 12_345,
            cached_input_tokens: 2_000,
            output_tokens: 678,
            reasoning_tokens: 97,
        })
    };
    let mut zero = token_overview(TokenUsageCounts::default());
    zero.profile_count = 0;
    zero.token_summary.log_count = 0;
    zero.token_summary.event_count = 0;
    zero.token_first_at = None;
    zero.token_last_at = None;
    let mut first_only = normal();
    first_only.token_last_at = None;
    let mut last_only = normal();
    last_only.token_first_at = None;
    let mut empty_times = normal();
    empty_times.token_first_at = Some(String::new());
    empty_times.token_last_at = Some(String::new());
    let mut unicode = normal();
    unicode.runtime_profile = " \t雪\"\\\n ".to_string();
    unicode.active_profile = "active\0雪".to_string();
    unicode.token_first_at = Some(" first 雪\" ".to_string());
    unicode.token_last_at = Some("last\n雪".to_string());
    let mut max = token_overview(TokenUsageCounts {
        input_tokens: u64::MAX,
        cached_input_tokens: u64::MAX,
        output_tokens: u64::MAX,
        reasoning_tokens: u64::MAX,
    });
    max.profile_count = usize::MAX;
    max.token_summary.log_count = usize::MAX;
    max.token_summary.event_count = usize::MAX;
    let mut long = normal();
    long.runtime_profile = "p".repeat(4097);
    long.active_profile = "a".repeat(4097);
    long.token_first_at = Some("f".repeat(4097));
    long.token_last_at = Some("l".repeat(4097));
    let rounded = token_overview(TokenUsageCounts {
        input_tokens: 999_950,
        cached_input_tokens: 99_995,
        output_tokens: 49,
        reasoning_tokens: 1_000,
    });
    vec![
        ("loading", None),
        ("zero", Some(zero)),
        ("ordinary", Some(normal())),
        ("first-only", Some(first_only)),
        ("last-only", Some(last_only)),
        ("empty-times", Some(empty_times)),
        ("unicode", Some(unicode)),
        ("max", Some(max)),
        ("long", Some(long)),
        ("rounded", Some(rounded)),
    ]
}

#[test]
fn status_token_panel_preserves_original_complete_buffers() {
    let mut snapshots = Vec::new();
    for (name, overview) in token_cases() {
        for (width, height) in [(120, 14), (80, 10), (24, 4)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal
                .draw(|frame| {
                    let area = frame.area();
                    render_token_panel(frame, area, overview.as_ref());
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
        serde_json::from_str(include_str!("token_presentation_baseline.json")).unwrap();
    assert_eq!(Value::Array(snapshots), expected);
}
