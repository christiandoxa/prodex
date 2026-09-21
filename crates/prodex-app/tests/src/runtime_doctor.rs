use super::*;

#[test]
fn summarize_runtime_log_tail_understands_json_lines() {
    let tail = br#"{"timestamp":"2026-04-08 10:00:00.000 +00:00","message":"request=7 profile_health profile=main route=responses score=4","fields":{"request":"7","profile":"main","route":"responses","score":"4"}}"#;
    let summary = summarize_runtime_log_tail(tail);

    assert_eq!(summary.line_count, 1);
    assert_eq!(
        summary.marker_counts.get("profile_health").copied(),
        Some(1)
    );
    assert_eq!(
        summary.first_timestamp.as_deref(),
        Some("2026-04-08 10:00:00.000 +00:00")
    );
    assert_eq!(
        summary
            .marker_last_fields
            .get("profile_health")
            .and_then(|fields| fields.get("profile"))
            .map(String::as_str),
        Some("main")
    );
}

#[test]
fn runtime_doctor_json_value_includes_selection_markers() {
    let mut summary = RuntimeDoctorSummary {
        line_count: 3,
        ..RuntimeDoctorSummary::default()
    };
    summary
        .marker_counts
        .insert("selection_pick".to_string(), 2);
    summary
        .marker_counts
        .insert("selection_skip_current".to_string(), 1);
    summary.marker_last_fields.insert(
        "selection_pick".to_string(),
        std::collections::BTreeMap::from([
            ("profile".to_string(), "main".to_string()),
            ("route".to_string(), "responses".to_string()),
        ]),
    );

    let value = runtime_doctor_json_value(&summary);

    assert_eq!(value["marker_counts"]["selection_pick"], 2);
    assert_eq!(value["marker_counts"]["selection_skip_current"], 1);
    assert_eq!(
        value["marker_last_fields"]["selection_pick"]["profile"],
        "main"
    );
    assert_eq!(
        value["marker_last_fields"]["selection_pick"]["route"],
        "responses"
    );
}
