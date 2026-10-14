use super::*;

#[test]
fn mojo_route_reason_finds_latest_matching_reason_and_unicode() {
    let events = [
        RuntimeDoctorRouteReasonEvent {
            profile: "alpha",
            route: "responses",
            detail: "status=1 reason=old",
        },
        RuntimeDoctorRouteReasonEvent {
            profile: "beta",
            route: "responses",
            detail: "reason=irrelevant",
        },
        RuntimeDoctorRouteReasonEvent {
            profile: "alpha",
            route: "responses",
            detail: "status=2",
        },
        RuntimeDoctorRouteReasonEvent {
            profile: "alpha",
            route: "responses",
            detail: "🧭\u{2003}outcome=ล่าสุด",
        },
        RuntimeDoctorRouteReasonEvent {
            profile: "alpha",
            route: "responses",
            detail: "reason= outcome=ignored",
        },
    ];
    assert_eq!(
        runtime_doctor_route_latest_reason(&events, 2, "alpha", "responses").unwrap(),
        Some("old")
    );
    assert_eq!(
        runtime_doctor_route_latest_reason(&events, 3, "alpha", "responses").unwrap(),
        Some("ล่าสุด")
    );
    assert_eq!(
        runtime_doctor_route_latest_reason(&events, 4, "alpha", "responses").unwrap(),
        Some("")
    );
    assert_eq!(
        runtime_doctor_route_latest_reason(&events, 4, "alpha", "compact").unwrap(),
        None
    );
    assert_eq!(
        runtime_doctor_route_latest_reason(&events, 4, "missing", "responses").unwrap(),
        None
    );
    assert_eq!(
        runtime_doctor_route_latest_reason(&events, 5, "alpha", "responses"),
        Err(MojoError::InvalidInput)
    );
}

#[test]
fn mojo_route_reason_rejects_bad_abi_without_touching_output() {
    let mut output = [99_i64; 3];
    unsafe {
        assert_eq!(
            prodex_mojo_runtime_doctor_route_latest_reason_v1(
                2,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                mojo_mut_pointer_address(output.as_mut_ptr()),
            ),
            4,
        );
        assert_eq!(
            prodex_mojo_runtime_doctor_route_latest_reason_v1(
                1,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                mojo_mut_pointer_address(output.as_mut_ptr()),
            ),
            1,
        );
    }
    assert_eq!(output, [99; 3]);
}
