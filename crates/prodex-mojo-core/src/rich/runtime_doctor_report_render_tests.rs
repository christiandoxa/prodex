use super::*;

fn input() -> RuntimeDoctorReportInput<'static> {
    RuntimeDoctorReportInput {
        pointer_path: "fixture",
        pointer_exists: true,
        log_path: None,
        log_exists: false,
        line_count: 0,
        diagnosis: "",
        selection_pressure: "",
        transport_pressure: "",
        persistence_pressure: "",
        quota_freshness_pressure: "",
        profile_count: 0,
        binding_state: "binding",
        degraded_routes: &[],
        suspect_continuations: &[],
        runtime_brokers: &[],
        legacy_previous: 0,
        compact_final: 0,
        sync_probe: 0,
        probe_pressure: 0,
        compact_exit: None,
        compact_reason: None,
        compact_last_failure: None,
        sync_jobs: None,
        sync_profiles: None,
        probe_profile: None,
        hot_reason: None,
    }
}

#[test]
fn compiled_report_preserves_complete_default_field_contract() {
    assert_eq!(
        runtime_doctor_report_fields_json(input()).unwrap(),
        r#"[["Log pointer","fixture (exists)"],["Latest log","- (missing)"],["Log sample","0 lines"],["Diagnosis",""],["Selection pressure",""],["Transport pressure",""],["Persistence pressure",""],["Quota freshness pressure",""],["Profiles","0"],["Binding state","binding"],["Degraded routes","-"],["Suspect continuations","-"],["Runtime brokers","-"]]"#,
    );
}

#[test]
fn compiled_report_preserves_optional_text_escaping_counts_and_list_limits() {
    let list = ["", "東京", "duplicate", "duplicate", "five", "hidden"].map(String::from);
    let result = runtime_doctor_report_fields_json(RuntimeDoctorReportInput {
        line_count: u64::MAX,
        log_path: Some(""),
        log_exists: true,
        degraded_routes: &list,
        diagnosis: "quote\"slash\\line\n\0",
        legacy_previous: u64::MAX,
        compact_final: 7,
        sync_probe: 9,
        probe_pressure: 11,
        compact_exit: Some(""),
        compact_reason: Some("overload"),
        sync_jobs: Some(""),
        sync_profiles: Some("23"),
        hot_reason: Some("東京 (5)"),
        ..input()
    })
    .unwrap();
    assert_eq!(
        result,
        concat!(
            r#"[["Log pointer","fixture (exists)"],["Latest log"," (exists)"],["Log sample","18446744073709551615 lines"],["Diagnosis","quote\"slash\\line\n\u0000"],"#,
            r#"["Selection pressure",""],["Transport pressure",""],["Persistence pressure",""],["Quota freshness pressure",""],["Profiles","0"],["Binding state","binding"],"#,
            r#"["Degraded routes","; 東京; duplicate; duplicate; five"],["Suspect continuations","-"],["Runtime brokers","-"],"#,
            r#"["Legacy prev recovery","18446744073709551615"],["Compact final","7"],["Compact exit",""],["Compact reason","overload"],["Compact last fail","-"],"#,
            r#"["Sync-probe deferred"," job(s)"],["Probe pressure profile","-"],["Hot reason","東京 (5)"]]"#,
        )
    );
    for (jobs, profiles, expected) in [
        (None, Some("23"), "23 profile(s)"),
        (None, None, "-"),
        (Some(""), Some("23"), " job(s)"),
    ] {
        let result = runtime_doctor_report_fields_json(RuntimeDoctorReportInput {
            sync_probe: 1,
            sync_jobs: jobs,
            sync_profiles: profiles,
            ..input()
        })
        .unwrap();
        assert!(result.ends_with(&format!(r#",["Sync-probe deferred","{expected}"]]"#)));
    }
}

#[test]
fn report_abi_rejects_bad_records_utf8_and_short_capacity_without_partial_output() {
    let mut values = [view(""); 15];
    let mut ffi = ReportInput {
        values: values.as_ptr() as u64,
        presence: 253,
        pointer_exists: 0,
        log_exists: 0,
        line_count: 0,
        profile_count: 0,
        legacy_previous: 0,
        compact_final: 0,
        sync_probe: 0,
        probe_pressure: 0,
        lists: [ReportList {
            values: 0,
            count: 0,
        }; 3],
    };
    let mut output = [73_u8; 2048];
    let mut written = 83_i64;
    let mut call = |version, input: *const ReportInput, capacity, result| unsafe {
        prodex_mojo_runtime_doctor_report_render_v1(
            version,
            input as u64,
            output.as_mut_ptr() as u64,
            capacity,
            result,
        )
    };
    let address = &ffi as *const ReportInput;
    let result = &mut written as *mut i64 as u64;
    assert_eq!(call(2, address, 2048, result), 4);
    assert_eq!(call(1, std::ptr::null(), 2048, result), 1);
    assert_eq!(call(1, address.wrapping_byte_add(1), 2048, result), 1);
    assert_eq!(call(1, address, -1, result), 1);
    assert_eq!(call(1, address, 2048, 0), 1);
    assert_eq!(call(1, address, 2048, result + 1), 1);
    assert_eq!(call(1, address, 1, result), 3);
    ffi.presence = 0;
    assert_eq!(call(1, &ffi, 2048, result), 1);
    ffi.presence = 32768 | 253;
    assert_eq!(call(1, &ffi, 2048, result), 1);
    ffi.presence = 253;
    ffi.lists[0].count = -1;
    assert_eq!(call(1, &ffi, 2048, result), 1);
    ffi.lists[0].count = 1;
    assert_eq!(call(1, &ffi, 2048, result), 1);
    ffi.lists[0].count = 0;
    let invalid = [0xff_u8];
    values[2].ptr = invalid.as_ptr() as u64;
    values[2].len = 1;
    ffi.values = values.as_ptr() as u64;
    assert_eq!(call(1, &ffi, 2048, result), 1);
    assert_eq!(output, [73; 2048]);
    assert_eq!(written, 83);
}
