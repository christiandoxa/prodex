use super::bounded_text;

#[test]
fn shared_expose_text_cut_preserves_exact_byte_budgets() {
    assert_eq!(bounded_text("aé雪🙂z", 6), "aé雪");
    assert_eq!(bounded_text("aé雪🙂z", usize::MAX), "aé雪🙂z");
    assert_eq!(bounded_text("�a", 2), "");
    assert_eq!(bounded_text("�a", 3), "�");
    assert_eq!(bounded_text("�a", 4), "�a");
    assert_eq!(bounded_text("text", 0), "");
}
