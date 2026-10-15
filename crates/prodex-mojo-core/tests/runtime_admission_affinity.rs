#![cfg(feature = "mojo-runtime")]

use prodex_mojo_core::runtime_state::runtime_proxy_admission_affinity;

#[test]
fn required_mojo_admission_affinity_preserves_noncompact_binding_rules() {
    const { assert!(prodex_mojo_core::MOJO_REQUIRED) };

    for (route, previous, turn_state, expected) in [
        (0, false, false, false),
        (0, true, false, true),
        (1, true, true, false),
        (2, false, true, true),
        (3, true, true, false),
    ] {
        assert_eq!(
            runtime_proxy_admission_affinity(route, previous, turn_state)
                .expect("compiled Mojo affinity policy should accept route facts")
                .owned,
            expected,
            "route={route} previous={previous} turn_state={turn_state}"
        );
    }
}

#[test]
fn admission_affinity_caller_rejects_invalid_route() {
    assert_eq!(
        runtime_proxy_admission_affinity(4, true, false),
        Err(prodex_mojo_core::MojoError::InvalidInput)
    );
}
