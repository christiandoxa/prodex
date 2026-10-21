use super::{bounded_redacted_text, bounded_text};

#[test]
fn shared_expose_text_cut_preserves_lossy_utf8_and_redaction() {
    assert_eq!(bounded_text("aé雪🙂z", 6), "aé雪");
    assert_eq!(bounded_text("aé雪🙂z", usize::MAX), "aé雪🙂z");
    assert_eq!(bounded_redacted_text(&[255, b'a'], 2), "");
    assert_eq!(bounded_redacted_text(&[255, b'a'], 3), "�");
    assert_eq!(bounded_redacted_text(&[255, b'a'], 4), "�a");
    let text = bounded_redacted_text(b"Authorization: Bearer fixture-text-token", usize::MAX);
    assert!(text.contains("Bearer <redacted>"));
    assert!(!text.contains("fixture-text-token"));
    assert!(bounded_redacted_text(b"Authorization: Bearer fixture-text-token", 0).is_empty());
}
