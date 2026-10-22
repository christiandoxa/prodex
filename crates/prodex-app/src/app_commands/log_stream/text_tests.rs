use super::{
    add_log_detail, add_log_endpoint_detail, add_log_percent_detail, bounded_log_value,
    display_log_field,
};
use std::collections::BTreeMap;

#[test]
fn compiled_operational_text_cut_preserves_scalar_ellipsis_and_exact_boundaries() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) }
    assert_eq!(bounded_log_value("", 0), "");
    assert_eq!(bounded_log_value("text", 0), "…");
    assert_eq!(bounded_log_value("é🦀e\u{301}", 3), "é🦀e…");
    assert_eq!(bounded_log_value("é🦀e\u{301}", 4), "é🦀e\u{301}");
    assert_eq!(bounded_log_value("  spaced ", usize::MAX), "  spaced ");
    assert_eq!(
        bounded_log_value(&"é".repeat(193), 192),
        format!("{}…", "é".repeat(192))
    );
    assert_eq!(
        bounded_log_value(&"7".repeat(33), 32),
        format!("{}…", "7".repeat(32))
    );
}

#[test]
fn compiled_operational_field_filter_rejects_controls_without_trimming_whitespace() {
    for value in [
        "", "a\0b", "a\nb", "a\tb", "a\u{7f}b", "a\u{85}b", "a\u{9f}b",
    ] {
        let fields = BTreeMap::from([("model".into(), value.into())]);
        assert_eq!(display_log_field(&fields, "model"), None);
        let mut details = Vec::new();
        add_log_detail(&mut details, &fields, "model", "model");
        add_log_percent_detail(&mut details, &fields, "model", "model");
        add_log_endpoint_detail(&mut details, &fields, "model", "model");
        assert!(details.is_empty());
    }
    for value in [" ", "\u{a0}", "é🦀\u{200b}\u{feff}"] {
        let fields = BTreeMap::from([("model".into(), value.into())]);
        assert_eq!(display_log_field(&fields, "model"), Some(value));
    }
    assert_eq!(display_log_field(&BTreeMap::new(), "model"), None);
}

#[test]
fn compiled_operational_field_plans_keep_detail_sentinels_percent_and_endpoint_behavior() {
    let fields = BTreeMap::from([
        ("model".into(), "fixture".into()),
        ("remaining".into(), "12".into()),
        (
            "path".into(),
            "https://example.com/responses?fake=1#fragment".into(),
        ),
        ("reason".into(), "-".into()),
    ]);
    let mut details = Vec::new();
    add_log_detail(&mut details, &fields, "model", "model");
    add_log_percent_detail(&mut details, &fields, "remaining", "left");
    add_log_endpoint_detail(&mut details, &fields, "path", "path");
    add_log_detail(&mut details, &fields, "reason", "reason");
    add_log_percent_detail(&mut details, &fields, "reason", "reason");
    assert_eq!(
        details,
        ["model=fixture", "left=12%", "path=/responses", "reason=-%"]
    );
}
