use super::{human_event_name, operational_event_summary};
use std::collections::BTreeMap;

fn show(event: &str, source: &str, fields: &[(&str, &str)]) -> String {
    operational_event_summary(
        event,
        source,
        &fields
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect::<BTreeMap<_, _>>(),
    )
}

#[test]
fn compiled_operational_details_preserve_order_duplicate_labels_and_plain_sentinels() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) }
    assert_eq!(
        show(
            "request_captured",
            "request",
            &[
                ("profile", "profile-a"),
                ("model", "fixture"),
                ("reason", "-"),
                ("path", "/responses?fake=1"),
                ("url", "https://example.com/secondary?fake=1"),
                ("active", "0"),
                ("ignored", "excluded"),
            ]
        ),
        format!(
            "{}  profile=profile-a · model=fixture · path=/responses · path=/secondary · active=0",
            human_event_name("request_captured")
        )
    );
}

#[test]
fn compiled_operational_details_reject_controls_without_trimming_noncontrols() {
    let title = human_event_name("request_captured");
    for value in [
        "", "a\0b", "a\nb", "a\tb", "a\u{7f}b", "a\u{85}b", "a\u{9f}b",
    ] {
        assert_eq!(
            show("request_captured", "request", &[("model", value)]),
            title
        );
    }
    for value in [" ", "\u{a0}", "é🦀\u{200b}\u{feff}"] {
        assert_eq!(
            show("request_captured", "request", &[("model", value)]),
            format!("{title}  model={value}")
        );
    }
    assert_eq!(show("request_captured", "request", &[]), title);
}

#[test]
fn compiled_operational_details_keep_percent_sentinels_and_raw_scalar_limits() {
    assert_eq!(
        show(
            "quota_blocked",
            "quota",
            &[
                ("quota_band", "-"),
                ("five_hour_remaining", "-"),
                ("weekly_remaining", ""),
                ("reason", "-")
            ]
        ),
        format!("{}  5h=-%", human_event_name("quota_blocked"))
    );
    let number = "7".repeat(33);
    assert_eq!(
        show("quota_blocked", "quota", &[("weekly_remaining", &number)]),
        format!(
            "{}  week={}…%",
            human_event_name("quota_blocked"),
            "7".repeat(32)
        )
    );
}

#[test]
fn compiled_operational_details_clip_unicode_and_endpoint_text_at_the_original_limits() {
    let text = format!("{}e\u{301}", "é".repeat(191));
    assert_eq!(
        show("request_captured", "request", &[("model", &text)]),
        format!(
            "{}  model={}e…",
            human_event_name("request_captured"),
            "é".repeat(191)
        )
    );
    let path = format!("/{}?fake=1#fragment", "é".repeat(193));
    assert_eq!(
        show("upstream_start", "upstream", &[("url", &path)]),
        format!(
            "{}  path=/{}…",
            human_event_name("upstream_start"),
            "é".repeat(191)
        )
    );
    assert_eq!(
        show("upstream_start", "upstream", &[("url", "invalid")]),
        format!("{}  path=upstream", human_event_name("upstream_start"))
    );
}

#[test]
fn compiled_operational_details_keep_ttft_latency_and_unknown_source_behavior() {
    for (event, label) in [
        ("first_local_chunk", "ttft_ms"),
        ("stream_complete", "latency_ms"),
    ] {
        assert_eq!(
            show(event, "stream", &[("elapsed_ms", "7"), ("chunks", "0")]),
            format!("{}  {label}=7 · chunks=0", human_event_name(event))
        );
    }
    assert_eq!(
        show("unknown_event", "unknown", &[("ignored", "excluded")]),
        human_event_name("unknown_event")
    );
}

#[test]
fn compiled_operational_details_preserve_secret_redaction_before_display() {
    let text = show(
        "request_captured",
        "request",
        &[("command", "Authorization: Bearer fixture-token")],
    );
    assert!(text.starts_with(&format!(
        "{}  command=",
        human_event_name("request_captured")
    )));
    assert!(text.contains("<redacted>"));
    assert!(!text.contains("fixture-token"));
}
