use super::{OUTPUT_MAX_BYTES, RunEvent, helpers::append_output, tests::record};

#[test]
fn output_marker_planning_uses_the_deque_after_normal_event_eviction() {
    for position in [0, 1, 255] {
        let mut record = record();
        record.output = "x".repeat(OUTPUT_MAX_BYTES);
        record.next_seq = 256;
        record.events = (0..256)
            .map(|seq| RunEvent {
                seq,
                event_type: if seq == position {
                    "output_truncated"
                } else {
                    "stdout"
                }
                .into(),
                text: seq.to_string(),
            })
            .collect();
        append_output(&mut record, "stdout", b"tail");
        assert_eq!(record.events.len(), 256);
        assert_eq!(
            record
                .events
                .iter()
                .filter(|event| event.event_type == "output_truncated")
                .count(),
            1
        );
        if position == 0 {
            assert_eq!(record.next_seq, 258);
            assert_eq!(record.events.front().unwrap().seq, 2);
            assert_eq!(record.events.back().unwrap().seq, 257);
            assert_eq!(record.events.back().unwrap().text, "output limit reached");
        } else {
            assert_eq!(record.next_seq, 257);
            assert_eq!(record.events.front().unwrap().seq, 1);
            assert_eq!(record.events.back().unwrap().seq, 256);
            assert_eq!(record.events.back().unwrap().text, "tail");
        }
    }
}

#[test]
fn output_planning_preserves_empty_input_and_a_marker_typed_chunk() {
    let mut record = record();
    record.output = "x".repeat(OUTPUT_MAX_BYTES);
    record.output_truncated = true;
    record.next_seq = u64::MAX;
    append_output(&mut record, "stdout", b"");
    assert!(record.events.is_empty());
    assert_eq!(record.next_seq, u64::MAX);
    assert_eq!(record.output.len(), OUTPUT_MAX_BYTES);
    append_output(&mut record, "output_truncated", b"marker");
    assert_eq!(record.events.len(), 1);
    assert_eq!(record.events[0].seq, u64::MAX);
    assert_eq!(record.events[0].event_type, "output_truncated");
    assert_eq!(record.events[0].text, "marker");
    assert_eq!(record.next_seq, u64::MAX);
}

#[test]
fn output_planning_preserves_lossy_redaction_and_event_only_clipping() {
    let mut record = record();
    append_output(&mut record, "stdout", &[255, b'a']);
    assert_eq!(record.output, "�a");
    assert_eq!(record.events[0].text, "�a");
    append_output(
        &mut record,
        "stdout",
        b"Authorization: Bearer fixture-output-token",
    );
    assert!(record.output.contains("Bearer <redacted>"));
    assert!(!record.output.contains("fixture-output-token"));
    assert!(!record.events[1].text.contains("fixture-output-token"));
    let mut record = super::tests::record();
    let text = format!("{}é", "x".repeat(8191));
    append_output(&mut record, "stderr", text.as_bytes());
    assert_eq!(record.output, "x".repeat(8191));
    assert_eq!(record.events[0].text, record.output);
    assert!(!record.output_truncated);
    assert_eq!(record.events.len(), 1);
}
