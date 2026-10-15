#![cfg(feature = "mojo-runtime")]

use prodex_mojo_core::runtime::profile_selection_jitter;

#[test]
fn required_mojo_selection_jitter_preserves_default_hasher_vectors() {
    const { assert!(prodex_mojo_core::MOJO_REQUIRED) };

    for (sequence, profile, route, expected) in [
        (42, "alpha", 0, 13_720_159_197_250_930_926),
        (43, "alpha", 0, 2_052_999_783_678_899_254),
        (42, "beta", 0, 5_134_239_319_966_320_861),
        (42, "alpha", 2, 19_596_772_748_915_481),
        (0, "", 3, 11_978_717_415_126_589_746),
        (u64::MAX, "x", 1, 3_201_082_204_192_371_675),
    ] {
        assert_eq!(
            profile_selection_jitter(sequence, profile, route),
            Ok(expected),
            "sequence={sequence} profile={profile:?} route={route}"
        );
    }
}

#[test]
fn selection_jitter_caller_rejects_route_and_profile_bounds() {
    assert_eq!(
        profile_selection_jitter(0, "", -1),
        Err(prodex_mojo_core::MojoError::InvalidInput)
    );
    assert_eq!(
        profile_selection_jitter(0, &"x".repeat(4_097), 0),
        Err(prodex_mojo_core::MojoError::InvalidInput)
    );
}
