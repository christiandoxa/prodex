use super::super::log_stream_tui_text;
use super::{LogStreamItem, TranscriptEvent, log_event_label};
use std::collections::VecDeque;

fn header(source: &str) -> String {
    let items = VecDeque::from([LogStreamItem::Transcript(TranscriptEvent {
        timestamp: "2026-01-10 12:00:00".into(),
        source: source.into(),
        text: "readable".into(),
    })]);
    let text = log_stream_tui_text(&items, 8, 80);
    assert!(
        text.lines
            .iter()
            .flat_map(|line| &line.spans)
            .any(|span| span.content.contains("readable"))
    );
    text.lines[0]
        .spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
}

#[test]
fn compiled_log_labels_reach_tui_headers_for_protocol_roles_and_aliases() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) }
    for (source, label) in [
        ("request", "REQUEST"),
        ("mcp", "MCP"),
        ("ws", "WEBSOCKET"),
        ("turn-context", "MODEL"),
        ("session-context", "SESSION"),
        ("prompt-engineering", "PROMPT"),
        ("tool-output", "TOOL RESULT"),
        ("tokens", "TOKENS"),
        ("hook", "HOOK"),
        ("user", "USER"),
        ("assistant", "ASSISTANT"),
        ("reasoning", "REASONING"),
        ("tool-call:exec", "TOOL CALL"),
    ] {
        assert_eq!(header(source), format!("2026-01-10 12:00:00 {label}"));
    }
}

#[test]
fn compiled_log_label_fallback_keeps_case_unicode_and_empty_tool_call_suffix() {
    for (source, label) in [
        ("", "stream "),
        ("REQUEST", "stream REQUEST"),
        ("tool-call:", "TOOL CALL"),
        ("tool-call:é", "TOOL CALL"),
        ("Tool-call:exec", "stream Tool-call:exec"),
        ("tool-call", "stream tool-call"),
        ("未知", "stream 未知"),
    ] {
        assert_eq!(log_event_label(source), label);
        assert_eq!(header(source), format!("2026-01-10 12:00:00 {label}"));
    }
}
