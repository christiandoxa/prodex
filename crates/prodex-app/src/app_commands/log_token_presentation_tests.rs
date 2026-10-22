use super::{InfoTokenUsageEvent, LogStreamItem, log_stream_tui_text_for_view};
use crate::app_commands::log::log_stream::{log_stream_item_json, project_token_log};
use crate::app_commands::log_format::render_log_block;
use prodex_mojo_core::json::LogTokenPresentation;
use serde_json::{Value, json};
use std::collections::VecDeque;

fn token_presentation_corpus() -> Value {
    let mut events = Vec::new();
    for request in [None, Some(0), Some(u64::MAX)] {
        for generation_ms in [None, Some(0), Some(1), Some(u64::MAX)] {
            for output_tokens_per_second in [
                None,
                Some(0.0),
                Some(-0.0),
                Some(-1.0),
                Some(0.01),
                Some(99.95),
                Some(100.0),
                Some(f64::MAX),
                Some(f64::NAN),
                Some(f64::INFINITY),
                Some(f64::NEG_INFINITY),
            ] {
                events.push(InfoTokenUsageEvent {
                    timestamp: "synthetic time".to_string(),
                    request,
                    profile: "profile-a".to_string(),
                    transport: "sse".to_string(),
                    source: "responses".to_string(),
                    input_tokens: u64::MAX,
                    cached_input_tokens: 0,
                    output_tokens: 15,
                    reasoning_tokens: 2,
                    generation_ms,
                    output_tokens_per_second,
                });
            }
        }
    }
    events.push(InfoTokenUsageEvent::default());
    events.push(InfoTokenUsageEvent {
        timestamp: "\u{2003}雪\n\t\0\"\\".to_string(),
        profile: "Profile a:b 雪".to_string(),
        transport: "WebSocket".to_string(),
        source: " \u{200b}source\r ".to_string(),
        ..InfoTokenUsageEvent::default()
    });
    events.push(InfoTokenUsageEvent {
        profile: "雪".repeat(4097),
        ..InfoTokenUsageEvent::default()
    });
    Value::Array(
        events
            .into_iter()
            .map(|event| {
                let metadata: Vec<(String, String)> =
                    project_token_log(&event, LogTokenPresentation::Metadata).unwrap();
                let borrowed_metadata = metadata
                    .iter()
                    .map(|(key, value)| (key.as_str(), value.clone()))
                    .collect::<Vec<_>>();
                let plain = [20, 40, 80, 120].map(|width| {
                    render_log_block(&event.timestamp, "TOKENS", &borrowed_metadata, &[], width)
                });
                let items = VecDeque::from([LogStreamItem::TokenUsage(event.clone())]);
                let tui = [20, 80].map(|width| {
                    [0, 3, 100].map(|height| {
                        format!(
                            "{:?}",
                            log_stream_tui_text_for_view(&items, height, width, None, 0)
                        )
                    })
                });
                let search = [
                    "",
                    "SSE",
                    "sse profile-a",
                    "profile-a sse",
                    "responses 18446744073709551615",
                    "15 2",
                    "2 18446744073709551615",
                    "100.0",
                    "synthetic time",
                    "Profile",
                    "雪",
                    "雪 雪",
                    "\u{2003}",
                    "\0",
                ]
                .map(|query| {
                    (
                        query,
                        format!(
                            "{:?}",
                            log_stream_tui_text_for_view(&items, 100, 80, Some(query), 0)
                        ),
                    )
                });
                json!({
                    "event": event,
                    "rate_bits": event.output_tokens_per_second.map(f64::to_bits),
                    "json": log_stream_item_json(&items[0]).unwrap(),
                    "metadata": metadata,
                    "plain": plain,
                    "tui": tui,
                    "search": search,
                })
            })
            .collect(),
    )
}

#[test]
fn token_log_presentations_preserve_original_complete_objects() {
    use sha2::{Digest as _, Sha256};
    let corpus = token_presentation_corpus();
    let bytes = serde_json::to_vec(&corpus).unwrap();
    let digest = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let actual =
        json!({"cases": corpus.as_array().unwrap().len(), "bytes": bytes.len(), "sha256": digest});
    let expected: Value =
        serde_json::from_str(include_str!("log_token_presentation_baseline.json")).unwrap();
    assert_eq!(actual, expected);
}
