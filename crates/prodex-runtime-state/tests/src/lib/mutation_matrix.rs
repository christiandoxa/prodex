#[test]
fn runtime_state_mojo_policy_covers_every_mutation_variant_and_usize_boundaries() {
    fn sections(
        state: RuntimeStateSaveStateSection,
        continuations: bool,
        profile_scores: bool,
        usage_snapshots: bool,
        backoffs: bool,
    ) -> RuntimeStateSaveSections {
        RuntimeStateSaveSections {
            state,
            continuations,
            profile_scores,
            usage_snapshots,
            backoffs,
        }
    }

    let full = RuntimeStateSaveSections::full();
    let core_profile = sections(RuntimeStateSaveStateSection::Core, true, true, false, false);
    let core = sections(
        RuntimeStateSaveStateSection::Core,
        true,
        false,
        false,
        false,
    );
    let cases = vec![
        (RuntimeStateMutation::FullState, full, false, false),
        (RuntimeStateMutation::StartupAudit, full, false, false),
        (
            RuntimeStateMutation::StartupContinuationMigration,
            full,
            false,
            false,
        ),
        (
            RuntimeStateMutation::StartupBackoffSoften,
            sections(
                RuntimeStateSaveStateSection::None,
                false,
                false,
                false,
                true,
            ),
            false,
            false,
        ),
        (
            RuntimeStateMutation::ResponseIds("x".into()),
            core_profile,
            true,
            true,
        ),
        (
            RuntimeStateMutation::PreviousResponseOwner("x".into()),
            core_profile,
            true,
            true,
        ),
        (
            RuntimeStateMutation::PreviousResponseNegativeCache("x".into()),
            core_profile,
            false,
            false,
        ),
        (
            RuntimeStateMutation::PreviousResponseRelease("x".into()),
            core_profile,
            true,
            false,
        ),
        (
            RuntimeStateMutation::ResponseTouch("x".into()),
            core,
            false,
            true,
        ),
        (
            RuntimeStateMutation::TurnState("x".into()),
            core,
            true,
            true,
        ),
        (
            RuntimeStateMutation::TurnStateTouch("x".into()),
            core,
            false,
            true,
        ),
        (
            RuntimeStateMutation::SessionId("x".into()),
            core,
            true,
            true,
        ),
        (
            RuntimeStateMutation::SessionTouch("x".into()),
            core,
            false,
            true,
        ),
        (
            RuntimeStateMutation::SessionAffinityRelease("x".into()),
            core,
            true,
            false,
        ),
        (
            RuntimeStateMutation::CompactLineage("x".into()),
            core,
            true,
            true,
        ),
        (
            RuntimeStateMutation::CompactLineageRelease("x".into()),
            core,
            true,
            true,
        ),
        (
            RuntimeStateMutation::CompactSessionTouch("x".into()),
            core,
            false,
            true,
        ),
        (
            RuntimeStateMutation::CompactTurnStateTouch("x".into()),
            core,
            false,
            true,
        ),
        (
            RuntimeStateMutation::DeadResponseBindingClear("x".into()),
            core,
            true,
            false,
        ),
        (
            RuntimeStateMutation::QuotaRelease("x".into()),
            core,
            true,
            false,
        ),
        (
            RuntimeStateMutation::AuthFailedRelease("x".into()),
            full,
            true,
            false,
        ),
        (
            RuntimeStateMutation::ContinuationStale("x".into()),
            core,
            false,
            false,
        ),
        (
            RuntimeStateMutation::ProfileCommit("x".into()),
            sections(RuntimeStateSaveStateSection::Core, false, true, false, true),
            false,
            false,
        ),
        (
            RuntimeStateMutation::UsageSnapshot("x".into()),
            sections(RuntimeStateSaveStateSection::None, false, false, true, true),
            false,
            false,
        ),
        (
            RuntimeStateMutation::ProfileRetryBackoff("x".into()),
            sections(RuntimeStateSaveStateSection::None, false, false, true, true),
            false,
            false,
        ),
        (
            RuntimeStateMutation::ProfileTransportBackoff("x".into()),
            sections(
                RuntimeStateSaveStateSection::None,
                false,
                false,
                false,
                true,
            ),
            false,
            false,
        ),
        (
            RuntimeStateMutation::ProfileCircuitHalfOpenProbe("x".into()),
            sections(
                RuntimeStateSaveStateSection::None,
                false,
                false,
                false,
                true,
            ),
            false,
            false,
        ),
        (
            RuntimeStateMutation::ProfileHealth("x".into()),
            sections(RuntimeStateSaveStateSection::None, false, true, false, true),
            false,
            false,
        ),
        (
            RuntimeStateMutation::ProfileCircuitClear("x".into()),
            sections(RuntimeStateSaveStateSection::None, false, true, false, true),
            false,
            false,
        ),
        (
            RuntimeStateMutation::ProfileBadPairing("x".into()),
            sections(
                RuntimeStateSaveStateSection::None,
                false,
                true,
                false,
                false,
            ),
            false,
            false,
        ),
        (
            RuntimeStateMutation::ProfileAuthBackoff("x".into()),
            sections(
                RuntimeStateSaveStateSection::None,
                false,
                true,
                false,
                false,
            ),
            false,
            false,
        ),
        (
            RuntimeStateMutation::ProfileAuthBackoffCleared("x".into()),
            sections(
                RuntimeStateSaveStateSection::None,
                false,
                true,
                false,
                false,
            ),
            false,
            false,
        ),
    ];

    for (mutation, expected_sections, expected_journal, expected_hot) in cases {
        assert_eq!(
            runtime_state_save_sections(&mutation),
            expected_sections,
            "mutation={mutation:?}"
        );
        assert_eq!(
            runtime_state_save_requires_continuation_journal(&mutation),
            expected_journal,
            "mutation={mutation:?}"
        );
        assert_eq!(
            runtime_hot_continuation_state_mutation(&mutation),
            expected_hot,
            "mutation={mutation:?}"
        );
    }

    assert_eq!(
        runtime_background_enqueue_backlog(usize::MAX),
        usize::MAX - 1
    );
    let thresholds = RuntimeBackgroundQueuePressureThresholds {
        state_save: usize::MAX,
        continuation_journal: usize::MAX,
        probe_refresh: usize::MAX,
    };
    assert!(runtime_proxy_queue_pressure_active(
        usize::MAX,
        0,
        0,
        thresholds
    ));
    assert_eq!(
        runtime_background_queue_enqueue_plan(
            RuntimeBackgroundQueueKind::StateSave,
            usize::MAX,
            thresholds
        ),
        RuntimeBackgroundQueueEnqueuePlan {
            backlog: usize::MAX - 1,
            pressure_active: false,
        },
    );
}

