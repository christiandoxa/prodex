use super::{StatusTokenPanelInput, status_token_panel_json};
use crate::info_render::TokenUsageCounts;

fn observation() -> StatusTokenPanelInput<'static> {
    StatusTokenPanelInput {
        tokens: TokenUsageCounts {
            input_tokens: 10,
            cached_input_tokens: 2,
            output_tokens: 3,
            reasoning_tokens: 1,
        },
        event_count: 7,
        log_count: 4,
        profile_count: 2,
        runtime_profile: "runtime",
        active_profile: "active",
        first_at: Some("first"),
        last_at: Some("last"),
    }
}

#[test]
fn status_token_bridge_preserves_complete_styled_rows_and_history_title() {
    assert_eq!(
        status_token_panel_json(Some(observation())).unwrap(),
        r#"{"lines":[[["secondary","total "],["metric","13"],["muted","  7 event(s) / 4 log(s)"]],[["raw","in 10  cached 2  out 3  reasoning 1"]],[["secondary","token efficiency "],["accent","cache hit 20.0% · output share 23.1%"]],[["secondary","active/config "],["accent","runtime / active"],["muted","  2 profile(s)"]]],"history_title":" recent events first → last "}"#
    );
}

#[test]
fn status_token_bridge_keeps_loading_unsigned_saturation_escaping_and_timestamp_presence() {
    assert_eq!(
        status_token_panel_json(None).unwrap(),
        r#"{"lines":[[["raw","loading token history…"]]],"history_title":""}"#
    );
    let text = status_token_panel_json(Some(StatusTokenPanelInput {
        tokens: TokenUsageCounts {
            input_tokens: u64::MAX,
            cached_input_tokens: u64::MAX,
            output_tokens: u64::MAX,
            reasoning_tokens: u64::MAX,
        },
        runtime_profile: "雪\"\n",
        active_profile: "active\0",
        first_at: Some(""),
        last_at: Some(""),
        ..observation()
    }))
    .unwrap();
    assert!(text.contains(r#"["metric","18446744073.7B"]"#));
    assert!(text.contains("cache hit 100.0% · output share 100.0%"));
    assert!(text.contains(r#"["accent","雪\"\n / active\u0000"]"#));
    assert!(text.contains(r#""history_title":" recent events  →  ""#));
    let text = status_token_panel_json(Some(StatusTokenPanelInput {
        last_at: None,
        ..observation()
    }))
    .unwrap();
    assert!(text.contains(r#""history_title":" no token events ""#));
}
