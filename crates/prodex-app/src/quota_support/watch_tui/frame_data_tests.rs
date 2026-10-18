use super::*;

#[test]
fn compiled_quota_row_assembly_preserves_workspace_credits_and_provider_order() {
    let facts = serde_json::json!({
        "kind": "openai", "profile": " profile ", "active": true, "auth": "chatgpt",
        "account": "\u{2003}test@example.com\u{3000}", "plan": " plus ",
        "main": "5h full", "compact_main": "5h compact", "status": "Ready",
        "reset": "5h 1h", "reset_credits": 2,
        "workspace_name": " \u{2003} ", "workspace_id": " fake-workspace ",
        "additional": ["extra-one", "extra-東京"], "updated": "now",
    });
    let row: serde_json::Value = quota_watch_projection(&facts, QuotaWatchPresentation::Row, true);
    assert_eq!(
        row,
        serde_json::json!({
            "profile": [" profile "], "current": ["*"], "auth": ["chatgpt"],
            "account": ["test@example.com"], "plan": ["plus"], "status": ["Ready"],
            "remaining": ["5h compact"],
            "detail": ["resets: 5h 1h | workspace: profile | reset credits: 2 available | extra-one | extra-東京"],
        })
    );
    let fields: Vec<(String, String)> =
        quota_watch_projection(&facts, QuotaWatchPresentation::ProfileFields, false);
    assert_eq!(
        fields,
        vec![
            ("Account".to_string(), "test@example.com".to_string()),
            ("Plan".to_string(), "plus".to_string()),
            ("Status".to_string(), "Ready".to_string()),
            ("Main".to_string(), "5h full".to_string()),
            ("Last Updated".to_string(), "now".to_string()),
        ]
    );
    let fields: serde_json::Value =
        quota_watch_projection(&facts, QuotaWatchPresentation::ProfileFields, true);
    assert_eq!(
        fields[4],
        serde_json::json!(["Reset", "5h 1h; reset credits: 2 available"])
    );
    let compact: serde_json::Value =
        quota_watch_projection(&facts, QuotaWatchPresentation::Row, false);
    assert_eq!(compact["detail"], serde_json::json!([]));
}

#[test]
fn compiled_quota_rows_preserve_empty_present_plan_and_error_passthrough() {
    for (plan, expected) in [
        (serde_json::Value::Null, "fallback"),
        (serde_json::json!(" \u{2003} "), "-"),
    ] {
        let facts = serde_json::json!({
            "kind": "copilot", "plan": plan, "fallback_plan": "fallback",
            "account": null, "status": "Ready", "main": "90%", "reset": null,
            "updated": "now", "profile": "main", "auth": "oauth", "active": false,
        });
        let row: serde_json::Value =
            quota_watch_projection(&facts, QuotaWatchPresentation::Row, true);
        assert_eq!(row["plan"], serde_json::json!([expected]));
        assert_eq!(row["account"], serde_json::json!(["-"]));
        assert_eq!(row["detail"], serde_json::json!(["resets: unavailable"]));
        let fields: serde_json::Value =
            quota_watch_projection(&facts, QuotaWatchPresentation::ProfileFields, false);
        assert_eq!(fields[4], serde_json::json!(["Reset", "unavailable"]));
    }
    let facts = serde_json::json!({"kind":"error", "status":"Blocked unauthorized",
        "reset":"error: bad \"credential\" 🧭", "profile":"broken", "auth":"oauth"});
    let row: serde_json::Value = quota_watch_projection(&facts, QuotaWatchPresentation::Row, true);
    assert_eq!(
        row["detail"],
        serde_json::json!(["error: bad \"credential\" 🧭"])
    );
    assert_eq!(row["remaining"], serde_json::json!(["-"]));
    assert!(quota_human_tui_has_compact_label(
        "\u{2003}Model provider: example"
    ));
    assert!(!quota_human_tui_has_compact_label(
        "model provider: example"
    ));
}
