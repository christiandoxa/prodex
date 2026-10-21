use super::*;

#[test]
fn dead_turn_state_release_requires_matching_owner_and_no_surviving_lineage() {
    for binding_matches in [false, true] {
        for surviving_lineage in [false, true] {
            assert_eq!(
                dead_turn_state_release_plan(binding_matches, surviving_lineage),
                Ok(binding_matches && !surviving_lineage),
                "binding_matches={binding_matches}, surviving_lineage={surviving_lineage}"
            );
        }
    }
    assert_eq!(
        unsafe { prodex_runtime_lineage_dead_turn_state_plan_v1(0, 1, 0) },
        -4,
        "ABI version mismatch must fail closed"
    );
    for invalid in [(2, 0), (-1, 0), (0, 2), (0, -1)] {
        assert_eq!(
            unsafe {
                prodex_runtime_lineage_dead_turn_state_plan_v1(ABI_VERSION, invalid.0, invalid.1)
            },
            -1,
            "non-boolean ABI input must fail closed"
        );
    }
}

#[test]
fn release_plan_preserves_session_when_response_or_turn_state_is_present() {
    assert_eq!(
        release_plan(true, true, false, false, true, true, true).unwrap(),
        RuntimeLineageReleasePlan {
            response: true,
            turn_state: false,
            session: false,
            compact_session: false,
        }
    );
    assert_eq!(
        release_plan(false, false, true, true, true, true, true).unwrap(),
        RuntimeLineageReleasePlan {
            response: false,
            turn_state: true,
            session: false,
            compact_session: false,
        }
    );
}

#[test]
fn release_plan_releases_session_lineage_only_without_response_or_turn_state() {
    assert_eq!(
        release_plan(false, false, false, false, true, true, true).unwrap(),
        RuntimeLineageReleasePlan {
            response: false,
            turn_state: false,
            session: true,
            compact_session: true,
        }
    );
    assert_eq!(
        release_plan(false, false, false, false, true, false, true).unwrap(),
        RuntimeLineageReleasePlan {
            response: false,
            turn_state: false,
            session: false,
            compact_session: true,
        }
    );
}
