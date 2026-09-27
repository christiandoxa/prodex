use super::*;

#[test]
fn operational_event_summary_uses_mojo_detail_plan() {
    let fields = [
        ("profile", "profile-a"),
        ("route", "responses"),
        ("status", "200"),
        ("path", "/backend-api/codex/responses?secret=<redacted>"),
    ]
    .into_iter()
    .map(|(key, value)| (key.to_string(), value.to_string()))
    .collect::<BTreeMap<_, _>>();
    for (event, source) in [
        ("request_captured", "request"),
        ("route_decision", "route"),
        ("quota_blocked", "quota"),
        ("unknown_event", "unknown"),
    ] {
        let summary = operational_event_summary(event, source, &fields);
        assert!(!summary.is_empty(), "event={event} source={source}");
        assert!(
            !summary.contains("secret=<redacted>"),
            "event={event} source={source}"
        );
    }
}
