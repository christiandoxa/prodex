use super::*;

#[test]
fn broker_continuity_kernel_smoke() {
    const { assert!(crate::MOJO_REQUIRED) };
    let plan = continuity_line_plan(
        "request=3 chain_retried_owner profile=second reason=\"quoted reason\"",
        None,
        None,
        None,
    )
    .unwrap();
    assert_eq!(plan.event, Some(ContinuityEvent::ChainRetriedOwner));
    assert_eq!(plan.reason_source, Some(ContinuityReasonSource::RawLine));

    assert_eq!(effective_score(4, 90, 100, 5).unwrap(), 2);
    assert!(stale_verified(true, None, Some(80), None, 100, 10).unwrap());
    assert_eq!(
        route_kind("websocket").unwrap(),
        ContinuityRouteKind::Websocket
    );
    assert_eq!(
        health_key_kind("__route_health__:responses:main").unwrap(),
        HealthKeyKind::Route
    );
    let current = BrokerBinaryIdentityView {
        version: Some("0.7.0"),
        sha256: Some("abc123"),
        path_present: true,
    };
    let same_sha = BrokerBinaryIdentityView {
        version: Some("0.8.0"),
        sha256: Some("abc123"),
        path_present: false,
    };
    let different = BrokerBinaryIdentityView {
        version: Some("0.8.0"),
        sha256: Some("def456"),
        path_present: false,
    };
    assert!(binary_identity_present(current).unwrap());
    assert!(binary_identity_matches(current, same_sha).unwrap());
    assert_eq!(
        binary_identity_replacement_reason(current, different).unwrap(),
        BrokerReplacementReason::Sha256Mismatch
    );
    assert!(binary_identity_version_mismatch(current, different).unwrap());
    assert_eq!(
        version_guard_plan(
            true,
            current,
            BrokerBinaryIdentityView {
                version: Some("0.7.0"),
                sha256: None,
                path_present: false,
            },
            different,
            0,
            0
        )
        .unwrap(),
        BrokerVersionGuardPlan {
            outcome: BrokerVersionGuardOutcome::Replaced,
            use_version_identity: true,
            replacement_reason: Some(BrokerReplacementReason::VersionMismatch),
        }
    );
    assert_eq!(
        version_guard_plan(
            true,
            current,
            BrokerBinaryIdentityView {
                version: Some("0.7.0"),
                sha256: None,
                path_present: false,
            },
            different,
            1,
            0,
        )
        .unwrap()
        .outcome,
        BrokerVersionGuardOutcome::DeferredActiveRequests
    );
    assert_eq!(
        parse_prodex_version(
            "  prodex 0.7.0
"
        )
        .unwrap(),
        Some("0.7.0")
    );
    assert_eq!(parse_prodex_version("codex 0.7.0").unwrap(), None);
    assert_eq!(
        continuity_event_kind("chain_dead_upstream_confirmed").unwrap(),
        Some(ContinuityEvent::ChainDeadUpstreamConfirmed)
    );
    assert_eq!(
        log_fingerprint_relation(20, 2, 0, Some((10, 1, 0))).unwrap(),
        BrokerLogFingerprintRelation::Append
    );
    assert_eq!(
        log_fingerprint_relation(9, 2, 0, Some((10, 1, 0))).unwrap(),
        BrokerLogFingerprintRelation::Rotated
    );
    assert_eq!(lru_evict_index(&[5, 2, 9], Some(1)).unwrap(), Some(0));
    assert_eq!(lru_evict_index(&[5], Some(0)).unwrap(), Some(0));
    assert_eq!(lru_evict_index(&[], None).unwrap(), None);
}

#[test]
fn broker_registry_reuse_plan_is_mojo_authoritative() {
    let matching = registry_reuse_plan(BrokerRegistryReuseInput {
        registry_upstream_base_url: "https://upstream.example",
        registry_include_code_review: true,
        registry_upstream_no_proxy: false,
        registry_smart_context_enabled: true,
        launch_upstream_base_url: "https://upstream.example",
        launch_include_code_review: true,
        launch_upstream_no_proxy: false,
        launch_smart_context_enabled: true,
        health_present: true,
        health_matches: true,
    })
    .unwrap();
    assert_eq!(matching, BrokerRegistryReuseDecision::Reuse);
    assert_eq!(
        registry_reuse_plan(BrokerRegistryReuseInput {
            registry_upstream_base_url: "https://upstream.example",
            registry_include_code_review: true,
            registry_upstream_no_proxy: false,
            registry_smart_context_enabled: true,
            launch_upstream_base_url: "https://other.example",
            launch_include_code_review: true,
            launch_upstream_no_proxy: false,
            launch_smart_context_enabled: true,
            health_present: true,
            health_matches: true,
        })
        .unwrap(),
        BrokerRegistryReuseDecision::LaunchConfigMismatch
    );
    assert_eq!(
        registry_reuse_plan(BrokerRegistryReuseInput {
            registry_upstream_base_url: "https://upstream.example",
            registry_include_code_review: true,
            registry_upstream_no_proxy: false,
            registry_smart_context_enabled: true,
            launch_upstream_base_url: "https://upstream.example",
            launch_include_code_review: true,
            launch_upstream_no_proxy: false,
            launch_smart_context_enabled: true,
            health_present: false,
            health_matches: false,
        })
        .unwrap(),
        BrokerRegistryReuseDecision::MissingMatchingHealth
    );
    assert!(
        registry_reuse_plan(BrokerRegistryReuseInput {
            registry_upstream_base_url: "https://upstream.example",
            registry_include_code_review: true,
            registry_upstream_no_proxy: false,
            registry_smart_context_enabled: true,
            launch_upstream_base_url: "https://upstream.example",
            launch_include_code_review: true,
            launch_upstream_no_proxy: false,
            launch_smart_context_enabled: true,
            health_present: false,
            health_matches: true,
        })
        .is_err()
    );
}

#[test]
fn broker_reuse_policy_uses_all_flags_and_health_only_after_config_match() {
    const A: &str = "https://upstream.example";
    const B: &str = "https://UPSTREAM.example";
    for registry_bits in 0_u8..8 {
        for launch_bits in 0_u8..8 {
            for health_present in [false, true] {
                for health_matches in [false, true] {
                    if health_matches && !health_present {
                        assert_eq!(
                            registry_reuse_plan(BrokerRegistryReuseInput {
                                registry_upstream_base_url: A,
                                registry_include_code_review: registry_bits & 1 != 0,
                                registry_upstream_no_proxy: registry_bits & 2 != 0,
                                registry_smart_context_enabled: registry_bits & 4 != 0,
                                launch_upstream_base_url: A,
                                launch_include_code_review: launch_bits & 1 != 0,
                                launch_upstream_no_proxy: launch_bits & 2 != 0,
                                launch_smart_context_enabled: launch_bits & 4 != 0,
                                health_present,
                                health_matches,
                            }),
                            Err(MojoError::InvalidOutput),
                        );
                        continue;
                    }
                    let expected = if registry_bits != launch_bits {
                        BrokerRegistryReuseDecision::LaunchConfigMismatch
                    } else if health_present && health_matches {
                        BrokerRegistryReuseDecision::Reuse
                    } else {
                        BrokerRegistryReuseDecision::MissingMatchingHealth
                    };
                    assert_eq!(
                        registry_reuse_plan(BrokerRegistryReuseInput {
                            registry_upstream_base_url: A,
                            registry_include_code_review: registry_bits & 1 != 0,
                            registry_upstream_no_proxy: registry_bits & 2 != 0,
                            registry_smart_context_enabled: registry_bits & 4 != 0,
                            launch_upstream_base_url: A,
                            launch_include_code_review: launch_bits & 1 != 0,
                            launch_upstream_no_proxy: launch_bits & 2 != 0,
                            launch_smart_context_enabled: launch_bits & 4 != 0,
                            health_present,
                            health_matches,
                        }),
                        Ok(expected),
                        "registry_bits={registry_bits:03b}, launch_bits={launch_bits:03b}",
                    );
                }
            }
        }
    }
    let input = BrokerRegistryReuseInput {
        registry_upstream_base_url: A,
        registry_include_code_review: true,
        registry_upstream_no_proxy: false,
        registry_smart_context_enabled: true,
        launch_upstream_base_url: B,
        launch_include_code_review: true,
        launch_upstream_no_proxy: false,
        launch_smart_context_enabled: true,
        health_present: true,
        health_matches: true,
    };
    assert_eq!(
        registry_reuse_plan(input),
        Ok(BrokerRegistryReuseDecision::LaunchConfigMismatch)
    );
}

#[test]
fn broker_startup_grace_matches_ceil_plus_one_for_integer_boundaries() {
    let times = [
        0,
        1,
        999,
        1_000,
        1_001,
        9_999,
        10_000,
        u32::MAX as u64,
        u64::MAX,
    ];
    let grace = [-100, -1, 0, 1, 5, 17, i64::MAX];
    for milliseconds in times {
        for idle in grace {
            let expected = (milliseconds.div_ceil(1_000) as i64)
                .saturating_add(1)
                .max(idle);
            assert_eq!(
                startup_grace_seconds(milliseconds, idle),
                Ok(expected),
                "milliseconds={milliseconds}, idle={idle}"
            );
        }
    }
}

#[test]
fn broker_startup_grace_plan_preserves_timeout_rounding_and_idle_floor() {
    assert_eq!(startup_grace_seconds(1_250, 5).unwrap(), 5);
    assert_eq!(startup_grace_seconds(5_250, 1).unwrap(), 7);
    assert_eq!(
        startup_grace_seconds(u64::MAX, 0).unwrap(),
        18_446_744_073_709_553
    );
}

#[test]
fn broker_process_identity_and_termination_plans_are_fail_closed() {
    let proven = |path_check_enabled, recheck_enabled| {
        process_identity_plan(
            false,
            true,
            true,
            true,
            path_check_enabled,
            true,
            true,
            recheck_enabled,
            true,
            true,
        )
        .unwrap()
    };
    assert_eq!(
        process_identity_plan(false, true, true, true, true, true, true, true, true, true),
        Ok(BrokerProcessIdentityPlan::Proven)
    );
    assert_eq!(
        process_identity_plan(
            true, true, false, false, false, false, false, false, false, false
        ),
        Ok(BrokerProcessIdentityPlan::Absent)
    );
    assert_eq!(
        process_identity_plan(
            false, false, false, false, false, false, false, false, false, false
        ),
        Ok(BrokerProcessIdentityPlan::OwnershipUnproven)
    );
    assert_eq!(
        process_identity_plan(
            false, true, true, false, false, false, false, false, false, false
        ),
        Ok(BrokerProcessIdentityPlan::OwnershipChanged)
    );
    assert_eq!(
        process_identity_plan(
            false, true, true, true, true, true, false, false, false, false
        ),
        Ok(BrokerProcessIdentityPlan::OwnershipChanged)
    );
    assert_eq!(
        process_identity_plan(false, true, true, true, true, true, true, true, true, false),
        Ok(BrokerProcessIdentityPlan::OwnershipChanged)
    );
    assert_eq!(proven(false, false), BrokerProcessIdentityPlan::Proven);
    assert_eq!(
        termination_signal_plan(BrokerProcessIdentityPlan::Absent),
        Ok(BrokerTerminationSignalAction::Skip)
    );
    assert_eq!(
        termination_signal_plan(BrokerProcessIdentityPlan::Proven),
        Ok(BrokerTerminationSignalAction::Signal)
    );
    assert_eq!(
        termination_signal_plan(BrokerProcessIdentityPlan::OwnershipUnproven),
        Ok(BrokerTerminationSignalAction::Refuse)
    );
}

#[test]
fn broker_lease_lifecycle_covers_cleanup_expiry_and_reuse_actions() {
    assert_eq!(
        lease_lifecycle_plan(
            BrokerLeaseLifecycleOperation::Cleanup,
            false,
            false,
            false,
            0,
            false,
        ),
        Ok(BrokerLeaseLifecycleAction::Ignore)
    );
    assert_eq!(
        lease_lifecycle_plan(
            BrokerLeaseLifecycleOperation::Cleanup,
            true,
            true,
            false,
            0,
            false,
        ),
        Ok(BrokerLeaseLifecycleAction::Remove)
    );
    assert_eq!(
        lease_lifecycle_plan(
            BrokerLeaseLifecycleOperation::Cleanup,
            true,
            false,
            true,
            0,
            false,
        ),
        Ok(BrokerLeaseLifecycleAction::Keep)
    );
    assert_eq!(
        lease_lifecycle_plan(
            BrokerLeaseLifecycleOperation::Cleanup,
            true,
            false,
            false,
            0,
            true,
        ),
        Ok(BrokerLeaseLifecycleAction::Remove)
    );
    assert_eq!(
        lease_lifecycle_plan(
            BrokerLeaseLifecycleOperation::Cleanup,
            true,
            true,
            false,
            1,
            true,
        ),
        Ok(BrokerLeaseLifecycleAction::Keep)
    );
    assert_eq!(
        lease_lifecycle_plan(
            BrokerLeaseLifecycleOperation::Acquire,
            true,
            false,
            true,
            0,
            false,
        ),
        Ok(BrokerLeaseLifecycleAction::Acquire)
    );
    assert_eq!(
        lease_lifecycle_plan(
            BrokerLeaseLifecycleOperation::Renew,
            true,
            false,
            true,
            0,
            false,
        ),
        Ok(BrokerLeaseLifecycleAction::Renew)
    );
}

#[test]
fn broker_readiness_idle_and_cleanup_precedence_respect_boundaries() {
    assert_eq!(
        readiness_plan(false, false, false, false, 0, 10),
        Ok(BrokerReadinessDecision::Wait)
    );
    assert_eq!(
        readiness_plan(true, true, true, true, 9, 10),
        Ok(BrokerReadinessDecision::Ready)
    );
    assert_eq!(
        readiness_plan(true, true, true, true, 10, 10),
        Ok(BrokerReadinessDecision::Timeout)
    );
    assert!(readiness_plan(false, true, false, false, 0, 10).is_err());

    assert_eq!(
        idle_plan(false, 0, 0, 100, 5),
        Ok(BrokerIdleDecision::Reset)
    );
    assert_eq!(idle_plan(true, 1, 0, 100, 5), Ok(BrokerIdleDecision::Reset));
    assert_eq!(idle_plan(true, 0, 0, 4, 5), Ok(BrokerIdleDecision::Wait));
    assert_eq!(
        idle_plan(true, 0, 0, 5, 5),
        Ok(BrokerIdleDecision::Shutdown)
    );

    assert_eq!(
        registry_process_plan(BrokerProcessIdentityPlan::Absent, 3, 4),
        Ok(BrokerRegistryProcessAction::Remove)
    );
    assert_eq!(
        registry_process_plan(BrokerProcessIdentityPlan::OwnershipChanged, 0, 0),
        Ok(BrokerRegistryProcessAction::DiscardStale)
    );
    assert_eq!(
        registry_process_plan(BrokerProcessIdentityPlan::OwnershipUnproven, 3, 4),
        Ok(BrokerRegistryProcessAction::Keep)
    );
    assert_eq!(
        termination_outcome_plan(0),
        Ok(BrokerTerminationOutcomePlan::Cleanup)
    );
    assert_eq!(
        termination_outcome_plan(2),
        Ok(BrokerTerminationOutcomePlan::DiscardStale)
    );
    assert_eq!(
        termination_outcome_plan(4),
        Ok(BrokerTerminationOutcomePlan::Failure)
    );
}
