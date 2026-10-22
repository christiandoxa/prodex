use prodex_runtime_doctor::{
    RuntimeDoctorBindingProfileSummary, RuntimeDoctorBindingSourceSummary,
    RuntimeDoctorBindingStateSummary, RuntimeDoctorSummary, runtime_doctor_fields_for_summary,
};
use std::path::Path;

fn binding_text(state: RuntimeDoctorBindingStateSummary) -> String {
    let fields = runtime_doctor_fields_for_summary(
        &RuntimeDoctorSummary {
            binding_state: state,
            ..Default::default()
        },
        Path::new("fixture-pointer"),
    );
    assert_eq!(fields[9].0, "Binding state");
    fields[9].1.clone()
}

#[test]
fn binding_report_preserves_all_source_order_top_profile_order_and_missing_sum() {
    let source = |number: usize| RuntimeDoctorBindingSourceSummary {
        response_bindings: number,
        session_bindings: number + 1,
        turn_state_bindings: number + 2,
        session_id_bindings: number + 3,
        total_bindings: number + 4,
        profile_count: number + 5,
        missing_profile_bindings: number + 6,
        profiles: vec![
            RuntimeDoctorBindingProfileSummary {
                profile: format!("profile-{number}"),
                total_bindings: number + 7,
                ..Default::default()
            },
            RuntimeDoctorBindingProfileSummary {
                profile: "alpha".into(),
                total_bindings: 999,
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    assert_eq!(
        binding_text(RuntimeDoctorBindingStateSummary {
            active_profile: Some("東京\nline=1".into()),
            profile_count: 17,
            last_run_selected_profiles: 19,
            state: source(1),
            runtime_continuations: source(11),
            continuation_journal: source(21),
            merged_continuations: source(31),
        }),
        concat!(
            "active=東京\nline=1 profiles=17 selected=19",
            " | state r=1 s=2 t=3 sid=4 total=5 profiles=6 top=profile-1=8",
            " | runtime r=11 s=12 t=13 sid=14 total=15 profiles=16 top=profile-11=18",
            " | journal r=21 s=22 t=23 sid=24 total=25 profiles=26 top=profile-21=28",
            " | merged r=31 s=32 t=33 sid=34 total=35 profiles=36 top=profile-31=38 | missing=88",
        )
    );
}

#[test]
fn binding_report_preserves_empty_names_and_unsigned_extrema() {
    let source = RuntimeDoctorBindingSourceSummary {
        response_bindings: usize::MAX,
        session_bindings: usize::MAX,
        turn_state_bindings: usize::MAX,
        session_id_bindings: usize::MAX,
        total_bindings: usize::MAX,
        profile_count: usize::MAX,
        missing_profile_bindings: usize::MAX,
        profiles: vec![RuntimeDoctorBindingProfileSummary {
            profile: "".into(),
            total_bindings: usize::MAX,
            ..Default::default()
        }],
        ..Default::default()
    };
    let empty = "r=0 s=0 t=0 sid=0 total=0 profiles=0 top=-";
    let max = usize::MAX;
    assert_eq!(
        binding_text(RuntimeDoctorBindingStateSummary {
            active_profile: Some("".into()),
            profile_count: max,
            last_run_selected_profiles: max,
            state: source,
            ..Default::default()
        }),
        format!(
            "active= profiles={max} selected={max} | state r={max} s={max} t={max} sid={max} total={max} profiles={max} top=={max} | runtime {empty} | journal {empty} | merged {empty} | missing={max}"
        )
    );
    assert_eq!(
        binding_text(RuntimeDoctorBindingStateSummary::default()),
        format!(
            "active=- profiles=0 selected=0 | state {empty} | runtime {empty} | journal {empty} | merged {empty} | missing=0"
        )
    );
}
