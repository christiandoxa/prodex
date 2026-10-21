use super::{
    RunEvent,
    helpers::{events_page, push_event},
    tests::record,
};

#[test]
fn event_pages_keep_deque_order_and_report_gaps_for_empty_pages() {
    let mut record = record();
    record.next_seq = u64::MAX;
    record.events = [9, 1, 9, u64::MAX, u64::MAX]
        .into_iter()
        .enumerate()
        .map(|(index, seq)| RunEvent {
            seq,
            event_type: "stdout".into(),
            text: format!("event-{index}"),
        })
        .collect();
    let page = events_page(&record, 8, 3).unwrap();
    assert_eq!(
        page.events
            .iter()
            .map(|event| (event.seq, event.text.as_str()))
            .collect::<Vec<_>>(),
        [(9, "event-0"), (9, "event-2"), (u64::MAX, "event-3")],
    );
    assert_eq!(page.next_seq, u64::MAX);
    assert!(!page.truncated);
    let empty = events_page(&record, 0, 0).unwrap();
    assert!(empty.events.is_empty());
    assert!(empty.truncated);
    record.events.clear();
    record.next_seq = 100;
    assert!(events_page(&record, 98, 5).unwrap().truncated);
    assert!(!events_page(&record, 99, 5).unwrap().truncated);
}

#[test]
fn event_retention_saturates_sequence_and_recovers_an_oversized_deque() {
    let mut record = record();
    record.next_seq = u64::MAX - 1;
    for index in 0..257 {
        push_event(&mut record, "stdout", &format!("event-{index}"));
    }
    assert_eq!(record.events.len(), 256);
    assert_eq!(record.next_seq, u64::MAX);
    let page = events_page(&record, u64::MAX - 1, usize::MAX).unwrap();
    assert_eq!(page.events.len(), 256);
    assert_eq!(page.events.first().unwrap().text, "event-1");
    assert_eq!(page.events.last().unwrap().text, "event-256");
    assert!(page.events.iter().all(|event| event.seq == u64::MAX));
    assert!(!page.truncated);
    let empty = events_page(&record, u64::MAX, usize::MAX).unwrap();
    assert!(empty.events.is_empty());
    assert!(!empty.truncated);
    record.events = (0..300)
        .map(|seq| RunEvent {
            seq,
            event_type: "stderr".into(),
            text: seq.to_string(),
        })
        .collect();
    record.next_seq = 300;
    push_event(&mut record, "stdout", "tail");
    assert_eq!(record.events.len(), 256);
    assert_eq!(record.events.front().unwrap().seq, 45);
    assert_eq!(record.events.back().unwrap().seq, 300);
    assert_eq!(record.next_seq, 301);
}
#[test]
fn compiled_text_cuts_keep_output_and_event_caps_on_utf8_boundaries() {
    let mut record = record();
    record.output = "x".repeat(super::OUTPUT_MAX_BYTES - 2);
    super::helpers::append_output(&mut record, "stdout", "🦀tail".as_bytes());
    assert_eq!(record.output, "x".repeat(super::OUTPUT_MAX_BYTES - 2));
    assert!(record.output_truncated);
    assert_eq!(
        record
            .events
            .iter()
            .map(|event| (event.event_type.as_str(), event.text.as_str()))
            .collect::<Vec<_>>(),
        [
            ("stdout", "🦀tail"),
            ("output_truncated", "output limit reached")
        ],
    );
    push_event(&mut record, "stderr", &format!("{}é", "x".repeat(8191)));
    assert_eq!(record.events.back().unwrap().text, "x".repeat(8191));
}
