use super::*;

fn input() -> RuntimeDoctorBindingDisplayInput<'static> {
    RuntimeDoctorBindingDisplayInput {
        active_profile: None,
        profile_count: 0,
        selected_profiles: 0,
        sources: [RuntimeDoctorBindingDisplaySource::default(); 4],
    }
}

#[test]
fn compiled_binding_display_preserves_defaults_and_full_unsigned_counts() {
    let empty = "r=0 s=0 t=0 sid=0 total=0 profiles=0 top=-";
    assert_eq!(
        runtime_doctor_binding_display(input()).unwrap(),
        format!(
            "active=- profiles=0 selected=0 | state {empty} | runtime {empty} | journal {empty} | merged {empty} | missing=0"
        )
    );
    let max = u64::MAX;
    let source = RuntimeDoctorBindingDisplaySource {
        response_bindings: max,
        session_bindings: max,
        turn_state_bindings: max,
        session_id_bindings: max,
        total_bindings: max,
        profile_count: max,
        missing_profile_bindings: max,
        top_profile: Some(("", max)),
    };
    assert_eq!(
        runtime_doctor_binding_display(RuntimeDoctorBindingDisplayInput {
            active_profile: Some("東京\nline"),
            profile_count: max,
            selected_profiles: max,
            sources: [
                source,
                Default::default(),
                Default::default(),
                Default::default()
            ],
        })
        .unwrap(),
        format!(
            "active=東京\nline profiles={max} selected={max} | state r={max} s={max} t={max} sid={max} total={max} profiles={max} top=={max} | runtime {empty} | journal {empty} | merged {empty} | missing={max}"
        )
    );
}

#[test]
fn compiled_binding_display_preserves_source_order_and_missing_count_aggregation() {
    let sources = [1_u64, 11, 21, 31].map(|count| RuntimeDoctorBindingDisplaySource {
        response_bindings: count,
        missing_profile_bindings: count + 6,
        top_profile: Some(("top", count + 7)),
        ..Default::default()
    });
    assert_eq!(
        runtime_doctor_binding_display(RuntimeDoctorBindingDisplayInput {
            active_profile: Some(""),
            sources,
            ..input()
        })
        .unwrap(),
        concat!(
            "active= profiles=0 selected=0",
            " | state r=1 s=0 t=0 sid=0 total=0 profiles=0 top=top=8",
            " | runtime r=11 s=0 t=0 sid=0 total=0 profiles=0 top=top=18",
            " | journal r=21 s=0 t=0 sid=0 total=0 profiles=0 top=top=28",
            " | merged r=31 s=0 t=0 sid=0 total=0 profiles=0 top=top=38 | missing=88",
        )
    );
}

#[test]
fn binding_display_abi_rejects_versions_flags_alignment_utf8_and_short_capacity() {
    let source = BindingSource {
        response: 0,
        session: 0,
        turn_state: 0,
        session_id: 0,
        total: 0,
        profiles: 0,
        missing: 0,
        top_present: 0,
        top_name: view(""),
        top_count: 0,
    };
    let mut ffi = BindingInput {
        active_present: 0,
        active: view(""),
        profiles: 0,
        selected: 0,
        sources: [source; 4],
    };
    let mut output = [73_u8; 2048];
    let mut written = 83_i64;
    let result = &mut written as *mut i64 as u64;
    let mut call = |version, input: *const BindingInput, capacity, result| unsafe {
        prodex_mojo_runtime_doctor_binding_display_v1(
            version,
            input as u64,
            output.as_mut_ptr() as u64,
            capacity,
            result,
        )
    };
    let address = &ffi as *const BindingInput;
    assert_eq!(call(2, address, 2048, result), 4);
    assert_eq!(call(1, std::ptr::null(), 2048, result), 1);
    assert_eq!(call(1, address.wrapping_byte_add(1), 2048, result), 1);
    assert_eq!(call(1, address, -1, result), 1);
    assert_eq!(call(1, address, 2048, result + 1), 1);
    assert_eq!(call(1, address, 1, result), 3);
    ffi.active_present = 2;
    assert_eq!(call(1, &ffi, 2048, result), 1);
    ffi.active_present = 0;
    ffi.sources[0].top_present = 2;
    assert_eq!(call(1, &ffi, 2048, result), 1);
    ffi.sources[0].top_present = 1;
    let invalid = [0xff_u8];
    ffi.sources[0].top_name = RichStringView {
        ptr: invalid.as_ptr() as u64,
        len: 1,
    };
    assert_eq!(call(1, &ffi, 2048, result), 1);
    assert_eq!(output, [73; 2048]);
    assert_eq!(written, 83);
}
