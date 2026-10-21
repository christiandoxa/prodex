use super::*;

#[test]
fn identity_fallback_mojo_distinguishes_empty_from_invalid_presence() {
    use RuntimeLineageIdentityFallback::{Empty, Invalid};
    let whitespace = [
        None,
        Some(""),
        Some("  "),
        Some("\u{2003}\u{3000}"),
        Some("\u{00a0}"),
    ];
    for value in whitespace {
        for location in 0..3 {
            let mut inputs = [None, None, None];
            inputs[location] = value;
            assert_eq!(
                identity_fallback_plan(inputs[0], inputs[1], inputs[2]),
                Ok(Empty),
                "only trimmed whitespace is not an identity"
            );
        }
    }
    for value in ["x", "\u{200b}", "\0", "\u{007f}"] {
        for location in 0..3 {
            let mut inputs = [None, None, None];
            inputs[location] = Some(value);
            assert_eq!(
                identity_fallback_plan(inputs[0], inputs[1], inputs[2]),
                Ok(Invalid),
                "present component must not silently become an empty identity"
            );
        }
    }
}

#[test]
fn binding_identity_source_prefers_credential_then_profile_fallback() {
    use RuntimeLineageBindingIdentitySource::{Credential, None, Profile};

    assert_eq!(
        binding_identity_source(true, true, true, true),
        Ok(Credential)
    );
    assert_eq!(
        binding_identity_source(true, false, true, true),
        Ok(Profile)
    );
    assert_eq!(
        binding_identity_source(false, false, true, true),
        Ok(Profile)
    );
    assert_eq!(binding_identity_source(true, false, true, false), Ok(None));
    assert_eq!(
        binding_identity_source(true, true, true, false),
        Ok(Credential)
    );
    assert_eq!(
        unsafe { prodex_runtime_lineage_binding_identity_source_v1(0, 0, 0, 0, 0) },
        -4,
    );
    assert_eq!(
        unsafe { prodex_runtime_lineage_binding_identity_source_v1(ABI_VERSION, 0, 1, 0, 0) },
        -1,
    );
}

#[test]
fn candidate_and_resolution_precedence_is_mojo_authoritative() {
    assert_eq!(
        candidate_plan(true, false, true, true, false, false).unwrap(),
        RuntimeLineageCandidatePlan {
            kind: RuntimeLineageCandidateKind::Owner,
            identity_action: RuntimeLineageIdentityAction::Set,
        }
    );
    assert_eq!(
        candidate_plan(true, false, true, true, true, false).unwrap(),
        RuntimeLineageCandidatePlan {
            kind: RuntimeLineageCandidateKind::Conflict,
            identity_action: RuntimeLineageIdentityAction::Conflict,
        }
    );
    assert_eq!(
        resolution_plan(false, 1, 0).unwrap(),
        RuntimeLineageResolutionKind::Owned
    );
    assert_eq!(
        resolution_plan(false, 0, 1).unwrap(),
        RuntimeLineageResolutionKind::Unavailable
    );
    assert_eq!(
        resolution_plan(false, 1, 1).unwrap(),
        RuntimeLineageResolutionKind::Conflict
    );
    assert_eq!(
        lookup_affinity_plan(true, false, true).unwrap(),
        RuntimeLineageLookupAffinity::Fallback
    );
    assert_eq!(
        owner_lookup_plan(RuntimeLineageOwnerKind::Owned, true, false).unwrap(),
        RuntimeLineageOwnerLookupPlan {
            owner_kind: RuntimeLineageOwnerKind::Unavailable,
            profile_action: RuntimeLineageOwnerProfileAction::ConflictSentinel,
        }
    );
}
