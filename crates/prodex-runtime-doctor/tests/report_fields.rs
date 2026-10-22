use prodex_runtime_doctor::{
    RuntimeDoctorBindingProfileSummary, RuntimeDoctorBindingSourceSummary,
    RuntimeDoctorBindingStateSummary, RuntimeDoctorSummary, runtime_doctor_fields_for_summary,
};
use std::collections::BTreeMap;
use std::path::Path;

fn fields(summary: &RuntimeDoctorSummary) -> Vec<(String, String)> {
    runtime_doctor_fields_for_summary(summary, Path::new("fixture-pointer"))
}

#[test]
fn report_fields_preserve_default_row_order_and_empty_text() {
    let source = "r=0 s=0 t=0 sid=0 total=0 profiles=0 top=-";
    assert_eq!(
        fields(&RuntimeDoctorSummary::default()),
        [
            ("Log pointer", "fixture-pointer (missing)".into()),
            ("Latest log", "- (missing)".into()),
            ("Log sample", "0 lines".into()),
            ("Diagnosis", "".into()),
            ("Selection pressure", "".into()),
            ("Transport pressure", "".into()),
            ("Persistence pressure", "".into()),
            ("Quota freshness pressure", "".into()),
            ("Profiles", "0".into()),
            (
                "Binding state",
                format!("active=- profiles=0 selected=0 | state {source} | runtime {source} | journal {source} | merged {source} | missing=0"),
            ),
            ("Degraded routes", "-".into()),
            ("Suspect continuations", "-".into()),
            ("Runtime brokers", "-".into()),
        ]
        .map(|(label, value)| (label.to_string(), value)),
    );
}

#[test]
fn report_fields_preserve_first_five_lists_and_binding_source_order() {
    let source = RuntimeDoctorBindingSourceSummary {
        response_bindings: 1,
        session_bindings: 2,
        turn_state_bindings: 3,
        session_id_bindings: 4,
        total_bindings: 10,
        profile_count: 2,
        profiles: vec![
            RuntimeDoctorBindingProfileSummary {
                profile: "東京".into(),
                total_bindings: usize::MAX,
                ..Default::default()
            },
            RuntimeDoctorBindingProfileSummary {
                profile: "alpha".into(),
                total_bindings: 1,
                ..Default::default()
            },
        ],
        missing_profile_bindings: 7,
        ..Default::default()
    };
    let summary = RuntimeDoctorSummary {
        pointer_exists: true,
        log_exists: true,
        log_path: Some("fixture-log".into()),
        line_count: usize::MAX,
        binding_state: RuntimeDoctorBindingStateSummary {
            active_profile: Some("".into()),
            profile_count: 2,
            last_run_selected_profiles: 1,
            state: source,
            ..Default::default()
        },
        degraded_routes: ["", "second", "duplicate", "duplicate", "東京", "hidden"]
            .map(String::from)
            .to_vec(),
        suspect_continuation_bindings: vec!["line\nbreak".into()],
        runtime_broker_identities: vec!["".into()],
        ..Default::default()
    };
    let rendered = fields(&summary);
    assert_eq!(rendered[0].1, "fixture-pointer (exists)");
    assert_eq!(rendered[1].1, "fixture-log (exists)");
    assert_eq!(rendered[2].1, format!("{} lines", usize::MAX));
    let empty = "r=0 s=0 t=0 sid=0 total=0 profiles=0 top=-";
    assert_eq!(
        rendered[9].1,
        format!(
            "active= profiles=2 selected=1 | state r=1 s=2 t=3 sid=4 total=10 profiles=2 top=東京={} | runtime {empty} | journal {empty} | merged {empty} | missing=7",
            usize::MAX
        )
    );
    assert_eq!(rendered[10].1, "; second; duplicate; duplicate; 東京");
    assert_eq!(rendered[11].1, "line\nbreak");
    assert_eq!(rendered[12].1, "");
}

#[test]
fn report_fields_preserve_optional_order_missing_values_and_sync_source_precedence() {
    let mut summary = RuntimeDoctorSummary {
        marker_counts: BTreeMap::from([
            ("previous_response_fresh_fallback".into(), 5),
            ("compact_final_failure".into(), 7),
            ("selection_skip_sync_probe".into(), 9),
            ("profile_probe_refresh_backpressure".into(), 11),
        ]),
        marker_last_fields: BTreeMap::from([
            (
                "compact_final_failure".into(),
                BTreeMap::from([
                    ("exit".into(), "".into()),
                    ("reason".into(), "overload".into()),
                ]),
            ),
            (
                "selection_skip_sync_probe".into(),
                BTreeMap::from([
                    ("cold_start_jobs".into(), "".into()),
                    ("cold_start_profiles".into(), "23".into()),
                ]),
            ),
        ]),
        ..Default::default()
    };
    let expected = [
        ("Legacy prev recovery", "5"),
        ("Compact final", "7"),
        ("Compact exit", ""),
        ("Compact reason", "overload"),
        ("Compact last fail", "-"),
        ("Sync-probe deferred", " job(s)"),
        ("Probe pressure profile", "-"),
    ]
    .map(|(label, value)| (label.to_string(), value.to_string()));
    assert_eq!(fields(&summary)[13..], expected);
    summary
        .marker_last_fields
        .get_mut("selection_skip_sync_probe")
        .unwrap()
        .remove("cold_start_jobs");
    assert_eq!(fields(&summary)[18].1, "23 profile(s)");
    summary.marker_last_fields.clear();
    assert_eq!(fields(&summary)[18].1, "-");
    summary
        .facet_counts
        .insert("reason".into(), BTreeMap::from([("東京".into(), 5)]));
    assert_eq!(
        fields(&summary).last(),
        Some(&("Hot reason".into(), "東京 (5)".into()))
    );
}
